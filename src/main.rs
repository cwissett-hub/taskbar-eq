mod config;
mod log;
mod dsp;
mod geom;
mod render;
mod themes;
mod win;

use anyhow::Result;
use config::Config;
use dsp::ballistics::Smoother;
use dsp::gate::Gate;
use render::canvas::Canvas;
use render::FrameData;
use win::capture::Frame;
use win::tray::{Tray, TrayEvent};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

/// Consecutive rect-discovery misses tolerated before the overlay gives up a known-good rect.
///
/// Module scope, not inside `main`, because `--diagnose` has to resolve the rect through exactly the
/// same constants the render loop uses. A diagnostic that computes a different answer from the code
/// it is diagnosing is worse than none.
const RECT_MISS_LIMIT: u32 = 4;
const FALLBACK_GAP: i32 = 4;
const FALLBACK_WIDTH: i32 = 190;

/// Pixels of clearance kept between the display's left edge and whatever is next to it.
///
/// Non-zero because the overlay receives its own clicks (it does not set WS_EX_TRANSPARENT - see
/// win::overlay), so a pixel it covers is a pixel of taskbar that can no longer be clicked. Butting
/// right up against a pinned app button would make that button feel dead along its edge.
const WIDEN_MARGIN: i32 = 8;

/// Width change, in pixels, below which the display keeps its current width.
///
/// The available clearance shifts whenever any window opens, closes or is minimised, and the scope
/// family clears its persistence buffers on a canvas resize - so without hysteresis an unrelated bit
/// of taskbar activity wipes the phosphor trail once a second.
const WIDTH_HYSTERESIS: i32 = 12;

/// Prints why the overlay would or would not be drawn, then exits.
///
/// Exists because the Windows 10 report - "anchoring worked but it showed a generic icon instead of
/// rendering" - was not diagnosable from the outside at all. Every gate is checked in the same order
/// the render loop checks them, so the first `NO` in this report is the answer.
fn diagnose() -> Result<()> {
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

    let inputs = win::visibility::Inputs {
        widget: rect,
        notification_state: quns,
        taskbar_visible: win::placement::taskbar_visible(),
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
            "NO  <- no audio frames arrived at all, so the reveal gate can never open. This is              indistinguishable from 'nothing renders' and is the first thing to rule out."
        } else if peak < 0.0005 {
            "silent - play something and run --diagnose again"
        } else {
            "YES"
        }
    ));

    log::write(&format!("report written to {}", log::path().display()));
    Ok(())
}

