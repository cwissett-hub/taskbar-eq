// Task 2 introduces this logic standalone; Task 3 wires should_show into
// main's polling loop via win::placement. Until then, rustc's binary-crate
// dead-code check flags these items as unused even though the tests below
// exercise them.
#![allow(dead_code)]

use crate::geom::Rect;

/// QUNS_RUNNING_D3D_FULL_SCREEN
pub const QUNS_FULLSCREEN: i32 = 3;
/// QUNS_PRESENTATION_MODE
pub const QUNS_PRESENTATION: i32 = 4;

pub struct Inputs {
    pub widget: Option<Rect>,
    pub notification_state: i32,
    pub taskbar_visible: bool,
    /// A borderless-fullscreen app is covering the screen. See `covers_monitor`.
    pub fullscreen_foreground: bool,
}

/// What the foreground window looks like, for `covers_monitor`.
#[derive(Clone, Debug)]
pub struct Foreground {
    /// The window's own rect, in physical pixels.
    pub window: Rect,
    /// The full bounds - NOT the work area - of the monitor it is on.
    pub monitor: Rect,
    /// True for the shell's own windows and the desktop, which are never "a fullscreen app".
    pub is_shell: bool,
    /// The window class, carried only so `--diagnose` can say which window it decided about. On a
    /// machine where the overlay is drawing over a game, the game's own class is the most useful single
    /// fact in the report - it is what says whether the exclusion list wrongly caught it.
    pub class: String,
}

/// How many pixels a window may fall short of the monitor on any edge and still count as fullscreen.
///
/// Small on purpose. The distinction this has to preserve is fullscreen versus MAXIMISED, and a
/// maximised window is sized to the WORK AREA - the monitor minus the taskbar - so it misses by the
/// taskbar's height, tens of pixels. 2px only absorbs games that round their own dimensions.
const COVER_SLOP: i32 = 2;

/// Whether the foreground window is covering its whole monitor.
///
/// THIS IS THE CHECK `SHQueryUserNotificationState` DOES NOT DO, and its absence was a real bug:
/// reported as "the equaliser sometimes renders on top of fullscreen programs", and on the same machine
/// "the lockup and stuttering after the machine had only been running for 30-45 minutes".
///
/// Windows sets `QUNS_RUNNING_D3D_FULL_SCREEN` only for **exclusive** fullscreen Direct3D. Borderless
/// windowed fullscreen - which is the default for most modern games, and what you get from "Fullscreen"
/// in a lot of launchers - leaves the notification state at "accepts notifications" and leaves the
/// taskbar nominally visible. So every signal the suspend logic had said "nothing is covering you", and
/// the overlay carried on drawing a topmost layered window over the game and carried on making UI
/// Automation calls into the shell.
///
/// Both of those are expensive in exactly the way that was reported. A topmost layered window over a
/// fullscreen app denies it independent flip, so it composites through the desktop compositor instead of
/// presenting directly - which costs far more than the overlay's own frame does. And the UIA calls keep
/// blocking inside an `explorer.exe` that a busy game has starved.
///
/// Geometry rather than a window style, because styles vary: some games are `WS_POPUP`, some keep a
/// caption they have moved off screen, some use a borderless window at exactly the monitor size. What
/// they all have in common is covering the monitor.
pub fn covers_monitor(fg: Option<&Foreground>) -> bool {
    let Some(fg) = fg else { return false };
    // The shell owning the foreground is the normal desktop case, and treating it as fullscreen would
    // suspend the overlay permanently on an idle machine - a deadlock in all but name, since the
    // suspend path does no rediscovery.
    if fg.is_shell {
        return false;
    }
    let (w, m) = (fg.window, fg.monitor);
    // Degenerate rects mean "we could not read it", which is not evidence of a game.
    if m.w <= 0 || m.h <= 0 || w.w <= 0 || w.h <= 0 {
        return false;
    }
    w.x <= m.x + COVER_SLOP
        && w.y <= m.y + COVER_SLOP
        && w.x + w.w >= m.x + m.w - COVER_SLOP
        && w.y + w.h >= m.y + m.h - COVER_SLOP
}

/// Whether the SHELL is covering the overlay: a fullscreen or presentation-mode app, or no taskbar.
///
/// Separate from `should_show` deliberately, and the separation is load-bearing. `should_show` is also
/// false when the widget rect is unknown, and the suspend path skips the rect rediscovery - so
/// suspending on the whole of `should_show` deadlocks: rect is None, therefore blocked, therefore no
/// rediscovery, therefore rect stays None, for ever. The meter would simply never appear, and the
/// symptom would be indistinguishable from the app not starting.
///
/// So this is the question "is something covering the taskbar", which is the only reason to go to
/// sleep. "Do I know where to draw" is a different question with a different answer.
pub fn shell_blocks(notification_state: i32, taskbar_visible: bool, fullscreen_foreground: bool) -> bool {
    !taskbar_visible
        || notification_state == QUNS_FULLSCREEN
        || notification_state == QUNS_PRESENTATION
        // The geometric check, which is the only one of the four that sees a borderless-fullscreen
        // game. See `covers_monitor`.
        || fullscreen_foreground
}

