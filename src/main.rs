mod config;
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

fn main() -> Result<()> {
    win::dpi::set_per_monitor_v2()?;
    // S_OK and S_FALSE both map to Ok; a real Err (e.g. RPC_E_CHANGED_MODE)
    // matters because UI Automation and WASAPI both require COM initialised.
    // CoInitializeEx returns HRESULT, not Result - `.ok()` is required and maps
    // S_OK/S_FALSE to Ok(()), leaving only genuine failures as Err.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        eprintln!("CoInitializeEx failed: {e}");
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
    const RECT_MISS_LIMIT: u32 = 4;
    // Fallback placement, used when there is no Widgets button: sit this far left of
    // the tray's overflow chevron, at this width.
    const FALLBACK_GAP: i32 = 4;
    const FALLBACK_WIDTH: i32 = 190;
    let mut rect_misses: u32 = 0;
    // Hot-reload debounce: wait this long after the last filesystem event before
    // reparsing, so one save produces one reload.
    const RELOAD_DEBOUNCE_MS: u64 = 150;

/// Pixels of clearance kept between the display's left edge and whatever is next to it.
///
/// Non-zero because the overlay receives its own clicks (it does not set WS_EX_TRANSPARENT -
/// see win::overlay), so a pixel it covers is a pixel of taskbar that can no longer be
/// clicked. Butting right up against a pinned app button would make that button feel dead
/// along its edge.
const WIDEN_MARGIN: i32 = 8;

/// Width change, in pixels, below which the display keeps its current width.
///
/// The available clearance shifts whenever any window opens, closes or is minimised, and the
/// scope family clears its persistence buffers on a canvas resize - so without hysteresis an
/// unrelated bit of taskbar activity wipes the phosphor trail once a second.
const WIDTH_HYSTERESIS: i32 = 12;
    let mut reload_pending = false;
    let mut last_change: Option<std::time::Instant> = None;
    // Real frame interval. The loop sleeps a fixed 16ms, so the actual period is that plus
    // however long the frame's capture, DSP and render took - measured, not assumed.
    let mut last_frame = std::time::Instant::now();

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
            };
            family.draw(&mut canvas, &theme, &data);
            // Apply the reveal/hide fade. The gate has computed this opacity - with
            // tests - since it was written, but nothing ever consumed it, so the
            // overlay popped in and out instead of fading. That is the whole bug.
            canvas.scale_alpha(opacity);
            overlay.show(r, &canvas)?;
        } else {
            overlay.hide()?;
        }

        overlay.pump_messages();
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    Ok(())
}