/// Captures real audio and reports what the DSP actually emits, then exits.
///
/// Exists because every audio-to-pixel mapping in this project is calibrated against a claim -
/// "real music sits at band levels of roughly 0.15-0.65" - that was never measured. It is written
/// into the comments of the VU needle, the scope gain, the valve response and the vaporwave terrain.
/// If it is wrong, all four are miscalibrated in the same direction, and the vaporwave grid reading
/// as unresponsive after three separate fixes is exactly what that would look like.
fn measure_levels() -> Result<()> {
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
            smoother.update(&f.bands);
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
        let dst = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/real-music-bands.csv");
        if let Some(d) = dst.parent() {
            let _ = std::fs::create_dir_all(d);
        }
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
        "rms             p50 {:.4}  p90 {:.4}  max {:.4}",
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
        "bass mean       p50 {:.4}  p90 {:.4}  max {:.4}  (BOLT_FLOOR is 0.35)",
        pick(&smoothed_bass.iter().copied().collect::<Vec<_>>().clone(), 0.5),
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
        "spectral flux   p50 {:.3}  p90 {:.3}  p99 {:.3}  max {:.3}",
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

fn main() -> Result<()> {
    if std::env::args().any(|a| a == "--levels") {
        return measure_levels();
    }
    // `--diagnose` prints the whole decision chain and exits. The first NO in its output is the
    // reason the overlay is not on screen.
    if std::env::args().any(|a| a == "--diagnose") {
        return diagnose();
    }
    // Before anything else, so a failure during startup is recoverable from the log rather than
    // flashing past on a console that closes with the process.
    log::init();
    log::write(&format!("dpi awareness: {}", win::dpi::set_per_monitor_v2()));
    // S_OK and S_FALSE both map to Ok; a real Err (e.g. RPC_E_CHANGED_MODE)
    // matters because UI Automation and WASAPI both require COM initialised.
    // CoInitializeEx returns HRESULT, not Result - `.ok()` is required and maps
    // S_OK/S_FALSE to Ok(()), leaving only genuine failures as Err.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        log::write(&format!("CoInitializeEx failed: {e}"));
    }

    let mut cfg = Config::load();
    // Built-ins merged with any `%APPDATA%\taskbar-eq\themes\*.toml` overrides. A
    // malformed external file is skipped, not fatal - it is hand-authored data.
    let (mut all_themes, theme_warnings) = themes::registry();
    for w in &theme_warnings {
        eprintln!("theme: {w}");
    }
    let theme_menu: Vec<win::tray::MenuItem> = all_themes
        .iter()
        .map(|t| win::tray::MenuItem::new(&t.id, &t.name, &t.family))
        .collect();
    // Watches `%APPDATA%\taskbar-eq\themes` and flags a batch of edits so the
    // loop below can reload without a restart. A watch that could not be
    // established (e.g. the directory is unwatchable) degrades to
    // `changed()` always returning false - already warned about inside
    // `Watcher::new` - rather than crashing the app.
    let watcher = themes::watch::Watcher::new();

    let mut theme = all_themes
        .iter()
        .find(|t| t.id == cfg.theme)
        .cloned()
        .unwrap_or_else(themes::builtin::vfd_ice);
    let mut family = render::family_for(&theme.family);
    let mut smoother = Smoother::new(theme.ballistics);
    let mut gate = Gate::new(cfg.gate_config());
    let overlay = win::overlay::Overlay::new()?;
    // The tray icon is not decoration: when nothing is playing the overlay
    // does not exist, so this is the only way to quit the app.
    let mut tray = Tray::new(&theme_menu)?;
    let rx = win::capture::start();

    let mut latest = Frame::default();
    let mut rect_tick = 0u32;
    let mut rect: Option<geom::Rect> = None;
    // Consecutive rect-discovery misses tolerated before the overlay gives up and
    // hides. At one probe per second this is a few seconds of grace.

    // Fallback placement, used when there is no Widgets button: sit this far left of
    // the tray's overflow chevron, at this width.


    let mut rect_misses: u32 = 0;
    // Hot-reload debounce: wait this long after the last filesystem event before
    // reparsing, so one save produces one reload.
    const RELOAD_DEBOUNCE_MS: u64 = 150;
    let mut reload_pending = false;
    let mut last_change: Option<std::time::Instant> = None;
    // Real frame interval. The loop sleeps a fixed 16ms, so the actual period is that plus
    // however long the frame's capture, DSP and render took - measured, not assumed.
    let mut last_frame = std::time::Instant::now();
    let mut time_s: f32 = 0.0;

    loop {
        while let Ok(f) = rx.try_recv() {
            latest = f;
        }

        // Reload themes when a file under the themes directory changes. Keeps
        // the current selection if it survives the reload, and never resets
        // the meter for this - only a deliberate theme switch (below) does
        // that. `set_themes` pushes the fresh list into the tray so both
        // right-click entry points see it immediately, without a restart.
        // Debounce. The atomic dirty flag already coalesces events within a single
        // tick, but editors commonly emit two or three filesystem events per save
        // (write-then-rename), and those can straddle ticks - which would reparse every
        // theme file two or three times for one Ctrl+S. Cheap to avoid.
        if watcher.changed() {
            reload_pending = true;
            last_change = Some(std::time::Instant::now());
        }
        let debounce_elapsed = last_change
            .map(|t| t.elapsed() >= std::time::Duration::from_millis(RELOAD_DEBOUNCE_MS))
            .unwrap_or(false);
        if reload_pending && debounce_elapsed {
            reload_pending = false;
            last_change = None;
            let (fresh, warnings) = themes::registry();
            for w in &warnings {
                eprintln!("themes: {w}");
            }
            all_themes = fresh;
            let resolved = themes::reconcile_reload(&all_themes, &cfg.theme);
            let selection_changed = resolved.id != cfg.theme;
            theme = resolved;
            family = render::family_for(&theme.family);
            smoother.set_ballistics(theme.ballistics);
            if selection_changed {
                // The previously-selected theme's file was deleted - fall back
                // rather than leaving the app pointing at nothing, and persist
                // the fallback so a restart does not reselect a ghost id.
                cfg.theme = theme.id.clone();
                if let Err(e) = cfg.save() {
                    eprintln!("config save failed: {e}");
                }
            }
            let live_menu: Vec<win::tray::MenuItem> = all_themes
                .iter()
                .map(|t| win::tray::MenuItem::new(&t.id, &t.name, &t.family))
                .collect();
            tray.set_themes(&live_menu);
            println!("themes: reloaded {} colourways", all_themes.len());
        }

        // poll() never synthesises Quit - it only records that a right-click
        // happened. The menu is caller-driven, and Quit comes only from the
        // menu returning ID_QUIT (see win::tray's module docs).
        tray.poll();

        // The overlay itself is clickable while it is up. Right-click opens the same
        // menu as the tray icon (one implementation, two entry points); left-click
        // sends Win+W, because the overlay is covering the Widgets button and without
        // it the weather would simply be unreachable while music plays.
        let overlay_click = overlay.take_event();
        if overlay_click == Some(win::overlay::OverlayEvent::LeftClick) {
            if let Err(e) = win::overlay::open_widgets_panel() {
                eprintln!("could not open the widgets panel: {e}");
            }
        }
        let want_menu =
            tray.take_right_click() || overlay_click == Some(win::overlay::OverlayEvent::RightClick);

        if want_menu {
            let chosen = tray.show_menu(win::autostart::is_enabled(), &theme.id);
            match chosen {
                Some(TrayEvent::Quit) => break,
                Some(TrayEvent::SelectTheme(id)) => {
                    if let Some(t) = all_themes.iter().find(|t| t.id == id) {
                        theme = t.clone();
                        family = render::family_for(&theme.family);
                        smoother = Smoother::new(theme.ballistics);
                        cfg.theme = theme.id.clone();
                        if let Err(e) = cfg.save() {
                            eprintln!("config save failed: {e}");
                        }
                    }
                }
                Some(TrayEvent::ToggleAutostart) => {
                    let want = !win::autostart::is_enabled();
                    match win::autostart::set(want) {
                        Ok(()) => {
                            cfg.autostart = want;
                            if let Err(e) = cfg.save() {
                                eprintln!("config save failed: {e}");
                            }
                        }
                        Err(e) => eprintln!("autostart toggle failed: {e}"),
                    }
                }
                None => {}
            }
        }

        // Re-discover the rect once a second - it moves with the weather text.
        if rect_tick == 0 {
            // ONE UIA snapshot per tick, shared by the widget lookup, the clearance
            // measurement and the chevron fallback. Reading the clearance from a different
            // enumeration than the widget rect would let the two disagree about where things
            // are while the taskbar is mid-animation, and the clearance is what stops the
            // overlay covering a pinned button.
            let elements = win::placement::taskbar_elements().unwrap_or_default();
            let bar = win::placement::taskbar_rect();

            match win::placement::widget_rect_in(&elements) {
                Some(widget) => {
                    let want = match bar {
                        // Widen leftward into the dead taskbar between the last pinned app and
                        // the widget. `cfg.width` is a request; this clamps it to the room that
                        // actually exists.
                        Some(bar) => win::placement::widened(
                            widget,
                            bar,
                            win::placement::left_limit(&elements, widget),
                            cfg.width,
                            WIDEN_MARGIN,
                        ),
                        None => widget,
                    };
                    // Hysteresis on the width only.
                    //
                    // The clearance moves every time a window opens, closes or is minimised, and
                    // the scope family reallocates - and therefore CLEARS - its persistence
                    // buffers on any canvas resize. Without this, a taskbar that is busy for
                    // unrelated reasons wipes the phosphor trail once a second. `x` is allowed to
                    // track freely because moving the window costs nothing.
                    rect = Some(match rect {
                        Some(prev) if (prev.w - want.w).abs() < WIDTH_HYSTERESIS => {
                            geom::Rect { x: want.x + (want.w - prev.w), ..prev }
                        }
                        _ => want,
                    });
                    rect_misses = 0;
                }
                // A transient failure must NOT throw away a known-good rect.
                //
                // UI Automation can fail for a tick - notably while a popup menu is
                // open, which is exactly when a theme is being chosen. Clearing `rect`
                // on the first miss hid the overlay for a second and the real weather
                // showed through underneath, which reads as the EQ "bleeding". Only
                // give up after several consecutive misses, which still handles the
                // genuine cases (the widget switched off, or a Windows version that
                // has no Widgets button at all - see the chevron fallback below).
                None => {
                    rect_misses += 1;
                    if rect_misses >= RECT_MISS_LIMIT {
                        rect = None;
                    }
                }
            }

            // No Widgets button at all? Anchor to the tray's overflow chevron instead.
            // That element exists on Windows 10 as well ("Show hidden icons"), which is
            // the only reason this app shows anything at all on that OS.
            if rect.is_none() {
                if let (Some(chev), Some(bar)) = (win::placement::chevron_rect_in(&elements), bar) {
                    // Widened the same way, so the Windows 10 path gets the wide display too
                    // rather than being stuck at the old fixed fallback width.
                    let anchored =
                        win::placement::rect_left_of(chev, bar, FALLBACK_GAP, FALLBACK_WIDTH);
                    rect = Some(win::placement::widened(
                        anchored,
                        bar,
                        win::placement::left_limit(&elements, anchored),
                        cfg.width,
                        WIDEN_MARGIN,
                    ));
                }
            }
        }
        rect_tick = (rect_tick + 1) % 60;

        let inputs = win::visibility::Inputs {
            widget: rect,
            notification_state: win::placement::notification_state(),
            taskbar_visible: win::placement::taskbar_visible(),
        };

        let now = std::time::Instant::now();
        // Clamped: a debugger pause or a suspend/resume can hand back an enormous interval,
        // which would otherwise jump the gate straight through its hide delay and snap the
        // scroll phase forward.
        let dt_ms = (now.duration_since(last_frame).as_secs_f32() * 1000.0).clamp(1.0, 100.0);
        last_frame = now;
        // Wrapped rather than unbounded - see FrameData::time_s.
        time_s = (time_s + dt_ms / 1000.0) % 3600.0;

        // Previously a hardcoded 16, which meant the gate's configured millisecond timings
        // were really being applied against an assumed frame rate the loop does not hit -
        // the 4500ms hide delay actually ran nearer 5.5s. Feeding the measured interval
        // makes the configured values mean what they say.
        let opacity = gate.update(latest.rms, dt_ms.round() as u32);
        smoother.update(&latest.bands);

        if win::visibility::should_show(&inputs) && gate.is_visible() {
            let r = rect.unwrap();
            let mut canvas = Canvas::new(r.w, r.h);
            let data = FrameData {
                levels: *smoother.levels(),
                peaks: *smoother.peaks(),
                waveform: latest.waveform,
                rms_l: latest.rms_l,
                rms_r: latest.rms_r,
                dt_ms,
                time_s,
            };
            family.draw(&mut canvas, &theme, &data);
            // Apply the reveal/hide fade. The gate has computed this opacity - with
            // tests - since it was written, but nothing ever consumed it, so the
            // overlay popped in and out instead of fading. That is the whole bug.
            canvas.scale_alpha(opacity);
            // NOT fatal, and this is the bug behind the Windows 10 report. A single failing
            // draw used to propagate out of `main` with `?` and end the process - so the tray icon
            // appeared, the first frame failed, and the app was simply gone, with its explanation
            // printed to a console that closed at the same moment. A draw that fails this frame may
            // well succeed the next, and even if it never does, a running app that logs why is
            // diagnosable where a vanished one is not.
            if let Err(e) = overlay.show(r, &canvas) {
                log::write(&format!(
                    "overlay draw failed at {}x{} ({},{}) - {e}",
                    r.w, r.h, r.x, r.y
                ));
            }
        } else {
            let _ = overlay.hide();
        }

        overlay.pump_messages();
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    Ok(())
}
