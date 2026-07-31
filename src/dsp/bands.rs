// Task 6 introduces this type standalone; a later task wires BandMapper into
// the audio-capture -> render pipeline in main. Until then, rustc's
// binary-crate dead-code check flags these items as unused even though the
// tests below exercise them.
#![allow(dead_code)]

use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use std::sync::Arc;

pub const NUM_BANDS: usize = 64;
pub const FFT_SIZE: usize = 2048;
pub const HOP: usize = 512;
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
const DB_FLOOR: f32 = -70.0;
// Bass-compensating tilt, applied as a dB OFFSET (not a linear multiplier -
// see above: a linear multiplier can't recover tens of dB of deficit).
// Music's natural spectral rolloff means bass energy dominates and the
// treble end of the display would otherwise stay permanently dark even at
// a realistic listening level. ~0.30dB per band index, over NUM_BANDS - 1
// = 63 steps, is about +19dB of lift at the top band.
const TILT_DB_PER_BAND: f32 = 0.30;

pub struct BandMapper {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    scratch: Vec<Complex32>,
    fft_scratch: Vec<Complex32>,
    edges: Vec<usize>,
    sample_rate: f32,
}

impl BandMapper {
    pub fn new(sample_rate: f32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        // Hann window - reduces spectral leakage so a pure tone stays in one band.
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                let t = i as f32 / (FFT_SIZE - 1) as f32;
                0.5 - 0.5 * (t * std::f32::consts::TAU).cos()
            })
            .collect();

        // Log-spaced band edges in bin space, forced strictly ascending so the
        // low bands (where bins are sparse) never collapse to zero width.
        let bin_hz = sample_rate / FFT_SIZE as f32;
        let mut edges = Vec::with_capacity(NUM_BANDS + 1);
        let mut last = 0usize;
        for b in 0..=NUM_BANDS {
            let t = b as f32 / NUM_BANDS as f32;
            let f = F_LOW * (F_HIGH / F_LOW).powf(t);
            let bin = (f / bin_hz).round() as usize;
            let bin = bin.max(last + if b == 0 { 0 } else { 1 });
            edges.push(bin.min(FFT_SIZE / 2 - 1));
            last = *edges.last().unwrap();
        }

        let fft_scratch_len = fft.get_inplace_scratch_len();

        BandMapper {
            fft,
            window,
            scratch: vec![Complex32::new(0.0, 0.0); FFT_SIZE],
            fft_scratch: vec![Complex32::new(0.0, 0.0); fft_scratch_len],
            edges,
            sample_rate,
        }
    }

    /// `mono` must be exactly FFT_SIZE samples. Writes normalised 0.0..=1.0 levels.
    ///
    /// # Panics
    ///
    /// Panics (in both debug and release builds) if `mono.len() != FFT_SIZE`.
    pub fn process(&mut self, mono: &[f32], out: &mut [f32; NUM_BANDS]) {
        assert_eq!(mono.len(), FFT_SIZE, "BandMapper::process requires exactly FFT_SIZE samples");

        for i in 0..FFT_SIZE {
            self.scratch[i] = Complex32::new(mono[i] * self.window[i], 0.0);
        }
        self.fft.process_with_scratch(&mut self.scratch, &mut self.fft_scratch);

        // Hann coherent gain is 0.5, so a full-scale sine yields FFT_SIZE/4.
        // `norm` is chosen so a full-scale, bin-aligned sine reads mag ==
        // 1.0, i.e. 0dBFS - the reference the dB mapping below is built on.
        let norm = 4.0 / FFT_SIZE as f32;

        for b in 0..NUM_BANDS {
            let (lo, hi) = (self.edges[b], self.edges[b + 1]);
            let mut mag = 0.0f32;
            for bin in lo..hi.max(lo + 1) {
                mag = mag.max(self.scratch[bin].norm() * norm);
            }

            // mag == 0.0 (silence, or a bin with no energy) is handled
            // explicitly: log10(0) is -inf, and letting that literal -inf
            // flow into the tilt addition and division below would still
            // work out to -inf (never NaN), but doing it explicitly makes
            // the silence case obvious rather than relying on IEEE-754
            // infinity arithmetic to happen to do the right thing.
            let db = if mag > 0.0 { 20.0 * mag.log10() } else { f32::NEG_INFINITY };

            // dB-offset bass tilt (see TILT_DB_PER_BAND above) - band 0 gets
            // +0dB (no offset), the top band gets ~+19dB.
            let tilted_db = db + TILT_DB_PER_BAND * b as f32;

            // Map [DB_FLOOR, 0dB] onto [0.0, 1.0]. Guard NaN/inf explicitly:
            // a non-finite value reaching the ballistics smoother poisons
            // its state permanently (see ballistics.rs::Smoother::update),
            // so silence (-inf) and any other non-finite result must map to
            // exactly 0.0, never propagate.
            out[b] = if tilted_db.is_finite() {
                ((tilted_db - DB_FLOOR) / -DB_FLOOR).clamp(0.0, 1.0)
            } else {
                0.0
            };
        }
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
        let mut m = BandMapper::new(48_000.0);
        let mut lo = [0.0f32; NUM_BANDS];
        let mut hi = [0.0f32; NUM_BANDS];
        m.process(&sine(80.0, 48_000.0, FFT_SIZE), &mut lo);
        m.process(&sine(8000.0, 48_000.0, FFT_SIZE), &mut hi);
        let peak = |a: &[f32; NUM_BANDS]| {
            a.iter().enumerate().max_by(|x, y| x.1.partial_cmp(y.1).unwrap()).unwrap().0
        };
        assert!(peak(&lo) < NUM_BANDS / 3, "80Hz must land in the low third");
        assert!(peak(&hi) > NUM_BANDS * 2 / 3, "8kHz must land in the high third");
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
        let bin_hz = 48_000.0 / FFT_SIZE as f32;
        let bin_aligned_bin0_freq = m.edges[0] as f32 * bin_hz;
        m.process(&sine(bin_aligned_bin0_freq, 48_000.0, FFT_SIZE), &mut out);
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
        for pair in m.edges.windows(2) {
            assert!(pair[1] > pair[0], "edges must ascend: {:?}", m.edges);
        }
        assert_eq!(m.edges.len(), NUM_BANDS + 1);
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
}
