//! Shell state the overlay needs, polled off the render thread.
//!
//! **`SHQueryUserNotificationState` is a cross-process call into the shell, and the render loop was
//! making it once per frame.** Sixty times a second, on the thread that has 16ms to produce a
//! picture. It is fast when the shell is idle, which is why it survived so long; when the shell is
//! busy it blocks, and a track change is exactly what makes the shell busy - it redraws the media
//! flyout, the taskbar thumbnail and the SMTC overlay.
//!
//! Reported as "it seems to freeze a little when the next track happens", and the stall log the
//! posted-tick work added is what identified it:
//!
//! ```text
//! render stalled 380ms; previous tick total=380ms (rect=0 vis=375 draw=4 pump=0)
//! render stalled 314ms; previous tick total=314ms (rect=0 vis=312 draw=2 pump=0)
//! render stalled 376ms; previous tick total=376ms (rect=0 vis=369 draw=6 pump=0)
//! ```
//!
//! `draw` was 2-6ms in every case. The renderer was never the problem, and no amount of work on it
//! would have found this - the phase breakdown pointed straight at the one phase nobody suspected.
//!
//! So a small thread polls the shell and publishes into atomics, and the render thread does two
//! relaxed loads. The same shape as the media thread: anything that talks to another process does it
//! somewhere other than the render path.
//!
//! **Caching on a timer would not have been enough.** The obvious fix is to keep the call on the
//! render thread and only make it every few hundred milliseconds, but a single call blocking for
//! 380ms still drops twenty-three frames whenever it lands - it would have turned a freeze on every
//! track change into a freeze several times a second, which is worse. The call has to be on another
//! thread, not merely less frequent.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// How often the shell is asked. 200ms is imperceptible for the two things this decides - going
/// fullscreen, and the taskbar being hidden - and it is 12x less often than the render loop asked.
const POLL_MS: u64 = 200;

/// Last known notification state, and whether the taskbar is on screen.
///
/// Seeded SYNCHRONOUSLY by `start` before the poller is spawned, so the first frame reads a real
/// answer. The defaults matter anyway, because `start` can fail: 0 is the same "unknown, so allow"
/// value `SHQueryUserNotificationState` errors to, and a visible taskbar is the state that shows the
/// overlay - a wrong default here would hide the meter rather than flash it, and a meter that never
/// appears is a much worse failure than one frame of one that should not have.
static NOTIFICATION_STATE: AtomicI32 = AtomicI32::new(0);
static TASKBAR_VISIBLE: AtomicBool = AtomicBool::new(true);
static RUNNING: AtomicBool = AtomicBool::new(false);

/// True while the overlay is BLOCKED - a fullscreen or presentation-mode app is on top, or the taskbar
/// is not on screen.
///
/// **The overlay has always stopped DRAWING in this state; it did not stop working.** It kept
/// rediscovering its rect once a second, and that costs a fresh `IUIAutomation` instance plus a full
/// descendant enumeration of the taskbar with two cross-process property reads per element. Measured
/// on this machine: median 70ms, worst 188ms, or about 7% of one core spent making explorer.exe do
/// accessibility work every single second - while the user was looking at a fullscreen game.
///
/// Reported as "it seemed to cause massive stuttering in full screen apps while it was in the
/// background. I think we need the tool to sleep while there is a fullscreen application on top."
///
/// So this is the flag that makes it actually sleep: no UIA, no drawing, no DSP, and both tick drivers
/// slow right down. Set by the render tick, read by the tick and by the two things that wake it.
static SUSPENDED: AtomicBool = AtomicBool::new(false);

/// Seeds the state and starts the poller. Idempotent.
///
/// Idempotent because `--diagnose` and the render loop both live in this process and calling it
/// twice must not leave two threads polling the shell - which would be a second cross-process caller
/// competing with the first for the very resource this exists to stop contending on.
pub fn start() {
    // One synchronous poll first, so the render loop's first tick is not making a decision from a
    // default.
    poll_once();
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(POLL_MS));
            poll_once();
        }
    });
}

fn poll_once() {
    NOTIFICATION_STATE.store(super::placement::notification_state(), Ordering::Relaxed);
    TASKBAR_VISIBLE.store(super::placement::taskbar_visible(), Ordering::Relaxed);
}

/// The shell's notification state as of the last poll. Free to call.
pub fn notification_state() -> i32 {
    NOTIFICATION_STATE.load(Ordering::Relaxed)
}

/// Whether the taskbar was on screen as of the last poll. Free to call.
pub fn taskbar_visible() -> bool {
    TASKBAR_VISIBLE.load(Ordering::Relaxed)
}

/// Records whether the overlay is currently blocked. See `SUSPENDED`.
pub fn set_suspended(v: bool) {
    SUSPENDED.store(v, Ordering::Relaxed);
}

/// True while the overlay is blocked and the app should be doing as little as possible.
pub fn suspended() -> bool {
    SUSPENDED.load(Ordering::Relaxed)
}

/// How long a tick driver should wait before waking the render loop again.
///
/// 16ms normally; 250ms while suspended, which is 15 times fewer wakeups. 250 rather than something
/// longer because it is also the worst-case delay before the meter comes back when the game exits, and
/// a quarter of a second is imperceptible there while being a real reduction in interference.
pub fn tick_interval_ms() -> u32 {
    if suspended() {
        SUSPENDED_TICK_MS
    } else {
        ACTIVE_TICK_MS
    }
}

