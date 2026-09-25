# Health Fixes Implementation Plan (Part A)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land every Part A fix from the spec, in user-benefit order, each behind a test that failed first, ending in a v0.2.1 GitHub Release with the exe out of git.

**Architecture:** Small, independent changes to existing modules; one new file (`src/probes.rs`) that receives the CLI probes from `main.rs`; docs split into `docs/`; CI workflow added. No new dependencies.

**Tech Stack:** Rust 2021, `windows` 0.62, `rustfft`, `serde`/`toml`, `gh` CLI (authenticated as cwissett-hub), GitHub Actions `windows-latest`.

**Spec:** `docs/superpowers/specs/2026-09-25-health-fixes-and-virtual-self-design.md` (Part A). The spec is the authority; this plan is its argument.

## Global Constraints

- Nothing may stall the 60 fps render path or run I/O inside a wndproc.
- Every behaviour change has a test that was watched to fail first (RED) then pass (GREEN); docs-only tasks are exempt.
- `cargo test` stays green after every task; from Task 8 onward `cargo clippy --all-targets -- -D warnings` must also be green.
- Nothing light-taskbar related is built (user's ruling).
- Commit per task with a message in the repo's style (`area: what`); keep `TODO.md`'s "Last updated" current at the END (Task 10), not per task.
- Do NOT rewrite git history; do NOT force-push.
- Tests must never read or write the real `%APPDATA%\taskbar-eq` (Task 2 makes that possible; earlier tasks do not touch config).

## Review Focus

Inputs the spec implies but no task's tests would otherwise exercise, most likely to bite first. Each has a test pinned to the owning task.

1. **A config file from v0.2.0 with no new keys and an unknown `media_backend` string** must load with theme/width/hotkeys intact, not fall to defaults. (Task 2.)
2. **A `config.toml.tmp` left by a crash** must not be read as the config and must be overwritten by the next save. (Task 2.)
3. **A 31 ms frame** (menu open) must decay the meter the same distance as two 16 ms frames. (Task 5.)
4. **A 48 Hz sub-bass tone** must not vanish or land in band 0 alongside a 100 Hz tone after the two-FFT change. (Task 4.)
5. **`--help` typed by a friend** must print usage and exit, never launch the overlay. (Task 9.)

---

### Task 1: `apply_all` refuses an unchanged binding as a duplicate of itself

**Files:**
- Modify: `src/win/hotkeys.rs` (`Registry::apply_all`, ~line 231; tests module)

**Interfaces:**
- Consumes: `apply_one(r, slot, text, others)`, the `Registrar` trait and its test fake (see the existing tests module: it has a recording fake registrar).
- Produces: no API change.

- [ ] **Step 1: Write the failing test**

In the `hotkeys.rs` tests module, next to the existing `apply_one` tests (reuse their fake `Registrar` type; if `Registry` cannot be built without an `HWND`, add a `#[cfg(test)] fn apply_all_with(&mut self, reg: &mut dyn Registrar, texts)` that `apply_all` delegates to, so the test injects the fake):

```rust
    #[test]
    fn apply_all_twice_keeps_every_binding_registered() {
        let mut fake = FakeRegistrar::default(); // whatever the existing fake is called
        let mut reg = Registry::for_test();
        let texts: [&str; SLOTS] = ["Win+Ctrl+Space", "Win+Ctrl+Period", "Win+Ctrl+Comma", "Win+Ctrl+R", "Win+Ctrl+C", "Win+Ctrl+F", "Win+Ctrl+T", "Win+Ctrl+I"];
        let first = reg.apply_all_with(&mut fake, texts);
        assert!(first.iter().all(|o| o.is_working()), "{first:?}");
        // Second pass with the SAME texts - what "Use suggested keys" does. Every slot must still work.
        let second = reg.apply_all_with(&mut fake, texts);
        assert!(second.iter().all(|o| o.is_working()), "second pass refused: {second:?}");
    }

    #[test]
    fn apply_all_still_refuses_a_real_duplicate_between_two_slots() {
        let mut fake = FakeRegistrar::default();
        let mut reg = Registry::for_test();
        let texts: [&str; SLOTS] = ["Win+Ctrl+Space", "Win+Ctrl+Space", "", "", "", "", "", ""];
        let out = reg.apply_all_with(&mut fake, texts);
        assert!(out[0].is_working());
        assert!(matches!(out[1], Outcome::Refused(_, Reject::DuplicateOfOtherAction)), "{:?}", out[1]);
    }
```

- [ ] **Step 2: Run to verify failure** - `cargo test apply_all_ 2>&1 | tail -15` - Expected: the first test FAILS on the second pass (Refused DuplicateOfOtherAction), the second passes or fails to compile until the helper exists.

- [ ] **Step 3: Fix**

In `apply_all`, build `others` excluding the slot being applied:

```rust
            let others: Vec<Chord> = self
                .live
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != Self::idx(*slot))
                .filter_map(|(_, c)| *c)
                .collect();
```

Keep the comment, and add: "EXCLUDING this slot's own previous chord: re-applying an unchanged binding must not be refused as a duplicate of itself, which is what made 'Use suggested keys' kill every other key."

- [ ] **Step 4: Run** - `cargo test hotkeys 2>&1 | tail -3` - Expected: all pass.

- [ ] **Step 5: Commit** - `git commit -am "hotkeys: re-applying an unchanged binding is not a duplicate of itself (Use suggested keys killed the other keys)"`

---

### Task 2: Config integrity

**Files:**
- Modify: `src/config.rs` (`Config::dir`, `Config::save`, `Config::load`, `media_backend` deserialisation, tests module ~lines 190-370)
- Modify: `src/win/media.rs` (`Backend` gets a lenient `Deserialize`)

**Interfaces:**
- Produces: `Config::dir()` honours env var `TASKBAR_EQ_CONFIG_DIR`; `Config::save` is atomic; `Backend` deserialises any string (unknown -> `Session`, logged).

- [ ] **Step 1: Write the failing tests** (replace the backup/restore dance in the existing tests with a per-test temp dir):

```rust
    fn with_temp_dir<T>(name: &str, f: impl FnOnce(&std::path::Path) -> T) -> T {
        let _g = CONFIG_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("taskbar-eq-cfg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("TASKBAR_EQ_CONFIG_DIR", &dir);
        let r = f(&dir);
        std::env::remove_var("TASKBAR_EQ_CONFIG_DIR");
        let _ = std::fs::remove_dir_all(&dir);
        r
    }
    static CONFIG_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn dir_honours_the_test_override() {
        with_temp_dir("dir", |d| assert_eq!(Config::dir(), d));
    }

    #[test]
    fn save_is_atomic_and_leaves_no_tmp_behind() {
        with_temp_dir("atomic", |d| {
            let mut c = Config::default();
            c.width = 123;
            c.save().unwrap();
            assert!(d.join("config.toml").exists());
            assert!(!d.join("config.toml.tmp").exists());
            assert_eq!(Config::load().width, 123);
        });
    }

    #[test]
    fn a_stale_tmp_does_not_shadow_the_real_file() {
        with_temp_dir("stale", |d| {
            let mut c = Config::default();
            c.width = 321;
            c.save().unwrap();
            std::fs::write(d.join("config.toml.tmp"), "width = 1\n").unwrap();
            assert_eq!(Config::load().width, 321);
            c.save().unwrap();
            assert!(!d.join("config.toml.tmp").exists());
        });
    }

    #[test]
    fn an_unknown_media_backend_does_not_lose_the_rest_of_the_config() {
        with_temp_dir("backend", |d| {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join("config.toml"), "theme = \"vu-cream\"\nwidth = 300\nmedia_backend = \"nonsense\"\n").unwrap();
            let c = Config::load();
            assert_eq!(c.theme, "vu-cream");
            assert_eq!(c.width, 300);
            assert_eq!(c.media_backend, crate::win::media::Backend::Session);
        });
    }

    #[test]
    fn a_v020_config_without_the_new_keys_loads_intact() {
        with_temp_dir("v020", |d| {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join("config.toml"), "theme = \"tube-soviet\"\nwidth = 380\n[hotkeys]\nplay_pause = \"Win+Ctrl+Space\"\n").unwrap();
            let c = Config::load();
            assert_eq!(c.theme, "tube-soviet");
            assert_eq!(c.hotkeys.play_pause, "Win+Ctrl+Space");
            assert_eq!(c.hotkeys.identify_song, "");
        });
    }
```

Then convert every existing test that touched `Config::path()` to run inside `with_temp_dir` and delete its backup/restore code.

- [ ] **Step 2: Run to verify failure** - `cargo test config:: 2>&1 | tail -12` - Expected: `dir_honours_the_test_override` fails (env var ignored), atomic tests fail on `.tmp` assertions or `Backend` parse failure.

- [ ] **Step 3: Implement**

`Config::dir`:
```rust
    pub fn dir() -> PathBuf {
        if let Ok(d) = std::env::var("TASKBAR_EQ_CONFIG_DIR") {
            if !d.is_empty() {
                return PathBuf::from(d);
            }
        }
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(base).join("taskbar-eq")
    }
```
`Config::save`:
```rust
    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(Self::dir())?;
        let tmp = Self::dir().join("config.toml.tmp");
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        // Rename is atomic on NTFS, so a crash mid-write can only lose the .tmp, never the config.
        std::fs::rename(&tmp, Self::path())?;
        Ok(())
    }
```
`Config::load`: replace `eprintln!("config: {e}; using defaults")` with `crate::log::write(&format!("config: {e}; using defaults"))`.

`Backend` in `media.rs`: replace `Deserialize` derive with a manual impl that accepts `"session"`/`"media-keys"` and maps anything else to `Session` after `log::write`:
```rust
impl<'de> serde::Deserialize<'de> for Backend {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(match s.as_str() {
            "session" => Backend::Session,
            "media-keys" => Backend::MediaKeys,
            other => {
                crate::log::write(&format!("config: media_backend {other:?} is not a backend; using session"));
                Backend::Session
            }
        })
    }
}
```
Keep `Serialize` derived (`rename_all = "kebab-case"`).

- [ ] **Step 4: Run** - `cargo test config:: media:: 2>&1 | tail -3` then `cargo test 2>&1 | tail -3` - Expected: green. Confirm the real `%APPDATA%\taskbar-eq\config.toml` mtime did not change during the run.

- [ ] **Step 5: Commit** - `git commit -am "config: atomic save, lenient media_backend, log on fallback, tests off the real APPDATA"`

---

### Task 3: Kaleido quiet floor

**Files:**
- Modify: `src/render/kaleido.rs` (`LEVEL_FLOOR` ~line 125, `draw`)
- Test: same file's tests module (or `src/render/mod.rs` tests if kaleido has none)

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn quiet_material_still_shows_a_rosette_at_every_size() {
        for id in crate::themes::builtin::all().iter().filter(|t| t.family == "kaleido").map(|t| t.id.clone()) {
            let t = crate::themes::builtin::by_id(&id).unwrap();
            for (w, h) in [(190, 48), (380, 48), (380, 60)] {
                let mut fam = Kaleido::default();
                let mut c = Canvas::new(w, h);
                let mut d = FrameData::default();
                for v in d.levels.iter_mut() { *v = 0.12; }
                d.rms = 0.05;
                for _ in 0..30 { fam.draw(&mut c, &t, &d); }
                let lit = c.bits().iter().filter(|p| (**p >> 24) > 0 && (**p & 0xffffff) != panel_rgb(&t)).count();
                assert!(lit as f32 >= 0.05 * (w * h) as f32, "{id} {w}x{h}: only {lit} lit pixels at level 0.12");
            }
        }
    }
```
(Adapt `all()`/`by_id`/`bits()`/panel colour access to the actual helper names in `themes::builtin` and `Canvas`; look at how `render/mod.rs::dispatch_tests` iterates colourways and reuse that.)

- [ ] **Step 2: Run to verify failure** - `cargo test quiet_material_still_shows 2>&1 | tail -5` - Expected: FAIL with a low lit count for at least one colourway.

- [ ] **Step 3: Fix** - in `draw`, clamp the rosette drive so the radius never falls below a floor (e.g. `MIN_RADIUS_FRAC = 0.35` of the half-height) and the lowest ring is always drawn at a dim alpha; keep `LEVEL_FLOOR` as the point where motion starts. Document in the family header: "a kaleidoscope that stops dead reads as a freeze, so the frame never empties."

- [ ] **Step 4: Run** - `cargo test kaleido 2>&1 | tail -3`; then `cargo test --release dump_ -- --ignored` and LOOK at `target/eyeball/kaleido-*.png` (Read tool) to confirm the calm frame shows structure and the loud frame is unchanged.

- [ ] **Step 5: Commit** - `git commit -am "kaleido: never an empty slab on quiet material"`

---

### Task 4: Log-spaced bass via a second, longer FFT

**Files:**
- Modify: `src/dsp/bands.rs` (`BandMapper::new`, `process`, constants)
- Modify: `src/win/capture.rs` only if the ring must hold more history (it trims to `FFT_SIZE * 2`; the low path needs 8192 samples - keep a SEPARATE low-rate history inside `BandMapper`, so capture is untouched)
- Test: `src/dsp/bands.rs` tests

**Interfaces:**
- Produces: `BandMapper::process(&mut self, mono: &[f32], out: &mut [f32; NUM_BANDS])` unchanged signature; `NUM_BANDS` unchanged.

- [ ] **Step 1: Write the failing tests**

```rust
    fn tone(freq: f32, secs: f32) -> Vec<f32> { (0..(48_000.0 * secs) as usize).map(|i| 0.5 * (2.0 * std::f32::consts::PI * freq * i as f32 / 48_000.0).sin()).collect() }
    fn peak_band(m: &mut BandMapper, sig: &[f32]) -> usize {
        let mut out = [0.0; NUM_BANDS];
        for chunk in sig.chunks(HOP) { m.process(chunk, &mut out); }
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
            assert!((got as i32 - expect(f) as i32).abs() <= 1, "{f} Hz: band {got}, expected ~{}", expect(f));
        }
    }
