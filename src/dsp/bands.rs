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
        let norm = 4.0 / FFT_SIZE as f32;

        for b in 0..NUM_BANDS {
            let (lo, hi) = (self.edges[b], self.edges[b + 1]);
            let mut peak = 0.0f32;
            for bin in lo..hi.max(lo + 1) {
                peak = peak.max(self.scratch[bin].norm() * norm);
            }
            // Bass-weighted tilt: without it low-frequency energy dominates and
            // the top two-thirds of the display barely moves.
            let t = b as f32 / (NUM_BANDS - 1) as f32;
            let tilt = 1.0 + 2.2 * t;
            out[b] = (peak * tilt).clamp(0.0, 1.0);
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
        // caught: band 0 always has tilt == 1.0 (t = 0 / (NUM_BANDS - 1)),
        // so a full-scale sine placed exactly on band 0's first bin is
        // untouched by the bass tilt and should read out at very close to
        // the Hann-coherent-gain-derived amplitude of 1.0 (norm = 4 /
        // FFT_SIZE is chosen to exactly compensate a Hann window's 0.5
        // coherent gain for a bin-aligned full-scale sine).
        let bin_hz = 48_000.0 / FFT_SIZE as f32;
        let bin_aligned_bin0_freq = m.edges[0] as f32 * bin_hz;
        m.process(&sine(bin_aligned_bin0_freq, 48_000.0, FFT_SIZE), &mut out);
        assert!(
            out[0] > 0.9,
            "a full-scale, bin-aligned tone in band 0 (tilt == 1.0 there) \
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
}
