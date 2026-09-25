//! Shazam audio signature generation.
//!
//! A port of the fingerprinting code in SongRec (https://github.com/marin-m/SongRec, by marin-m,
//! GPL-3.0), which reverse-engineered Shazam's signature format. This file is the reason the repo
//! carries a GPL-3 licence. Changes from SongRec: the Hann window is computed rather than tabulated,
//! `rustfft`'s complex FFT is used instead of `realfft`, and the peak-slope assertion is a skip so a
//! rounding edge can never panic a background thread.
//!
//! Pipeline: 16 kHz mono s16 -> 128-sample hops into a 2048 ring -> Hann -> FFT -> power spectrum
//! -> frequency- and time-domain "spreading" -> peak picking 46 passes behind -> four frequency
//! bands of (pass, magnitude, sub-bin frequency) peaks -> binary TLV with CRC-32 -> base64 data URI.

use base64::Engine;
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

pub const TARGET_RATE: u32 = 16_000;
const DATA_URI_PREFIX: &str = "data:audio/vnd.shazam.sig;base64,";

/// Downsamples to 16 kHz by area-averaging: each output sample is the mean of the input samples
/// covering its interval. That is a box low-pass followed by decimation, adequate for a
/// fingerprint (Shazam's own client resamples crudely) and correct for non-integer ratios such as
/// 44.1 kHz -> 16 kHz.
pub fn resample_to_16k(src: &[f32], src_rate: u32) -> Vec<f32> {
    if src_rate == TARGET_RATE || src_rate == 0 || src.is_empty() {
        return src.to_vec();
    }
    let step = src_rate as f64 / TARGET_RATE as f64;
    let out_len = (src.len() as f64 / step).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for n in 0..out_len {
        let start = (n as f64 * step) as usize;
        let end = (((n + 1) as f64 * step) as usize).min(src.len()).max(start + 1);
        let sum: f32 = src[start..end].iter().sum();
        out.push(sum / (end - start) as f32);
    }
    out
}

/// SongRec's `HANNING_WINDOW_2048_MULTIPLIERS`: a symmetric Hann window of length 2050 with its two
/// zero endpoints trimmed, i.e. `0.5 * (1 - cos(2 pi (i+1) / 2049))`. Verified against the first,
/// middle and last table entries in the test below.
fn hann_2048() -> Vec<f32> {
    (0..2048)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f64::consts::PI * (i as f64 + 1.0) / 2049.0).cos()))
        .map(|v| v as f32)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrequencyPeak {
    pub fft_pass_number: u32,
    pub peak_magnitude: u16,
    pub corrected_peak_frequency_bin: u16,
}

/// Band 0: 250-520 Hz, 1: 520-1450, 2: 1450-3500, 3: 3500-5500.
#[derive(Debug, Clone)]
pub struct Signature {
    pub sample_rate_hz: u32,
    pub number_samples: u32,
    pub bands: [Vec<FrequencyPeak>; 4],
}

struct Generator {
    ring: [i16; 2048],
    ring_index: usize,
    hann: Vec<f32>,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    scratch: Vec<Complex<f32>>,
    /// 256-deep ring of power spectra.
    fft_outputs: Vec<[f32; 1025]>,
    fft_index: u8,
    /// 256-deep ring of spread spectra.
    spread: Vec<[f32; 1025]>,
    spread_index: u8,
    passes_done: u32,
    sig: Signature,
}

impl Signature {
    pub fn from_mono_16k(samples: &[f32]) -> Signature {
        let mut g = Generator {
            ring: [0; 2048],
            ring_index: 0,
            hann: hann_2048(),
            fft: FftPlanner::<f32>::new().plan_fft_forward(2048),
            scratch: vec![Complex::new(0.0, 0.0); 2048],
            fft_outputs: vec![[0.0; 1025]; 256],
            fft_index: 0,
            spread: vec![[0.0; 1025]; 256],
            spread_index: 0,
            passes_done: 0,
            sig: Signature {
                sample_rate_hz: TARGET_RATE,
                number_samples: samples.len() as u32,
                bands: Default::default(),
            },
        };
        let s16: Vec<i16> = samples
            .iter()
            .map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        for chunk in s16.chunks_exact(128) {
            g.do_fft(chunk);
            g.do_peak_spreading();
            g.passes_done += 1;
            if g.passes_done >= 46 {
                g.do_peak_recognition();
            }
        }
        g.sig
    }

    pub fn sample_ms(&self) -> u32 {
        (self.number_samples as f64 / self.sample_rate_hz as f64 * 1000.0) as u32
    }

