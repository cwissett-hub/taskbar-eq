//! Mono audio -> 64 log-spaced, dB-mapped band levels.
//!
//! # Two FFTs, one display
//!
//! The bands are log-spaced from 40 Hz to 16 kHz (`band = 64 * ln(f/40) /
//! ln(400)`), so band 0 is 3.9 Hz wide and band 63 is 1.5 kHz wide. A
//! single 2048-point FFT cannot serve both ends: at 48 kHz a bin is 23.4 Hz,
//! wider than every band below ~480 Hz, and the forced-ascending edge guard
//! then hands the collapsed low bands one consecutive bin each. That made
//! bands 0..=31 a LINEAR 23.4 Hz-per-band scale wearing log labels - a 60 Hz
//! kick and a 120 Hz bass note landed in bands 1 and 3 instead of ~4 and
//! ~12, so the bottom half of every meter moved as a block.
//!
//! Fix: a second, 8192-point FFT (`LOW_FFT_SIZE`, 5.86 Hz bins, 170 ms
//! window) computes the bands below the crossover; the 2048-point FFT
//! (`FFT_SIZE`, 43 ms) keeps the bands above it, where its resolution is
//! sufficient and its shorter window keeps the mids and highs snappy.
//! Both use the same Hann window, the same 0 dBFS normalisation and the same
//! `db_map`, so a tone reads the same level from either path.
//!
//! * Crossover: the first band whose 2048-point edges are back on the log
//!   grid and at least two bins wide (`crossover_band`). Measured at
//!   construction: **band 32 at 48 kHz** (800 Hz), band 31 at 44.1 kHz.
//!   Bands 0..crossover-1 come from the long FFT, bands crossover+1.. from
//!   the short one, and the two bands either side of the crossover (31 and
//!   32 at 48 kHz) are a 50/50 blend of both in the dB-mapped domain.
//! * Cadence: BOTH FFTs run on every `process` call. Measured (release,
//!   `cost_of_process_per_call`, 1000 x push_history(480 samples) +
//!   process on broadband noise): 17 us per call for both together, so an
//!   every-other-call cache would save ~7 us per 10 ms packet for a
//!   visible half-rate flicker in the bass. Nothing is allocated after
//!   construction.
//! * Two feeds. The capture thread calls `push_history(&fresh_packet)`
//!   with each packet's NEW mono samples, which `BandMapper` keeps in an
//!   8192-sample history for the long FFT, and then `process(&window,
//!   out)` with the newest FFT_SIZE samples of its own ring, which the
//!   short FFT analyses directly, exactly as before. Novelty is never
//!   inferred from the window: processing the same window twice without
//!   pushing leaves the bass reading unchanged.
//!
//! Known trade-offs, measured:
//! * On broadband noise the long FFT reads ~0.05 (about 3 dB of the 55 dB
//!   display range) LOWER than the short one at the crossover: a 4x longer
//!   FFT spreads noise over 4x more bins (-6 dB per bin), partly won back
//!   by taking the max over 4x more bins per band. Tones match exactly, so
//!   no level compensation is applied; the crossfade halves the step.
//! * The long FFT's Hann window centres 85 ms in the past against 21 ms
//!   for the short one, so bass transients display ~64 ms later than the
//!   mids. This is the price of resolving 6 Hz.
//! * Bands 0..=8 remain single consecutive bins even at 8192 points (their
//!   nominal width is under 5.86 Hz), so they are placed to within one bin
//!   (~6 Hz), not exactly on the log grid.

// The capture thread is the only production caller; some helpers exist for
// the tests below and would otherwise trip the binary-crate dead-code lint.
#![allow(dead_code)]

use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use std::sync::Arc;

pub const NUM_BANDS: usize = 64;
pub const FFT_SIZE: usize = 2048;
pub const HOP: usize = 512;
/// Second, longer FFT that resolves the log spacing below the crossover
/// band (see the module header).
pub const LOW_FFT_SIZE: usize = 8192;
const F_LOW: f32 = 40.0;
const F_HIGH: f32 = 16_000.0;

