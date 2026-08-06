//! Scheduling policy for the render tick.
//!
//! The render tick has TWO drivers, and this module is the pure logic that lets them cooperate
//! without either one being able to double-render or starve.
//!
//! WHY TWO DRIVERS. The tick used to be the body of the main loop, which meant anything that
//! blocked the loop froze the visualiser. `TrackPopupMenu` does exactly that: it runs its own
//! modal message loop and does not return until the menu is dismissed, so right-clicking for the
//! theme menu stopped the meter dead - and stopped `rx.try_recv()` draining, so captured audio
//! frames were evicted while it was open. A settings dialog makes it worse, because a title-bar
//! drag is also a nested modal loop.
//!
//! The fix is to also drive the tick from a `WM_TIMER` on the tray window, because a modal loop
//! pumps messages even though it never returns. Measured, not assumed - see
//! `win::tray::tests::wm_timer_is_delivered_during_a_popup_menu_modal_loop`: across 1194ms of open
//! menu, `WM_TIMER` was dispatched 42 times.
//!
//! WHY THE TIMER CANNOT BE THE ONLY DRIVER. Those same numbers are 28.4ms apiece, not the 16ms
//! requested. `SetTimer` is quantised to the system timer granularity (~15.6ms), so a 16ms request
//! becomes ~31ms and the app would silently drop from ~62fps to ~32fps ALL THE TIME. Raising the
//! global timer resolution with `timeBeginPeriod` would fix the rate and charge the whole machine's
//! power budget for it, which is not a trade an ornament gets to make.
//!
//! So the main loop stays the primary driver at its 16ms cadence and the timer is a backstop that
//! only actually does work when the loop is blocked.
//!
//! WHAT THIS ACTUALLY BUYS, measured end to end by holding the real menu open for 4.0s and reading
//! the stall log, with the timer install commented out for the control:
//!
//! | | worst gap in the tick |
//! |---|---|
//! | no timer (the behaviour that was reported) | one 2512ms freeze spanning the whole menu |
//! | timer installed | three gaps of ~550ms, ticks running in between |
//!
//! The residual ~550ms gaps are NOT this app blocking. The stall report carries a phase breakdown
//! and every phase reads 0ms, including the whole-tick total - the tick is instant and simply is not
//! being CALLED, because `WM_TIMER` is a low-priority message that the system only synthesises when
//! the queue is otherwise empty, and a menu's modal loop does not leave it empty in a steady way.
//! Raising the timer frequency cannot fix that; only taking the render off the message loop
//! entirely can, which is a bigger change and is the right next step when the settings dialog
//! lands, since a dialog is open for far longer than a menu.

/// Smallest gap between two ticks that will be honoured, in milliseconds.
///
/// With both drivers live the main loop asks every ~16ms and the timer every ~31ms, so without a
/// floor the two would interleave into ~90fps of redundant compositing. 10ms caps the combined
/// rate at 100fps while staying below the 16ms the loop actually wants, so the loop is never the
/// one being skipped - the timer is.
pub const MIN_INTERVAL_MS: u32 = 10;

/// Gap above which a tick is reported to the log as a stall.
///
/// This exists because the freeze it guards was shipped and only found when a user reported it by
/// eye. A stall is invisible in every other diagnostic the app has: the process is alive, the
/// window is up, the last frame is still on screen, and nothing errors. 250ms is ~15 missed frames
/// - far longer than any scheduling jitter, short enough to catch a menu being opened.
pub const STALL_MS: u32 = 250;

/// What a driver should do with its tick request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Too soon since the last tick; the other driver already did the work.
    Skip,
    /// Render. `stalled_ms` is `Some` when the gap was long enough to be worth logging.
    Run { stalled_ms: Option<u32> },
}

/// Decides whether a tick requested `since_last_ms` after the previous one should run.
///
/// Pure and clock-free so it can be tested at its exact boundaries rather than by sleeping, which
/// would make the test both slow and flaky.
pub fn decide(since_last_ms: u32) -> Decision {
    if since_last_ms < MIN_INTERVAL_MS {
        return Decision::Skip;
    }
    Decision::Run {
        stalled_ms: if since_last_ms > STALL_MS { Some(since_last_ms) } else { None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_floor_is_pinned_at_its_exact_boundary() {
        // Both sides of the boundary, because a test that only checked `decide(2) == Skip` and
        // `decide(100) == Run` would pass with the floor moved anywhere in between - and the floor's
        // whole job is to sit BELOW the main loop's 16ms so the loop is never the driver that gets
        // skipped. Moving it to 20 would silently make the timer the primary driver and halve the
        // frame rate, which is the exact bug this module exists to avoid.
        assert_eq!(decide(MIN_INTERVAL_MS - 1), Decision::Skip);
        assert_eq!(decide(MIN_INTERVAL_MS), Decision::Run { stalled_ms: None });
        assert_eq!(decide(0), Decision::Skip);
        assert!(
            MIN_INTERVAL_MS < 16,
            "the floor must stay under the main loop's 16ms cadence, else the loop starts being \
             skipped in favour of the coarser timer"
        );
    }

    #[test]
    fn a_stall_is_reported_only_past_its_boundary() {
        assert_eq!(decide(STALL_MS), Decision::Run { stalled_ms: None });
        assert_eq!(decide(STALL_MS + 1), Decision::Run { stalled_ms: Some(STALL_MS + 1) });
        // The reported figure is the measured gap, not the threshold. Asserting only
        // `stalled_ms.is_some()` would pass against a version that logged a constant, which would
        // make every stall in the log look identical and useless for diagnosing one.
        assert_eq!(decide(4000), Decision::Run { stalled_ms: Some(4000) });
    }

    #[test]
    fn an_ordinary_frame_interval_runs_and_is_not_a_stall() {
        // The cadences both drivers actually produce, from the measurements in the module docs.
        for gap in [16, 17, 28, 31, 33, 50] {
            assert_eq!(
                decide(gap),
                Decision::Run { stalled_ms: None },
                "a normal {gap}ms interval must render without being flagged"
            );
        }
    }

    #[test]
    fn the_two_thresholds_cannot_cross() {
        // A guard on the constants themselves: if STALL_MS were ever set below MIN_INTERVAL_MS,
        // every admitted tick would report a stall and the log would drown in them.
        assert!(STALL_MS > MIN_INTERVAL_MS);
    }
}
