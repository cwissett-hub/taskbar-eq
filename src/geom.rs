// Task 2 introduces this type standalone; Task 3 wires it into
// win::placement and main's polling loop. Until then, rustc's binary-crate
// dead-code check flags it as unused even though the tests below exercise it.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    /// Guards against UIA handing back a degenerate or absurd rect, which would
    /// otherwise place a glowing rectangle somewhere random.
    pub fn is_plausible_widget(&self) -> bool {
        self.w >= 40 && self.w <= 600 && self.h >= 20 && self.h <= 200
    }
}

/// Where to anchor the context menu, for `TrackPopupMenu` with `TPM_RIGHTALIGN | TPM_BOTTOMALIGN`.
///
/// Under those flags the y coordinate is the menu's BOTTOM edge, not its top - so passing the cursor
/// position, as this did originally, puts the bottom of the menu wherever inside the taskbar the click
/// landed. The menu then covers the taskbar from that point down: a click near the taskbar's top edge
/// looked almost right, and a click near the bottom buried most of the taskbar. The overlay sits ON the
/// taskbar, so every right-click of it lands inside that band and the error was never zero.
///
/// Anchoring y to the taskbar's TOP edge instead lands the menu's bottom exactly on it, so the menu
/// stacks on the taskbar rather than over it. That is the same edge the overlay itself is placed at
/// (`rect_left_of` returns `y: taskbar.y`), so the menu's bottom edge and the display's top edge agree
/// by construction rather than by coincidence.
///
/// x stays on the cursor: with `TPM_RIGHTALIGN` the menu's right edge sits at the click, which for a
/// display anchored near the right of the taskbar opens it leftward, into the screen.
///
/// A taskbar mounted at the TOP of the screen would want `TPM_TOPALIGN` against its bottom edge, and
/// this does not do that. Windows 11 cannot move the taskbar, and Windows 10 - which can - is a path
/// this project has never been able to test. `TrackPopupMenu` keeps the menu on-screen by itself, so
/// the worst case there is the menu overlapping the taskbar exactly as it does now: no new failure.
pub fn menu_anchor(cursor: (i32, i32), taskbar: Option<Rect>) -> (i32, i32) {
    match taskbar {
        Some(bar) => (cursor.0, bar.y),
        // No `Shell_TrayWnd` means no taskbar to sit on top of, and the cursor is the only position
        // information left. Nothing is anchored in this state anyway - the overlay is not placed either.
        None => cursor,
    }
}

#[cfg(test)]
mod menu_anchor_tests {
    use super::*;

    /// A 1920x1080 screen with the Windows 11 taskbar at its default height.
    const BAR: Rect = Rect { x: 0, y: 1032, w: 1920, h: 48 };

    #[test]
    fn anchors_the_menus_bottom_edge_to_the_top_of_the_taskbar() {
        // A click 23px below the taskbar's top edge - i.e. in the lower half of the display, which is
        // where the pointer naturally lands on a 48px-tall target.
        let (_, y) = menu_anchor((1700, 1055), Some(BAR));
        assert_eq!(y, BAR.y, "the anchor must be the taskbar's top edge");
        assert_ne!(
            y, 1055,
            "anchoring to the cursor is the bug: under TPM_BOTTOMALIGN it would put the bottom of the \
             menu 23px INSIDE the taskbar, covering it from there down"
        );
    }

    #[test]
    fn keeps_x_on_the_cursor_not_on_the_taskbar() {
        // The taskbar spans the whole screen, so its own x carries no information about where the click
        // was. A menu anchored to it would open at the far left of the screen.
        let (x, _) = menu_anchor((1700, 1055), Some(BAR));
        assert_eq!(x, 1700);
        assert_ne!(x, BAR.x, "the taskbar's left edge is not where the click happened");
    }

    #[test]
    fn falls_back_to_the_cursor_when_there_is_no_taskbar() {
        assert_eq!(menu_anchor((640, 400), None), (640, 400));
    }
}