```
Also keep the existing fixture-CSV tests passing (they compare band energies on real music; if their tolerances trip, widen them ONLY for bands < 16 and say why in the assertion message).

- [ ] **Step 2: Run to verify failure** - `cargo test bands:: 2>&1 | tail -8` - Expected: the first two FAIL (currently 60 and 120 Hz are ~3 bands apart).

- [ ] **Step 3: Implement**

In `BandMapper`: add `low_fft` (8192-point real FFT via `rustfft`, Hann window), a `low_history: Vec<f32>` ring of 8192 mono samples fed every `process` call, and `low_edges` computed like `edges` but with `bin_hz = sample_rate / 8192`. Choose `CROSSOVER_BAND` = the first band whose high-res edge spacing at 2048 points is >= 2 bins (compute at `new`, it lands around band 30 at 48 kHz). In `process`: run the 2048 path as today for bands >= crossover; run the 8192 path for bands < crossover (only every other call is fine - it is a 170 ms window; cache the result); apply the same dB floor/tilt mapping; crossfade the two bands either side of the crossover 50/50 so there is no seam. Keep NaN sanitisation. Document the design and the measured crossover band in the module header.

- [ ] **Step 4: Run** - `cargo test dsp:: 2>&1 | tail -3`; then `cargo run --release -- --levels` for 20 s with music playing and confirm the printed bands now spread the bass. Eyeball one `dump_` PNG.

- [ ] **Step 5: Commit** - `git commit -am "bands: log-spaced bass via a second 8192-point FFT below the crossover band"`

---

### Task 5: Ballistics per millisecond

**Files:**
- Modify: `src/dsp/ballistics.rs` (`Smoother::update` -> `update(&mut self, target, dt_ms: f32)`), `src/main.rs:434` and `:853` (pass `dt_ms`; `measure_levels` can pass 16.667)

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn two_short_frames_decay_as_far_as_one_long_frame() {
        let b = Ballistics { attack: 0.9, decay: 0.2, peak_fall: 0.01 };
        let full = [1.0; NUM_BANDS];
        let zero = [0.0; NUM_BANDS];
        let mut a = Smoother::new(b);
        let mut c = Smoother::new(b);
        a.update(&full, 16.667); c.update(&full, 16.667);
        a.update(&zero, 16.667); a.update(&zero, 16.667);
        c.update(&zero, 33.333);
        assert!((a.levels()[0] - c.levels()[0]).abs() < 1e-3, "{} vs {}", a.levels()[0], c.levels()[0]);
        assert!((a.peaks()[0] - c.peaks()[0]).abs() < 1e-3);
    }
```

