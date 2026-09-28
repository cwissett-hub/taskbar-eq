//! The out-of-band probes: `--diagnose`, `--levels`, `--stress`, and the console-attach helper they
//! all share.
//!
//! Moved out of `main.rs` so that file can be read as "arg parsing, then the run path" - these are
//! one-shot diagnostics, not part of the app that runs every day, and they were crowding out the
//! render loop they exist to debug.

use crate::config::Config;
use crate::dsp;
use crate::dsp::ballistics::Smoother;
use crate::log;
use crate::themes;
use crate::win;
use crate::{FALLBACK_GAP, FALLBACK_WIDTH, WIDEN_MARGIN};
use anyhow::Result;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

/// Walks up from the current directory looking for a `tests/fixtures` directory, so `--levels`
/// can find where to write its captured fixture without a build-machine path compiled into the
/// exe.
///
/// `tests/fixtures` is committed to the repo (it already holds the checked-in
/// `real-music-bands.csv` and friends), so any checkout that still has that directory - whether
/// the current directory IS the repo root (`cargo run --release -- --levels`) or somewhere
/// inside it - finds it. Returns `None` rather than a default guess when nothing is found, so the
/// caller can say exactly what it was looking for instead of writing to a path nobody asked for.
fn fixtures_dir() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join("tests/fixtures");
        if candidate.is_dir() {
            return Some(candidate);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

/// Gives this process somewhere to print, when there is a reason to.
///
/// A GUI-subsystem binary starts with no stdout at all, so `println!` and `eprintln!` write into
/// nothing. That is the right default for a taskbar ornament, but several cases still need output:
///
///   * `--console` allocates a fresh window on purpose, for watching it run;
///   * `--diagnose`, `--levels` and `--stress` exist to be READ, so they allocate one too if they
///     cannot inherit;
///   * `--help`, `--version` and an unrecognised flag exist to be read from a terminal too;
///   * launched from an existing terminal, ATTACH_PARENT_PROCESS inherits that terminal, so
///     `taskbar-eq.exe --diagnose` behaves exactly as it did before this change.
///
/// Nothing here is load-bearing for diagnostics regardless: the log file is written and flushed per
/// line whether or not a console exists, which is the whole reason it was added.
pub fn attach_console_if_wanted(force: bool) {
    use windows::Win32::System::Console::{AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        // Inheriting the caller's terminal is always preferable - it puts the output where whoever
        // ran the command is already looking.
        if AttachConsole(ATTACH_PARENT_PROCESS).is_ok() {
            return;
        }
        if force {
            let _ = AllocConsole();
        }
    }
}

/// Prints why the overlay would or would not be drawn, then exits.
///
/// Exists because the Windows 10 report - "anchoring worked but it showed a generic icon instead of
/// rendering" - was not diagnosable from the outside at all. Every gate is checked in the same order
/// the render loop checks them, so the first `NO` in this report is the answer.
pub fn diagnose() -> Result<()> {
    log::init();
    let dpi = win::dpi::set_per_monitor_v2();
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        log::write(&format!("CoInitializeEx FAILED: {e}  <- UIA and WASAPI both need this"));
    }
    let cfg = Config::load();

    log::write(&format!("dpi awareness: {dpi}"));
    log::write(&format!("configured width: {} px", cfg.width));
    log::write(&format!("selected theme: {}", cfg.theme));

    let bar = win::placement::taskbar_rect();
    log::write(&match bar {
        Some(b) => format!("taskbar rect: {}x{} at ({},{})  YES", b.w, b.h, b.x, b.y),
        None => "taskbar rect: NOT FOUND  <- Shell_TrayWnd is missing; nothing can be anchored".into(),
    });

    let elements = match win::placement::taskbar_elements() {
        Ok(e) => {
            log::write(&format!("UIA elements found: {}  YES", e.len()));
            e
        }
        Err(e) => {
            log::write(&format!("UIA enumeration FAILED: {e}  <- cannot find anything to anchor to"));
            Vec::new()
        }
    };

    let widget = win::placement::widget_rect_in(&elements);
    log::write(&match widget {
        Some(r) => format!("Widgets button: {}x{} at ({},{})  YES", r.w, r.h, r.x, r.y),
        None => "Widgets button: NOT FOUND  (expected on Windows 10 - using the chevron)".into(),
    });
    let chevron = win::placement::chevron_rect_in(&elements);
    log::write(&match chevron {
        Some(r) => format!("overflow chevron: {}x{} at ({},{})  YES", r.w, r.h, r.x, r.y),
        None => "overflow chevron: NOT FOUND  <- on Windows 10 this is the only anchor".into(),
    });

    // Resolve the rect exactly as the loop does.
    let rect = match (widget, bar) {
        (Some(w), Some(b)) => Some(win::placement::widened(
            w,
            b,
            win::placement::left_limit(&elements, w),
            cfg.width,
            WIDEN_MARGIN,
        )),
        _ => match (chevron, bar) {
            (Some(c), Some(b)) => {
                let anchored = win::placement::rect_left_of(c, b, FALLBACK_GAP, FALLBACK_WIDTH);
                Some(win::placement::widened(
                    anchored,
                    b,
                    win::placement::left_limit(&elements, anchored),
                    cfg.width,
                    WIDEN_MARGIN,
                ))
            }
            _ => None,
        },
    };
    log::write(&match rect {
        Some(r) => format!("chosen overlay rect: {}x{} at ({},{})  YES", r.w, r.h, r.x, r.y),
        None => "chosen overlay rect: NONE  <- nothing to draw into".into(),
    });

    // The sanity check that silently suppresses drawing if it fails.
    if let Some(r) = rect {
        let ok = r.is_plausible_widget();
        log::write(&format!(
            "rect passes the plausibility check: {}  {}",
            if ok { "YES" } else { "NO" },
            if ok {
                String::new()
            } else {
                format!(
                    "<- needs w 40..600 and h 20..200, got w {} h {}; the overlay is suppressed",
                    r.w, r.h
                )
            }
        ));
    }

    let quns = win::placement::notification_state();
    let blocked = quns == win::visibility::QUNS_FULLSCREEN || quns == win::visibility::QUNS_PRESENTATION;
    log::write(&format!(
        "notification state: {quns}  {}",
        if blocked { "NO  <- fullscreen or presentation mode suppresses the overlay" } else { "YES" }
    ));
    log::write(&format!(
        "taskbar visible: {}",
        if win::placement::taskbar_visible() { "YES" } else { "NO  <- auto-hide taskbar?" }
    ));

    // The geometric fullscreen check, reported separately because it is the one that catches a
    // borderless game - and therefore the line to read on a machine where the overlay is drawing over
    // one. `SHQueryUserNotificationState` above will say 5 (accepts notifications) in that case.
    let fg = win::placement::foreground_window();
    let fullscreen_foreground = win::visibility::covers_monitor(fg.as_ref());
    log::write(&match &fg {
        Some(f) => format!(
            "foreground window: class \"{}\" {}x{} at {},{} on a {}x{} monitor{} => covers the \
             monitor: {}",
            f.class,
            f.window.w,
            f.window.h,
            f.window.x,
            f.window.y,
            f.monitor.w,
            f.monitor.h,
            if f.is_shell { " (the shell's own, so never counted)" } else { "" },
            if fullscreen_foreground { "YES  <- the overlay will suspend" } else { "no" }
        ),
        None => "foreground window: none readable (minimised, or nothing focused)".to_string(),
    });

    let inputs = win::visibility::Inputs {
        widget: rect,
        notification_state: quns,
        taskbar_visible: win::placement::taskbar_visible(),
        fullscreen_foreground,
        // A diagnose run cannot be looking at an off display - it is being read on the screen it would
        // be reporting about.
        display_off: false,
    };
    log::write(&format!(
        "=> would draw: {}",
        if win::visibility::should_show(&inputs) { "YES" } else { "NO" }
    ));

    // Audio last, because a silent capture looks exactly like "nothing renders": the reveal gate
    // never opens, so the overlay is never shown even when every check above passes.
    log::write("starting audio capture for 2s to see whether any audio is reaching us...");
    let rx = win::capture::start();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut frames = 0u32;
    let mut peak = 0.0f32;
    while std::time::Instant::now() < deadline {
        while let Ok(f) = rx.try_recv() {
            frames += 1;
            peak = peak.max(f.rms);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    log::write(&format!(
        "capture: {frames} frames, peak rms {peak:.4}  {}",
        if frames == 0 {
            "NO  <- no audio frames arrived at all, so the reveal gate can never open.
             This is indistinguishable from 'nothing renders' and is the first thing to rule out."
        } else if peak < 0.0005 {
            "silent - play something and run --diagnose again"
        } else {
            "YES"
        }
    ));

    log::write(&format!("report written to {}", log::path().display()));
    // ---- transport control ---------------------------------------------------------------------
    // Deliberately does NOT attempt registration. `--diagnose` is normally run WHILE the app is
    // running - it autostarts, so that is the usual state - and the live instance already owns the
    // combinations, so every attempt would come back 1409 and print NO for a setup that is working
    // perfectly. That would poison this file's own "the first NO is the answer" contract. What can be
    // checked without side effects is whether each binding parses and passes validation, which is
    // exactly where a hand-edited config.toml goes wrong.
    let running = unsafe {
        windows::Win32::UI::WindowsAndMessaging::FindWindowW(windows::core::w!("TaskbarEqTray"), None)
    }
    .is_ok();
    log::write(&format!(
        "another taskbar-eq is running: {}",
        if running { "YES  <- it owns the hotkeys, so they are not re-tested here" } else { "no" }
    ));

    match win::media::probe() {
        Ok((_, win::media::Status::NoSession)) => log::write(
            "spotify session: none  NO  <- start Spotify and play something once, then re-run",
        ),
        Ok((id, st)) => log::write(&format!("spotify session: {id} ({st:?})  YES")),
        Err(e) => log::write(&format!("spotify session: probe failed - {e}  NO")),
    }

    let texts = [
        (win::hotkeys::Slot::PlayPause, &cfg.hotkeys.play_pause),
        (win::hotkeys::Slot::NextTrack, &cfg.hotkeys.next_track),
        (win::hotkeys::Slot::PrevTrack, &cfg.hotkeys.prev_track),
    ];
    let mut bound = Vec::new();
    for (slot, text) in texts {
        if text.trim().is_empty() {
            log::write(&format!("hotkey {}: not set", slot.label()));
            continue;
        }
        match win::hotkey::Chord::parse(text) {
            Err(e) => log::write(&format!(
                "hotkey {}: cannot read {text:?} - {e}  NO",
                slot.label()
            )),
            Ok(c) => match c.validate(&bound) {
                Err(why) => log::write(&format!(
                    "hotkey {}: {c} refused - {}  NO",
                    slot.label(),
                    why.message()
                )),
                Ok(adv) => {
                    bound.push(c);
                    let note = if adv.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "  (note: {})",
                            adv.iter().map(|a| a.message()).collect::<Vec<_>>().join("; ")
                        )
                    };
                    log::write(&format!("hotkey {}: {c} valid{note}  YES", slot.label()));
                }
            },
        }
    }
    log::write(&format!("transport backend: {:?}", cfg.media_backend));

    Ok(())
}