    pub fn encode_to_binary(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::with_capacity(4096);
        let put32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
        put32(&mut out, 0xcafe2580); // magic1
        put32(&mut out, 0); // crc32, patched below
        put32(&mut out, 0); // size minus header, patched below
        put32(&mut out, 0x94119c00); // magic2
        put32(&mut out, 0);
        put32(&mut out, 0);
        put32(&mut out, 0);
        let rate_id: u32 = match self.sample_rate_hz {
            8000 => 1,
            11025 => 2,
            16000 => 3,
            32000 => 4,
            44100 => 5,
            48000 => 6,
            _ => 3,
        };
        put32(&mut out, rate_id << 27);
        put32(&mut out, 0);
        put32(&mut out, 0);
        put32(&mut out, self.number_samples + (self.sample_rate_hz as f32 * 0.24) as u32);
        put32(&mut out, (15 << 19) + 0x40000);
        // First TLV: fixed type, repeats size-minus-header.
        put32(&mut out, 0x40000000);
        put32(&mut out, 0); // patched below
        for (band, peaks) in self.bands.iter().enumerate() {
            if peaks.is_empty() {
                continue;
            }
            let mut body: Vec<u8> = Vec::with_capacity(peaks.len() * 5);
            let mut pass = 0u32;
            for p in peaks {
                let fp = p.fft_pass_number.max(pass);
                if fp - pass >= 255 {
                    body.push(0xff);
                    body.extend_from_slice(&fp.to_le_bytes());
                    pass = fp;
                }
                body.push((fp - pass) as u8);
                body.extend_from_slice(&p.peak_magnitude.to_le_bytes());
                body.extend_from_slice(&p.corrected_peak_frequency_bin.to_le_bytes());
                pass = fp;
            }
            put32(&mut out, 0x60030040 + band as u32);
            put32(&mut out, body.len() as u32);
            out.extend_from_slice(&body);
            for _ in 0..((4 - body.len() % 4) % 4) {
                out.push(0);
            }
        }
        let size_minus_header = (out.len() - 48) as u32;
        out[8..12].copy_from_slice(&size_minus_header.to_le_bytes());
        out[52..56].copy_from_slice(&size_minus_header.to_le_bytes());
        let crc = crc32(&out[8..]);
        out[4..8].copy_from_slice(&crc.to_le_bytes());
        out
    }

    pub fn encode_to_uri(&self) -> String {
        format!(
            "{DATA_URI_PREFIX}{}",
            base64::prelude::BASE64_STANDARD.encode(self.encode_to_binary())
        )
    }
}

impl Generator {
    fn do_fft(&mut self, chunk: &[i16]) {
        self.ring[self.ring_index..self.ring_index + 128].copy_from_slice(chunk);
        self.ring_index = (self.ring_index + 128) & 2047;
        for (i, m) in self.hann.iter().enumerate() {
            let s = self.ring[(i + self.ring_index) & 2047] as f32 * m;
            self.scratch[i] = Complex::new(s, 0.0);
        }
        self.fft.process(&mut self.scratch);
        let out = &mut self.fft_outputs[self.fft_index as usize];
        for (o, c) in out.iter_mut().zip(self.scratch.iter().take(1025)) {
            *o = ((c.re * c.re + c.im * c.im) / (1u32 << 17) as f32).max(0.0000000001);
        }
        self.fft_index = self.fft_index.wrapping_add(1);
    }

    fn do_peak_spreading(&mut self) {
        let src = self.fft_outputs[self.fft_index.wrapping_sub(1) as usize];
        let mut spread = src;
        for i in 0..=1022 {
            spread[i] = spread[i].max(spread[i + 1]).max(spread[i + 2]);
        }
        self.spread[self.spread_index as usize] = spread;
        for former in [1u8, 3, 6] {
            let idx = self.spread_index.wrapping_sub(former) as usize;
            for i in 0..=1024 {
                self.spread[idx][i] = self.spread[idx][i].max(spread[i]);
            }
        }
        self.spread_index = self.spread_index.wrapping_add(1);
    }