- [ ] **Step 2: Run to verify failure** - Expected: compile error (extra argument).

- [ ] **Step 3: Implement** - `fn per_frame(rate: f32, dt_ms: f32) -> f32 { 1.0 - (1.0 - rate.clamp(0.0, 1.0)).powf((dt_ms.clamp(1.0, 250.0)) / 16.667) }` applied to attack and decay; `peak_fall` scales linearly: `peak_fall * dt_ms / 16.667`. Update callers to pass `dt_ms`.

- [ ] **Step 4: Run** - `cargo test 2>&1 | tail -3` - Expected: green (the smoother tests that assumed per-frame rates pass 16.667).

- [ ] **Step 5: Commit** - `git commit -am "ballistics: frame-rate independent attack, decay and peak fall"`

---

### Task 6: Capture device poll throttle and stress rows

**Files:**
- Modify: `src/win/capture.rs` (`capture_loop` ~line 406: throttle; add `pub fn should_poll_device(last: Instant, now: Instant) -> bool`)
- Modify: `src/main.rs` `stress()` (~line 1092): two new rows

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn device_poll_is_at_most_once_per_second() {
        let t0 = std::time::Instant::now();
        assert!(!should_poll_device(t0, t0 + std::time::Duration::from_millis(999)));
        assert!(should_poll_device(t0, t0 + std::time::Duration::from_millis(1000)));
    }