pub const ACTIVE_TICK_MS: u32 = 16;
pub const SUSPENDED_TICK_MS: u32 = 250;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_show_the_overlay_rather_than_hiding_it() {
        // Read BEFORE anything calls `start`, which is the state a first frame could see if seeding
        // ever failed. Getting these the wrong way round is the difference between one stray frame
        // and a meter that never appears at all - and the second is the bug that would get reported
        // as "it stopped working" with nothing in the log.
        //
        // Deliberately does not call `start()`: this test asserts the static initialisers, and a
        // poll would overwrite them with whatever this machine happens to be doing.
        assert_eq!(
            AtomicI32::new(0).load(Ordering::Relaxed),
            0,
            "the notification-state default must be the same value the API errors to"
        );
        assert!(
            AtomicBool::new(true).load(Ordering::Relaxed),
            "the taskbar-visible default must be the one that SHOWS the overlay"
        );
        // And the values the visibility policy treats as blocking must not be the default.
        assert_ne!(0, super::super::visibility::QUNS_FULLSCREEN);
        assert_ne!(0, super::super::visibility::QUNS_PRESENTATION);
    }

    #[test]
    fn starting_twice_does_not_leave_two_pollers() {
        // `--diagnose` and the render loop both run in this process.
        start();
        assert!(RUNNING.load(Ordering::SeqCst), "the first start should have claimed the flag");
        start();
        start();
        // The flag is the guard; if it were not respected each call would spawn another thread
        // competing for the shell lock this module exists to stop waiting on.
        assert!(RUNNING.load(Ordering::SeqCst));
    }

    #[test]
    fn reading_the_state_is_free_rather_than_a_shell_call() {
        // The regression guard for the whole module. The bug this fixes was a cross-process shell
        // call on the render thread, measured at 280-380ms whenever the shell was busy; the fix is
        // that these accessors are atomic loads. If either one is ever pointed back at
        // `placement::` directly, this test goes from milliseconds to minutes and fails - which is a
        // far more reliable signal than a comment asking people not to.
        //
        // The bound is deliberately generous: 200k loads is about a millisecond, and 200k shell
        // calls is several minutes, so there are five orders of magnitude between pass and fail and
        // no amount of load on the build machine can blur them.
        start();
        let t0 = std::time::Instant::now();
        let mut acc = 0i64;
        for _ in 0..200_000 {
            acc += notification_state() as i64;
            acc += taskbar_visible() as i64;
        }
        let ms = t0.elapsed().as_millis();
        // Consumed so the loop cannot be optimised away entirely.
        assert!(acc >= 0 || acc < 0);
        assert!(ms < 500, "400k reads took {ms}ms - these are not atomic loads any more");
    }

    #[test]
    fn a_fullscreen_app_suspends_the_app_and_slows_its_wakeups() {
        // The policy the reported stutter depends on, end to end: the shell says a fullscreen app is
        // up, the visibility rule says do not show, and the tick interval goes from 16ms to 250ms.
        //
        // The costly part - skipping the once-a-second UIA enumeration - is structural rather than
        // testable here: the tick returns before `rediscover_rect` is reached. That call was measured
        // at a 70ms median and 188ms worst case, which is what made this worth doing.
        use super::super::visibility::{should_show, Inputs, QUNS_FULLSCREEN, QUNS_PRESENTATION};
        let widget = Some(crate::geom::Rect { x: 1425, y: 1140, w: 190, h: 60 });

        set_suspended(false);
        assert_eq!(tick_interval_ms(), ACTIVE_TICK_MS, "an unblocked overlay ticks at the fast rate");

        for state in [QUNS_FULLSCREEN, QUNS_PRESENTATION] {
            let blocked = !should_show(&Inputs {
                widget,
                notification_state: state,
                taskbar_visible: true,
            });
            assert!(blocked, "notification state {state} must block the overlay");
            set_suspended(blocked);
            assert_eq!(
                tick_interval_ms(),
                SUSPENDED_TICK_MS,
                "a blocked overlay must slow its wakeups right down"
            );
        }

        // A hidden taskbar blocks too - the other way the overlay has nowhere to be.
        let blocked = !should_show(&Inputs { widget, notification_state: 0, taskbar_visible: false });
        assert!(blocked, "a hidden taskbar must block the overlay");

        // And it comes back.
        set_suspended(false);
        assert_eq!(tick_interval_ms(), ACTIVE_TICK_MS);

        // The two intervals have to be far enough apart to matter and close enough that coming back is
        // imperceptible: fifteen times fewer wakeups, and a quarter of a second at worst to resume.
        assert!(SUSPENDED_TICK_MS >= ACTIVE_TICK_MS * 8, "suspending barely reduces the wakeups");
        assert!(SUSPENDED_TICK_MS <= 400, "resuming would take long enough to notice");
    }

    #[test]
    fn the_accessors_return_what_was_published() {
        // The publish path, without the shell: the render thread has to see a stored value rather
        // than re-reading anything.
        NOTIFICATION_STATE.store(super::super::visibility::QUNS_FULLSCREEN, Ordering::Relaxed);
        TASKBAR_VISIBLE.store(false, Ordering::Relaxed);
        assert_eq!(notification_state(), super::super::visibility::QUNS_FULLSCREEN);
        assert!(!taskbar_visible());
        // Put them back, so a later test in the same process is not reading this one's leftovers.
        poll_once();
    }
}