/// Captures real audio and reports what the DSP actually emits, then exits.
///
/// Exists because every audio-to-pixel mapping in this project is calibrated against a claim -
/// "real music sits at band levels of roughly 0.15-0.65" - that was never measured. It is written
/// into the comments of the VU needle, the scope gain, the valve response and the vaporwave terrain.
/// If it is wrong, all four are miscalibrated in the same direction, and the vaporwave grid reading
/// as unresponsive after three separate fixes is exactly what that would look like.
pub fn measure_levels() -> Result<()> {
    log::init();
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        log::write(&format!("CoInitializeEx failed: {e}"));
    }
    let rx = win::capture::start();
    // The VAPORWAVE theme's ballistics, not the defaults. Ballistics are applied upstream per
    // theme, so measuring with the defaults measures a signal no vapor colourway ever sees: the
    // first run of this probe did exactly that and reported the lightning trigger firing 0 times in
    // 8s, while the user was watching it fire on every snare. attack 0.88 against the default 0.55
    // makes frame-to-frame rises far larger, which is the whole quantity the trigger reads.
    let vapor_ballistics = themes::builtin::vapor_sunset().ballistics;
    log::write(&format!(
        "measuring with the vapor ballistics: attack {:.2} decay {:.2}",
        vapor_ballistics.attack, vapor_ballistics.decay
    ));
    let mut smoother = Smoother::new(vapor_ballistics);
    let mut samples: Vec<[f32; dsp::bands::NUM_BANDS]> = Vec::new();
    let mut raw_peak = 0.0f32;
    let mut raw_frames: Vec<[f32; dsp::bands::NUM_BANDS]> = Vec::new();
    let mut rms_seen: Vec<f32> = Vec::new();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    while std::time::Instant::now() < deadline {
        while let Ok(f) = rx.try_recv() {
            raw_peak = raw_peak.max(f.bands.iter().copied().fold(0.0f32, f32::max));
            raw_frames.push(f.bands);
            rms_seen.push(f.rms);
            smoother.update(&f.bands, 16.667);
            samples.push(*smoother.levels());
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }

    // Record the RAW per-frame bands to disk as a reusable fixture.
    //
    // Real audio is only available while someone is playing music, and every calibration in this
    // project has so far been done against a synthetic spectrum I invented - which is precisely why
    // the terrain has now been "fixed" four times. A captured fixture means every future tuning
    // decision can be measured against real material with no music playing.
    if !raw_frames.is_empty() {
        let mut out = String::new();
        for f in &raw_frames {
            out.push_str(
                &f.iter().map(|v| format!("{v:.5}")).collect::<Vec<_>>().join(","),
            );
            out.push('\n');
        }
        // Found at RUNTIME, not baked in via `env!("CARGO_MANIFEST_DIR")`: that constant is the
        // path on the machine that BUILT the exe, and this exe ships to other machines and other
        // users - a build path is not this probe's business to carry, and for a public exe it is
        // also someone's username. `--levels` is documented as a source-checkout tool (see the
        // README and TODO.md), run either as `cargo run --release -- --levels` (cwd is already
        // the checkout root) or as the built exe from an existing checkout - both leave
        // `tests/fixtures` reachable by walking up from the current directory.
        match fixtures_dir() {
            Some(dir) => {
                let dst = dir.join("real-music-bands.csv");
                match std::fs::write(&dst, out) {
                    Ok(()) => log::write(&format!(
                        "wrote {} raw frames x {} bands to {}",
                        raw_frames.len(),
                        dsp::bands::NUM_BANDS,
                        dst.display()
                    )),
                    Err(e) => log::write(&format!("could not write the fixture: {e}")),
                }
            }
            None => log::write(
                "could not find a tests/fixtures directory under the current directory or any \
                 parent - --levels writes its fixture from a source checkout of taskbar-eq; run \
                 it from inside (or under) the repo, e.g. `cargo run --release -- --levels`",
            ),
        }
    }

    if samples.is_empty() {
        log::write("no audio frames captured - is anything playing?");
        return Ok(());
    }

    // Distribution over every (frame, band) pair, which is what a per-band element actually sees.
    let mut all: Vec<f32> = samples.iter().flat_map(|s| s.iter().copied()).collect();
    all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f32| all[((all.len() - 1) as f32 * p) as usize];
    // And the per-frame MAX band, which is what the vaporwave auto-ranger normalises against.
    let mut frame_max: Vec<f32> = samples
        .iter()
        .map(|s| s.iter().copied().fold(0.0f32, f32::max))
        .collect();
    frame_max.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let fq = |p: f32| frame_max[((frame_max.len() - 1) as f32 * p) as usize];
    rms_seen.sort_by(|a, b| a.partial_cmp(b).unwrap());

    log::write(&format!("frames {}, bands {}", samples.len(), dsp::bands::NUM_BANDS));
    log::write(&format!(
        "PER-BAND level  p10 {:.4}  p50 {:.4}  p90 {:.4}  p99 {:.4}  max {:.4}",
        q(0.10), q(0.50), q(0.90), q(0.99), all[all.len() - 1]
    ));
    log::write(&format!(
        "FRAME max band  p10 {:.4}  p50 {:.4}  p90 {:.4}  max {:.4}",
        fq(0.10), fq(0.50), fq(0.90), frame_max[frame_max.len() - 1]
    ));
    log::write(&format!(
        "rms p50 {:.4}  p90 {:.4}  max {:.4}",
        rms_seen[rms_seen.len() / 2],
        rms_seen[(rms_seen.len() * 9) / 10],
        rms_seen[rms_seen.len() - 1]
    ));
    log::write(&format!("raw (unsmoothed) peak band {raw_peak:.4}"));
    log::write("--- the assumption written throughout the code is 0.15-0.65 per active band ---");

    // ---- onset behaviour, for the lightning trigger ----
    //
    // Measured on three signals, because the bolt currently reads the most smoothed of the three:
    // the RAW per-frame bands, the DSP-smoothed levels, and the bass mean the detector actually uses.
    let raws: Vec<[f32; dsp::bands::NUM_BANDS]> = Vec::new();
    let _ = raws;
    let bass_of = |b: &[f32]| b[..4].iter().sum::<f32>() / 4.0;
    let smoothed_bass: Vec<f32> = samples.iter().map(|s| bass_of(s)).collect();
    let mut rises: Vec<f32> = smoothed_bass
        .windows(2)
        .map(|w| (w[1] - w[0]).max(0.0))
        .collect();
    // What the shipped condition would do: rise > need AND bass > floor.
    let need = 0.04 + (1.0 - 0.55) * 0.26;
    let fires = smoothed_bass
        .windows(2)
        .filter(|w| w[1] - w[0] > need && w[1] > 0.35)
        .count();
    // Spectral flux: the sum of POSITIVE change across every band, the standard onset measure.
    let flux: Vec<f32> = samples
        .windows(2)
        .map(|w| {
            w[0].iter()
                .zip(w[1].iter())
                .map(|(a, b)| (b - a).max(0.0))
                .sum::<f32>()
        })
        .collect();
    let mut fs = flux.clone();
    fs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    rises.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pick = |v: &Vec<f32>, p: f32| v[((v.len() - 1) as f32 * p) as usize];
    let secs = samples.len() as f32 / 99.0;

    log::write(&format!(
        "bass mean p50 {:.4}  p90 {:.4}  max {:.4}  (BOLT_FLOOR is 0.35)",
        pick(&smoothed_bass.to_vec().clone(), 0.5),
        pick(&{ let mut v = smoothed_bass.clone(); v.sort_by(|a,b| a.partial_cmp(b).unwrap()); v }, 0.9),
        smoothed_bass.iter().copied().fold(0.0f32, f32::max)
    ));
    log::write(&format!(
        "bass RISE/frame p50 {:.4}  p90 {:.4}  p99 {:.4}  max {:.4}  (needs > {:.3})",
        pick(&rises, 0.5), pick(&rises, 0.9), pick(&rises, 0.99), rises[rises.len()-1], need
    ));
    log::write(&format!(
        "=> the shipped trigger fired {fires} times in {secs:.1}s  ({:.2}/s)",
        fires as f32 / secs
    ));
    log::write(&format!(
        "spectral flux p50 {:.3}  p90 {:.3}  p99 {:.3}  max {:.3}",
        pick(&fs, 0.5), pick(&fs, 0.9), pick(&fs, 0.99), fs[fs.len()-1]
    ));
    for k in [0.90f32, 0.95, 0.97] {
        let th = pick(&fs, k);
        let n = flux.windows(3).filter(|w| w[1] > th && w[1] >= w[0] && w[1] >= w[2]).count();
        log::write(&format!(
            "   flux peaks above p{:.0}: {n} in {secs:.1}s ({:.2}/s)",
            k * 100.0, n as f32 / secs
        ));
    }
    Ok(())
}

