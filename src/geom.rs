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