    fn do_peak_recognition(&mut self) {
        let fft_m46 = &self.fft_outputs[self.fft_index.wrapping_sub(46) as usize];
        let spread_m49 = &self.spread[self.spread_index.wrapping_sub(49) as usize];
        for bin in 10..=1014usize {
            if !(fft_m46[bin] >= 1.0 / 64.0 && fft_m46[bin] >= spread_m49[bin - 1]) {
                continue;
            }
            let mut max_nb: f32 = 0.0;
            for off in [-10i32, -7, -4, -3, 1, 2, 5, 8] {
                max_nb = max_nb.max(spread_m49[(bin as i32 + off) as usize]);
            }
            if fft_m46[bin] <= max_nb {
                continue;
            }
            let mut max_other = max_nb;
            for off in [-53i32, -45, 165, 172, 179, 186, 193, 200, 214, 221, 228, 235, 242, 249] {
                let idx = ((self.spread_index as i32 + off) & 255) as usize;
                max_other = max_other.max(self.spread[idx][bin - 1]);
            }
            if fft_m46[bin] <= max_other {
                continue;
            }
            let pass = self.passes_done - 46;
            let mag = |x: f32| x.ln().max(1.0 / 64.0) * 1477.3 + 6144.0;
            let m = mag(fft_m46[bin]);
            let before = mag(fft_m46[bin - 1]);
            let after = mag(fft_m46[bin + 1]);
            let var1 = m * 2.0 - before - after;
            if var1 <= 0.0 {
                // SongRec asserts here; a rounding edge must not take a background thread down.
                continue;
            }
            let var2 = (after - before) * 32.0 / var1;
            let corrected_bin = ((bin as i32 * 64) + var2 as i32) as u16;
            let hz = corrected_bin as f32 * (16000.0 / 2.0 / 1024.0 / 64.0);
            let band = match hz as i32 {
                250..=519 => 0,
                520..=1449 => 1,
                1450..=3499 => 2,
                3500..=5500 => 3,
                _ => continue,
            };
            self.sig.bands[band].push(FrequencyPeak {
                fft_pass_number: pass,
                peak_magnitude: m as u16,
                corrected_peak_frequency_bin: corrected_bin,
            });
        }
    }
}