// A spectrum display must scale in decibels, not linearly. A full-scale
// PURE SINE at one bin reads 1.0 under the linear normalisation below, but
// real music spreads energy across the spectrum, so no bin gets anywhere
// near full scale - and treble commonly sits 20-40dB below bass for the
// same perceived loudness. A linear factor cannot claw back tens of dB, so
// the fix maps magnitude to dB the way a real spectrum analyser does.
//
// Anything at or below this floor (dBFS, i.e. relative to the 0dB == full
// scale reference `norm` establishes) maps to 0.0; 0dB maps to 1.0.
//
// Recalibrated (was -70.0): -70dB compressed so much of the usable range
// into "looks lit" territory that ordinary listening levels already read
// close to full - measured against synthetic broadband signals at RMS
// 0.02/0.06/0.2 ("quiet"/"normal"/"loud"), -70dB put "normal"'s median
// around 0.5-0.55 and "quiet" around 0.3-0.4, nowhere near quiet enough to
// look quiet. Tightening the floor doesn't change WHEN a band saturates
// (out reaches 1.0 exactly at tilted_db == 0, regardless of the floor -
// only TILT_DB_PER_BAND controls that threshold, see below); it changes
// how much of the display lights up on the way there, so moderate energy
// no longer looks like a peak. -55dB puts "normal" at a median of ~0.35
// and "quiet" at ~0.18 - inside the brief's 0.30-0.45 / clearly-lower
// targets - while a genuinely loud transient can still ride up into
// 0.9-1.0.
const DB_FLOOR: f32 = -55.0;
// Bass-compensating tilt, applied as a dB OFFSET (not a linear multiplier -
// see above: a linear multiplier can't recover tens of dB of deficit).
// Music's natural spectral rolloff means bass energy dominates and the
// treble end of the display would otherwise stay permanently dark even at
// a realistic listening level.
//
// Recalibrated (was 0.30, i.e. ~+19dB of lift at the top band): a band
// saturates (`db_map` clamps to 1.0) exactly when db >= -TILT_DB_PER_BAND *
// band, so the tilt alone sets the saturation threshold - independent of
// DB_FLOOR above. At the old 0.30, band 63 pegged at any -18.9dB (mag >=
// 0.11), which real broadband/percussive content reaches easily; that's
// the arithmetic root of the "every band pegged" bug. 0.20 (+12.6dB at the
// top band, mag >= 0.23 to saturate) still comfortably keeps the treble
// guard test's mix visible (band 41+ reaches ~0.76, versus the >0.25 the
// old darkness bug needed to be excluded) while roughly halving how easily
// the top bands saturate on real broadband energy.
const TILT_DB_PER_BAND: f32 = 0.20;

