pub mod canvas;
pub mod golden;
pub mod scope;
pub mod segmented;
pub mod vu;

use crate::dsp::bands::NUM_BANDS;
use crate::themes::Theme;
use canvas::Canvas;

pub struct FrameData {
    pub levels: [f32; NUM_BANDS],
    pub peaks: [f32; NUM_BANDS],
    // `waveform`/`rms_l`/`rms_r` have no reader yet: `segmented` (the only
    // family implemented so far) only looks at `levels`/`peaks`. The scope
    // family (Task 13) reads `waveform`; the VU family (Task 14) reads
    // `rms_l`/`rms_r`. Both stubs already take `&FrameData` so wiring them up
    // later touches no call site.
    #[allow(dead_code)]
    pub waveform: [f32; 256],
    #[allow(dead_code)]
    pub rms_l: f32,
    #[allow(dead_code)]
    pub rms_r: f32,
}

impl Default for FrameData {
    fn default() -> Self {
        FrameData {
            levels: [0.0; NUM_BANDS],
            peaks: [0.0; NUM_BANDS],
            waveform: [0.0; 256],
            rms_l: 0.0,
            rms_r: 0.0,
        }
    }
}

/// A family is a renderer with its own geometry and its own per-frame state.
/// Adding one means a new file plus one line in `family_for` - no existing
/// family is touched.
pub trait Family {
    // Round-trips the registry key `family_for` dispatches on. Not called
    // from main's straight-line loop yet - it is for a future theme-switching
    // / diagnostics UI that needs to ask a live `Box<dyn Family>` what it is.
    #[allow(dead_code)]
    fn id(&self) -> &'static str;
    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData);
}

pub fn family_for(id: &str) -> Box<dyn Family> {
    match id {
        "scope" => Box::new(scope::Scope::default()),
        "vu" => Box::new(vu::Vu::default()),
        _ => Box::new(segmented::Segmented),
    }
}
