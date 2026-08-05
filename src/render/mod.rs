pub mod canvas;
pub mod golden;
pub mod scope;
pub mod vapor;
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
    /// Milliseconds since the previous frame.
    ///
    /// The render loop sleeps a fixed 16ms, so its real period is that plus however long
    /// the frame took - a per-frame animation step drifts with load. Only the vaporwave
    /// family reads this; the older families' ballistics were tuned per-frame and are left
    /// alone rather than silently retimed.
    pub dt_ms: f32,
}

impl Default for FrameData {
    fn default() -> Self {
        FrameData {
            levels: [0.0; NUM_BANDS],
            peaks: [0.0; NUM_BANDS],
            waveform: [0.0; 256],
            rms_l: 0.0,
            rms_r: 0.0,
            dt_ms: 16.7,
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

/// Every family the renderer can dispatch on.
///
/// Shared with the theme tests rather than duplicated there: `family_for` falls back to
/// `segmented` for anything unrecognised, so a theme carrying a typo'd or unimplemented
/// family name would silently render as the wrong meter instead of failing. This list is
/// what lets that be asserted, and adding a family is a one-line change in one place.
pub const KNOWN_FAMILIES: [&str; 4] = ["segmented", "scope", "vu", "vapor"];

pub fn family_for(id: &str) -> Box<dyn Family> {
    // Say so when falling back. A theme file with a typo'd or not-yet-implemented family
    // otherwise renders as a segmented meter with no indication of why, which is a
    // confusing thing to debug from the outside - and it is a plausible mistake now that
    // authoring a theme by hand is a supported workflow.
    if !KNOWN_FAMILIES.contains(&id) {
        eprintln!(
            "themes: unknown family {id:?}, falling back to segmented (known: {})",
            KNOWN_FAMILIES.join(", ")
        );
    }
    match id {
        "scope" => Box::new(scope::Scope::default()),
        "vapor" => Box::new(vapor::Vapor::default()),
        "vu" => Box::new(vu::Vu::default()),
        _ => Box::new(segmented::Segmented),
    }
}
