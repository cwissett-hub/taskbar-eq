use super::canvas::Canvas;
use super::{Family, FrameData};
use crate::themes::Theme;

#[derive(Default)]
pub struct Vu;

impl Family for Vu {
    fn id(&self) -> &'static str {
        "vu"
    }
    fn draw(&mut self, _c: &mut Canvas, _t: &Theme, _d: &FrameData) {
        // Implemented in Task 14.
    }
}
