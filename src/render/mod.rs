// Canvas is a pure rasteriser with no consumer yet - the render loop that
// draws into it and blits it via UpdateLayeredWindow arrives in a later
// task. Until then every item here is legitimately unused from main's
// point of view, so silence dead_code rather than let it hide a real
// warning later.
#[allow(dead_code)]
pub mod canvas;