pub fn should_show(i: &Inputs) -> bool {
    if shell_blocks(i.notification_state, i.taskbar_visible, i.fullscreen_foreground) {
        return false;
    }
    match i.widget {
        Some(r) => r.is_plausible_widget(),
        None => false,
    }
}

#[cfg(test)]
mod fullscreen_tests {
    use super::*;

    const MON: Rect = Rect { x: 0, y: 0, w: 2560, h: 1440 };
    /// A maximised window is sized to the WORK AREA, so it stops short of the taskbar. 48px is a
    /// typical Windows 11 taskbar at 100% scaling.
    const TASKBAR_H: i32 = 48;

    fn fg(window: Rect, is_shell: bool) -> Foreground {
        Foreground { window, monitor: MON, is_shell, class: "TestClass".into() }
    }


    #[test]
    fn a_borderless_fullscreen_game_covers_the_monitor() {
        // The case the shell's notification state cannot see, and the reason this exists: a window at
        // exactly the monitor's bounds with no border. Reported as "the equaliser sometimes renders on
        // top of fullscreen programs".
        assert!(covers_monitor(Some(&fg(MON, false))));
    }

    #[test]
    fn a_maximised_window_is_not_fullscreen() {
        // THE DISTINCTION THE WHOLE CHECK TURNS ON. A maximised window is the common case - most people
        // have one most of the time - and calling it fullscreen would suspend the overlay almost
        // permanently. It is separable only because a maximised window is sized to the work area, so it
        // misses the monitor by the taskbar's height, which is tens of pixels rather than the 2px of
        // slop allowed here.
        let maximised = Rect { x: 0, y: 0, w: MON.w, h: MON.h - TASKBAR_H };
        assert!(!covers_monitor(Some(&fg(maximised, false))));
        // And with the taskbar at the top, which is where the miss is on the other edge.
        let top_taskbar = Rect { x: 0, y: TASKBAR_H, w: MON.w, h: MON.h - TASKBAR_H };
        assert!(!covers_monitor(Some(&fg(top_taskbar, false))));
    }

    #[test]
    fn a_game_that_rounds_its_own_size_still_counts() {
        // Some titles report a window an odd pixel short. 2px of slop, which is far below a taskbar.
        let nearly = Rect { x: 1, y: 1, w: MON.w - 2, h: MON.h - 2 };
        assert!(covers_monitor(Some(&fg(nearly, false))));
    }

    #[test]
    fn the_shell_owning_the_foreground_is_never_fullscreen() {
        // The desktop, the taskbar and the Start menu all legitimately cover the screen. Treating them
        // as a game would suspend the overlay the moment the user clicked the desktop - and because the
        // suspend path skips rect rediscovery, that is a deadlock in all but name.
        assert!(!covers_monitor(Some(&fg(MON, true))));
    }

    #[test]
    fn no_readable_foreground_is_not_evidence_of_a_game() {
        // None has to mean "carry on", not "suspend". The failure modes are not symmetric: a false
        // positive hides the meter for as long as the condition lasts, and this path returns None
        // whenever GetWindowRect or GetMonitorInfoW fails.
        assert!(!covers_monitor(None));
        // Degenerate rects are the same class of non-answer.
        assert!(!covers_monitor(Some(&fg(Rect { x: 0, y: 0, w: 0, h: 0 }, false))));
        assert!(!covers_monitor(Some(&Foreground {
            window: MON,
            monitor: Rect { x: 0, y: 0, w: 0, h: 0 },
            is_shell: false,
            class: "TestClass".into(),
        })));
    }

    #[test]
    fn a_window_on_a_second_monitor_is_judged_against_that_monitor() {
        // The rects are in virtual-desktop coordinates, so a fullscreen window on a monitor to the
        // right has a large positive x. Comparing against the monitor's own origin rather than against
        // zero is what makes this work, and getting it wrong would mean the check never fired on any
        // multi-monitor setup - which is most gaming machines.
        let mon2 = Rect { x: 2560, y: 0, w: 1920, h: 1080 };
        let full = Foreground {
            window: mon2,
            monitor: mon2,
            is_shell: false,
            class: "TestClass".into(),
        };
        assert!(covers_monitor(Some(&full)));
        let maximised = Foreground {
            window: Rect { x: 2560, y: 0, w: 1920, h: 1080 - TASKBAR_H },
            monitor: mon2,
            is_shell: false,
            class: "TestClass".into(),
        };
        assert!(!covers_monitor(Some(&maximised)));
    }