```

- [ ] **Step 2: Run to verify failure** - compile error.

- [ ] **Step 3: Implement** - `pub fn should_poll_device(last, now) -> bool { now.duration_since(last) >= Duration::from_secs(1) }`; in `capture_loop` keep `let mut last_poll = Instant::now();` and only run the `GetDefaultAudioEndpoint` check when `should_poll_device(last_poll, now)`, updating `last_poll`. Comment: two cross-process COM calls per packet was up to 250 calls/s and was never in the leak hunt. In `stress()`, add rows following the existing pattern: `capture steady state` (run `capture_loop` on a thread for 10 s, sample handles/threads before and after) and `capture reopen` (start/stop the loop 20 times).

- [ ] **Step 4: Run** - `cargo test capture 2>&1 | tail -3`; `cargo run --release -- --stress 2>&1 | tail -12` and read the two new rows.

- [ ] **Step 5: Commit** - `git commit -am "capture: poll the default device once a second, and put capture in the leak stress"`

---

### Task 7: Linear-light blending and OKLCH rainbow

**Files:**
- Modify: `src/render/canvas.rs` (`blend_over`, `sample_stops`, `lerp`/`mix` helpers, `bloom`; add `srgb_to_linear`/`linear_to_srgb` LUTs)
- Modify: `src/themes/mod.rs` (`rainbow_hsv` -> returns OKLCH; rename to `rainbow_oklch` and update `render::tint`)
- Modify: `tests/golden/*.txt` (regenerate ONCE, after a visual check)

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn half_white_over_black_is_linear_light() {
        let mut c = Canvas::new(1, 1);
        c.fill(Rgba::new(0, 0, 0, 255));
        c.blend_over(0, 0, Rgba::new(255, 255, 255, 128));
        let r = (c.bits()[0] >> 16) & 0xff;
        assert!((186..=190).contains(&r), "got {r}, expected ~188 (linear), not 128 (gamma)");
    }

    #[test]
    fn gradient_midpoint_is_the_linear_mean() {
        let stops = [(0.0, Rgba::new(0, 0, 0, 255)), (1.0, Rgba::new(255, 255, 255, 255))];
        let mid = sample_stops(&stops, 0.5);
        assert!((186..=190).contains(&(mid.r as u32)), "got {}", mid.r);
    }

    #[test]
    fn rainbow_hues_share_one_lightness() {
        let t = crate::themes::builtin::by_id("segmented-rainbow").unwrap(); // any rainbow colourway
        let ls: Vec<f32> = (0..12).map(|i| { let c = crate::render::tint(&t, i as f32 / 12.0, 0.0, false, "#ffffff", 1.0); oklab_l_of(c) }).collect();
        let (mn, mx) = (ls.iter().cloned().fold(1.0, f32::min), ls.iter().cloned().fold(0.0, f32::max));
        assert!(mx - mn < 0.02, "L spread {mn}..{mx}");
    }
```
(Adapt names: `fill`, `bits`, `sample_stops` signature, `oklab_l_of` visibility.)

- [ ] **Step 2: Run to verify failure** - Expected: the first two FAIL (~128), the third FAILS (HSV rainbows vary in L).

- [ ] **Step 3: Implement** - LUTs: `static SRGB_TO_LIN: [f32; 256]` built with `OnceLock`; `fn lin_to_srgb(v: f32) -> u8` via the standard piecewise curve. Apply in `blend_over` (alpha-weighted mean in linear per channel), `sample_stops` and every colour `lerp`/`mix` helper in canvas (grep `fn lerp`/`fn mix` in `render/`; families that have their own private lerp should call the canvas one - change those call sites, do not leave two implementations), and `bloom` (convert to linear, blur, convert back). `rainbow_oklch`: keep hue/ink logic, output `(l, c, h)` with `l` per colourway (existing `v`/`s` map to `l = 0.62 + 0.25*v`, `c = oklch_max_chroma(l, h) * s`); `tint` uses `Rgba::from_oklch`.

- [ ] **Step 4: Visual check then goldens** - `cargo test --release dump_ -- --ignored`; Read 4-6 PNGs (a gradient-heavy family such as vapor or blossom, a rainbow colourway, a bloom-heavy one such as rave). Then `cargo test golden 2>&1 | tail` will fail; regenerate via the goldens' documented update path (see `render/golden.rs`) and commit the new text files in the SAME commit with the reason.

- [ ] **Step 5: Run** - `cargo test 2>&1 | tail -3` - green.

- [ ] **Step 6: Commit** - `git commit -am "canvas: blend, gradients and bloom in linear light; rainbows in OKLCH (goldens regenerated for the gamma change)"`

---

### Task 8: Lint and test hygiene

**Files:**
- Modify: `src/win/shell_state.rs` (tests), `src/win/tray.rs` (tests), `src/render/mod.rs` (sweeps), `src/render/fluid.rs` (sweeps), `src/render/opacity.rs`, `src/render/rwr.rs:995`, plus whatever clippy names.

- [ ] **Step 1: Write the replacement structural guard (failing first only if something is currently black - it is expected to PASS; note that in the ledger)**

In `render/mod.rs` tests:
```rust
    /// Cheap enough for the default suite: every colourway at three sizes, two levels, asserting
    /// the frame is not empty and not one flat colour. This is the guard that would have caught
    /// pipes shipping black under 49 rows.
    #[test]
    fn every_colourway_is_visibly_alive_at_three_sizes() {
        for t in crate::themes::builtin::all() {
            for (w, h) in [(190, 48), (380, 48), (380, 60)] {
                for level in [0.15f32, 0.6] {
                    let mut fam = family_for(&t.family);
                    let mut c = Canvas::new(w, h);
                    let mut d = FrameData::default();
                    for v in d.levels.iter_mut() { *v = level; }
                    d.rms = level;
                    for _ in 0..20 { fam.draw(&mut c, &t, &d); }
                    let px = c.bits();
                    let lit = px.iter().filter(|p| (**p >> 24) > 8).count();
                    let distinct: std::collections::HashSet<u32> = px.iter().map(|p| *p & 0xffffff).collect();
                    assert!(lit as f32 >= 0.02 * px.len() as f32, "{} {w}x{h} @{level}: {lit} lit", t.id);
                    assert!(distinct.len() >= 2, "{} {w}x{h} @{level}: one flat colour", t.id);
                }
            }
        }
    }
```

- [ ] **Step 2: Move the slow sweeps** - rename the five >60 s tests with a `slow_` prefix and `#[ignore]`, with a doc comment: run via `cargo test --release slow_ -- --ignored`. Add that line to README "Build from source" and `TODO.md` notes.

- [ ] **Step 3: shell_state tests** - add `static SERIAL: Mutex<()>` held by every test that reads or writes the module statics; replace the vacuous defaults test with one that calls a `#[cfg(test)] fn reset()` then asserts the statics.

- [ ] **Step 4: tray tests that create real icons** - `#[ignore]` with "creates a real tray icon on the desktop; run by hand".

- [ ] **Step 5: clippy** - `cargo clippy --all-targets 2>&1 | grep -E "^(warning|error)" | sort | uniq -c | sort -rn`. Fix the two deny-level tautologies (`shell_state.rs:227`, `rwr.rs:995`) with `std::hint::black_box`, then the rest mechanically. Where a lint is wrong for the code, `#[allow(clippy::...)]` with a one-line reason. Target: `cargo clippy --all-targets -- -D warnings` exits 0.

- [ ] **Step 6: Run** - `cargo test 2>&1 | tail -3` (note the wall time; expect well under 60 s), `cargo clippy --all-targets -- -D warnings 2>&1 | tail -2`.

- [ ] **Step 7: Commit** - `git commit -am "tests: structural liveness guard in the default suite, slow sweeps behind --ignored, shell_state serialised, clippy clean"`

---

### Task 9: CLI, binary hygiene, probes out of main.rs

**Files:**
- Create: `src/probes.rs` (receives `diagnose`, `measure_levels`, `stress`, `attach_console_if_wanted` and their private helpers from `main.rs` lines ~593-1170)
- Modify: `src/main.rs` (arg parsing; `mod probes;`), `Cargo.toml` (`rust-version`, `panic = "abort"` in release)

- [ ] **Step 1: Write the failing test** (pure arg parser):

```rust
    #[test]
    fn help_and_version_and_unknown_flags_never_launch_the_overlay() {
        assert_eq!(parse_args(&["--help".into()]), Cli::Help);
        assert_eq!(parse_args(&["--version".into()]), Cli::Version);
        assert_eq!(parse_args(&["--diagnose".into()]), Cli::Diagnose);
        assert_eq!(parse_args(&["--bogus".into()]), Cli::Unknown("--bogus".into()));
        assert_eq!(parse_args(&[]), Cli::Run { console: false });
        assert_eq!(parse_args(&["--console".into()]), Cli::Run { console: true });
    }
```

- [ ] **Step 2: Run to verify failure** - compile error.

- [ ] **Step 3: Implement** - `enum Cli { Run { console: bool }, Help, Version, Diagnose, Levels, Stress, Unknown(String) }` and `fn parse_args(args: &[String]) -> Cli` in `main.rs`; `main` matches it: `Help` prints usage (list every flag with one line each) and returns; `Version` prints `taskbar-eq {CARGO_PKG_VERSION}`; `Unknown` prints usage to stderr and exits 1. Move the probes to `src/probes.rs` (`pub fn diagnose()`, `pub fn measure_levels()`, `pub fn stress()`, `pub fn attach_console_if_wanted()`); behaviour unchanged. `Cargo.toml`: `rust-version = "<current stable as reported by rustc --version>"`; `[profile.release] panic = "abort"`.

- [ ] **Step 4: Run** - `cargo test 2>&1 | tail -3`; `cargo build --release && grep -c cwisset target/release/taskbar-eq.exe` - Expected 0. If not 0, add `RUSTFLAGS=--remap-path-prefix` guidance to README and CI (Task 10) and record the count in the ledger. `target\release\taskbar-eq.exe --help` prints usage and exits; `--version` prints the version.

- [ ] **Step 5: Commit** - `git commit -am "cli: --help/--version, unknown flags refuse to launch; probes move to src/probes.rs; no build path in the exe"`

---

### Task 10: Docs, privacy, changelog, release, CI

**Files:**
- Modify: `README.md`, `TODO.md`, `docs/theme-backlog.md`, `Cargo.toml` (0.2.1), `.gitignore`
- Create: `docs/status.md`, `docs/known-gaps.md`, `docs/themes.md`, `docs/theme-prompt.md`, `docs/lessons.md`, `CHANGELOG.md`, `docs/screenshot.png`, `.github/workflows/ci.yml`
- Delete: `HANDOVER.md`, `dist/taskbar-eq.exe`

No tests (docs), but every moved section is verified present in its new file with `grep`.

- [ ] **Step 1: Split README** - move `## Status` -> `docs/status.md`, `## Known gaps` (if present) -> `docs/known-gaps.md`, `## Themes` catalogue -> `docs/themes.md` (fix "93 built-ins" -> current count; list all 22 families; document that blossom/flame/nixie/patchbay/reel read the `[tube]` table), `## Prompt: generate more themes` -> `docs/theme-prompt.md`. README keeps Install / Using it / Song identification / Configuration / Build from source / Licence and links to the docs. Under Song identification add:

```markdown
### What leaves your machine

Only when you press the Identify key or menu item. A Shazam audio *fingerprint* (spectral peaks,
not audio) is posted to `amp.shazam.com`, along with a fixed fake location (45N 2E, Europe/Paris)
and an Android user-agent, exactly as the open-source SongRec client does. Nothing is sent at any
other time. Locally, `taskbar-eq.log` records each identified title and `songs.jsonl` keeps every
find with its links; both stay in `%APPDATA%\taskbar-eq`.
```

- [ ] **Step 2: Screenshot** - `cargo test --release dump_wide -- --ignored`, pick one strong family PNG from `target/eyeball/`, copy to `docs/screenshot.png`, add `![taskbar-eq](docs/screenshot.png)` under the README title.

- [ ] **Step 3: HANDOVER -> lessons; TODO -> CHANGELOG** - `docs/lessons.md` = the "Things that cost real time" section only; `git rm HANDOVER.md`; fix the README link. `CHANGELOG.md` (Keep a Changelog): `## [0.2.1]` (this plan's tasks, one line each), `## [0.2.0]` song identification, `## [0.1.0]` everything before, summarised from TODO's Done section. `TODO.md`: one `## In progress` (empty or the VS families), `## Waiting on you` trimmed to genuine open judgement calls, `## Open, unresolved` kept, Done section replaced by "see CHANGELOG.md". `docs/theme-backlog.md`: done items collapsed to one ledger line, the open "use the empty taskbar to the left" idea moved to TODO.

- [ ] **Step 4: Release plumbing** - `Cargo.toml` version 0.2.1. `.gitignore` += `dist/`. `git rm -r dist`. README download link -> `https://github.com/cwissett-hub/taskbar-eq/releases/latest/download/taskbar-eq.exe`, with a line on verifying the SHA-256 from the release notes and the unsigned-exe / Defender ML caveat.

`.github/workflows/ci.yml`:
```yaml
name: ci
on: { push: { branches: [main], tags: ["v*"] }, pull_request: {} }
jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - run: cargo test
      - run: cargo clippy --all-targets -- -D warnings
      - uses: actions/upload-artifact@v4
        with: { name: taskbar-eq, path: target/release/taskbar-eq.exe }
      - if: startsWith(github.ref, 'refs/tags/v')
        run: gh release upload ${{ github.ref_name }} target/release/taskbar-eq.exe --clobber
        env: { GH_TOKEN: ${{ github.token }} }
```

- [ ] **Step 5: Verify, commit, tag, release** - `cargo test 2>&1 | tail -3`; `cargo build --release`; `sha256sum target/release/taskbar-eq.exe`. Commit: `git commit -am "docs: README split, privacy note, CHANGELOG, lessons; release via GitHub Releases, dist/ retired; CI"`. Then `git tag -a v0.2.1 -m "v0.2.1"`, `git push && git push --tags`, and `gh release create v0.2.1 target/release/taskbar-eq.exe --title "v0.2.1" --notes "<CHANGELOG 0.2.1 section + SHA-256 line>"`. Confirm `gh release view v0.2.1` lists the asset. If GitHub Actions is blocked for the account, say so in the ledger; the release still exists because it was created from the local exe.

---

## Self-review notes

- Spec coverage: A1 T1, A2 T2, A3 T3, A4 T4, A5 T5, A6 T6, A7 T7, A8 T8, A9 T9, A10 T10. Out-of-scope list respected.
- Type consistency: `Smoother::update(target, dt_ms)` in T5 matches both call sites named; `should_poll_device(Instant, Instant)` in T6; `parse_args(&[String]) -> Cli` in T9; `Config::dir()` env override in T2 used by no other task.
- Review Focus 1-5 each have a named test in T2, T2, T5, T4, T9.
- Risk: T4 is the only algorithmic change; its fixture-CSV regression tests are the safety net and the `--levels` probe is the live check. T7 regenerates goldens - allowed once, in the same commit, after a visual check.