/// Hunts the resource leak by repetition, and reports what each suspect costs per thousand calls.
///
/// # Why this exists
///
/// The leak is real and severe - 18,962 threads and 131,454 handles, which took a fullscreen game from
/// 160fps to 30 with dropped input - and it has never been reproducible on demand, so it has stayed a
/// mystery bounded by `win::health` rather than a bug that got fixed.
///
/// It was originally thought to need days of uptime. It does not: it was later reported after 30-45
/// minutes, on a machine that plays games in borderless fullscreen and never on one that does no
/// fullscreen rendering. `visibility::covers_monitor` fixes the suspend gap that allowed it, but whether
/// that is the whole story is inference until something measures it. This is that something.
///
/// # How to read it
///
/// Each row is handles and threads gained per 1,000 iterations. A leak of one handle per call reads as
/// ~1000. Noise from the rest of the process is a few units either way, so small values and small
/// negatives mean nothing.
///
/// # What it cannot tell you
///
/// This machine has never shown the fault. The leading hypothesis needs a starved `explorer.exe` - UIA
/// calls into a shell that a fullscreen game is monopolising block for far longer, and RPC worker
/// threads accumulate while they do - and that condition is not reproduced here. So a clean sheet is
/// weak evidence, while any row that leaks is a strong finding. Written down because the temptation on
/// seeing five zeroes is to call the leak explained.
///
/// Output goes through `log::write`, not `println!`. Rust's stdout is block-buffered when piped, so an
/// earlier version of this appeared to hang for ten minutes with an empty output file while its header
/// sat in an 8KB buffer. The log is flushed per line and is also what a report would attach.
pub fn stress() -> Result<()> {
    log::init();
    // MTA, because UIA and WASAPI both need COM and the render thread uses the same mode. Without this
    // the UIA row measures the failure path and reports a clean zero - which is exactly how an earlier
    // leak probe wasted an afternoon.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        log::write(&format!("CoInitializeEx failed: {e}  <- the UIA row below is meaningless"));
    }
    log::write("stress: hunting the resource leak. Each row is the cost per 1,000 iterations.");
    log::write(&format!(
        "  {:<26} {:>7} {:>11} {:>11} {:>9}",
        "suspect", "iters", "handles/1k", "threads/1k", "ms/call"
    ));

    // Runs `f` `iters` times and reports what it cost. The thread count needs a Toolhelp snapshot of
    // every thread on the machine, so it is sampled twice per row rather than per iteration.
    let run = |name: &str, iters: u32, mut f: Box<dyn FnMut() + '_>| {
        let (h0, t0) = (win::health::handle_count(), win::health::thread_count());
        let start = std::time::Instant::now();
        for _ in 0..iters {
            f();
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        // A beat, so anything merely slow to be reclaimed is not reported as leaked.
        std::thread::sleep(std::time::Duration::from_millis(500));
        let (h1, t1) = (win::health::handle_count(), win::health::thread_count());
        let per = |a: Option<u32>, b: Option<u32>| -> String {
            match (a, b) {
                (Some(a), Some(b)) => format!("{:+.1}", (b as f64 - a as f64) * 1000.0 / iters as f64),
                _ => "?".to_string(),
            }
        };
        log::write(&format!(
            "  {name:<26} {iters:>7} {:>11} {:>11} {:>9.2}",
            per(h0, h1),
            per(t0, t1),
            ms / iters as f64
        ));
    };

    // THE PRIME SUSPECT: a fresh IUIAutomation instance plus a full descendant enumeration of the
    // taskbar, which the render loop does every two seconds for as long as it is running. 1,000 rather
    // than more because it is ~52ms a call, so this row alone is about a minute.
    run("UIA taskbar_elements", 1_000, Box::new(|| {
        let _ = win::placement::taskbar_elements();
    }));
    // Bare COM object creation, to separate "creating any COM object leaks" from "creating THIS one and
    // then walking the tree with it does".
    run("CoCreateInstance(UIA)", 5_000, Box::new(|| {
        let _: std::result::Result<windows::Win32::UI::Accessibility::IUIAutomation, _> = unsafe {
            windows::Win32::System::Com::CoCreateInstance(
                &windows::Win32::UI::Accessibility::CUIAutomation,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )
        };
    }));
    // The cheap shell calls beside it, for contrast: if UIA leaks and these do not, that localises it to
    // the UIA client rather than to COM or to talking to the shell at all.
    run("taskbar_rect", 20_000, Box::new(|| {
        let _ = win::placement::taskbar_rect();
    }));
    run("notification_state", 20_000, Box::new(|| {
        let _ = win::placement::notification_state();
    }));
    // The new fullscreen check, which now runs five times a second for the life of the process. Worth
    // measuring precisely because it is new: a leak here would be one I had just introduced.
    run("foreground_window", 20_000, Box::new(|| {
        let _ = win::visibility::covers_monitor(win::placement::foreground_window().as_ref());
    }));
    // The WinRT media session poll, which runs every 400ms. The real GSMTC round trip, not the cached
    // string `now_playing` returns - hammering the cache would measure a mutex read and report a
    // reassuring zero.
    run("media session poll", 1_000, Box::new(|| {
        let _ = win::media::poll_for_stress();
    }));

    // The capture thread's default-device poll, newly throttled to once a
    // second. Unlike the per-1k suspects above these are not per-call: a
    // /1,000 scaling would be meaningless when "steady state" is one 10s loop
    // and "reopen" is 20 start/stops, so these rows report the ABSOLUTE
    // handle/thread growth across the run instead (the `iters` column says
    // which). The question is the same: does capture leak? This path carries
    // WASAPI device/client objects and, before the throttle, made two
    // cross-process COM calls per packet, so it belongs in the leak hunt.
    let cap = |name: &str, note: &str, mut f: Box<dyn FnMut() -> Result<()>>| {
        let (h0, t0) = (win::health::handle_count(), win::health::thread_count());
        let res = f();
        // Same beat as `run`, so anything merely slow to be reclaimed is not
        // reported as leaked.
        std::thread::sleep(std::time::Duration::from_millis(500));
        let (h1, t1) = (win::health::handle_count(), win::health::thread_count());
        let d = |a: Option<u32>, b: Option<u32>| -> String {
            match (a, b) {
                (Some(a), Some(b)) => format!("{:+}", b as i64 - a as i64),
                _ => "?".to_string(),
            }
        };
        match res {
            Ok(()) => log::write(&format!(
                "  {name:<26} {note:>7} {:>11} {:>11} {:>9}",
                d(h0, h1),
                d(t0, t1),
                "(abs)"
            )),
            Err(e) => log::write(&format!(
                "  {name:<26} {note:>7}   no capture ({e}) - likely no audio device"
            )),
        }
    };
    cap("capture steady state", "10s", Box::new(|| win::capture::stress_steady(10)));
    cap("capture reopen", "20x", Box::new(|| win::capture::stress_reopen(20)));

    log::write(
        "A leak of one handle per call reads as ~1000. NOTE: this machine has never shown the fault, and \
         the leading hypothesis needs an explorer.exe starved by a fullscreen game - so a clean sheet \
         here is weak evidence, while any row that leaks is a strong finding.",
    );
    Ok(())
}