/// Magnitude -> dB -> bass-tilted -> normalised 0..=1, for one band.
fn db_map(mag: f32, band: usize) -> f32 {
    // mag == 0.0 (silence, or a bin with no energy) is handled explicitly:
    // log10(0) is -inf, and letting that literal -inf flow into the tilt
    // addition and division below would still work out to -inf (never
    // NaN), but doing it explicitly makes the silence case obvious rather
    // than relying on IEEE-754 infinity arithmetic to happen to do the
    // right thing.
    let db = if mag > 0.0 { 20.0 * mag.log10() } else { f32::NEG_INFINITY };

    // dB-offset bass tilt (see TILT_DB_PER_BAND above) - band 0 gets +0dB
    // (no offset), the top band gets the full tilt.
    let tilted_db = db + TILT_DB_PER_BAND * band as f32;

    // Map [DB_FLOOR, 0dB] onto [0.0, 1.0]. Guard NaN/inf explicitly: a
    // non-finite value reaching the ballistics smoother poisons its state
    // permanently (see ballistics.rs::Smoother::update), so silence
    // (-inf) and any other non-finite result must map to exactly 0.0,
    // never propagate.
    if tilted_db.is_finite() {
        ((tilted_db - DB_FLOOR) / -DB_FLOOR).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// One FFT path: a planned FFT of `size` points, its Hann window, in-place
/// scratch, log-spaced band edges in ITS bin space, and the normalisation
/// that puts a full-scale bin-aligned sine at mag == 1.0 (0dBFS).
struct Analyser {
    size: usize,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    scratch: Vec<Complex32>,
    fft_scratch: Vec<Complex32>,
    edges: Vec<usize>,
    norm: f32,
}

impl Analyser {
    fn new(planner: &mut FftPlanner<f32>, size: usize, sample_rate: f32) -> Self {
        let fft = planner.plan_fft_forward(size);

        // Hann window - reduces spectral leakage so a pure tone stays in one band.
        let window: Vec<f32> = (0..size)
            .map(|i| {
                let t = i as f32 / (size - 1) as f32;
                0.5 - 0.5 * (t * std::f32::consts::TAU).cos()
            })
            .collect();

        // Log-spaced band edges in bin space, forced strictly ascending so the
        // low bands (where bins are sparse) never collapse to zero width.
        let bin_hz = sample_rate / size as f32;
        let mut edges = Vec::with_capacity(NUM_BANDS + 1);
        let mut last = 0usize;
        for b in 0..=NUM_BANDS {
            let bin = (band_edge_hz(b) / bin_hz).round() as usize;
            let bin = bin.max(last + if b == 0 { 0 } else { 1 });
            edges.push(bin.min(size / 2 - 1));
            last = *edges.last().unwrap();
        }

        let fft_scratch_len = fft.get_inplace_scratch_len();
        Analyser {
            size,
            fft,
            window,
            scratch: vec![Complex32::new(0.0, 0.0); size],
            fft_scratch: vec![Complex32::new(0.0, 0.0); fft_scratch_len],
            edges,
            // Hann coherent gain is 0.5, so a full-scale sine yields size/4.
            // `norm` is chosen so a full-scale, bin-aligned sine reads mag ==
            // 1.0, i.e. 0dBFS - the reference the dB mapping is built on.
            norm: 4.0 / size as f32,
        }
    }

    /// FFT of `input` (exactly `self.size` samples, oldest first) and
    /// per-band peak-magnitude extraction for `bands` only, with no dB
    /// mapping. Split out from [`BandMapper::process`] so the two concerns
    /// (FFT -> per-band magnitude, and magnitude -> displayed level) can be
    /// reasoned about and tested independently.
    fn magnitudes(&mut self, input: &[f32], bands: std::ops::Range<usize>, mags: &mut [f32; NUM_BANDS]) {
        debug_assert_eq!(input.len(), self.size);
        for ((s, &x), &w) in self.scratch.iter_mut().zip(input).zip(&self.window) {
            *s = Complex32::new(x * w, 0.0);
        }
        self.fft.process_with_scratch(&mut self.scratch, &mut self.fft_scratch);

        for b in bands {
            let (lo, hi) = (self.edges[b], self.edges[b + 1]);
            let mut mag = 0.0f32;
            for bin in lo..hi.max(lo + 1) {
                mag = mag.max(self.scratch[bin].norm() * self.norm);
            }
            mags[b] = mag;
        }
    }
}

/// Lower edge frequency of band `b` (b == NUM_BANDS gives the top edge).
fn band_edge_hz(b: usize) -> f32 {
    F_LOW * (F_HIGH / F_LOW).powf(b as f32 / NUM_BANDS as f32)
}

/// The first band whose FFT_SIZE-point lower edge sits at its nominal
/// (log-spaced) bin AND spans at least two bins: from here up the short
/// FFT resolves the log spacing on its own, below it the LOW_FFT_SIZE path
/// takes over.
///
/// This is judged on the FORCED edges, not the nominal band width. The
/// forced-ascending guard hands every collapsed low band one bin in turn,
/// which pushes the edges ABOVE their nominal frequencies until the
/// nominal edge catches up again; until that rejoin point every band is a
/// single, misplaced bin even where its nominal width would already be two
/// bins (at 48 kHz the nominal width passes 2 bins at band 27 but the
/// edges only rejoin at band 32, and bands 27..=30 sit 1-4 bins too high).
/// The width test alone is not enough either: at 44.1 kHz band 30 is the
/// first two-bin band but its lower edge is still one bin high; band 31
/// is where the edges are back on the log grid.
fn crossover_band(edges: &[usize], bin_hz: f32) -> usize {
    (1..NUM_BANDS)
        .find(|&b| {
            let nominal = (band_edge_hz(b) / bin_hz).round() as usize;
            edges[b] == nominal && edges[b + 1] - edges[b] >= 2
        })
        .unwrap_or(NUM_BANDS - 1)
}

pub struct BandMapper {
    /// FFT_SIZE-point path: bands >= crossover - 1.
    high: Analyser,
    /// LOW_FFT_SIZE-point path: bands <= crossover.
    low: Analyser,
    /// The newest LOW_FFT_SIZE mono samples given to [`Self::push_history`],
    /// oldest first; the long FFT's input.
    history: Vec<f32>,
    /// See [`crossover_band`]; 32 at 48 kHz, 31 at 44.1 kHz.
    crossover: usize,
    sample_rate: f32,
}

impl BandMapper {
    pub fn new(sample_rate: f32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let high = Analyser::new(&mut planner, FFT_SIZE, sample_rate);
        let low = Analyser::new(&mut planner, LOW_FFT_SIZE, sample_rate);
        // Always within 1..NUM_BANDS-1, so the crossfade has a band on each
        // side.
        let crossover = crossover_band(&high.edges, sample_rate / FFT_SIZE as f32);

        BandMapper {
            high,
            low,
            history: vec![0.0; LOW_FFT_SIZE],
            crossover,
            sample_rate,
        }
    }

    /// Appends fresh mono samples to the long FFT's history, dropping the
    /// oldest. Call it with each capture packet's NEW samples (never with
    /// an analysis window - that would stack overlapping copies), then call
    /// [`Self::process`]. A run of zeros is just zeros: a paused player
    /// flushes the bass within LOW_FFT_SIZE samples.
    pub fn push_history(&mut self, fresh: &[f32]) {
        // Only the newest LOW_FFT_SIZE samples can ever matter.
        let fresh = &fresh[fresh.len().saturating_sub(LOW_FFT_SIZE)..];
        if fresh.is_empty() {
            return;
        }
        let keep = LOW_FFT_SIZE - fresh.len();
        self.history.copy_within(fresh.len().., 0);
        self.history[keep..].copy_from_slice(fresh);
    }

    /// `mono` must be exactly FFT_SIZE samples - the newest window of the
    /// stream whose fresh samples have been given to [`Self::push_history`].
    /// Writes normalised 0.0..=1.0 levels: bands below the crossover from
    /// the long FFT over the history, the rest from the short FFT over
    /// `mono`.
    ///
    /// # Panics
    ///
    /// Panics (in both debug and release builds) if `mono.len() != FFT_SIZE`.
    pub fn process(&mut self, mono: &[f32], out: &mut [f32; NUM_BANDS]) {
        assert_eq!(mono.len(), FFT_SIZE, "BandMapper::process requires exactly FFT_SIZE samples");

        let c = self.crossover;
        // Bands 0..=c from the long FFT, bands c-1.. from the short FFT, so
        // bands c-1 and c are computed by BOTH for the crossfade. The low
        // path's values for those two are parked in `low_pair` because the
        // high path overwrites them in `mags`.
        let mut mags = [0.0f32; NUM_BANDS];
        self.low.magnitudes(&self.history, 0..c + 1, &mut mags);
        let low_pair = [mags[c - 1], mags[c]];
        self.high.magnitudes(mono, c - 1..NUM_BANDS, &mut mags);

        for b in 0..NUM_BANDS {
            out[b] = db_map(mags[b], b);
        }
        // 50/50 crossfade of the two bands either side of the crossover, in
        // the displayed (dB-mapped) domain so the seam blends LEVELS rather
        // than raw magnitudes of two differently-sized FFTs.
        out[c - 1] = 0.5 * out[c - 1] + 0.5 * db_map(low_pair[0], c - 1);
        out[c] = 0.5 * out[c] + 0.5 * db_map(low_pair[1], c);
        let _ = self.sample_rate;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (i as f32 / rate * freq * std::f32::consts::TAU).sin())
            .collect()
    }

    fn band_of(freq: f32) -> usize {
        // log-spaced band index a frequency should land in
        let t = (freq / F_LOW).ln() / (F_HIGH / F_LOW).ln();
        ((t * NUM_BANDS as f32) as usize).min(NUM_BANDS - 1)
    }

    /// Mimics the capture thread: every packet's fresh samples go to
    /// `push_history`, are appended to a ring, and the ring's newest
    /// FFT_SIZE samples go to `process` once there are that many.
    struct Stream {
        m: BandMapper,
        ring: Vec<f32>,
        out: [f32; NUM_BANDS],
    }

    impl Stream {
        fn new(rate: f32) -> Self {
            Stream { m: BandMapper::new(rate), ring: Vec::new(), out: [0.0; NUM_BANDS] }
        }

        fn feed(&mut self, chunk: &[f32]) {
            self.m.push_history(chunk);
            self.ring.extend_from_slice(chunk);
            if self.ring.len() >= FFT_SIZE {
                let start = self.ring.len() - FFT_SIZE;
                self.m.process(&self.ring[start..], &mut self.out);
                self.ring.drain(..start);
            }
        }

        fn feed_all(&mut self, sig: &[f32], chunk: usize) {
            for c in sig.chunks(chunk) {
                self.feed(c);
            }
        }

        fn peak(&self) -> usize {
            self.out.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0
        }
    }

    #[test]
    fn silence_produces_no_energy() {
        let mut m = BandMapper::new(48_000.0);
        let mut out = [0.0f32; NUM_BANDS];
        m.process(&vec![0.0; FFT_SIZE], &mut out);
        assert!(out.iter().all(|&v| v < 1e-4), "silence must be flat, got {out:?}");
    }

    #[test]
    fn a_1khz_sine_peaks_in_the_1khz_band() {
        let mut m = BandMapper::new(48_000.0);
        let mut out = [0.0f32; NUM_BANDS];
        m.process(&sine(1000.0, 48_000.0, FFT_SIZE), &mut out);
        let peak = out
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        let expected = band_of(1000.0);
        assert!(
            peak.abs_diff(expected) <= 1,
            "1kHz should peak near band {expected}, peaked at {peak}"
        );
    }

    #[test]
    fn a_bass_tone_peaks_low_and_a_treble_tone_peaks_high() {
        // 80 Hz is below the crossover, so it needs the long FFT's history
        // filled: stream it the way capture would.
        let mut lo = Stream::new(48_000.0);
        lo.feed_all(&sine(80.0, 48_000.0, LOW_FFT_SIZE), FFT_SIZE);
        let mut hi = Stream::new(48_000.0);
        hi.feed_all(&sine(8000.0, 48_000.0, FFT_SIZE), FFT_SIZE);
        assert!(lo.peak() < NUM_BANDS / 3, "80Hz must land in the low third, got {}", lo.peak());
        assert!(hi.peak() > NUM_BANDS * 2 / 3, "8kHz must land in the high third");
    }

    #[test]
    fn output_is_normalised_within_range() {
        let mut m = BandMapper::new(48_000.0);
        let mut out = [0.0f32; NUM_BANDS];
        // Full-scale sine - the loudest realistic input.
        m.process(&sine(500.0, 48_000.0, FFT_SIZE), &mut out);
        assert!(out.iter().all(|&v| (0.0..=1.0).contains(&v)), "got {out:?}");

        // The bound above can never fail on its own: it would even pass
        // against a no-op `process` that leaves a zero-initialised `out`
        // untouched, since 0.0 is inside 0.0..=1.0. Exercise the brief's
        // actual normalisation claim so a broken/empty implementation is
        // caught: band 0 always gets a 0dB tilt offset (TILT_DB_PER_BAND *
        // 0 == 0), so a full-scale sine placed exactly on band 0's first
        // bin is untouched by the bass tilt and should read out at very
        // close to 1.0 - it sits at 0dBFS (norm = 4 / FFT_SIZE is chosen to
        // exactly compensate a Hann window's 0.5 coherent gain for a
        // bin-aligned full-scale sine, so mag == 1.0 there), and 0dBFS maps
        // to the top of the DB_FLOOR..=0dB range, i.e. 1.0.
        //
        // Band 0 comes from the LOW_FFT_SIZE path, so the tone is aligned to
        // THAT path's band-0 bin and streamed for a whole LOW_FFT_SIZE
        // window (in FFT_SIZE chunks of one continuous sine, the way the
        // capture thread would deliver it) so the Hann window is full of it.
        let bin_hz = 48_000.0 / LOW_FFT_SIZE as f32;
        let bin_aligned_bin0_freq = m.low.edges[0] as f32 * bin_hz;
        let mut st = Stream::new(48_000.0);
        st.feed_all(&sine(bin_aligned_bin0_freq, 48_000.0, LOW_FFT_SIZE), FFT_SIZE);
        let out = st.out;
        assert!(
            out[0] > 0.9,
            "a full-scale, bin-aligned tone in band 0 (0dB tilt offset there) \
             should normalise to close to 1.0; got {}, full output {out:?}",
            out[0]
        );
    }

    #[test]
    fn band_edges_are_strictly_ascending() {
        let m = BandMapper::new(48_000.0);
        for edges in [&m.high.edges, &m.low.edges] {
            for pair in edges.windows(2) {
                assert!(pair[1] > pair[0], "edges must ascend: {edges:?}");
            }
            assert_eq!(edges.len(), NUM_BANDS + 1);
        }
    }

    #[test]
    fn crossover_band_is_the_documented_one() {
        // The module header quotes these; keep them honest.
        assert_eq!(BandMapper::new(48_000.0).crossover, 32);
        assert_eq!(BandMapper::new(44_100.0).crossover, 31);
        for rate in [48_000.0, 44_100.0] {
            // Below the crossover the FFT_SIZE path really is bin-starved
            // (the forced-ascending guard has made consecutive single-bin
            // bands), and from it upwards every band spans >= 2 bins.
            let m = BandMapper::new(rate);
            let widths: Vec<usize> = m.high.edges.windows(2).map(|p| p[1] - p[0]).collect();
            assert!(widths[..m.crossover - 1].iter().all(|&w| w == 1), "{rate}: {widths:?}");
            assert!(widths[m.crossover..].iter().all(|&w| w >= 2), "{rate}: {widths:?}");
            // And the band the high path hands over at is placed where the
            // log spacing says it should be (the rejoin point), so the
            // crossfade blends two readings of the same frequency range.
            let bin_hz = rate / FFT_SIZE as f32;
            let nominal = (band_edge_hz(m.crossover) / bin_hz).round() as usize;
            assert_eq!(m.high.edges[m.crossover], nominal, "{rate}");
        }
    }

    #[test]
    fn history_holds_exactly_the_samples_pushed() {
        let sig = sine(60.0, 48_000.0, LOW_FFT_SIZE + 3 * 480);
        let mut m = BandMapper::new(48_000.0);
        for chunk in sig.chunks(480) {
            m.push_history(chunk);
        }
        assert_eq!(&m.history[..], &sig[sig.len() - LOW_FFT_SIZE..]);

        // A run of zeros is just zeros: digital silence after music flushes
        // the history rather than freezing the bass on the last thing heard.
        m.push_history(&vec![0.0f32; LOW_FFT_SIZE]);
        assert!(m.history.iter().all(|&x| x == 0.0));
        let mut out = [0.0f32; NUM_BANDS];
        m.process(&vec![0.0; FFT_SIZE], &mut out);
        assert!(out.iter().all(|&v| v < 1e-4), "silence must be flat, got {out:?}");
    }

    #[test]
    fn processing_the_same_window_twice_does_not_move_the_bass() {
        // The capture thread re-runs `process` on an unchanged window when a
        // packet is flagged silent. Novelty must come only from
        // `push_history`, so the second call must not double-count the
        // window into the long FFT: every band, bass included, reads the
        // same.
        let mut st = Stream::new(48_000.0);
        st.feed_all(&sine(60.0, 48_000.0, LOW_FFT_SIZE), 480);
        let first = st.out;
        let window: Vec<f32> = st.ring[st.ring.len() - FFT_SIZE..].to_vec();
        let mut again = [0.0f32; NUM_BANDS];
        st.m.process(&window, &mut again);
        assert_eq!(first, again);
        assert!(first[..st.m.crossover].iter().any(|&v| v > 0.5), "the 60 Hz tone must be visible: {first:?}");
    }

    /// Cost evidence for the module header. Run with
    /// `cargo test --release bands::tests::cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cost_of_process_per_call() {
        let sig = broadband_signal(48_000.0, 48_000, 0.1, -3.0, 0x5EED);
        let mut m = BandMapper::new(48_000.0);
        let mut out = [0.0f32; NUM_BANDS];
        let calls = 1000;
        let t = std::time::Instant::now();
        let mut end = FFT_SIZE;
        for _ in 0..calls {
            m.push_history(&sig[end - 480..end]);
            m.process(&sig[end - FFT_SIZE..end], &mut out);
            end += 480;
            if end > sig.len() {
                end = FFT_SIZE;
            }
        }
        let per_call = t.elapsed() / calls;
        println!("push_history(480) + process (both FFTs): {per_call:?} per call");
    }

    #[test]
    fn works_at_other_sample_rates() {
        // Do not hardcode 48kHz - a virtual endpoint may report 44.1k.
        let mut m = BandMapper::new(44_100.0);
        let mut out = [0.0f32; NUM_BANDS];
        m.process(&sine(1000.0, 44_100.0, FFT_SIZE), &mut out);
        let peak = out.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
        assert!(peak.abs_diff(band_of(1000.0)) <= 2);
    }

    /// Synthesise a broadband signal with a caller-chosen dB/octave tilt
    /// (dense random-phase partials shaped via an FFT, not a handful of
    /// bin-aligned test tones) and scale it to a target overall RMS.
    fn broadband_signal(rate: f32, n: usize, target_rms: f32, db_per_octave: f32, seed: u64) -> Vec<f32> {
        // Real percussive/broadband content (cymbals, hi-hats, distortion,
        // brickwall-limited masters) is much closer to filtered NOISE than
        // to a handful of clean discrete tones: every bin in a band's range
        // carries its own random amount of energy. That distinction matters
        // a lot here because `BandMapper::process` takes the MAX bin within
        // a band's range, not an average or sum - so a wide high band built
        // from many independently-random bins can produce a much higher
        // peak than a smooth deterministic envelope would suggest, purely
        // from picking the max of many samples (an extreme-value effect).
        // So: shape genuine white noise in the frequency domain (random
        // magnitude AND phase per bin) rather than summing a few dozen
        // clean sines, to exercise that max-of-many-bins behaviour
        // faithfully.
        let mut state = seed | 1;
        let mut next_u = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 11) as f64 / (1u64 << 53) as f64) as f32
        };

        let mut planner = FftPlanner::<f32>::new();
        let fwd = planner.plan_fft_forward(n);
        let inv = planner.plan_fft_inverse(n);
        let scratch_len = fwd.get_inplace_scratch_len().max(inv.get_inplace_scratch_len());
        let mut scratch = vec![Complex32::new(0.0, 0.0); scratch_len];

        // Gaussian white noise via Box-Muller.
        let mut buf: Vec<Complex32> = (0..n)
            .map(|_| {
                let u1 = next_u().max(1e-9);
                let u2 = next_u();
                let r = (-2.0 * u1.ln()).sqrt();
                Complex32::new(r * (u2 * std::f32::consts::TAU).cos(), 0.0)
            })
            .collect();
        fwd.process_with_scratch(&mut buf, &mut scratch);

        // Shape the spectrum: zero outside the display's frequency range
        // (real music's energy budget is overwhelmingly inside it too),
        // apply the caller-chosen dB/octave tilt inside it. Bins above
        // Nyquist mirror the negative-frequency bin's magnitude, so use the
        // mirrored frequency for those.
        let bin_hz = rate / n as f32;
        for (bin, c) in buf.iter_mut().enumerate() {
            let f = bin as f32 * bin_hz;
            let f = if f > rate / 2.0 { rate - f } else { f };
            let env = if !(F_LOW..=F_HIGH).contains(&f) {
                0.0
            } else {
                let octaves_above_low = (f / F_LOW).log2();
                10f32.powf(db_per_octave * octaves_above_low / 20.0)
            };
            *c *= env;
        }

        inv.process_with_scratch(&mut buf, &mut scratch);
        // rustfft's inverse is unnormalised (scales amplitude by n).
        let mut mix: Vec<f32> = buf.iter().map(|c| c.re / n as f32).collect();

        let rms = (mix.iter().map(|x| x * x).sum::<f32>() / mix.len() as f32).sqrt();
        let scale = if rms > 0.0 { target_rms / rms } else { 0.0 };
        for s in mix.iter_mut() {
            *s *= scale;
        }
        mix
    }

    /// Regression test for the reported bug: a loud, heavily brickwall
    /// -limited/clipped broadband passage (RMS 1.2, clamped to the valid
    /// [-1,1] PCM range - this models the "loudness war" mastering that
    /// routinely clips real commercial tracks on purpose, not an
    /// impossible signal) must not read as a solid saturated block.
    ///
    /// The brief this fix responds to asked for "fewer than a third of the
    /// bands exceed 0.95". Measurement (see the fix report) shows that
    /// bound is unreachable by ANY valid-amplitude PCM signal under this
    /// formula shape - `BandMapper` takes the peak bin within each band's
    /// range, and Parseval's theorem caps how much per-bin magnitude a
    /// bounded signal can spread across the hundreds of bins the upper
    /// bands span; even driving this exact signal into much heavier
    /// clipping never crosses ~10-12 of 64 bands. So this test uses the
    /// strongest bound that is actually achievable and still discriminates
    /// the bug: the old constants (DB_FLOOR = -70.0, TILT_DB_PER_BAND =
    /// 0.30) peg 8 of 64 bands on this exact signal; the recalibrated
    /// constants peg 0, with wide margin even at higher RMS. `NUM_BANDS /
    /// 8` keeps that margin as a round, generous number rather than
    /// hard-coding the measured "0".
    #[test]
    fn loud_broadband_signal_does_not_peg_the_display() {
        let rate = 48_000.0;
        let raw = broadband_signal(rate, FFT_SIZE, 1.2, 0.0, 0xBEEF);
        let sig: Vec<f32> = raw.iter().map(|&s| s.clamp(-1.0, 1.0)).collect();

        let mut m = BandMapper::new(rate);
        let mut out = [0.0f32; NUM_BANDS];
        m.process(&sig, &mut out);

        let pegged = out.iter().filter(|&&v| v >= 0.95).count();
        assert!(
            pegged < NUM_BANDS / 8,
            "a loud broadband passage should not read as a solid block of pegged \
             bands - got {pegged}/{NUM_BANDS} bands >= 0.95: {out:?}"
        );
    }

    #[test]
    fn realistic_broadband_music_lights_the_treble_bands() {
        // Real music isn't one bin-aligned test tone: it spreads energy
        // across many partials, and treble sits far below bass in level.
        // Simulate that with several tones spanning bass to treble, scaled
        // to a realistic listening RMS (0.1 - well below full scale, unlike
        // the single full-scale sines the other tests use). Linear scaling
        // (the old, buggy behaviour) cannot make a signal this quiet visible
        // in the upper bands: a ~20dB-down treble partial needs a ~20dB
        // (10x) boost, and no linear multiplier in the old code got close
        // to that. This is the regression test for the measured bug: on
        // real music the display reached only ~35% height and only the
        // bottom third of the bars ever lit.
        let rate = 48_000.0;
        let freqs = [100.0, 300.0, 1000.0, 3000.0, 8000.0];
        let mut mix = vec![0.0f32; FFT_SIZE];
        for &f in &freqs {
            for (i, s) in mix.iter_mut().enumerate() {
                *s += (i as f32 / rate * f * std::f32::consts::TAU).sin();
            }
        }
        let rms = (mix.iter().map(|x| x * x).sum::<f32>() / mix.len() as f32).sqrt();
        let target_rms = 0.1;
        let scale = target_rms / rms;
        for s in mix.iter_mut() {
            *s *= scale;
        }

        let mut m = BandMapper::new(rate);
        let mut out = [0.0f32; NUM_BANDS];
        m.process(&mix, &mut out);

        let treble_max = out[41..].iter().cloned().fold(0.0f32, f32::max);
        assert!(
            treble_max > 0.25,
            "a realistic broadband mix (RMS {target_rms}) should light some band \
             above index 40 to at least 0.25 - got max {treble_max} in {:?}",
            &out[41..]
        );
    }

    // ---- Task 4: log-spaced bass via the second, longer FFT ----

    fn tone(freq: f32, secs: f32) -> Vec<f32> {
        (0..(48_000.0 * secs) as usize)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * freq * i as f32 / 48_000.0).sin())
            .collect()
    }

    /// Streams `sig` through a mapper in HOP-sized packets the way capture
    /// does (push_history + process) and returns the loudest band of the
    /// LAST output.
    fn peak_band(m: &mut BandMapper, sig: &[f32]) -> usize {
        let mut ring: Vec<f32> = Vec::new();
        let mut out = [0.0; NUM_BANDS];
        for chunk in sig.chunks(HOP) {
            m.push_history(chunk);
            ring.extend_from_slice(chunk);
            if ring.len() >= FFT_SIZE {
                let start = ring.len() - FFT_SIZE;
                m.process(&ring[start..], &mut out);
                ring.drain(..start);
            }
        }
        out.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0
    }

    #[test]
    fn kick_and_bass_are_at_least_six_bands_apart() {
        let mut m = BandMapper::new(48_000.0);
        let a = peak_band(&mut m, &tone(60.0, 1.0));
        let mut m = BandMapper::new(48_000.0);
        let b = peak_band(&mut m, &tone(120.0, 1.0));
        assert!(b >= a + 6, "60 Hz -> band {a}, 120 Hz -> band {b}");
    }

    #[test]
    fn sub_bass_does_not_vanish_or_collide() {
        let mut m = BandMapper::new(48_000.0);
        let a = peak_band(&mut m, &tone(48.0, 1.0));
        let mut m = BandMapper::new(48_000.0);
        let b = peak_band(&mut m, &tone(100.0, 1.0));
        assert!(a < b, "48 Hz band {a} must be below 100 Hz band {b}");
    }

    #[test]
    fn mids_and_highs_still_land_where_they_did() {
        // Log spacing from 40 Hz to 16 kHz over 64 bands: band = 64 * ln(f/40) / ln(400).
        let expect = |f: f32| (64.0 * (f / 40.0).ln() / (16_000.0f32 / 40.0).ln()).round() as usize;
        for f in [1_000.0, 2_000.0, 5_000.0] {
            let mut m = BandMapper::new(48_000.0);
            let got = peak_band(&mut m, &tone(f, 1.0));
            assert!(
                (got as i32 - expect(f) as i32).abs() <= 1,
                "{f} Hz: band {got}, expected ~{}",
                expect(f)
            );
        }
    }

    #[test]
    fn constructs_and_processes_at_44_1k_without_panicking() {
        let mut st = Stream::new(44_100.0);
        st.feed_all(&sine(100.0, 44_100.0, 44_100), HOP);
        assert!(st.out.iter().all(|v| v.is_finite()), "got {:?}", st.out);
    }
}
