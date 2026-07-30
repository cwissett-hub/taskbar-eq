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
}

pub fn should_show(i: &Inputs) -> bool {
    if !i.taskbar_visible {
        return false;
    }
    if i.notification_state == QUNS_FULLSCREEN || i.notification_state == QUNS_PRESENTATION {
        return false;
    }
    match i.widget {
        Some(r) => r.is_plausible_widget(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good_rect() -> Rect {
        Rect { x: 1416, y: 1140, w: 190, h: 60 }
    }

    fn base() -> Inputs {
        Inputs { widget: Some(good_rect()), notification_state: 5, taskbar_visible: true }
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
}
