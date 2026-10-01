// No console window.
//
// Built as a CONSOLE subsystem app until now, so launching it from Explorer or from a
// Start-with-Windows entry popped a black terminal that then sat there for the life of the process.
// For a taskbar ornament that is simply wrong.
//
// The subsystem is fixed at LINK time - there is no way to turn a console on or off per run - so the
// honest options are a GUI binary that can attach or allocate a console when asked, or a console
// binary that hides its own window (which flickers visibly before it manages to). This takes the
// first: see `probes::attach_console_if_wanted`.
#![windows_subsystem = "windows"]
// Clippy allows that are wrong for THIS code, not blanket silencing:
// - needless_range_loop: the DSP and render loops index by position, usually across two or three
//   parallel arrays or with neighbour access (`x[i]`, `x[i+1]`), where the index form reads clearer
//   than a zipped enumerate.
// - manual_clamp: `x.max(a).min(b)` and `x.clamp(a, b)` differ on NaN inputs, and the
//   poisoned-spectrum guards deliberately push NaN through the render path; keep the saturating form.
// - field_reassign_with_default: test setup builds fixtures by assigning a field or two after
//   `default()`, which is clearer than a struct-update literal listing every default.
// - assertions_on_constants: several tests assert relationships between tuning constants on purpose,
//   so the build fails if the constants are ever retuned into overlap.
// - doc_lazy_continuation: the doc prose wraps clauses onto a line that starts "- ", used as a dash,
//   not as a markdown bullet.
#![allow(clippy::needless_range_loop)]
#![allow(clippy::manual_clamp)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::doc_lazy_continuation)]

mod config;
mod log;
mod dsp;
mod geom;
mod render;
mod themes;
mod tick;
mod win;
mod net;
mod songs;
mod history;
mod identify;
mod probes;

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


/// The one-click transport bindings offered by the tray menu.
///
/// `Win+Ctrl` rather than `Ctrl+Alt`: on a UK layout - which this is developed and used on - AltGr is
/// LeftCtrl+RightAlt in hardware, so a `Ctrl+Alt+<letter>` binding would stop the user typing whatever
/// AltGr makes of that letter. Comma and period because `VK_OEM_COMMA` and `VK_OEM_PERIOD` are
/// documented identical across every layout, unlike the `VK_OEM_1..8` range.
const SUGGESTED: [&str; 3] = ["Win+Ctrl+Space", "Win+Ctrl+Period", "Win+Ctrl+Comma"];

/// Re-applies every binding and logs the result. Used by the menu's bind and clear actions.
fn rebind(
    hotkeys: &mut win::hotkeys::Registry,
    cfg: &Config,
) -> [win::hotkeys::Outcome; win::hotkeys::SLOTS] {
    // Built from the indexed accessor rather than a hand-written list, so adding a slot cannot leave
    // one unbound - the array length is checked against SLOTS by the compiler either way, but a list
    // is easy to get in the wrong ORDER and that would bind the right key to the wrong action.
    let mut texts: [&str; win::hotkeys::SLOTS] = [""; win::hotkeys::SLOTS];
    for (i, t) in texts.iter_mut().enumerate() {
        *t = cfg.hotkeys.slot(i).unwrap_or("");
    }
    hotkeys.apply_all(texts)
}

/// What the tray menu should show, derived from the LIVE outcomes rather than from the config file.
///
/// The distinction matters: a chord that another program grabbed first is present in the config and
/// absent from the machine, so a menu built from the config would claim it is set up while pressing
/// it did nothing.
fn transport_state(
    outcomes: &[win::hotkeys::Outcome; win::hotkeys::SLOTS],
    cfg: &Config,
) -> win::tray::TransportState {
    let keys = std::array::from_fn(|i| match &outcomes[i] {
        // `label`, not `to_string`: the menu shows the key as this keyboard prints it, while the
        // config file keeps the canonical layout-independent spelling.
        win::hotkeys::Outcome::Registered(c, _) => c.label(),
        win::hotkeys::Outcome::Unbound => "not set".to_string(),
        win::hotkeys::Outcome::Taken(c) => format!("{}  (in use elsewhere)", c.label()),
        win::hotkeys::Outcome::Refused(c, _) => format!("{}  (not allowed)", c.label()),
        win::hotkeys::Outcome::Unreadable(_) => "unreadable".to_string(),
        win::hotkeys::Outcome::Failed(c, _) => format!("{}  (failed)", c.label()),
    });
    win::tray::TransportState {
        flourishes_on: cfg.flourishes,
        keys,
        broken: outcomes.iter().any(|o| o.is_broken()),
        media_keys_backend: cfg.media_backend == win::media::Backend::MediaKeys,
    }
}

/// Applies a shuffle, returning the chosen theme if anything changed.
///
/// Seeded from the clock, because the picker is deliberately pure - which is what makes its rules
/// ("never the one already showing", "stay in the family") testable exactly rather than by chance.
fn shuffle(all_themes: &[themes::Theme], current: &str, kind: themes::pick::RandomKind) -> Option<themes::Theme> {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x51ed_2701);
    themes::pick::pick(all_themes, current, kind, seed).map(|i| all_themes[i].clone())
}

/// How often the backstop timer asks for a tick, in milliseconds.
///
/// Requested 15 rather than 16 because `SetTimer` is quantised to the system timer granularity
/// (~15.6ms) and asking for 16 rounds UP to two quanta, ~31ms. Measured over an open menu, a 16ms
/// request delivered 42 ticks in 1194ms - 28.4ms apiece. Asking for 15 lands on one quantum.
const TICK_TIMER_MS: u32 = 15;

