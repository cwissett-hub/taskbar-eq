# Song Identification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A hotkey or menu click fingerprints the PC's current audio with Shazam, names the song in the taskbar banner, keeps every find, and opens a dark cyberpunk HTML history page with Spotify / Apple Music / YouTube / Shazam links per song.

**Architecture:** A `Recorder` sink on the existing WASAPI capture thread collects up to 12 s of mono audio. `dsp::shazam_sig` (a port of SongRec) turns it into a Shazam binary signature; `net::shazam` POSTs it and parses the reply into a `Find`; `songs` appends finds to `songs.jsonl`; `history` renders `songs.html`; `identify` orchestrates on its own thread and publishes banner text. One new hotkey slot, one new tray submenu.

**Tech Stack:** Rust 2021, `windows` 0.62 (already), `rustfft` (already), `serde`/`toml` (already), NEW: `ureq` 3 (rustls), `serde_json` 1, `base64` 0.22. No async runtime.

**Spec:** `docs/superpowers/specs/2026-09-25-song-identify-design.md`

## Global Constraints

- Single portable exe; no Python, no sidecar processes, no extra files beside the exe.
- Nothing in this feature may stop the app starting or the visualiser rendering: every failure is a log line and a "no match" banner.
- No file I/O inside a wndproc. `identify::request()` only spawns a thread.
- Existing test discipline: every new pure function gets unit tests; no automated GUI/browser tests (user's standing preference).
- Repo becomes GPL-3 (SongRec is GPL-3). Attribution in `src/dsp/shazam_sig.rs` header and README.
- Commit after every task once its tests pass (user's standing instruction: one commit per verified feature). Keep `TODO.md` current.
- `cargo test` must stay green; the `SLOTS` round-trip tests in `config.rs` must still pass with 8 slots.
- Not a company project: the HTML page uses a dark cyberpunk/vaporwave palette, never the yellow-room theme.

## Deviations from the spec (decided while planning)

- **No `songs_hidden.json`.** The spec says hides are client-side `localStorage` only, so a hidden-keys file on disk has no reader. Dropped.
- **No SongRec golden signature.** The Hann table is computed, not copied, so signatures differ from SongRec in the last float digits and a byte-for-byte golden would fail for no real reason. Instead: encoder round-trips through a test-only port of SongRec's decoder, and the peak picker is tested on synthetic tones. Correctness against Shazam itself is the live check.
- **Banner text** uses "Title - Artist" to match the existing track banner, not "Title, Artist".
- **The raw Shazam reply is always saved** to `%APPDATA%\taskbar-eq\last_shazam.json` (overwritten each time). It is the only way to diagnose an API change and it gives us a real fixture for the parser test after the first live match.

## Review Focus

Inputs the spec implies but no test would otherwise exercise, most likely to bite first. Each has a test pinned to the owning task below.

1. **Silence or a dead capture thread when the key is pressed** - the recorder gets nothing; must end as "no match" in ~12 s, not hang forever with "listening..." on screen. (Task 6: timeout test on `wait_for_samples`.)
2. **A title containing `</script>` or `"`** reaching the HTML page - must not break out of the embedded JSON or the page. (Task 8: escaping test.)
3. **Two presses inside one identification** - second must be ignored, first must still complete and clear the busy flag even on error. (Task 6: busy-flag test.)
4. **Shazam returns 200 with `{"matches":[]}` versus a 200 with an unexpected body** - both must be non-fatal but the log must distinguish them. (Task 3: parser tests for both.)
5. **A mix format that is not 48 kHz** (44.1 kHz devices are common) - the resampler must produce 16 kHz correctly for a non-integer ratio. (Task 2: resample test at 44100.)

---

### Task 1: Dependencies, licence, module scaffolding

**Files:**
- Modify: `Cargo.toml`
- Create: `LICENSE`
- Create: `src/net/mod.rs`, `src/net/shazam.rs` (empty stub), `src/songs.rs` (stub), `src/history.rs` (stub), `src/identify.rs` (stub), `src/dsp/shazam_sig.rs` (stub)
- Modify: `src/main.rs:13-20` (mod lines), `src/dsp/mod.rs`

**Interfaces:**
- Produces: module paths `crate::dsp::shazam_sig`, `crate::net::shazam`, `crate::songs`, `crate::history`, `crate::identify` that later tasks fill in.

- [ ] **Step 1: Add dependencies**

In `Cargo.toml` under `[dependencies]` add:

```toml
serde_json = "1.0"
base64 = "0.22"
# Blocking HTTPS for the Shazam call. rustls so the exe stays self-contained; no async runtime.
ureq = { version = "3", default-features = false, features = ["rustls", "json"] }
```

- [ ] **Step 2: Add the licence**

Download the GPL-3 text into `LICENSE`:

```powershell
Invoke-WebRequest https://www.gnu.org/licenses/gpl-3.0.txt -OutFile LICENSE
```

If that is blocked, copy `LICENSE` from the SongRec clone in the scratchpad (it is the verbatim GPL-3 text). Add to `Cargo.toml` `[package]`: `license = "GPL-3.0-or-later"`.

- [ ] **Step 3: Create module stubs**

`src/net/mod.rs`:
```rust
//! Network: the one outbound call this app makes, to Shazam.
pub mod shazam;
```

`src/net/shazam.rs`, `src/songs.rs`, `src/history.rs`, `src/identify.rs`, `src/dsp/shazam_sig.rs`: each a one-line doc comment for now (`//! filled in by a later task`).

Add to `src/dsp/mod.rs`: `pub mod shazam_sig;`
Add to `src/main.rs` after `mod win;`:
```rust
mod net;
mod songs;
mod history;
mod identify;
```

- [ ] **Step 4: Build**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles (warnings about unused modules are fine for now). If the registry 401s, run `snapaccess` to refresh credentials first (see memory: registry creds expire ~20 h).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock LICENSE src/net src/songs.rs src/history.rs src/identify.rs src/dsp/shazam_sig.rs src/dsp/mod.rs src/main.rs
git commit -m "identify: dependencies, GPL-3 licence and module scaffolding for song identification"
```

---

### Task 2: Shazam signature generator (`dsp::shazam_sig`)

**Files:**
- Create: `src/dsp/shazam_sig.rs` (replace stub)

**Interfaces:**
- Produces:
  ```rust
  pub const TARGET_RATE: u32 = 16_000;
  pub fn resample_to_16k(src: &[f32], src_rate: u32) -> Vec<f32>;
  pub struct FrequencyPeak { pub fft_pass_number: u32, pub peak_magnitude: u16, pub corrected_peak_frequency_bin: u16 }
  pub struct Signature { pub sample_rate_hz: u32, pub number_samples: u32, pub bands: [Vec<FrequencyPeak>; 4] }
  impl Signature {
      pub fn from_mono_16k(samples: &[f32]) -> Signature;
      pub fn encode_to_binary(&self) -> Vec<u8>;
      pub fn encode_to_uri(&self) -> String;   // "data:audio/vnd.shazam.sig;base64,..."
      pub fn sample_ms(&self) -> u32;
  }
  pub fn crc32(data: &[u8]) -> u32;
  ```

- [ ] **Step 1: Write the failing tests**

Append to `src/dsp/shazam_sig.rs`:

```rust
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
        assert!(!sig.bands[1].is_empty(), "no peaks in 520-1450 band");
        assert!(!sig.bands[2].is_empty(), "no peaks in 1450-3500 band");
        assert!(sig.bands[0].is_empty(), "spurious low-band peaks");
        assert!(sig.bands[3].is_empty(), "spurious high-band peaks");
        // corrected bin * 64 -> Hz: bin = hz / (8000/1024) * 64
        for p in &sig.bands[1] {
            let hz = p.corrected_peak_frequency_bin as f32 * (8000.0 / 1024.0 / 64.0);
            assert!((hz - 1000.0).abs() < 15.0, "{hz}");
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
                assert_eq!((x.fft_pass_number, x.peak_magnitude, x.corrected_peak_frequency_bin),
                           (y.fft_pass_number, y.peak_magnitude, y.corrected_peak_frequency_bin));
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
        let rate = match u32_at(28) >> 27 { 3 => 16_000, other => panic!("rate id {other}") };
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test shazam_sig 2>&1 | tail -5`
Expected: compile error, functions not defined.

- [ ] **Step 3: Implement**

Replace `src/dsp/shazam_sig.rs` body (above the tests) with:

```rust
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
    fft_outputs: Vec<[f32; 1025]>,   // 256 ring
    fft_index: u8,
    spread: Vec<[f32; 1025]>,        // 256 ring
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
            8000 => 1, 11025 => 2, 16000 => 3, 32000 => 4, 44100 => 5, 48000 => 6,
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test shazam_sig 2>&1 | tail -15`
Expected: all 8 tests pass. If `two_tones_land_in_their_bands` finds no peaks, first check the s16 scaling (a 0.4 amplitude tone must be ~13000 counts; the `>= 1/64` threshold on power/2^17 assumes s16 scale).

- [ ] **Step 5: Commit**

```bash
git add src/dsp/shazam_sig.rs
git commit -m "identify: Shazam signature generator ported from SongRec, with round-trip and tone tests"
```

---

### Task 3: Shazam HTTP client and reply parser (`net::shazam`)

**Files:**
- Create: `src/net/shazam.rs` (replace stub)
- Create: `tests/fixtures/identify/shazam_match.json`, `tests/fixtures/identify/shazam_nomatch.json`

**Interfaces:**
- Consumes: `crate::dsp::shazam_sig::Signature` (`encode_to_uri`, `sample_ms`).
- Produces:
  ```rust
  pub enum Outcome { Match(crate::songs::Find), NoMatch, Error(String) }
  pub fn recognize(sig: &Signature) -> Outcome;                 // live network call
  pub fn parse_reply(body: &str, now_unix: i64) -> Outcome;    // pure
  pub fn request_body(sig: &Signature, now_ms: u64) -> String; // pure
  pub fn random_uuid() -> String;                              // pure-ish
  ```
- Depends on `songs::Find` from Task 4 - **implement Task 4's `Find` struct first** (it is a plain struct; Task 4's tests can come after).

- [ ] **Step 1: Write the fixtures**

`tests/fixtures/identify/shazam_nomatch.json`:
```json
{"matches":[],"timestamp":1758800000000,"timezone":"Europe/Paris","tagid":"00000000-0000-0000-0000-000000000000"}
```

`tests/fixtures/identify/shazam_match.json` (shape of a real reply, trimmed; replace with a real `last_shazam.json` after the first live match in Task 9):
```json
{
  "matches": [{"id": "123", "offset": 5.1, "timeskew": 0.0001, "frequencyskew": 0.0}],
  "timestamp": 1758800000000,
  "timezone": "Europe/Paris",
  "tagid": "00000000-0000-0000-0000-000000000000",
  "track": {
    "layout": "5",
    "type": "MUSIC",
    "key": "5933917",
    "title": "Resonance",
    "subtitle": "HOME",
    "isrc": "QZDA61474185",
    "url": "https://www.shazam.com/track/5933917/resonance",
    "share": {"href": "https://www.shazam.com/track/5933917/resonance", "subject": "Resonance - HOME"},
    "images": {"coverart": "https://is1-ssl.mzstatic.com/image/thumb/x/400x400bb.jpg", "coverarthq": "https://is1-ssl.mzstatic.com/image/thumb/x/800x800bb.jpg"},
    "hub": {
      "type": "APPLEMUSIC",
      "options": [{"caption": "OPEN", "actions": [{"name": "hub:applemusic:deeplink", "type": "applemusicopen", "uri": "https://music.apple.com/gb/album/resonance/1234?i=5678"}]}],
      "providers": [
        {"type": "SPOTIFY", "actions": [{"name": "hub:spotify:searchdeeplink", "type": "uri", "uri": "spotify:search:Resonance%20HOME"}]},
        {"type": "DEEZER", "actions": [{"name": "hub:deezer:searchdeeplink", "type": "uri", "uri": "deezer-query://www.deezer.com/play?query=x"}]}
      ]
    },
    "sections": [
      {"type": "SONG", "metadata": [{"title": "Album", "text": "Odyssey"}, {"title": "Label", "text": "Midwest Collective"}, {"title": "Released", "text": "2014"}]}
    ]
  }
}
```

- [ ] **Step 2: Write the failing tests**

Append to `src/net/shazam.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::shazam_sig::{FrequencyPeak, Signature};

    fn fixture(name: &str) -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/identify")
            .join(name);
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn parses_a_match_into_a_find() {
        let Outcome::Match(f) = parse_reply(&fixture("shazam_match.json"), 1_700_000_000) else {
            panic!("expected a match");
        };
        assert_eq!(f.title, "Resonance");
        assert_eq!(f.artist, "HOME");
        assert_eq!(f.album.as_deref(), Some("Odyssey"));
        assert_eq!(f.shazam_key, "5933917");
        assert_eq!(f.isrc.as_deref(), Some("QZDA61474185"));
        assert_eq!(f.when, 1_700_000_000);
        assert!(f.cover_url.as_deref().unwrap().starts_with("https://is1-ssl"));
        assert_eq!(f.shazam_url.as_deref(), Some("https://www.shazam.com/track/5933917/resonance"));
        assert!(f.apple_music_url.as_deref().unwrap().starts_with("https://music.apple.com/"));
        // A search deeplink is not a track link; it must not be reported as one.
        assert_eq!(f.spotify_uri, None);
        assert_eq!(f.app_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn spotify_track_uri_is_kept_when_present() {
        let body = fixture("shazam_match.json")
            .replace("spotify:search:Resonance%20HOME", "spotify:track:0Zxu4C6zA6Qw6Xy");
        let Outcome::Match(f) = parse_reply(&body, 0) else { panic!() };
        assert_eq!(f.spotify_uri.as_deref(), Some("spotify:track:0Zxu4C6zA6Qw6Xy"));
    }

    #[test]
    fn empty_matches_is_no_match() {
        assert!(matches!(parse_reply(&fixture("shazam_nomatch.json"), 0), Outcome::NoMatch));
    }

    #[test]
    fn garbage_and_shape_changes_are_errors_not_no_match() {
        assert!(matches!(parse_reply("<html>rate limited</html>", 0), Outcome::Error(_)));
        // Valid JSON, matches present, but no track object: an API change, not silence.
        assert!(matches!(parse_reply(r#"{"matches":[{"id":"1"}]}"#, 0), Outcome::Error(_)));
    }

    #[test]
    fn request_body_has_the_fields_shazam_requires() {
        let sig = Signature {
            sample_rate_hz: 16_000,
            number_samples: 64_000,
            bands: [vec![FrequencyPeak { fft_pass_number: 1, peak_magnitude: 7000, corrected_peak_frequency_bin: 3000 }], vec![], vec![], vec![]],
        };
        let body: serde_json::Value = serde_json::from_str(&request_body(&sig, 1_700_000_000_123)).unwrap();
        assert_eq!(body["signature"]["samplems"], 4000);
        assert!(body["signature"]["uri"].as_str().unwrap().starts_with("data:audio/vnd.shazam.sig;base64,"));
        assert!(body["timestamp"].is_number());
        assert!(body["geolocation"]["latitude"].is_number());
        assert_eq!(body["timezone"], "Europe/Paris");
    }

    #[test]
    fn uuid_has_the_v4_shape_and_varies() {
        let a = random_uuid();
        let b = random_uuid();
        assert_eq!(a.len(), 36);
        assert_eq!(a.as_bytes()[14], b'4');
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test net::shazam 2>&1 | tail -5`
Expected: compile errors.

- [ ] **Step 4: Implement**

`src/net/shazam.rs` (above tests):

```rust
//! The Shazam call, as SongRec and ShazamIO make it: POST a signature to the undocumented
//! discovery endpoint and read the track back. Unofficial, so every field read is optional except
//! the three that define a match (key, title, subtitle), and the raw body is saved to disk so a
//! format change can be diagnosed from `last_shazam.json` rather than guessed at.

use crate::dsp::shazam_sig::Signature;
use crate::songs::Find;
use serde_json::Value;

pub enum Outcome {
    Match(Find),
    NoMatch,
    Error(String),
}

const USER_AGENT: &str = "Dalvik/2.1.0 (Linux; U; Android 10; Pixel 4 Build/QQ3A.200805.001)";

/// Random v4-shaped UUID from the OS clock and a splitmix64 step. Not cryptographic; Shazam only
/// wants two distinct-looking ids per request.
pub fn random_uuid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEED: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    let mut x = SEED.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed) ^ t;
    let mut next = || {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let (a, b) = (next(), next());
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_le_bytes());
    bytes[8..].copy_from_slice(&b.to_le_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h = |r: std::ops::Range<usize>| bytes[r].iter().map(|b| format!("{b:02x}")).collect::<String>();
    format!("{}-{}-{}-{}-{}", h(0..4), h(4..6), h(6..8), h(8..10), h(10..16))
}

pub fn request_body(sig: &Signature, now_ms: u64) -> String {
    serde_json::json!({
        "geolocation": { "altitude": 300, "latitude": 45, "longitude": 2 },
        "signature": {
            "samplems": sig.sample_ms(),
            "timestamp": now_ms as u32,
            "uri": sig.encode_to_uri()
        },
        "timestamp": now_ms as u32,
        "timezone": "Europe/Paris"
    })
    .to_string()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Where the last raw reply is kept, for diagnosis and as a parser fixture.
pub fn last_reply_path() -> std::path::PathBuf {
    crate::config::Config::dir().join("last_shazam.json")
}

/// Blocking. Call from the identify thread only.
pub fn recognize(sig: &Signature) -> Outcome {
    let url = format!(
        "https://amp.shazam.com/discovery/v5/en/US/android/-/tag/{}/{}?sync=true&webv3=true&sampling=true&connected=&shazamapiversion=v3&sharehub=true&video=v3",
        random_uuid().to_uppercase(),
        random_uuid()
    );
    let body = request_body(sig, now_ms());
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .http_status_as_error(false)
        .build()
        .new_agent();
    let resp = agent
        .post(&url)
        .header("User-Agent", USER_AGENT)
        .header("Content-Language", "en_US")
        .header("Content-Type", "application/json")
        .send(body.as_bytes());
    let mut resp = match resp {
        Ok(r) => r,
        Err(e) => return Outcome::Error(format!("request failed: {e}")),
    };
    let status = resp.status().as_u16();
    let text = match resp.body_mut().read_to_string() {
        Ok(t) => t,
        Err(e) => return Outcome::Error(format!("HTTP {status}, body unreadable: {e}")),
    };
    let _ = std::fs::create_dir_all(crate::config::Config::dir());
    let _ = std::fs::write(last_reply_path(), &text);
    if status == 429 {
        return Outcome::Error("HTTP 429: rate limited by Shazam".into());
    }
    if status != 200 {
        let head: String = text.chars().take(200).collect();
        return Outcome::Error(format!("HTTP {status}: {head}"));
    }
    let now = (now_ms() / 1000) as i64;
    parse_reply(&text, now)
}

fn s(v: &Value) -> Option<String> {
    v.as_str().map(|x| x.to_string())
}

pub fn parse_reply(body: &str, now_unix: i64) -> Outcome {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            let head: String = body.chars().take(200).collect();
            return Outcome::Error(format!("reply is not JSON ({e}): {head}"));
        }
    };
    let matches = v["matches"].as_array();
    if matches.map(|m| m.is_empty()).unwrap_or(false) {
        return Outcome::NoMatch;
    }
    let track = &v["track"];
    let (Some(key), Some(title), Some(artist)) = (s(&track["key"]), s(&track["title"]), s(&track["subtitle"])) else {
        return Outcome::Error("reply has matches but no track key/title/subtitle - format changed?".into());
    };
    let mut album = None;
    if let Some(sections) = track["sections"].as_array() {
        for sec in sections {
            if sec["type"] == "SONG" {
                if let Some(meta) = sec["metadata"].as_array() {
                    for m in meta {
                        if m["title"] == "Album" {
                            album = s(&m["text"]);
                        }
                    }
                }
            }
        }
    }
    let mut apple = None;
    let mut spotify = None;
    if let Some(opts) = track["hub"]["options"].as_array() {
        for o in opts {
            if let Some(acts) = o["actions"].as_array() {
                for a in acts {
                    if a["type"] == "applemusicopen" {
                        apple = apple.or_else(|| s(&a["uri"]));
                    }
                }
            }
        }
    }
    if let Some(provs) = track["hub"]["providers"].as_array() {
        for p in provs {
            if p["type"] == "SPOTIFY" {
                if let Some(acts) = p["actions"].as_array() {
                    for a in acts {
                        if let Some(u) = s(&a["uri"]) {
                            if u.starts_with("spotify:track:") {
                                spotify = Some(u);
                            }
                        }
                    }
                }
            }
        }
    }
    Outcome::Match(Find {
        when: now_unix,
        title,
        artist,
        album,
        cover_url: s(&track["images"]["coverarthq"]).or_else(|| s(&track["images"]["coverart"])),
        shazam_url: s(&track["share"]["href"]).or_else(|| s(&track["url"])),
        apple_music_url: apple,
        spotify_uri: spotify,
        isrc: s(&track["isrc"]),
        shazam_key: key,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
```

The `Find` struct must exist in `src/songs.rs` for this to compile - add it now (it is defined in full in Task 4 Step 3; copy that struct definition and its derives in, leaving the rest of Task 4 for later).

- [ ] **Step 5: Run tests**

Run: `cargo test net::shazam 2>&1 | tail -12`
Expected: 6 passed. If `ureq` API names differ from those used in `recognize` (v3 API: `Agent::config_builder`, `http_status_as_error`, `body_mut().read_to_string()`), fix against `cargo doc` for the fetched version rather than guessing.

- [ ] **Step 6: Commit**

```bash
git add src/net/shazam.rs src/songs.rs tests/fixtures/identify
git commit -m "identify: Shazam request builder and reply parser with match / no-match / broken fixtures"
```

---

### Task 4: Finds store (`songs`)

**Files:**
- Modify: `src/songs.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct Find { when: i64, title, artist: String, album, cover_url, shazam_url, apple_music_url, spotify_uri, isrc: Option<String>, shazam_key: String, app_version: String }
  pub fn path() -> PathBuf;                                    // %APPDATA%\taskbar-eq\songs.jsonl
  pub fn append(f: &Find) -> std::io::Result<()>;
  pub fn append_to(path: &Path, f: &Find) -> std::io::Result<()>;
  pub fn load() -> Vec<Find>;                                  // never fails
  pub fn load_from(path: &Path) -> (Vec<Find>, usize /*bad lines*/);
  pub fn recent_distinct(finds: &[Find], n: usize) -> Vec<Find>;   // newest first, by shazam_key
  pub struct Grouped { pub find: Find /*latest*/, pub first: i64, pub last: i64, pub count: u32 }
  pub fn grouped(finds: &[Find]) -> Vec<Grouped>;              // last-heard desc
  pub fn spotify_url(f: &Find) -> String;
  pub fn apple_url(f: &Find) -> String;
  pub fn youtube_url(f: &Find) -> String;
  pub fn shazam_url(f: &Find) -> String;
  pub fn percent_encode(s: &str) -> String;
  ```

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn find(key: &str, when: i64) -> Find {
        Find {
            when,
            title: format!("Song {key}"),
            artist: "Artist & Co".into(),
            album: None,
            cover_url: None,
            shazam_url: None,
            apple_music_url: None,
            spotify_uri: None,
            isrc: None,
            shazam_key: key.into(),
            app_version: "t".into(),
        }
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("taskbar-eq-songs-test-{name}-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn append_then_load_round_trips_in_order() {
        let p = temp("rt");
        append_to(&p, &find("a", 1)).unwrap();
        append_to(&p, &find("b", 2)).unwrap();
        let (v, bad) = load_from(&p);
        assert_eq!(bad, 0);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].shazam_key, "a");
        assert_eq!(v[1].when, 2);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn a_bad_line_is_skipped_not_fatal() {
        let p = temp("bad");
        append_to(&p, &find("a", 1)).unwrap();
        std::fs::write(&p, format!("{}{{not json\n", std::fs::read_to_string(&p).unwrap())).unwrap();
        append_to(&p, &find("b", 2)).unwrap();
        let (v, bad) = load_from(&p);
        assert_eq!(bad, 1);
        assert_eq!(v.len(), 2);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn missing_file_is_empty() {
        let (v, bad) = load_from(&temp("missing"));
        assert!(v.is_empty());
        assert_eq!(bad, 0);
    }

    #[test]
    fn recent_distinct_is_newest_first_and_deduped() {
        let v = vec![find("a", 1), find("b", 2), find("a", 3), find("c", 4)];
        let r = recent_distinct(&v, 10);
        let keys: Vec<&str> = r.iter().map(|f| f.shazam_key.as_str()).collect();
        assert_eq!(keys, ["c", "a", "b"]);
        assert_eq!(r[1].when, 3, "the NEWEST occurrence is kept");
        assert_eq!(recent_distinct(&v, 2).len(), 2);
    }

    #[test]
    fn grouped_counts_and_spans() {
        let v = vec![find("a", 1), find("b", 2), find("a", 3)];
        let g = grouped(&v);
        assert_eq!(g[0].find.shazam_key, "a");
        assert_eq!((g[0].first, g[0].last, g[0].count), (1, 3, 2));
        assert_eq!((g[1].first, g[1].last, g[1].count), (2, 2, 1));
    }

    #[test]
    fn urls_prefer_real_links_and_fall_back_to_encoded_searches() {
        let mut f = find("a", 1);
        assert_eq!(spotify_url(&f), "https://open.spotify.com/search/Song%20a%20Artist%20%26%20Co");
        assert_eq!(youtube_url(&f), "https://www.youtube.com/results?search_query=Song%20a%20Artist%20%26%20Co");
        assert_eq!(apple_url(&f), "https://music.apple.com/search?term=Song%20a%20Artist%20%26%20Co");
        assert_eq!(shazam_url(&f), "https://www.shazam.com/search?q=Song%20a%20Artist%20%26%20Co");
        f.spotify_uri = Some("spotify:track:XYZ".into());
        f.apple_music_url = Some("https://music.apple.com/x".into());
        f.shazam_url = Some("https://www.shazam.com/track/1".into());
        assert_eq!(spotify_url(&f), "https://open.spotify.com/track/XYZ");
        assert_eq!(apple_url(&f), "https://music.apple.com/x");
        assert_eq!(shazam_url(&f), "https://www.shazam.com/track/1");
    }

    #[test]
    fn percent_encoding_covers_reserved_and_unicode() {
        assert_eq!(percent_encode("a b&c/d?e#f"), "a%20b%26c%2Fd%3Fe%23f");
        assert_eq!(percent_encode("Zoë"), "Zo%C3%AB");
        assert_eq!(percent_encode("safe-._~09Az"), "safe-._~09Az");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test songs:: 2>&1 | tail -5` - Expected: compile errors.

- [ ] **Step 3: Implement**

```rust
//! Every song ever identified, one JSON object per line in `songs.jsonl` beside `config.toml`.
//!
//! Append-only JSON lines rather than a database: a crash mid-write can only damage the last line,
//! there is no schema to migrate, and a few hundred rows is nothing. Repeat finds are separate
//! lines so the history can say first heard / last heard / how many times.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Find {
    /// Unix seconds.
    pub when: i64,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub cover_url: Option<String>,
    pub shazam_url: Option<String>,
    pub apple_music_url: Option<String>,
    /// `spotify:track:<id>` only; Shazam's `spotify:search:` deeplinks are not track links.
    pub spotify_uri: Option<String>,
    pub isrc: Option<String>,
    /// Shazam's track key. Identity for "same song".
    pub shazam_key: String,
    pub app_version: String,
}

impl Default for Find {
    fn default() -> Self {
        Find {
            when: 0, title: String::new(), artist: String::new(), album: None, cover_url: None,
            shazam_url: None, apple_music_url: None, spotify_uri: None, isrc: None,
            shazam_key: String::new(), app_version: String::new(),
        }
    }
}

pub fn path() -> PathBuf {
    crate::config::Config::dir().join("songs.jsonl")
}

pub fn append(f: &Find) -> std::io::Result<()> {
    std::fs::create_dir_all(crate::config::Config::dir())?;
    append_to(&path(), f)
}

pub fn append_to(path: &Path, f: &Find) -> std::io::Result<()> {
    let line = serde_json::to_string(f).map_err(std::io::Error::other)?;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")
}

/// Never fails: a missing file is an empty history and a bad line is logged and skipped.
pub fn load() -> Vec<Find> {
    let (v, bad) = load_from(&path());
    if bad > 0 {
        crate::log::write(&format!("songs.jsonl: skipped {bad} unreadable line(s)"));
    }
    v
}

pub fn load_from(path: &Path) -> (Vec<Find>, usize) {
    let Ok(file) = std::fs::File::open(path) else { return (Vec::new(), 0) };
    let mut out = Vec::new();
    let mut bad = 0;
    for line in std::io::BufReader::new(file).lines() {
        let Ok(line) = line else { bad += 1; continue };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Find>(&line) {
            Ok(f) if !f.shazam_key.is_empty() => out.push(f),
            _ => bad += 1,
        }
    }
    (out, bad)
}

/// Newest first, one entry per song (its newest occurrence).
pub fn recent_distinct(finds: &[Find], n: usize) -> Vec<Find> {
    let mut seen = std::collections::HashSet::new();
    let mut sorted: Vec<&Find> = finds.iter().collect();
    sorted.sort_by_key(|f| std::cmp::Reverse(f.when));
    sorted
        .into_iter()
        .filter(|f| seen.insert(f.shazam_key.clone()))
        .take(n)
        .cloned()
        .collect()
}

pub struct Grouped {
    /// The newest occurrence, whose links and cover are shown.
    pub find: Find,
    pub first: i64,
    pub last: i64,
    pub count: u32,
}

/// One row per song, last-heard descending.
pub fn grouped(finds: &[Find]) -> Vec<Grouped> {
    let mut map: std::collections::HashMap<&str, Grouped> = std::collections::HashMap::new();
    for f in finds {
        match map.get_mut(f.shazam_key.as_str()) {
            Some(g) => {
                g.first = g.first.min(f.when);
                if f.when >= g.last {
                    g.last = f.when;
                    g.find = f.clone();
                }
                g.count += 1;
            }
            None => {
                map.insert(&f.shazam_key, Grouped { find: f.clone(), first: f.when, last: f.when, count: 1 });
            }
        }
    }
    let mut v: Vec<Grouped> = map.into_values().collect();
    v.sort_by_key(|g| std::cmp::Reverse(g.last));
    v
}

/// RFC 3986 unreserved characters pass; everything else, byte-wise, becomes %XX.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn query(f: &Find) -> String {
    percent_encode(&format!("{} {}", f.title, f.artist))
}

pub fn spotify_url(f: &Find) -> String {
    match f.spotify_uri.as_deref().and_then(|u| u.strip_prefix("spotify:track:")) {
        Some(id) => format!("https://open.spotify.com/track/{id}"),
        None => format!("https://open.spotify.com/search/{}", query(f)),
    }
}

pub fn apple_url(f: &Find) -> String {
    f.apple_music_url.clone().unwrap_or_else(|| format!("https://music.apple.com/search?term={}", query(f)))
}

pub fn youtube_url(f: &Find) -> String {
    format!("https://www.youtube.com/results?search_query={}", query(f))
}

pub fn shazam_url(f: &Find) -> String {
    f.shazam_url.clone().unwrap_or_else(|| format!("https://www.shazam.com/search?q={}", query(f)))
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test songs:: 2>&1 | tail -12` - Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add src/songs.rs
git commit -m "identify: songs.jsonl store with distinct-recent, grouping and per-service links"
```

---

### Task 5: Capture recorder hook and sticky banner

**Files:**
- Modify: `src/win/capture.rs` (add recorder sink; hook in `capture_loop` after `interleaved_to_mono`)
- Modify: `src/render/banner.rs:70-90` (sticky mode)

**Interfaces:**
- Produces in `win::capture`:
  ```rust
  pub fn record_start();                              // installs an empty recorder
  pub fn record_snapshot() -> Option<(u32 /*rate*/, Vec<f32>)>;  // clone of what has arrived so far
  pub fn record_stop();
  pub fn record_push(rate: u32, mono: &[f32]);        // called by the capture loop; pub(crate) for tests
  ```
- Produces in `render::banner`: `Banner::new_sticky(text, interior_h) -> Option<Banner>` that never expires.

- [ ] **Step 1: Write failing tests (capture)**

Append to `capture.rs` tests module:

```rust
    #[test]
    fn recorder_collects_only_while_installed() {
        record_stop();
        record_push(48_000, &[0.1, 0.2]);
        assert!(record_snapshot().is_none(), "nothing installed, nothing kept");
        record_start();
        record_push(48_000, &[0.1, 0.2]);
        record_push(48_000, &[0.3]);
        let (rate, buf) = record_snapshot().unwrap();
        assert_eq!(rate, 48_000);
        assert_eq!(buf, vec![0.1, 0.2, 0.3]);
        record_stop();
        assert!(record_snapshot().is_none());
    }
```

Note: this test shares a process-global with nothing else in the suite; keep it the ONLY test touching the recorder so parallel test threads cannot interleave.

- [ ] **Step 2: Write failing test (banner)**

Append to `banner.rs` tests module (it exists - look for `mod tests`):

```rust
    #[test]
    fn sticky_banner_never_expires_and_stays_opaque() {
        let mut b = Banner::new_sticky("listening...", 40).unwrap();
        for _ in 0..1000 {
            assert!(b.advance(250.0));
        }
        assert!((b.opacity() - 1.0).abs() < 1e-6);
    }
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test recorder_collects sticky_banner 2>&1 | tail -5` - Expected: compile errors.

- [ ] **Step 4: Implement the recorder**

In `capture.rs` near the top (after the `Frame` type):

```rust
/// An optional second sink for the mono stream: while installed, every chunk the capture loop
/// downmixes is also appended here, so `identify` can take a few seconds of audio without owning
/// a device of its own. `None` costs one uncontended mutex try per chunk.
struct Recorder {
    rate: u32,
    buf: Vec<f32>,
}

static RECORDER: std::sync::Mutex<Option<Recorder>> = std::sync::Mutex::new(None);

fn recorder_lock() -> std::sync::MutexGuard<'static, Option<Recorder>> {
    RECORDER.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn record_start() {
    *recorder_lock() = Some(Recorder { rate: 0, buf: Vec::with_capacity(48_000 * 13) });
}

pub fn record_stop() {
    *recorder_lock() = None;
}

pub fn record_snapshot() -> Option<(u32, Vec<f32>)> {
    recorder_lock().as_ref().map(|r| (r.rate, r.buf.clone()))
}

/// Called by the capture loop. `try_lock` so the audio thread never waits on a snapshot clone.
pub fn record_push(rate: u32, mono: &[f32]) {
    if let Ok(mut g) = RECORDER.try_lock() {
        if let Some(r) = g.as_mut() {
            r.rate = rate;
            r.buf.extend_from_slice(mono);
        }
    }
}
```

In `capture_loop`, change
```rust
            ring.extend_from_slice(&interleaved_to_mono(slice, channels));
```
to
```rust
            let mono = interleaved_to_mono(slice, channels);
            record_push(rate as u32, &mono);
            ring.extend_from_slice(&mono);
```

(`rate` is the `f32` already in scope from `GetMixFormat`.)

- [ ] **Step 5: Implement the sticky banner**

In `banner.rs`, add a field and constructor:

```rust
pub struct Banner {
    mask: TextMask,
    /// Milliseconds since it appeared.
    age: f32,
    /// Holds at full opacity until replaced, for "listening..." which has no known duration.
    sticky: bool,
}
```

`new` sets `sticky: false`. Add:

```rust
    /// A banner that rises and then holds until something replaces it.
    pub fn new_sticky(text: &str, interior_h: i32) -> Option<Banner> {
        let mut b = Banner::new(text, interior_h)?;
        b.sticky = true;
        Some(b)
    }
```

In `advance`, after `self.age += dt;`:
```rust
        if self.sticky {
            self.age = self.age.min(RISE_MS);
            return true;
        }
```

(`opacity()` at `age == RISE_MS` falls into the hold branch and returns 1.0.) Make `opacity` `pub(crate)` if the test cannot see it (it is in the same module, so `fn` is fine).

- [ ] **Step 6: Run tests**

Run: `cargo test 2>&1 | tail -5` - Expected: whole suite green (619 + new).

- [ ] **Step 7: Commit**

```bash
git add src/win/capture.rs src/render/banner.rs
git commit -m "identify: optional recorder sink on the capture thread and a sticky banner mode"
```

---

### Task 6: Orchestrator (`identify`) and the hotkey slot

**Files:**
- Modify: `src/identify.rs`
- Modify: `src/win/hotkeys.rs` (Slot::IdentifySong, SLOTS 8, id 8, label, media_action arm, on_wm_hotkey arm)
- Modify: `src/config.rs` (Hotkeys.identify_song, slot()/slot_mut() index 7)
- Modify: `src/main.rs:66-78` (the texts array builder - check it uses `cfg.hotkeys.slot(i)` and so needs no change; if it lists fields, add the eighth)

**Interfaces:**
- Consumes: `capture::record_*`, `shazam_sig::{resample_to_16k, Signature}`, `net::shazam::{recognize, Outcome}`, `songs::append`.
- Produces:
  ```rust
  pub fn request();                          // safe from a wndproc: spawns or ignores
  pub fn banner() -> (String, u64, bool);    // text, change counter, sticky?
  pub fn is_busy() -> bool;
  pub fn wait_for_samples(deadline: Duration, want_secs: f32, snapshot: impl FnMut() -> Option<(u32, Vec<f32>)>) -> Option<(u32, Vec<f32>)>;  // pure-ish, tested
  ```

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn wait_gives_up_when_no_audio_ever_arrives() {
        let t0 = std::time::Instant::now();
        let got = wait_for_samples(Duration::from_millis(120), 1.0, || None);
        assert!(got.is_none());
        assert!(t0.elapsed() >= Duration::from_millis(100));
    }

    #[test]
    fn wait_returns_as_soon_as_enough_arrives() {
        let mut calls = 0;
        let got = wait_for_samples(Duration::from_secs(5), 0.5, || {
            calls += 1;
            Some((16_000, vec![0.0; 4_000 * calls]))
        });
        let (rate, buf) = got.unwrap();
        assert_eq!(rate, 16_000);
        assert!(buf.len() >= 8_000);
        assert!(calls <= 3);
    }

    #[test]
    fn wait_returns_what_it_has_at_the_deadline_if_any() {
        let got = wait_for_samples(Duration::from_millis(80), 100.0, || Some((48_000, vec![0.0; 10])));
        assert_eq!(got.unwrap().1.len(), 10);
    }

    #[test]
    fn busy_flag_blocks_a_second_run_and_clears_after() {
        assert!(try_begin());
        assert!(!try_begin(), "second press while running must be refused");
        end();
        assert!(try_begin());
        end();
    }

    #[test]
    fn banner_publish_bumps_the_counter_only_on_change() {
        let (_, s0, _) = banner();
        publish("listening...", true);
        let (t, s1, sticky) = banner();
        assert_eq!(t, "listening...");
        assert!(sticky);
        assert_ne!(s0, s1);
        publish("listening...", true);
        assert_eq!(banner().1, s1);
        publish("no match", false);
        assert_ne!(banner().1, s1);
        assert!(!banner().2);
    }
}
```

- [ ] **Step 2: Run to verify failure** - `cargo test identify:: 2>&1 | tail -3`

- [ ] **Step 3: Implement `identify.rs`**

```rust
//! "What song is this?" - the on-demand Shazam identification.
//!
//! Runs on its own thread so neither the render tick nor the wndproc ever waits on the network.
//! `request()` is the only entry point and is safe to call from a message handler: it flips a flag
//! and spawns. Results reach the user through `banner()`, polled by the render tick the same way
//! `media::now_playing()` is, and through `songs::append`.
//!
//! Timing follows SongRec: post at 4 s, again at 8 s and 12 s if there was no match. Most songs
//! match on the first post; the later ones catch quiet intros.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::dsp::shazam_sig::{resample_to_16k, Signature};
use crate::net::shazam::{self, Outcome};

static BUSY: AtomicBool = AtomicBool::new(false);
/// (text, change counter, sticky). Same protocol as `media::NOW_PLAYING`.
static BANNER: Mutex<(String, u64, bool)> = Mutex::new((String::new(), 0, false));

const POST_AT_SECS: [f32; 3] = [4.0, 8.0, 12.0];
const OVERALL_DEADLINE: Duration = Duration::from_secs(14);

pub fn banner() -> (String, u64, bool) {
    BANNER.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn publish(text: &str, sticky: bool) {
    let mut g = BANNER.lock().unwrap_or_else(|e| e.into_inner());
    if g.0 != text || g.2 != sticky {
        g.0 = text.to_string();
        g.1 = g.1.wrapping_add(1);
        g.2 = sticky;
    }
}

pub fn is_busy() -> bool {
    BUSY.load(Ordering::Relaxed)
}

fn try_begin() -> bool {
    BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed).is_ok()
}

fn end() {
    BUSY.store(false, Ordering::Release);
}

/// Polls `snapshot` until it holds `want_secs` of audio or `deadline` passes. Returns whatever
/// arrived, or `None` if nothing did - a dead capture thread must end as "no match", not a hang.
pub fn wait_for_samples(
    deadline: Duration,
    want_secs: f32,
    mut snapshot: impl FnMut() -> Option<(u32, Vec<f32>)>,
) -> Option<(u32, Vec<f32>)> {
    let t0 = Instant::now();
    let mut last: Option<(u32, Vec<f32>)> = None;
    loop {
        if let Some((rate, buf)) = snapshot() {
            if rate > 0 && buf.len() as f32 >= want_secs * rate as f32 {
                return Some((rate, buf));
            }
            if !buf.is_empty() && rate > 0 {
                last = Some((rate, buf));
            }
        }
        if t0.elapsed() >= deadline {
            return last;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Starts an identification, or does nothing if one is already running.
pub fn request() {
    if !try_begin() {
        crate::log::write("identify: already listening, press ignored");
        return;
    }
    if let Err(e) = std::thread::Builder::new().name("identify".into()).spawn(run) {
        crate::log::write(&format!("identify: could not start thread: {e}"));
        end();
    }
}

fn run() {
    crate::log::write("identify: listening");
    publish("listening...", true);
    crate::win::capture::record_start();
    let t0 = Instant::now();
    let mut result: Option<Outcome> = None;
    for (i, &at) in POST_AT_SECS.iter().enumerate() {
        let remaining = OVERALL_DEADLINE.saturating_sub(t0.elapsed());
        let Some((rate, buf)) = wait_for_samples(remaining, at, crate::win::capture::record_snapshot) else {
            break;
        };
        let secs = buf.len() as f32 / rate as f32;
        let mono16 = resample_to_16k(&buf, rate);
        let sig = Signature::from_mono_16k(&mono16);
        let peaks: usize = sig.bands.iter().map(|b| b.len()).sum();
        crate::log::write(&format!("identify: post {} with {secs:.1}s @ {rate} Hz, {peaks} peaks", i + 1));
        if peaks == 0 {
            // Silence. Posting it is a wasted call and a guaranteed no-match.
            result = Some(Outcome::NoMatch);
            if t0.elapsed() >= OVERALL_DEADLINE { break; } else { continue; }
        }
        let out = shazam::recognize(&sig);
        let done = matches!(out, Outcome::Match(_));
        result = Some(out);
        if done || t0.elapsed() >= OVERALL_DEADLINE {
            break;
        }
    }
    crate::win::capture::record_stop();
    match result {
        Some(Outcome::Match(f)) => {
            crate::log::write(&format!("identify: MATCH {} - {} (key {})", f.title, f.artist, f.shazam_key));
            publish(&format!("{} - {}", f.title, f.artist), false);
            if let Err(e) = crate::songs::append(&f) {
                crate::log::write(&format!("identify: could not save the find: {e}"));
            }
        }
        Some(Outcome::NoMatch) => {
            crate::log::write("identify: no match");
            publish("no match", false);
        }
        Some(Outcome::Error(e)) => {
            crate::log::write(&format!("identify: failed: {e}"));
            publish("no match", false);
        }
        None => {
            crate::log::write("identify: no audio arrived from the capture thread");
            publish("no match", false);
        }
    }
    end();
}
```

- [ ] **Step 4: Add the hotkey slot**

`src/win/hotkeys.rs`:
- Add variant after `FlourishToggle`:
  ```rust
      /// Identify the song currently playing, via Shazam. See `crate::identify`.
      IdentifySong,
  ```
- `SLOTS` 7 -> 8; add `Slot::IdentifySong` to `ALL`; `id()` arm `Slot::IdentifySong => 8`; `media_action()` add `| Slot::IdentifySong` to the `None` arm; `label()` arm `Slot::IdentifySong => "identify song"`.
- In `on_wm_hotkey`'s inner `match slot`, add before the transport arm:
  ```rust
              Slot::IdentifySong => crate::identify::request(),
  ```

`src/config.rs` `Hotkeys`: add field
```rust
    /// Identify the current song with Shazam and show it in the banner.
    pub identify_song: String,
```
and `7 => &self.identify_song,` / `7 => &mut self.identify_song,` in `slot` / `slot_mut`.

`src/main.rs:66-78`: confirm the texts array is built via `cfg.hotkeys.slot(i)` in a loop. If it enumerates fields, add `identify_song` as index 7.

- [ ] **Step 5: Run the whole suite**

Run: `cargo test 2>&1 | tail -5` - Expected: green, including the `config.rs` SLOTS round-trip tests and the hotkeys `on_wm_hotkey` tests.

- [ ] **Step 6: Commit**

```bash
git add src/identify.rs src/win/hotkeys.rs src/config.rs src/main.rs
git commit -m "identify: orchestrator thread with 4/8/12 s posts, plus the Identify Song hotkey slot"
```

---

### Task 7: Tray menu, events and main-loop wiring

**Files:**
- Modify: `src/win/tray.rs` (ids, `TrayEvent` variants, `SongEntry`, menu section, id mapping, `show_menu`/`show_menu_for` signature)
- Modify: `src/win/overlay.rs` (`open_url`)
- Modify: `src/main.rs` (banner block, event handling, both `show_menu` call sites)

**Interfaces:**
- Produces in `tray`:
  ```rust
  pub struct SongEntry { pub label: String, pub url: String }
  pub enum TrayEvent { ..., IdentifyNow, OpenUrl(String), SongHistory, OpenSongsFolder }
  pub fn show_menu(&self, autostart, current_theme, recents, transport, songs: &[SongEntry]) -> Option<TrayEvent>
  pub fn show_menu_for(&self, items, autostart, current_theme, recents, transport, songs: &[SongEntry]) -> Option<TrayEvent>
  ```
- Produces in `overlay`: `pub fn open_url(url: &str) -> anyhow::Result<()>`.
- Consumes: `identify::{request, banner}`, `songs::{load, recent_distinct, spotify_url}`, `history::open` (Task 8 - stub `pub fn open() -> anyhow::Result<()>` now, returning `Ok(())`).

- [ ] **Step 1: Write the failing test (tray id mapping is Win32-bound; test the pure helper)**

Add to `tray.rs` a pure helper and its test:

```rust
/// Builds the Songs submenu entries from the store: label as the menu shows it, URL it opens.
pub fn song_entries(recent: &[crate::songs::Find]) -> Vec<SongEntry> {
    recent
        .iter()
        .map(|f| SongEntry {
            label: format!("{} - {}", f.title, f.artist).replace('&', "&&"),
            url: crate::songs::spotify_url(f),
        })
        .collect()
}
```

Test (in `tray.rs` tests module, create one if absent):
```rust
    #[test]
    fn song_entries_escape_menu_ampersands_and_carry_spotify_urls() {
        let f = crate::songs::Find {
            title: "Rock & Roll".into(),
            artist: "X".into(),
            shazam_key: "k".into(),
            ..Default::default()
        };
        let e = song_entries(&[f]);
        assert_eq!(e[0].label, "Rock && Roll - X");
        assert!(e[0].url.starts_with("https://open.spotify.com/search/"));
    }
```

- [ ] **Step 2: Run to verify failure** - `cargo test song_entries 2>&1 | tail -3`

- [ ] **Step 3: Implement the tray changes**

Constants (after `ID_FLOURISH_TOGGLE`):
```rust
const ID_IDENTIFY_NOW: usize = 1124;
const ID_SONG_HISTORY: usize = 1125;
const ID_SONGS_FOLDER: usize = 1126;
/// Ten recent songs live here. Below ID_THEME_BASE (2000) because the theme arm is `>=`.
const ID_SONG_BASE: usize = 1900;
const SONG_SLOTS: usize = 10;
```

`TrayEvent` additions:
```rust
    /// Identify the current song now, from the menu, without a key bound.
    IdentifyNow,
    /// Open a link in the default browser.
    OpenUrl(String),
    /// Write and open the HTML song history.
    SongHistory,
    /// Reveal the folder holding songs.jsonl.
    OpenSongsFolder,
```

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SongEntry {
    pub label: String,
    pub url: String,
}
```

Menu section, inserted after the Flourishes block and before the trailing separator:

```rust
            // ---- Songs ------------------------------------------------------------------------
            // The identification is offered as an ACTION as well as a binding, like the shuffles
            // and flourishes: it must work before any key is set.
            if let Ok(sub) = CreatePopupMenu() {
                let _ = AppendMenuW(sub, MF_STRING, ID_IDENTIFY_NOW, w!("Identify this song now"));
                {
                    let text = format!("Identify key:  {}", transport.keys[7]);
                    let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = AppendMenuW(sub, MF_STRING, ID_BIND_BASE + 7, windows::core::PCWSTR(wide.as_mut_ptr()));
                }
                let _ = AppendMenuW(sub, MF_SEPARATOR, 0, None);
                if songs.is_empty() {
                    let _ = AppendMenuW(sub, MF_STRING | MF_DISABLED | MF_GRAYED, 0, w!("no songs yet"));
                }
                for (i, s) in songs.iter().take(SONG_SLOTS).enumerate() {
                    let mut wide: Vec<u16> = s.label.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = AppendMenuW(sub, MF_STRING, ID_SONG_BASE + i, windows::core::PCWSTR(wide.as_mut_ptr()));
                }
                let _ = AppendMenuW(sub, MF_SEPARATOR, 0, None);
                let _ = AppendMenuW(sub, MF_STRING, ID_SONG_HISTORY, w!("Song history..."));
                let _ = AppendMenuW(sub, MF_STRING, ID_SONGS_FOLDER, w!("Open songs folder"));
                let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, w!("Songs"));
            }
```

Id mapping, inserted BEFORE the `ID_BIND_BASE` range check:
```rust
            if id == ID_IDENTIFY_NOW {
                return Some(TrayEvent::IdentifyNow);
            }
            if id == ID_SONG_HISTORY {
                return Some(TrayEvent::SongHistory);
            }
            if id == ID_SONGS_FOLDER {
                return Some(TrayEvent::OpenSongsFolder);
            }
            if (ID_SONG_BASE..ID_SONG_BASE + SONG_SLOTS).contains(&id) {
                return songs.get(id - ID_SONG_BASE).map(|s| TrayEvent::OpenUrl(s.url.clone()));
            }
```

Add `songs: &[SongEntry]` as the last parameter of both `show_menu` and `show_menu_for`, passed through.

- [ ] **Step 4: `open_url` in overlay.rs**

```rust
/// Opens a URL in the default browser. Same `ShellExecuteW` as `open_path`, minus the Notepad
/// fallback - there is no sensible fallback for a link.
pub fn open_url(url: &str) -> Result<()> {
    let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let r = unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            windows::core::w!("open"),
            windows::core::PCWSTR(wide.as_ptr()),
            None,
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    if (r.0 as usize) > 32 {
        Ok(())
    } else {
        Err(anyhow!("ShellExecuteW returned {} for {url}", r.0 as usize))
    }
}
```

- [ ] **Step 5: Wire main.rs**

Banner block (`main.rs:451-470`): replace with

```rust
            // ---- banners ----------------------------------------------------------------------
            // Two sources share one banner: a track change (optional, `show_track_name`) and the
            // song identifier (always - the user asked for it by pressing a key). The identifier's
            // text is sticky while listening and normal once it has an answer.
            let (id_text, id_seq, id_sticky) = identify::banner();
            if id_seq != self.identify_seq {
                self.identify_seq = id_seq;
                self.banner = if id_sticky {
                    render::banner::Banner::new_sticky(&id_text, r.h - 4)
                } else {
                    render::banner::Banner::new(&id_text, r.h - 4)
                };
            } else if self.show_track_name {
                let (title, seq) = win::media::now_playing();
                if seq != self.track_seq {
                    self.track_seq = seq;
                    // An empty title means nothing is loaded, which is not an announcement.
                    self.banner = if title.trim().is_empty() {
                        None
                    } else {
                        render::banner::Banner::new(&title, r.h - 4)
                    };
                }
            }
            if let Some(b) = self.banner.as_mut() {
                if !b.advance(dt_ms) {
                    self.banner = None;
                }
            }
            if let Some(b) = self.banner.as_ref() {
                b.draw(&mut canvas, &self.theme, self.time_s);
            }
```

Add field `identify_seq: u64` to the ticker struct (next to `track_seq`), initialised `identify::banner().1`.

Event handling, next to `FlourishNow`:
```rust
                Some(TrayEvent::IdentifyNow) => identify::request(),
                Some(TrayEvent::OpenUrl(url)) => {
                    if let Err(e) = win::overlay::open_url(&url) {
                        log::write(&format!("could not open {url}: {e}"));
                    }
                }
                Some(TrayEvent::SongHistory) => {
                    if let Err(e) = history::open() {
                        log::write(&format!("song history: {e}"));
                    }
                }
                Some(TrayEvent::OpenSongsFolder) => {
                    if let Err(e) = win::overlay::open_path(&Config::dir()) {
                        log::write(&format!("could not open the songs folder: {e}"));
                    }
                }
```

Both `show_menu` call sites: build `let song_entries = win::tray::song_entries(&songs::recent_distinct(&songs::load(), 10));` immediately before showing the menu (a right-click is the only time this file is read) and pass `&song_entries`.

`history.rs` stub for now:
```rust
//! filled in by Task 8
pub fn open() -> anyhow::Result<()> { Ok(()) }
```

- [ ] **Step 6: Build, test, run**

Run: `cargo test 2>&1 | tail -3` then `cargo run --release` and right-click the tray icon: the Songs submenu shows "no songs yet"; "Identify this song now" makes the banner say "listening..." and, with a song playing in a browser, name it within ~5 s. Check `%APPDATA%\taskbar-eq\songs.jsonl` gained a line and `last_shazam.json` exists. Bind a key via "Identify key" and press it.

If the live call fails, read the log and `last_shazam.json` BEFORE changing code: HTTP status and the first 200 bytes are logged.

- [ ] **Step 7: Commit**

```bash
git add src/win/tray.rs src/win/overlay.rs src/main.rs src/history.rs
git commit -m "identify: Songs tray submenu, identify-now action, banner wiring and link opening"
```

---

### Task 8: History page (`history`)

**Files:**
- Modify: `src/history.rs`

**Interfaces:**
- Consumes: `songs::{load, grouped, Grouped, spotify_url, apple_url, youtube_url, shazam_url}`, `overlay::open_path`.
- Produces:
  ```rust
  pub fn render(rows: &[songs::Grouped]) -> String;   // full HTML document
  pub fn path() -> PathBuf;                           // %APPDATA%\taskbar-eq\songs.html
  pub fn open() -> anyhow::Result<()>;                // render + write + ShellExecute
  ```

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::songs::{Find, Grouped};

    fn row(title: &str, artist: &str) -> Grouped {
        let f = Find { title: title.into(), artist: artist.into(), shazam_key: "k1".into(), when: 1_700_000_000, ..Default::default() };
        Grouped { find: f, first: 1_700_000_000, last: 1_700_003_600, count: 2 }
    }

    #[test]
    fn page_is_self_contained_and_carries_the_data() {
        let html = render(&[row("Resonance", "HOME")]);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("Resonance"));
        assert!(html.contains("open.spotify.com"));
        assert!(html.contains("music.apple.com"));
        assert!(html.contains("youtube.com/results"));
        assert!(html.contains("shazam.com"));
        assert!(!html.contains("http://"), "no external assets over http");
        assert!(!html.contains("<link "), "no external stylesheets");
        assert!(!html.contains("<script src"), "no external scripts");
    }

    #[test]
    fn a_hostile_title_cannot_escape_the_embedded_json() {
        let html = render(&[row("</script><script>alert(1)</script>", "\"quoted\" & <b>")]);
        // The literal closing tag must never appear inside the data block.
        let data_start = html.find("id=\"data\"").unwrap();
        let data_end = html[data_start..].find("</script>").unwrap() + data_start;
        let block = &html[data_start..data_end];
        assert!(!block.contains("</script>"));
        assert!(block.contains("<\\/script>"));
    }

    #[test]
    fn empty_history_still_renders() {
        let html = render(&[]);
        assert!(html.contains("no songs yet"));
    }
}
```

- [ ] **Step 2: Run to verify failure** - `cargo test history:: 2>&1 | tail -3`

- [ ] **Step 3: Implement**

```rust
//! The song history page: one self-contained HTML file written next to the data and opened in the
//! default browser. Everything is inline - CSS, JS, the data as JSON - so it works from `file://`
//! with no network beyond the cover-art thumbnails.
//!
//! Hiding a row is client-side (`localStorage`) in this version; the tray submenu does not see it.
//! A native window later would own that state properly.

use crate::songs::{self, Grouped};
use std::path::PathBuf;

pub fn path() -> PathBuf {
    crate::config::Config::dir().join("songs.html")
}

pub fn open() -> anyhow::Result<()> {
    let rows = songs::grouped(&songs::load());
    std::fs::create_dir_all(crate::config::Config::dir())?;
    std::fs::write(path(), render(&rows))?;
    crate::win::overlay::open_path(&path())
}

#[derive(serde::Serialize)]
struct Row<'a> {
    key: &'a str,
    title: &'a str,
    artist: &'a str,
    album: &'a str,
    cover: &'a str,
    first: i64,
    last: i64,
    count: u32,
    spotify: String,
    apple: String,
    youtube: String,
    shazam: String,
}

pub fn render(rows: &[Grouped]) -> String {
    let data: Vec<Row> = rows
        .iter()
        .map(|g| Row {
            key: &g.find.shazam_key,
            title: &g.find.title,
            artist: &g.find.artist,
            album: g.find.album.as_deref().unwrap_or(""),
            cover: g.find.cover_url.as_deref().unwrap_or(""),
            first: g.first,
            last: g.last,
            count: g.count,
            spotify: songs::spotify_url(&g.find),
            apple: songs::apple_url(&g.find),
            youtube: songs::youtube_url(&g.find),
            shazam: songs::shazam_url(&g.find),
        })
        .collect();
    // `</` -> `<\/` is the one escape that makes JSON safe inside a <script> block.
    let json = serde_json::to_string(&data).unwrap_or_else(|_| "[]".into()).replace("</", "<\\/");
    TEMPLATE
        .replace("/*DATA*/", &json)
        .replace("/*COUNT*/", &rows.len().to_string())
        .replace("/*VERSION*/", env!("CARGO_PKG_VERSION"))
}

const TEMPLATE: &str = r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>SONG HISTORY</title>
<style>
:root{--bg:#0b0710;--panel:#140b1f;--ink:#e8e6f0;--dim:#8b86a3;--mag:#ff2bd6;--cyan:#19e6ff;--amber:#ffb347;--line:#2a1a3d}
*{box-sizing:border-box}
html,body{margin:0;background:var(--bg);color:var(--ink);font:14px/1.45 "Cascadia Code","Consolas",ui-monospace,monospace}
body::before{content:"";position:fixed;inset:0;pointer-events:none;z-index:9;
 background:repeating-linear-gradient(to bottom,rgba(255,255,255,.025) 0 1px,transparent 1px 3px)}
header{position:relative;padding:34px 28px 26px;overflow:hidden;border-bottom:1px solid var(--line)}
header::before{content:"";position:absolute;inset:-40% -10% auto -10%;height:240%;z-index:-1;
 background:radial-gradient(ellipse at 50% 110%,#ff5e7e 0%,#b0359a 22%,#4b1e7a 46%,transparent 70%);opacity:.55;filter:blur(18px)}
header::after{content:"";position:absolute;left:0;right:0;bottom:0;height:120px;z-index:-1;opacity:.35;
 background:linear-gradient(transparent 0,var(--bg) 100%),
 repeating-linear-gradient(90deg,var(--cyan) 0 1px,transparent 1px 48px),
 repeating-linear-gradient(0deg,var(--cyan) 0 1px,transparent 1px 24px);
 transform:perspective(300px) rotateX(60deg);transform-origin:bottom}
h1{margin:0;font-size:32px;letter-spacing:.35em;font-weight:700;color:#fff;
 text-shadow:0 0 6px var(--mag),0 0 22px var(--mag),2px 0 0 var(--cyan),-2px 0 0 var(--mag)}
.sub{margin-top:8px;color:var(--cyan);letter-spacing:.2em;font-size:12px;text-transform:uppercase}
.bar{display:flex;gap:14px;align-items:center;padding:16px 28px;border-bottom:1px solid var(--line);background:var(--panel)}
input{flex:1;max-width:520px;background:#0e0817;border:1px solid var(--line);color:var(--ink);padding:10px 14px;font:inherit;outline:none;border-radius:3px}
input:focus{border-color:var(--cyan);box-shadow:0 0 0 1px var(--cyan),0 0 18px rgba(25,230,255,.35)}
.count{color:var(--dim);letter-spacing:.1em}
table{width:100%;border-collapse:collapse}
th{position:sticky;top:0;background:var(--panel);color:var(--cyan);text-align:left;font-weight:600;letter-spacing:.12em;text-transform:uppercase;font-size:11px;padding:12px 14px;border-bottom:1px solid var(--line);cursor:pointer;user-select:none}
th.on{color:var(--mag)}
th.on::after{content:" ▾";}th.on.asc::after{content:" ▴"}
td{padding:10px 14px;border-bottom:1px solid var(--line);vertical-align:middle}
tr:hover td{background:rgba(255,43,214,.06)}
.cover{width:48px;height:48px;object-fit:cover;border:1px solid var(--cyan);box-shadow:0 0 10px rgba(25,230,255,.35);background:#000;display:block}
.cover.none{display:grid;place-items:center;color:var(--dim);font-size:10px}
.title{color:#fff;font-weight:600}
.dim{color:var(--dim)}
.links{display:flex;gap:6px;flex-wrap:wrap}
.links a{display:inline-block;padding:5px 9px;border:1px solid;border-radius:3px;font-size:11px;letter-spacing:.08em;text-transform:uppercase;text-decoration:none;transition:box-shadow .15s,transform .15s}
.links a:hover{transform:translateY(-1px)}
a.sp{color:#1ed760;border-color:#1ed760}a.sp:hover{box-shadow:0 0 14px rgba(30,215,96,.6)}
a.ap{color:#ff6b8a;border-color:#ff6b8a}a.ap:hover{box-shadow:0 0 14px rgba(255,107,138,.6)}
a.yt{color:#ff5555;border-color:#ff5555}a.yt:hover{box-shadow:0 0 14px rgba(255,85,85,.6)}
a.sz{color:var(--cyan);border-color:var(--cyan)}a.sz:hover{box-shadow:0 0 14px rgba(25,230,255,.6)}
button.hide{background:none;border:1px solid var(--line);color:var(--dim);padding:4px 8px;cursor:pointer;font:inherit;font-size:11px}
button.hide:hover{color:var(--amber);border-color:var(--amber)}
.empty{padding:60px;text-align:center;color:var(--dim);letter-spacing:.2em;text-transform:uppercase}
footer{padding:18px 28px;color:var(--dim);font-size:11px;letter-spacing:.1em;display:flex;justify-content:space-between}
footer a{color:var(--cyan)}
</style></head>
<body>
<header><h1>SONG HISTORY</h1><div class="sub">taskbar-eq // shazam identifications</div></header>
<div class="bar"><input id="q" type="search" placeholder="search title / artist / album" autofocus><span class="count" id="count"></span></div>
<table id="t"><thead><tr>
<th data-k="" style="width:64px"></th>
<th data-k="title">Title</th><th data-k="artist">Artist</th><th data-k="album">Album</th>
<th data-k="first">First heard</th><th data-k="last">Last heard</th><th data-k="count" style="width:70px">Times</th>
<th data-k="" style="width:330px">Listen</th><th data-k="" style="width:60px"></th>
</tr></thead><tbody id="rows"></tbody></table>
<div class="empty" id="empty" hidden>no songs yet</div>
<footer><span>/*COUNT*/ songs · v/*VERSION*/</span><span><a href="#" id="unhide">show hidden</a></span></footer>
<script id="data" type="application/json">/*DATA*/</script>
<script>
const DATA=JSON.parse(document.getElementById('data').textContent);
const HKEY='taskbar-eq.hidden';
let hidden=new Set(JSON.parse(localStorage.getItem(HKEY)||'[]'));
let sortK='last',asc=false,q='';
const esc=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const when=t=>new Date(t*1000).toLocaleString(undefined,{dateStyle:'medium',timeStyle:'short'});
function render(){
  const ql=q.toLowerCase();
  let rows=DATA.filter(r=>!hidden.has(r.key)&&(r.title+' '+r.artist+' '+r.album).toLowerCase().includes(ql));
  rows.sort((a,b)=>{let x=a[sortK],y=b[sortK];if(typeof x==='string'){x=x.toLowerCase();y=y.toLowerCase()}return (x<y?-1:x>y?1:0)*(asc?1:-1)});
  document.getElementById('rows').innerHTML=rows.map(r=>`<tr>
<td>${r.cover?`<img class="cover" loading="lazy" src="${esc(r.cover)}" alt="">`:`<div class="cover none">no art</div>`}</td>
<td class="title">${esc(r.title)}</td><td>${esc(r.artist)}</td><td class="dim">${esc(r.album)}</td>
<td class="dim">${when(r.first)}</td><td>${when(r.last)}</td><td>${r.count}</td>
<td class="links"><a class="sp" href="${esc(r.spotify)}" target="_blank" rel="noopener">Spotify</a><a class="ap" href="${esc(r.apple)}" target="_blank" rel="noopener">Apple</a><a class="yt" href="${esc(r.youtube)}" target="_blank" rel="noopener">YouTube</a><a class="sz" href="${esc(r.shazam)}" target="_blank" rel="noopener">Shazam</a></td>
<td><button class="hide" data-k="${esc(r.key)}" title="hide this song on this browser">hide</button></td></tr>`).join('');
  document.getElementById('count').textContent=rows.length+' / '+DATA.length+(hidden.size?' ('+hidden.size+' hidden)':'');
  document.getElementById('empty').hidden=rows.length>0;
  document.querySelectorAll('th').forEach(th=>{th.classList.toggle('on',th.dataset.k===sortK);th.classList.toggle('asc',asc)});
}
document.getElementById('q').addEventListener('input',e=>{q=e.target.value;render()});
document.querySelectorAll('th[data-k]').forEach(th=>th.addEventListener('click',()=>{const k=th.dataset.k;if(!k)return;if(sortK===k)asc=!asc;else{sortK=k;asc=(k==='title'||k==='artist'||k==='album')}render()}));
document.getElementById('rows').addEventListener('click',e=>{const b=e.target.closest('button.hide');if(!b)return;hidden.add(b.dataset.k);localStorage.setItem(HKEY,JSON.stringify([...hidden]));render()});
document.getElementById('unhide').addEventListener('click',e=>{e.preventDefault();hidden.clear();localStorage.removeItem(HKEY);render()});
render();
</script></body></html>
"##;
```

- [ ] **Step 4: Run tests**

Run: `cargo test history:: 2>&1 | tail -6` - Expected: 3 passed. Then `cargo run --release`, right-click, Songs -> Song history..., and eyeball the page at 100 % and 125 % DPI: header glow, sunset gradient, perspective grid, table readable, four coloured link buttons per row, search filters live, header clicks sort, hide works and "show hidden" restores.

- [ ] **Step 5: Commit**

```bash
git add src/history.rs
git commit -m "identify: cyberpunk song-history page with search, sort, per-service links and hide"
```

---

### Task 9: Docs, version, live verification, push

**Files:**
- Modify: `Cargo.toml` (version 0.1.0 -> 0.2.0)
- Modify: `README.md` (Song identification section; licence note)
- Modify: `TODO.md` (Last updated + Done entry)
- Replace: `tests/fixtures/identify/shazam_match.json` with a real reply

- [ ] **Step 1: Live end-to-end, judged by the user**

Play a song in a browser tab (not Spotify - the point is audio without metadata). Press the bound key. Expect: "listening..." then "Title - Artist" in the banner within ~5 s. Then a Discord voice channel or a YouTube video with music. Then silence: expect "no match" after ~12 s. Open Song history and click all four links on one row.

- [ ] **Step 2: Promote the real reply to the fixture**

Copy `%APPDATA%\taskbar-eq\last_shazam.json` from a successful match over `tests/fixtures/identify/shazam_match.json`. Update the `parses_a_match_into_a_find` assertions to the real song's values (title, artist, key, album, cover prefix). Run `cargo test net::shazam`. This is the spec's "saved real Shazam reply".

- [ ] **Step 3: README**

Add a section:

```markdown
## Song identification

Press the Identify key (bind it under Songs in the tray menu) while anything plays - a browser
video, a Discord call, a game - and the banner names the song. Every find is appended to
`%APPDATA%\taskbar-eq\songs.jsonl`; **Songs -> Song history...** opens a page listing them with
Spotify, Apple Music, YouTube and Shazam links.

The fingerprinting is a port of [SongRec](https://github.com/marin-m/SongRec)'s Shazam signature
code, which is why this repository is licensed under the GPL-3 (see `LICENSE`). It uses Shazam's
undocumented endpoint, so it can stop working without notice; when it does, the last raw reply is
in `last_shazam.json` beside the log.
```

- [ ] **Step 4: TODO.md and version**

Bump `version = "0.2.0"`. In `TODO.md`: update **Last updated**, add a Done entry "Song identification via Shazam (hotkey slot 8, Songs submenu, songs.jsonl, HTML history) - commit <hash>", and under Open unresolved: "hides on the history page are per-browser (localStorage), the tray submenu does not see them; a native window would fix it".

- [ ] **Step 5: Full suite, release build, commit, push**

```bash
cargo test 2>&1 | tail -3
cargo build --release 2>&1 | tail -2
git add -A
git commit -m "identify: v0.2.0 - README, TODO, real Shazam fixture from the live test"
git push
```

Copy `target/release/taskbar-eq.exe` to `dist/` if that is how the user runs it (check `HANDOVER.md:67`).

---

## Self-review notes

- Spec coverage: trigger/flow (T6, T7), signature (T2), request/parse (T3), retry 4/8/12 (T6), storage + views (T4), menu (T7), banner incl. listening/no match (T5, T6, T7), history page incl. styling/search/sort/links/hide (T8), failure table (T3 errors, T4 bad lines, T6 timeouts, T7 log-only failures), licence (T1, T9), testing list (each task), out-of-scope respected.
- Type consistency: `Find` fields identical in T3/T4/T8; `Outcome` variants identical in T3/T6; `record_snapshot` returns `Option<(u32, Vec<f32>)>` in T5 and is consumed as such in T6; `SongEntry`/`song_entries` in T7 only; `Grouped` in T4/T8.
- Known risk: `ureq` 3 method names in T3 Step 4 are from memory of the v3 API; verify against the fetched crate's docs on first compile rather than guessing.