    #[test]
    fn a_borderless_game_suspends_the_overlay() {
        // The end-to-end claim, through the predicate the render loop actually calls. The other three
        // signals are all set to "nothing is wrong", which is exactly what they report during a
        // borderless-fullscreen game: notification state 5 is QUNS_ACCEPTS_NOTIFICATIONS.
        assert!(
            shell_blocks(5, true, true),
            "a borderless game must suspend the overlay even when the shell says all is well"
        );
        assert!(
            !shell_blocks(5, true, false),
            "and an ordinary desktop must not"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good_rect() -> Rect {
        Rect { x: 1416, y: 1140, w: 190, h: 60 }
    }

    fn base() -> Inputs {
        Inputs { widget: Some(good_rect()), notification_state: 5, taskbar_visible: true, fullscreen_foreground: false }
    }

    #[test]
    fn shows_when_everything_is_normal() {
        assert!(should_show(&base()));
    }

    #[test]
    fn hides_when_widget_not_found() {
        let i = Inputs { widget: None, ..base() };
        assert!(!should_show(&i), "no widget means no anchor - must not guess a position");
    }

    #[test]
    fn hides_over_fullscreen_app() {
        let i = Inputs { notification_state: QUNS_FULLSCREEN, ..base() };
        assert!(!should_show(&i));
    }

    #[test]
    fn hides_in_presentation_mode() {
        let i = Inputs { notification_state: QUNS_PRESENTATION, ..base() };
        assert!(!should_show(&i));
    }

    // These two pin the literal values from the real Win32
    // QUERY_USER_NOTIFICATION_STATE enum (shellapi.h), independent of the
    // QUNS_FULLSCREEN / QUNS_PRESENTATION symbols above. A future edit that
    // breaks the symbol-to-real-value mapping (e.g. re-transposing them)
    // must fail here even though the symbol-based tests above would still
    // pass, because those only assert internal self-consistency.
    #[test]
    fn hides_on_real_win32_fullscreen_value() {
        // QUNS_RUNNING_D3D_FULL_SCREEN = 3
        let i = Inputs { notification_state: 3, ..base() };
        assert!(!should_show(&i));
    }

    #[test]
    fn hides_on_real_win32_presentation_value() {
        // QUNS_PRESENTATION_MODE = 4
        let i = Inputs { notification_state: 4, ..base() };
        assert!(!should_show(&i));
    }

    #[test]
    fn hides_when_taskbar_hidden() {
        let i = Inputs { taskbar_visible: false, ..base() };
        assert!(!should_show(&i));
    }

    #[test]
    fn hides_on_implausible_rect() {
        // A zero-width or absurd rect means UIA gave us something unusable.
        let i = Inputs { widget: Some(Rect { x: 0, y: 0, w: 0, h: 0 }), ..base() };
        assert!(!should_show(&i));
    }

    #[test]
    fn plausibility_accepts_the_measured_rect() {
        assert!(good_rect().is_plausible_widget());
    }

    #[test]
    fn plausibility_rejects_degenerate_rects() {
        assert!(!Rect { x: 0, y: 0, w: 0, h: 60 }.is_plausible_widget());
        assert!(!Rect { x: 0, y: 0, w: 190, h: 0 }.is_plausible_widget());
        assert!(!Rect { x: 0, y: 0, w: 5000, h: 60 }.is_plausible_widget());
    }
    #[test]
    fn an_unknown_widget_rect_hides_the_overlay_but_does_not_count_as_the_shell_blocking_it() {
        // THE distinction that stops the suspend path deadlocking, and it is worth a test of its own
        // because getting it wrong is invisible until the meter never comes back.
        //
        // The suspend path skips the once-a-second UIA rect rediscovery. If "I do not know where to
        // draw" counted as "the shell is covering me", then: rect None -> suspended -> no rediscovery
        // -> rect stays None -> suspended for ever. The overlay would never appear again, and the
        // symptom would be indistinguishable from the app failing to start.
        let no_rect = Inputs { widget: None, notification_state: 0, taskbar_visible: true, fullscreen_foreground: false };
        assert!(!should_show(&no_rect), "with no rect there is nowhere to draw");
        assert!(
            !shell_blocks(no_rect.notification_state, no_rect.taskbar_visible, no_rect.fullscreen_foreground),
            "not knowing the rect must NOT suspend the app - that is how it would never recover"
        );

        // The genuine reasons to suspend, on the other hand, must all report true.
        assert!(shell_blocks(QUNS_FULLSCREEN, true, false), "a fullscreen app must suspend");
        assert!(shell_blocks(QUNS_PRESENTATION, true, false), "presentation mode must suspend");
        assert!(shell_blocks(0, false, false), "a hidden taskbar must suspend");
        assert!(!shell_blocks(0, true, false), "an ordinary desktop must not suspend");

        // And the two agree wherever they can: anything the shell blocks is also not shown.
        for state in [0, 1, 2, QUNS_FULLSCREEN, QUNS_PRESENTATION] {
            for vis in [true, false] {
                let i = Inputs { widget: Some(base().widget.unwrap()), notification_state: state, taskbar_visible: vis, fullscreen_foreground: false };
                if shell_blocks(state, vis, false) {
                    assert!(!should_show(&i), "state {state}/vis {vis}: blocked but shown");
                }
            }
        }
    }

}