/// Everything one render tick needs, in one place, so the tick can be driven from two places.
///
/// It has to be reachable from the tray window's `WM_TIMER` handler, which is a bare
/// `extern "system"` function with nowhere to carry state - hence the thread-local below rather
/// than values threaded through the loop. Single-threaded by construction: every field is touched
/// only on the main thread, and `tick_now` refuses to nest.
struct Ticker {
    rx: win::capture::FrameReceiver,
    latest: Frame,
    overlay: win::overlay::Overlay,
    theme: themes::Theme,
    family: Box<dyn render::Family>,
    smoother: Smoother,
    gate: Gate,
    /// Requested display width. A REQUEST, not a guarantee - `placement::widened` clamps it to the
    /// clearance that actually exists.
    width_req: i32,
    rect: Option<geom::Rect>,
    rect_tick: u32,
    /// How many consecutive rediscoveries have found the rect unchanged. Drives the backoff.
    rect_stable: u32,
    rect_misses: u32,
    time_s: f32,
    /// The track-change banner, while one is on screen.
    banner: Option<render::banner::Banner>,
    /// The media thread's change counter as of the last tick, so a NEW track can be told from the
    /// same one still playing.
    ///
    /// Seeded from whatever the counter already is, deliberately: at startup there is usually a track
    /// loaded, and announcing it would mean a banner every time the app launches rather than when
    /// something actually changes.
    track_seq: u64,
    /// Change counter of `identify::banner()` last acted on.
    identify_seq: u64,
    /// Whether the banner is wanted at all.
    show_track_name: bool,
    /// How long the previous tick spent in each of its slow candidates, in milliseconds.
    ///
    /// A stall is reported by the tick that FOLLOWS the blockage, so what it needs to name is the
    /// previous tick's spending. Without this the log says only that something blocked, which is
    /// where the first measurement of this bug ran out of road: the menu fix left two ~545ms stalls
    /// per menu and the cause had to be inferred from their cadence rather than read off.
    phases: Phases,
    /// Recent UIA rediscovery timings in microseconds, summarised into the log once a minute.
    rect_us: Vec<u32>,
    /// When the current batch of those timings started, so the blocking share is measured against the
    /// real elapsed time rather than an assumed call rate.
    rect_window: std::time::Instant,
}

/// Per-tick timings for the calls that can realistically block, all in milliseconds.
#[derive(Debug, Clone, Copy, Default)]
struct Phases {
    /// The once-a-second UI Automation walk that finds the Widgets button.
    rect: u32,
    /// `notification_state` + `taskbar_visible`, read every tick.
    vis: u32,
    /// Compose plus `UpdateLayeredWindow`.
    draw: u32,
    /// Draining the overlay's message queue.
    pump: u32,
    /// The whole tick, so nothing can hide in an untimed gap between the phases above. The first
    /// pass at this measurement left the pump untimed and reported rect=0 vis=0 draw=0 for a 558ms
    /// stall, which said only that the blockage was somewhere else.
    total: u32,
}

thread_local! {
    static TICKER: std::cell::RefCell<Option<Ticker>> =
        const { std::cell::RefCell::new(None) };
    /// When the last tick actually ran, for the interval the gate and the gate's ballistics need.
    static LAST_TICK: std::cell::Cell<Option<std::time::Instant>> =
        const { std::cell::Cell::new(None) };
}

/// Runs the ticker's borrow for one operation, if it is installed and not already borrowed.
///
/// Returns `None` when the ticker is absent or busy, which the callers treat as "nothing to do".
/// Every caller in the loop deliberately releases this borrow before doing anything that can
/// block - `show_menu` above all - because that is precisely when `WM_TIMER` needs to get in.
fn with_ticker<R>(f: impl FnOnce(&mut Ticker) -> R) -> Option<R> {
    TICKER.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        slot.as_mut().map(f)
    })
}

/// One render tick, from either driver. Installed as the tray timer's callback.
///
/// This is the whole fix for "right-clicking for the menu freezes the visualiser". Rendering used
/// to be the tail of the main loop, so anything that blocked the loop stopped the meter and stopped
/// the capture channel draining. `TrackPopupMenu` blocks by design - measured at 1194ms for one
/// menu - and a modal loop pumps messages without ever returning, so a timer on the tray window is
/// the only way in. See `tick`'s module docs for why the loop still drives it too.
fn tick_now() {
    let now = std::time::Instant::now();
    let gap_ms = LAST_TICK
        .with(|c| c.get())
        .map(|t| now.duration_since(t).as_millis().min(u32::MAX as u128) as u32)
        // The first tick has no predecessor. Use the loop's nominal cadence so it is admitted and
        // is not reported as a stall.
        .unwrap_or(TICK_TIMER_MS + 1);
    let stalled = match tick::decide(gap_ms) {
        tick::Decision::Skip => return,
        tick::Decision::Run { stalled_ms } => stalled_ms,
    };
    TICKER.with(|cell| {
        // try_borrow_mut, NOT borrow_mut. `Ticker::tick` ends by pumping the overlay's messages,
        // which can dispatch a `WM_TIMER` straight back here; `borrow_mut` would panic and take the
        // app with it. Dropping the nested request is right - the outer tick is already doing it.
        let Ok(mut slot) = cell.try_borrow_mut() else { return };
        let Some(t) = slot.as_mut() else { return };
        LAST_TICK.with(|c| c.set(Some(now)));
        let p = t.phases;
        if let Some(ms) = stalled {
            // The freeze this whole change fixes was invisible to every other diagnostic - the
            // process alive, the window up, the last frame still on screen, nothing erroring - and
            // was found only because a user noticed it by eye. This is the instrument that would
            // have caught it. `log::write` collapses repeats, so a persistent stall cannot flood.
            log::write(&format!(
                "render stalled {ms}ms; previous tick total={}ms (rect={} vis={} draw={} pump={})",
                p.total, p.rect, p.vis, p.draw, p.pump
            ));
        }
        t.tick(gap_ms as f32);
    });
}

impl Ticker {
    /// Points the meter at a different colourway.
    ///
    /// `reset_meter` separates the two callers, a distinction the loop used to make inline: a hot
    /// reload must NOT wipe the meter, only a deliberate switch does.
    fn set_theme(&mut self, theme: themes::Theme, reset_meter: bool) {
        self.family = render::family_for(&theme.family);
        if reset_meter {
            self.smoother = Smoother::new(theme.ballistics);
        } else {
            self.smoother.set_ballistics(theme.ballistics);
        }
        self.theme = theme;
    }

