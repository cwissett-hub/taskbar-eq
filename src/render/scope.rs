use super::canvas::Canvas;
use super::{Family, FrameData};
use crate::themes::Theme;

#[derive(Default)]
pub struct Scope;

impl Family for Scope {
    fn id(&self) -> &'static str {
        "scope"
    }
    fn draw(&mut self, _c: &mut Canvas, _t: &Theme, _d: &FrameData) {
        // Implemented in Task 13.
    }
}