/// Standard CRC-32 (IEEE, reflected, 0xEDB88320), as `crc32fast` computes it. Ten lines beats a
/// dependency.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, rate: u32, secs: f32, amp: f32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        (0..n)
            .map(|i| amp * (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin())
            .collect()
    }

    /// Dominant frequency by zero-crossing count - good enough to tell 1 kHz from 3 kHz.
    fn dominant_hz(s: &[f32], rate: u32) -> f32 {
        let crossings = s.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        crossings as f32 / 2.0 / (s.len() as f32 / rate as f32)
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn hann_matches_songrec_endpoints_and_peak() {
        let w = hann_2048();
        assert!((w[0] - 0.0000023508).abs() < 1e-9, "{}", w[0]);
        assert!((w[2047] - 0.0000023508).abs() < 1e-9);
        assert!((w[1023] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn resample_48k_keeps_the_tone_and_thirds_the_length() {
        let src = tone(1000.0, 48_000, 1.0, 0.5);
        let out = resample_to_16k(&src, 48_000);
        assert_eq!(out.len(), 16_000);
        let hz = dominant_hz(&out, 16_000);
        assert!((hz - 1000.0).abs() < 20.0, "{hz}");
    }

    #[test]
    fn resample_44k1_handles_a_non_integer_ratio() {
        let src = tone(1000.0, 44_100, 1.0, 0.5);
        let out = resample_to_16k(&src, 44_100);
        assert!((out.len() as i64 - 16_000).abs() <= 1, "{}", out.len());
        let hz = dominant_hz(&out, 16_000);
        assert!((hz - 1000.0).abs() < 20.0, "{hz}");
    }

    #[test]
    fn resample_16k_is_identity() {
        let src = tone(440.0, 16_000, 0.5, 0.5);
        assert_eq!(resample_to_16k(&src, 16_000), src);
    }

    #[test]
    fn two_tones_land_in_their_bands() {
        // 1 kHz -> band 1 (520-1450), 3 kHz -> band 2 (1450-3500). 6 s at 16 kHz.
        let a = tone(1000.0, 16_000, 6.0, 0.4);
        let b = tone(3000.0, 16_000, 6.0, 0.4);
        let mix: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x + y).collect();
        let sig = Signature::from_mono_16k(&mix);
        assert_eq!(sig.number_samples, mix.len() as u32);
        // The detector also admits faint noise-floor peaks (magnitude ~7000, as SongRec's does), so
        // the assertions are about STRONG peaks: they must be the two tones, in the right bands, at
        // the right frequencies, and nowhere else.
        let strong = |b: usize| -> Vec<f32> {
            sig.bands[b]
                .iter()
                .filter(|p| p.peak_magnitude > 30_000)
                .map(|p| p.corrected_peak_frequency_bin as f32 * (8000.0 / 1024.0 / 64.0))
                .collect()
        };
        assert!(!strong(1).is_empty(), "no strong peaks in 520-1450 band");
        assert!(!strong(2).is_empty(), "no strong peaks in 1450-3500 band");
        assert!(strong(0).is_empty(), "strong peaks in 250-520 band: {:?}", strong(0));
        assert!(strong(3).is_empty(), "strong peaks in 3500-5500 band: {:?}", strong(3));
        for hz in strong(1) {
            assert!((hz - 1000.0).abs() < 15.0, "{hz}");
        }
        for hz in strong(2) {
            assert!((hz - 3000.0).abs() < 15.0, "{hz}");
        }
    }

    #[test]
    fn silence_produces_no_peaks() {
        let sig = Signature::from_mono_16k(&vec![0.0; 16_000 * 4]);
        assert!(sig.bands.iter().all(|b| b.is_empty()));
    }

    #[test]
    fn encode_round_trips_through_songrec_decoder() {
        let sig = Signature {
            sample_rate_hz: 16_000,
            number_samples: 96_000,
            bands: [
                vec![FrequencyPeak { fft_pass_number: 3, peak_magnitude: 7000, corrected_peak_frequency_bin: 2000 }],
                vec![
                    FrequencyPeak { fft_pass_number: 10, peak_magnitude: 7100, corrected_peak_frequency_bin: 8000 },
                    FrequencyPeak { fft_pass_number: 300, peak_magnitude: 7200, corrected_peak_frequency_bin: 8100 },
                ],
                vec![],
                vec![FrequencyPeak { fft_pass_number: 1, peak_magnitude: 6500, corrected_peak_frequency_bin: 30000 }],
            ],
        };
        let bin = sig.encode_to_binary();
        assert_eq!(&bin[0..4], &0xcafe2580u32.to_le_bytes());
        let back = decode_for_test(&bin);
        assert_eq!(back.number_samples, 96_000);
        assert_eq!(back.sample_rate_hz, 16_000);
        for band in 0..4 {
            assert_eq!(back.bands[band].len(), sig.bands[band].len(), "band {band}");
            for (x, y) in back.bands[band].iter().zip(&sig.bands[band]) {
                assert_eq!(
                    (x.fft_pass_number, x.peak_magnitude, x.corrected_peak_frequency_bin),
                    (y.fft_pass_number, y.peak_magnitude, y.corrected_peak_frequency_bin)
                );
            }
        }
        assert!(sig.encode_to_uri().starts_with("data:audio/vnd.shazam.sig;base64,"));
        assert_eq!(sig.sample_ms(), 6000);
    }

    /// Port of SongRec's `decode_from_binary`, test-only, so the encoder is checked against an
    /// independent reading of the format rather than against itself.
    fn decode_for_test(data: &[u8]) -> Signature {
        let u32_at = |p: usize| u32::from_le_bytes(data[p..p + 4].try_into().unwrap());
        let u16_at = |p: usize| u16::from_le_bytes(data[p..p + 2].try_into().unwrap());
        assert_eq!(u32_at(0), 0xcafe2580);
        assert_eq!(u32_at(8) as usize, data.len() - 48);
        assert_eq!(u32_at(4), crc32(&data[8..]));
        assert_eq!(u32_at(12), 0x94119c00);
        let rate = match u32_at(28) >> 27 {
            3 => 16_000,
            other => panic!("rate id {other}"),
        };
        let number_samples = u32_at(40) - (rate as f32 * 0.24) as u32;
        assert_eq!(u32_at(48), 0x40000000);
        assert_eq!(u32_at(52) as usize, data.len() - 48);
        let mut bands: [Vec<FrequencyPeak>; 4] = Default::default();
        let mut pos = 56;
        while pos < data.len() {
            let band = (u32_at(pos) - 0x60030040) as usize;
            let size = u32_at(pos + 4) as usize;
            pos += 8;
            let end = pos + size;
            let mut pass = 0u32;
            while pos < end {
                let off = data[pos];
                pos += 1;
                if off == 0xff {
                    pass = u32_at(pos);
                    pos += 4;
                } else {
                    pass += off as u32;
                    bands[band].push(FrequencyPeak {
                        fft_pass_number: pass,
                        peak_magnitude: u16_at(pos),
                        corrected_peak_frequency_bin: u16_at(pos + 2),
                    });
                    pos += 4;
                }
            }
            pos += (4 - size % 4) % 4;
        }
        Signature { sample_rate_hz: rate, number_samples, bands }
    }
}