    /// Composes and presents one frame. `gap_ms` is the measured interval since the last tick.
    fn tick(&mut self, gap_ms: f32) {
        let whole = std::time::Instant::now();
        while let Ok(f) = self.rx.try_recv() {
            self.latest = f;
        }

        self.phases = Phases::default();

        // BLOCKED? Then do as little as possible and go back to sleep.
        //
        // Checked BEFORE the rect rediscovery, which is the whole point: that is a UIA descendant
        // enumeration of the taskbar costing a measured 70ms median and 188ms worst case, and it was
        // running once a second even with a fullscreen game on top and nothing being drawn. See
        // `win::shell_state::SUSPENDED`.
        //
        // The gate is deliberately NOT part of this test. `gate.is_visible()` is false during ordinary
        // silence too, and suspending then would mean the meter took a quarter of a second to respond
        // to the first beat of a track. Blocked means something is covering the taskbar, not that the
        // music stopped.
        // Two relaxed atomic loads. These used to be two cross-process shell calls made on this
        // thread every frame - see `win::shell_state`.
        let t0 = std::time::Instant::now();
        let inputs = win::visibility::Inputs {
            widget: self.rect,
            notification_state: win::shell_state::notification_state(),
            taskbar_visible: win::shell_state::taskbar_visible(),
            fullscreen_foreground: win::shell_state::fullscreen_foreground(),
            display_off: win::shell_state::display_off(),
        };
        self.phases.vis = t0.elapsed().as_millis() as u32;

        // Only the SHELL can suspend us. NOT `should_show`, which is also false when the widget rect
        // is unknown - and since the suspend path skips the rect rediscovery, suspending on that would
        // deadlock: no rect, so blocked, so no rediscovery, so no rect, for ever. See
        // `visibility::shell_blocks`.
        let blocked = win::visibility::shell_blocks(
            inputs.notification_state,
            inputs.taskbar_visible,
            inputs.fullscreen_foreground,
            inputs.display_off,
        );
        if blocked != win::shell_state::suspended() {
            // Logged on the TRANSITION only, so the log stays useful rather than becoming a heartbeat.
            // This is the line that will say whether a report of interference on another machine is
            // this state or something else entirely.
            log::write(if blocked {
                "suspending: a fullscreen app or hidden taskbar is covering the overlay"
            } else {
                "resuming: the taskbar is visible again"
            });
            // Back to the fast rediscovery rate on either transition. Coming back from a game, the
            // taskbar may well have reflowed while the overlay was asleep - and the suspend path does no
            // rediscovery, so `rect_stable` is a count of how long ago it last looked, not of how long
            // the taskbar has actually been still. Trusting it across a suspend would leave the meter
            // misaligned for up to 8 seconds at exactly the moment the user is looking at it again.
            self.rect_stable = 0;
            self.rect_tick = 0;
        }
        win::shell_state::set_suspended(blocked);
        if blocked {
            // Taken off screen, not merely left undrawn: a topmost layered window can keep a game out
            // of exclusive fullscreen even with stale content on it.
            //
            // A failure here is not worth acting on: it means the window has already gone, which is
            // the same outcome, and the alternative - logging once per tick while suspended - would
            // fill the log with a heartbeat.
            let _ = self.overlay.hide();
            return;
        }

        // Re-discover the rect every two seconds - it moves with the weather text.
        //
        // TWO seconds, not one, and the reason is measured. This is a UIA descendant enumeration of the
        // taskbar and it blocks the render thread for a median of 52ms, so at once a second it costs
        // 5.2% of every second in blocking plus 86,400 COM client lifecycles a day. The weather text
        // changes width every few minutes at most, and the taskbar only reflows when something is
        // pinned or unpinned, so halving the rate costs nothing anyone can see and halves both numbers.
        //
        // Caching the `IUIAutomation` client instead was tried and REVERTED: measured cleanly on an
        // idle machine over three 60-call windows each, a cached client runs at a 68ms median against
        // 52ms for a fresh one per call. It is a pessimisation, not an optimisation - a long-lived UIA
        // client evidently does more work per enumeration than a throwaway one.
        if self.rect_tick == 0 {
            let t0 = std::time::Instant::now();
            let before = self.rect;
            self.rediscover_rect();
            // The backoff's input. Compared AFTER the call rather than inside it, so it sees the rect
            // the overlay will actually use - including the width hysteresis, which deliberately
            // absorbs small changes and is therefore part of what "unchanged" means here.
            if self.rect == before {
                self.rect_stable = self.rect_stable.saturating_add(1);
            } else {
                self.rect_stable = 0;
            }
            let us = t0.elapsed().as_micros() as u32;
            self.phases.rect = us / 1000;
            // Kept, and reported once a minute rather than per call.
            //
            // This is a UIA descendant enumeration of the taskbar - the single most expensive thing
            // the app does, and the reason the suspend path skips it. An out-of-process probe measured
            // it at a 70ms median, but the app's own figure is what matters: it runs in a long-lived
            // MTA with warm COM, and the whole-process CPU (4% of one core) says the real cost must be
            // far lower than the probe suggested. Guessing either way would be worse than logging it,
            // and on a machine that reports interference this line is the first thing to read.
            self.rect_us.push(us);
            if self.rect_us.len() >= 60 {
                let mut v = std::mem::take(&mut self.rect_us);
                v.sort_unstable();
                // Samples over two seconds are discarded as CLOCK ARTEFACTS, not slow calls.
                // `Instant::elapsed` keeps counting while the machine is asleep, so one call spanning a
                // suspend/resume is recorded as minutes: the first run of this logged a 196,763ms
                // maximum, which then dragged the mean to "333% of one core". A UIA call genuinely
                // taking two seconds would be a different and much louder problem.
                let real: Vec<u32> = v.iter().copied().filter(|us| *us < 2_000_000).collect();
                let dropped = v.len() - real.len();
                let blocked_us: u64 = real.iter().map(|x| *x as u64).sum();
                let window_s = self.rect_window.elapsed().as_secs_f32().max(0.001);
                self.rect_window = std::time::Instant::now();
                if !real.is_empty() {
                    let med = real[real.len() / 2];
                    log::write(&format!(
                        "placement (UIA) over {} calls in {:.0}s: min {:.1}ms median {:.1}ms max {:.1}ms; blocks the render thread {:.1}% of the time{}",
                        real.len(),
                        window_s,
                        real[0] as f32 / 1000.0,
                        med as f32 / 1000.0,
                        real[real.len() - 1] as f32 / 1000.0,
                        // Measured against the WALL CLOCK of the window, not against an assumed one
                        // call per second. The rediscovery interval has already changed once, and the
                        // first version of this line hardcoded the old rate - so after the change it
                        // reported 5.4% where the truth was 2.7%, and would have gone on being wrong
                        // silently.
                        blocked_us as f32 / (window_s * 1_000_000.0) * 100.0,
                        if dropped > 0 {
                            format!(" ({dropped} sample(s) discarded as clock artefacts)")
                        } else {
                            String::new()
                        }
                    ));
                }
            }
        }
        // Compared against the adaptive period rather than a fixed modulus: a modulus that grows can
        // step straight past its own zero and skip a whole cycle.
        self.rect_tick += 1;
        if self.rect_tick >= rediscover_period_ticks(self.rect_stable) {
            self.rect_tick = 0;
        }

        // Clamped: a debugger pause or a suspend/resume can hand back an enormous interval, which
        // would otherwise jump the gate straight through its hide delay and snap the scroll phase
        // forward.
        let dt_ms = gap_ms.clamp(1.0, 100.0);
        // Wrapped rather than unbounded - see FrameData::time_s.
        self.time_s = (self.time_s + dt_ms / 1000.0) % 3600.0;

        // Feeding the MEASURED interval is what makes the gate's configured millisecond timings
        // mean what they say; it was a hardcoded 16 once, which ran the 4500ms hide delay nearer
        // 5.5s. It matters more now than it did: with two drivers the real cadence varies, and
        // during an open menu it is nearer 31ms than 16ms.
        let opacity = self.gate.update(self.latest.rms, dt_ms.round() as u32);
        self.smoother.update(&self.latest.bands, dt_ms);

        // Drained HERE, every tick, before the visibility check below - not inside it. The hotkey,
        // the tray's `♪` line and the overlay's left click all set this flag asynchronously, so a
        // press while the meter happens to be hidden (no audio, or still inside the hide delay)
        // must not sit queued for however long it takes the gate to next open - it used to live
        // inside the `if` below, which left exactly that press to pop a stale banner minutes later
        // once something eventually played. Dropped instead: there is nowhere to show it right now,
        // and nothing playing is reported live if the user asks again once the meter is back.
        let now_playing_requested = win::hotkeys::take_now_playing_request();

        let t0 = std::time::Instant::now();
        // The shell part was decided above and returned early; what is left is whether the rect is
        // known and plausible, and whether the audio gate is open.
        if win::visibility::should_show(&inputs) && self.gate.is_visible() {
            let r = self.rect.unwrap();
            let mut canvas = Canvas::new(r.w, r.h);
            let data = FrameData {
                levels: *self.smoother.levels(),
                peaks: *self.smoother.peaks(),
                waveform: self.latest.waveform,
                rms_l: self.latest.rms_l,
                rms_r: self.latest.rms_r,
                dt_ms,
                time_s: self.time_s,
            };
            self.family.draw(&mut canvas, &self.theme, &data);

            // ---- track-change banner ----------------------------------------------------------
            // After the family, so it sits over whatever was drawn, and BEFORE `scale_alpha`, so the
            // reveal/hide fade applies to the banner too rather than leaving it at full strength
            // while the meter fades away underneath it.
            //
            // Three sources share the one banner: a track change (optional, `show_track_name`),
            // the song identifier (always - the user asked for it by pressing a key), and a direct
            // "show now playing" request (the tray's `♪` line, its hotkey, or a left click on the
            // meter - all three funnel through `hotkeys::request_now_playing`). The identifier's
            // text is sticky while listening and fades normally once it has an answer.
            //
            // `now_playing_requested` was already drained above, before this `if` - not here -
            // precisely so a press while the meter is hidden is dropped rather than left set and
            // sprung on some LATER, unrelated banner change once the gate reopens.
            let (id_text, id_seq, id_sticky) = identify::banner();
            if id_seq != self.identify_seq {
                self.identify_seq = id_seq;
                self.banner = if id_sticky {
                    render::banner::Banner::new_sticky(&id_text, r.h - 4)
                } else {
                    render::banner::Banner::new(&id_text, r.h - 4)
                };
            } else if now_playing_requested {
                // Nothing playing: do nothing, per spec - no empty banner. The request is still
                // consumed (above), so it cannot resurface once something starts playing later.
                let (title, _) = win::media::now_playing();
                if !title.trim().is_empty() {
                    self.banner = render::banner::Banner::new(&title, r.h - 4);
                }
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
            // Apply the reveal/hide fade. The gate has computed this opacity - with tests - since
            // it was written, but nothing ever consumed it, so the overlay popped in and out
            // instead of fading.
            canvas.scale_alpha(opacity);
            // NOT fatal, and this is the bug behind the Windows 10 report. A single failing draw
            // used to propagate out of `main` with `?` and end the process. A draw that fails this
            // frame may well succeed the next, and even if it never does, a running app that logs
            // why is diagnosable where a vanished one is not.
            if let Err(e) = self.overlay.show(r, &canvas) {
                log::write(&format!(
                    "overlay draw failed at {}x{} ({},{}) - {e}",
                    r.w, r.h, r.x, r.y
                ));
            }
        } else {
            let _ = self.overlay.hide();
        }
        self.phases.draw = t0.elapsed().as_millis() as u32;

        let t0 = std::time::Instant::now();
        self.overlay.pump_messages();
        self.phases.pump = t0.elapsed().as_millis() as u32;
        self.phases.total = whole.elapsed().as_millis() as u32;
    }

    /// The once-a-second UI Automation probe that decides where the overlay sits.
    fn rediscover_rect(&mut self) {
        // ONE UIA snapshot per tick, shared by the widget lookup, the clearance measurement and
        // the chevron fallback. Reading the clearance from a different enumeration than the widget
        // rect would let the two disagree about where things are while the taskbar is
        // mid-animation, and the clearance is what stops the overlay covering a pinned button.
        let elements = win::placement::taskbar_elements().unwrap_or_default();
        let bar = win::placement::taskbar_rect();

        match win::placement::widget_rect_in(&elements) {
            Some(widget) => {
                let want = match bar {
                    // Widen leftward into the dead taskbar between the last pinned app and the
                    // widget. `width_req` is a request; this clamps it to the room that exists.
                    Some(bar) => win::placement::widened(
                        widget,
                        bar,
                        win::placement::left_limit(&elements, widget),
                        self.width_req,
                        WIDEN_MARGIN,
                    ),
                    None => widget,
                };
                // Hysteresis on the width only.
                //
                // The clearance moves every time a window opens, closes or is minimised, and the
                // scope family reallocates - and therefore CLEARS - its persistence buffers on any
                // canvas resize. Without this, a taskbar that is busy for unrelated reasons wipes
                // the phosphor trail once a second. `x` is allowed to track freely because moving
                // the window costs nothing.
                self.rect = Some(match self.rect {
                    Some(prev) if (prev.w - want.w).abs() < WIDTH_HYSTERESIS => {
                        geom::Rect { x: want.x + (want.w - prev.w), ..prev }
                    }
                    _ => want,
                });
                self.rect_misses = 0;
            }
            // A transient failure must NOT throw away a known-good rect.
            //
            // UI Automation can fail for a tick - notably while a popup menu is open, which is
            // exactly when a theme is being chosen, and which now happens on far more ticks
            // because the timer keeps ticking THROUGH the menu. Clearing `rect` on the first miss
            // hid the overlay for a second and the real weather showed through underneath, which
            // reads as the EQ "bleeding". Only give up after several consecutive misses.
            None => {
                self.rect_misses += 1;
                if self.rect_misses >= RECT_MISS_LIMIT {
                    self.rect = None;
                }
            }
        }

        // No Widgets button at all? Anchor to the tray's overflow chevron instead. That element
        // exists on Windows 10 as well ("Show hidden icons"), which is the only reason this app
        // shows anything at all on that OS.
        if self.rect.is_none() {
            if let (Some(chev), Some(bar)) = (win::placement::chevron_rect_in(&elements), bar) {
                // Widened the same way, so the Windows 10 path gets the wide display too rather
                // than being stuck at the old fixed fallback width.
                let anchored =
                    win::placement::rect_left_of(chev, bar, FALLBACK_GAP, FALLBACK_WIDTH);
                self.rect = Some(win::placement::widened(
                    anchored,
                    bar,
                    win::placement::left_limit(&elements, anchored),
                    self.width_req,
                    WIDEN_MARGIN,
                ));
            }
        }
    }
}


/// Ticks to wait before walking the taskbar's accessibility tree again, given how long the rect has
/// been unchanged.
///
/// # Why this backs off at all
///
/// `win::placement::taskbar_elements` LEAKS, and this is the first measurement of it rather than a
/// suspicion. `--stress` on an idle machine: **+107 handles and +2 threads per 1,000 calls**, against a
/// flat zero for `CoCreateInstance(CUIAutomation)` on its own, for `taskbar_rect`, for
/// `notification_state` and for `foreground_window`. So it is not COM object creation and it is not
/// talking to the shell - it is walking the tree, and something inside UIA does not give everything back.
///
/// At a fixed two-second interval that is 193 handles an hour, which reaches the watchdog's warning
/// threshold in **15.6 hours** and its fatal threshold in **6.5 days**. On a machine left running for a
/// week, the app kills and restarts itself for no reason a user could see. Backing off to 8s while
/// nothing is moving takes that to 26 days.
///
/// # What this is NOT
///
/// It is not an explanation of the reported fault, and the arithmetic says so plainly. At this rate 45
/// minutes of running - when the stutter was reported - predicts 293 handles and 23 threads. What was
/// measured was 131,454 handles and 18,962 threads, which is roughly 450 and 800 times more. Even three
/// days only predicts 28,123 and 2,203. Whatever caused that is far faster than this and conditional on
/// something this machine does not do; the borderless-fullscreen suspend gap remains the leading
/// candidate. This is a real defect with a real consequence, and it is a different one.
///
/// # The trade
///
/// The rect moves when the weather text changes width, so a longer interval means the overlay can sit a
/// few pixels out of place for longer after that happens - up to 8 seconds, against 2 before, and only
/// after a full minute of no movement at all. Any change resets it to the fast rate immediately.
///
/// The better fix is a `SetWinEventHook` on `EVENT_OBJECT_LOCATIONCHANGE` for the taskbar's process,
/// which would walk the tree only when something actually moved and remove both the leak and the
/// latency. That is a larger change with its own risk of being chatty, and it is recorded in TODO.md
/// rather than attempted blind.
fn rediscover_period_ticks(stable: u32) -> u32 {
    // 120 ticks is ~2s at the 16ms active tick.
    const FAST: u32 = 120;
    match stable {
        // Still settling, or something just moved: stay responsive.
        0..=7 => FAST,
        // ~16s of stillness.
        8..=15 => FAST * 2,
        // Anything longer.
        _ => FAST * 4,
    }
}

/// What the command line asked for, parsed once and matched exhaustively so a new flag cannot be
/// half-wired: one branch reads it, `parse_args` decides it, `usage()` documents it.
///
/// `Unknown` exists so a typo or a stale flag from an old shortcut refuses to launch the overlay
/// silently - before this, any argument main.rs did not recognise fell straight through every `if`
/// below and started the app anyway, so `--dagnose` looked exactly like a clean run with nothing
/// wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cli {
    /// No flags, or only `--console`: run the overlay normally.
    Run { console: bool },
    Help,
    Version,
    Diagnose,
    Levels,
    Stress,
    /// An argument that matched none of the above, carried along so the error names it.
    Unknown(String),
}

/// Decides what the command line asked for. Pure and total - every input maps to exactly one
/// `Cli` value - so it is testable without a process, a console, or Windows at all.
///
/// Precedence mirrors what this file shipped with before `--help`/`--version`/`Unknown` existed:
/// `--levels` was checked before `--diagnose`, which was checked before `--stress`, regardless of
/// where each appeared among the arguments. `--help` and `--version` take precedence over all of
/// them - typing `--help` alongside other flags should always show help, not launch a probe.
fn parse_args(args: &[String]) -> Cli {
    if args.iter().any(|a| a == "--help") {
        return Cli::Help;
    }
    if args.iter().any(|a| a == "--version") {
        return Cli::Version;
    }
    if args.iter().any(|a| a == "--levels") {
        return Cli::Levels;
    }
    if args.iter().any(|a| a == "--diagnose") {
        return Cli::Diagnose;
    }
    if args.iter().any(|a| a == "--stress") {
        return Cli::Stress;
    }
    // Anything left that is not `--console` is unrecognised. `--console` is the only flag that
    // combines with a normal run rather than replacing it, so it is the only one skipped here.
    if let Some(unknown) = args.iter().find(|a| a.as_str() != "--console") {
        return Cli::Unknown(unknown.clone());
    }
    Cli::Run { console: args.iter().any(|a| a == "--console") }
}

/// One line per flag, so `--help` and a wrong flag's error both stay in sync with what `parse_args`
/// actually understands.
fn usage() -> String {
    format!(
        "taskbar-eq {}\n\
         Usage: taskbar-eq [FLAG]\n\
         \n\
         \x20 --console    allocate a console window and stay attached to it, for watching the app run\n\
         \x20 --diagnose   print the whole \"would the overlay draw?\" decision chain, then exit\n\
         \x20 --levels     capture 8 seconds of real audio and report what the DSP actually produces, then exit\n\
         \x20 --stress     hunt the process handle/thread leak by hammering each suspect path, then exit\n\
         \x20 --help       print this usage text and exit\n\
         \x20 --version    print the version and exit",
        env!("CARGO_PKG_VERSION")
    )
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        // `--help`, `--version` and an unrecognised flag all need to be READ, so the parent
        // console is attached first - see `probes::attach_console_if_wanted`. `force: true`: if
        // there is no parent console to inherit (launched from Explorer with a bad flag, say), a
        // fresh one is allocated rather than the message being silently lost.
        Cli::Help => {
            probes::attach_console_if_wanted(true);
            println!("{}", usage());
            return Ok(());
        }
        Cli::Version => {
            probes::attach_console_if_wanted(true);
            println!("taskbar-eq {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Cli::Unknown(flag) => {
            probes::attach_console_if_wanted(true);
            eprintln!("unknown flag: {flag}\n");
            eprintln!("{}", usage());
            std::process::exit(1);
        }
        Cli::Diagnose => {
            probes::attach_console_if_wanted(true);
            return probes::diagnose();
        }
        Cli::Levels => {
            probes::attach_console_if_wanted(true);
            return probes::measure_levels();
        }
        Cli::Stress => {
            probes::attach_console_if_wanted(true);
            return probes::stress();
        }
        Cli::Run { console } => probes::attach_console_if_wanted(console),
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
    // Seeds the shell state and starts its poller before the first frame. Two of the visibility
    // inputs are cross-process shell calls; keeping them on the render thread cost 280-380ms
    // whenever the shell was busy, which is what a track change makes it. See `win::shell_state`.
    win::shell_state::start();

    // Watches the app's own handle count. A long-lived instance was measured at 18,962 threads and
    // 131,454 handles, degrading a fullscreen app from 160fps to 30fps with input loss; this bounds
    // that and records the growth curve. See `win::health`.
    win::health::start();

    // The persisted flourish switch, pushed into the runtime flag the families read. Done once here so
    // the very first frame honours it rather than flourishing before the config is consulted.
    render::flourish_enabled(cfg.flourishes);

    let watcher = themes::watch::Watcher::new();

    let theme = all_themes
        .iter()
        .find(|t| t.id == cfg.theme)
        .cloned()
        .unwrap_or_else(themes::builtin::vfd_ice);
    let overlay = win::overlay::Overlay::new()?;
    // The tray icon is not decoration: when nothing is playing the overlay
    // does not exist, so this is the only way to quit the app.
    let mut tray = Tray::new(&theme_menu)?;
    let rx = win::capture::start();

    // The render tick lives in a thread-local from here on, so the tray window's WM_TIMER can
    // drive it while the main loop is blocked inside TrackPopupMenu. Installed before the timer,
    // so a timer that fires immediately finds it ready.
    TICKER.with(|cell| {
        *cell.borrow_mut() = Some(Ticker {
            rx,
            latest: Frame::default(),
            overlay,
            family: render::family_for(&theme.family),
            smoother: Smoother::new(theme.ballistics),
            gate: Gate::new(cfg.gate_config()),
            theme,
            width_req: cfg.width,
            rect: None,
            rect_tick: 0,
            rect_stable: 0,
            rect_misses: 0,
            time_s: 0.0,
            phases: Phases::default(),
            rect_us: Vec::new(),
            rect_window: std::time::Instant::now(),
            banner: None,
            track_seq: win::media::now_playing().1,
            identify_seq: identify::banner().1,
            show_track_name: cfg.show_track_name,
        });
    });
    win::tray::install_tick(tray.hwnd(), TICK_TIMER_MS, tick_now);

    // Ask for display power notifications on the tray window, and HOLD the handle: unregistering is what
    // stops them arriving, so letting this drop would register and immediately cancel. Bound with a
    // leading underscore rather than discarded with `_`, because `let _ = ...` drops immediately and that
    // is exactly the bug. See `win::power` for what this is worth - measured, this app was the second
    // largest energy consumer on the machine over eight days.
    let _display_watch = win::power::watch_display(tray.hwnd());
    if _display_watch.is_none() {
        log::write(
            "could not register for display power notifications - the overlay will keep drawing with              the screen off, which is the largest single battery cost it has",
        );
    }

    // Transport control. The media thread is started even with nothing bound, because it costs one
    // idle thread and it means the very first key press does not also pay for WinRT activation.
    win::hotkeys::install_media(win::media::start(), cfg.media_backend);
    let mut hotkeys = win::hotkeys::Registry::new(tray.hwnd());
    let mut outcomes = rebind(&mut hotkeys, &cfg);
    let working = outcomes.iter().filter(|o| o.is_working()).count();
    log::write(&format!(
        "hotkeys: {working} of {} bound and working",
        win::hotkeys::SLOTS
    ));
    if outcomes.iter().any(|o| o.is_broken()) {
        log::write("one or more transport keys are configured but NOT working - see the lines above");
    }
    // Consecutive rect-discovery misses tolerated before the overlay gives up and
    // hides. At one probe per second this is a few seconds of grace.

    // Fallback placement, used when there is no Widgets button: sit this far left of
    // the tray's overflow chevron, at this width.


    // Hot-reload debounce: wait this long after the last filesystem event before
    // reparsing, so one save produces one reload.
    const RELOAD_DEBOUNCE_MS: u64 = 150;
    let mut reload_pending = false;
    let mut last_change: Option<std::time::Instant> = None;

    loop {

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
            let resolved_id = resolved.id.clone();
            // A reload must not wipe the meter, so `reset_meter` is false here.
            with_ticker(|t| t.set_theme(resolved, false));
            if selection_changed {
                // The previously-selected theme's file was deleted - fall back
                // rather than leaving the app pointing at nothing, and persist
                // the fallback so a restart does not reselect a ghost id.
                cfg.note_theme(&resolved_id);
                if let Err(e) = cfg.save() {
                    log::write(&format!("config save failed: {e}"));
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
        // menu as the tray icon (one implementation, two entry points); left-click shows
        // the current track name, via the same request the hotkey and the tray's `♪`
        // line set - see `hotkeys::request_now_playing`.
        //
        // This used to send Win+W to open the Widgets panel the overlay sits on top of.
        // The user asked (30 Sep 2026) for that click-through to be removed in favour of
        // showing the song instead, so `open_widgets_panel` and the `SendInput` helper it
        // was the only caller of are gone - see git history if that behaviour is wanted
        // back.
        let overlay_click = with_ticker(|t| t.overlay.take_event()).flatten();
        if overlay_click == Some(win::overlay::OverlayEvent::LeftClick) {
            win::hotkeys::request_now_playing();
        }
        let want_menu =
            tray.take_right_click() || overlay_click == Some(win::overlay::OverlayEvent::RightClick);

        if want_menu {
            // The ticker's borrow is taken and RELEASED here, before `show_menu` blocks. That is
            // the whole point: TrackPopupMenu runs its own modal loop for as long as the menu is
            // open, and the WM_TIMER that keeps the meter alive through it has to be able to borrow
            // the ticker itself. Holding this borrow across the call would turn the fix into a
            // no-op - the timer would fire, fail to borrow, and drop every tick.
            let current = with_ticker(|t| t.theme.id.clone()).unwrap_or_default();
            // The song list is read from disk here, on a right-click, which is the only time
            // it is needed - so a find made a moment ago is already in the menu.
            let song_entries =
                win::tray::song_entries(&songs::recent_distinct(&songs::load(), 10));
            let chosen =
                tray.show_menu(
                    win::autostart::is_enabled(),
                    &current,
                    &cfg.recents,
                    &transport_state(&outcomes, &cfg),
                    &song_entries,
                );
            match chosen {
                Some(TrayEvent::Quit) => break,
                Some(TrayEvent::SelectTheme(id)) => {
                    if let Some(t) = all_themes.iter().find(|t| t.id == id) {
                        // A deliberate switch DOES reset the meter.
                        with_ticker(|k| k.set_theme(t.clone(), true));
                        cfg.note_theme(&t.id);
                        if let Err(e) = cfg.save() {
                            log::write(&format!("config save failed: {e}"));
                        }
                    }
                }
                Some(TrayEvent::IdentifyNow) => identify::request(),
                Some(TrayEvent::ShowNowPlaying) => {
                    // Routed through the SAME request the hotkey and the overlay's left click set,
                    // so all three show the banner through exactly one path - see
                    // `hotkeys::request_now_playing`.
                    win::hotkeys::request_now_playing();
                }
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
                Some(TrayEvent::FlourishNow) => {
                    // Straight through to the detector; the family consumes it on its next frame.
                    render::flourish_now();
                    log::write("flourish requested from the menu");
                }
                Some(TrayEvent::ToggleFlourishes) => {
                    // Routed through the SAME request the hotkey sets, so the flip and the config save
                    // happen in exactly one place - see `hotkeys::request_toggle`.
                    win::hotkeys::request_toggle();
                }
                Some(TrayEvent::RandomNow(kind)) => {
                    let current = with_ticker(|t| t.theme.id.clone()).unwrap_or_default();
                    match shuffle(&all_themes, &current, kind) {
                        Some(t) => {
                            log::write(&format!("{kind:?} from the menu: {current} -> {}", t.id));
                            cfg.note_theme(&t.id);
                            with_ticker(|k| k.set_theme(t, true));
                            if let Err(e) = cfg.save() {
                                log::write(&format!("config save failed: {e}"));
                            }
                        }
                        None => log::write(&format!("{kind:?}: nothing else to switch to")),
                    }
                }
                Some(TrayEvent::BindKey(i)) => {
                    // EVERY binding is released first. A registered hotkey consumes the keystroke,
                    // so the combinations most worth rebinding are exactly the ones that would fire
                    // play/pause instead of reaching the capture window.
                    hotkeys.release_all();
                    let label = win::hotkeys::Slot::ALL[i.min(win::hotkeys::SLOTS - 1)].label();
                    let dark = win::darkmode::windows_prefers_dark();
                    // The chords bound to the OTHER two actions, so the window can refuse a
                    // duplicate on the spot instead of storing one that quietly never fires.
                    let others: Vec<win::hotkey::Chord> = (0..win::hotkeys::SLOTS)
                        .filter(|j| *j != i)
                        .filter_map(|j| cfg.hotkeys.slot(j))
                        .filter_map(|t| win::hotkey::Chord::parse(t).ok())
                        .collect();
                    match win::capture_key::capture(tray.hwnd(), label, dark, &others) {
                        Some(win::capture_key::Captured::Chord(c)) => {
                            let text = c.to_string();
                            log::write(&format!("captured {text} for {label}"));
                            match cfg.hotkeys.slot_mut(i) {
                                Some(field) => *field = text,
                                // Unreachable while the menu only offers SLOTS entries, and a no-op
                                // rather than an alias if that ever stops being true. The `_ =>` this
                                // replaces would have written a sixth slot's chord into the fifth.
                                None => log::write(&format!("no hotkey slot {i} to bind")),
                            }
                            if let Err(e) = cfg.save() {
                                log::write(&format!("config save failed: {e}"));
                            }
                        }
                        Some(win::capture_key::Captured::Clear) => {
                            log::write(&format!("cleared the key for {label}"));
                            match cfg.hotkeys.slot_mut(i) {
                                Some(field) => field.clear(),
                                None => log::write(&format!("no hotkey slot {i} to clear")),
                            }
                            if let Err(e) = cfg.save() {
                                log::write(&format!("config save failed: {e}"));
                            }
                        }
                        None => log::write(&format!("key capture for {label} was cancelled")),
                    }
                    // Re-applied whatever happened, including on a cancel - otherwise a cancelled
                    // capture would leave the machine with no transport keys at all.
                    outcomes = rebind(&mut hotkeys, &cfg);
                }
                Some(TrayEvent::SuggestKeys) => {
                    // Registered IMMEDIATELY, so a combination another program already owns is
                    // reported now rather than silently failing at the next logon.
                    cfg.hotkeys = config::Hotkeys {
                        play_pause: SUGGESTED[0].into(),
                        next_track: SUGGESTED[1].into(),
                        prev_track: SUGGESTED[2].into(),
                        // The shuffles are left alone: "use suggested keys" is offered from the
                        // Spotify submenu and should not silently claim two more keys the user did
                        // not ask about.
                        ..cfg.hotkeys.clone()
                    };
                    outcomes = rebind(&mut hotkeys, &cfg);
                    if let Err(e) = cfg.save() {
                        log::write(&format!("config save failed: {e}"));
                    }
                }
                Some(TrayEvent::ClearKeys) => {
                    cfg.hotkeys = config::Hotkeys::default();
                    outcomes = rebind(&mut hotkeys, &cfg);
                    if let Err(e) = cfg.save() {
                        log::write(&format!("config save failed: {e}"));
                    }
                }
                Some(TrayEvent::SetBackend(b)) => {
                    cfg.media_backend = b;
                    win::hotkeys::set_backend(b);
                    log::write(&format!("transport backend set to {b:?}"));
                    if let Err(e) = cfg.save() {
                        log::write(&format!("config save failed: {e}"));
                    }
                }
                Some(TrayEvent::EditConfig) => {
                    // Written first. Until something is changed there may be no file on disk at all,
                    // and opening a path that does not exist gets the user an empty buffer with none
                    // of the keys they were sent there to edit.
                    if let Err(e) = cfg.save() {
                        log::write(&format!("config save failed before opening it: {e}"));
                    }
                    if let Err(e) = win::overlay::open_path(&Config::path()) {
                        log::write(&format!("could not open the config file: {e}"));
                    }
                }
                Some(TrayEvent::ToggleAutostart) => {
                    let want = !win::autostart::is_enabled();
                    match win::autostart::set(want) {
                        Ok(()) => {
                            cfg.autostart = want;
                            if let Err(e) = cfg.save() {
                                log::write(&format!("config save failed: {e}"));
                            }
                        }
                        Err(e) => log::write(&format!("autostart toggle failed: {e}")),
                    }
                }
                None => {}
            }
        }

        // A flourish on/off toggle asked for by a hotkey or the menu. Here rather than in the wndproc
        // because it persists to the config, and file I/O has no business in a message handler.
        if win::hotkeys::take_toggle_request() {
            cfg.flourishes = !cfg.flourishes;
            render::flourish_enabled(cfg.flourishes);
            log::write(if cfg.flourishes { "flourishes: on" } else { "flourishes: off" });
            if let Err(e) = cfg.save() {
                log::write(&format!("config save failed: {e}"));
            }
        }

        // A shuffle asked for by a hotkey. Done here rather than in the wndproc because the theme
        // list lives in this loop, and because switching theme resets the meter - work that has no
        // business happening inside a message handler.
        if let Some(kind) = win::hotkeys::take_random_request() {
            let current = with_ticker(|t| t.theme.id.clone()).unwrap_or_default();
            match shuffle(&all_themes, &current, kind) {
                Some(t) => {
                    log::write(&format!("{kind:?}: {} -> {}", current, t.id));
                    cfg.note_theme(&t.id);
                    with_ticker(|k| k.set_theme(t, true));
                    if let Err(e) = cfg.save() {
                        log::write(&format!("config save failed: {e}"));
                    }
                }
                None => log::write(&format!("{kind:?}: nothing else to switch to")),
            }
        }

        // The tick itself. Also driven by the tray window's WM_TIMER, which is what keeps the
        // meter alive while the menu above is blocking this loop.
        tick_now();
        // 16ms normally, 250ms while a fullscreen app is on top - see `win::shell_state::SUSPENDED`.
        std::thread::sleep(std::time::Duration::from_millis(
            win::shell_state::tick_interval_ms() as u64,
        ));
    }

    // Nothing is left holding a machine-wide key after the app closes.
    hotkeys.release_all();
    Ok(())
}


#[cfg(test)]
mod cli_tests {
    use super::{parse_args, Cli};

    #[test]
    fn help_and_version_and_unknown_flags_never_launch_the_overlay() {
        assert_eq!(parse_args(&["--help".into()]), Cli::Help);
        assert_eq!(parse_args(&["--version".into()]), Cli::Version);
        assert_eq!(parse_args(&["--diagnose".into()]), Cli::Diagnose);
        assert_eq!(parse_args(&["--bogus".into()]), Cli::Unknown("--bogus".into()));
        assert_eq!(parse_args(&[]), Cli::Run { console: false });
        assert_eq!(parse_args(&["--console".into()]), Cli::Run { console: true });
    }
}

#[cfg(test)]
mod backoff_tests {
    use super::rediscover_period_ticks;

    #[test]
    fn the_rediscovery_backs_off_only_after_real_stillness_and_never_below_the_fast_rate() {
        // The fast rate is what the app shipped with, and it must still be what a MOVING taskbar gets.
        // The point of the backoff is to spend fewer UIA walks while nothing is happening, not to make
        // the overlay lazier about following the widget.
        assert_eq!(rediscover_period_ticks(0), 120, "a rect that just moved must stay at ~2s");
        for stable in 0..8 {
            assert_eq!(rediscover_period_ticks(stable), 120, "stable={stable} is still settling");
        }
        // Monotonic, or the interval would oscillate.
        let mut prev = 0;
        for stable in 0..200 {
            let p = rediscover_period_ticks(stable);
            assert!(p >= prev, "the period went DOWN at stable={stable}: {prev} -> {p}");
            assert!(p >= 120, "never faster than the shipped rate, got {p} at stable={stable}");
            assert!(p <= 480, "never slower than ~8s, got {p} at stable={stable}");
            prev = p;
        }
        // And it must reach the cap, or the leak reduction this exists for does not happen.
        assert_eq!(rediscover_period_ticks(1_000), 480, "the backoff must reach ~8s");
    }

    #[test]
    fn the_backoff_buys_back_most_of_the_measured_leak() {
        // Arithmetic, because what is being protected IS arithmetic: 0.107 handles per UIA walk, as
        // measured by `--stress`, against the watchdog's 30,000 fatal threshold. At the fixed 2s rate
        // the app kills and restarts itself after about 6.5 days of uptime for no reason a user could
        // see, which is the defect this fixes.
        const LEAK_PER_CALL: f64 = 0.107;
        const FATAL: f64 = 30_000.0;
        const TICK_MS: f64 = 16.0;
        let days_to_fatal = |period_ticks: u32| -> f64 {
            let calls_per_hour = 3_600_000.0 / (period_ticks as f64 * TICK_MS);
            FATAL / (calls_per_hour * LEAK_PER_CALL) / 24.0
        };
        let before = days_to_fatal(120);
        let after = days_to_fatal(rediscover_period_ticks(1_000));
        assert!(
            before < 10.0,
            "sanity check on the model: the old rate really did self-destruct in days, got {before:.1}"
        );
        assert!(
            after > before * 3.0,
            "the backoff must buy back most of the leak: {before:.1} days -> {after:.1} days"
        );
    }
}
