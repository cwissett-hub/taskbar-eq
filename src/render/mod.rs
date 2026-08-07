pub mod canvas;
pub mod golden;
pub mod scope;
pub mod nixie;
pub mod patchbay;
pub mod chroma;
pub mod fluid;
pub mod pantone;
pub mod banner;
pub mod radar;
pub mod rwr;
pub mod text;
pub mod reel;
pub mod tube;
pub mod waterfall;
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
    /// Seconds since the process started, for animation that is not frame-counted.
    ///
    /// Wrapped at 3600 so f32 keeps useful precision - at an hour it still resolves 0.2ms, where an
    /// unwrapped accumulator would be down to whole milliseconds after a day. A hue phase can
    /// therefore step discontinuously once an hour if the cycle rate does not divide 3600 evenly,
    /// which is not worth more machinery than this comment.
    pub time_s: f32,
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
            time_s: 0.0,
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

/// The colour an element should be drawn in, honouring a rainbow colourway.
///
/// One resolver rather than a rainbow branch at every call site: families call this exactly where
/// they used to call `Rgba::from_hex`, so a colourway that is not a rainbow one takes the identical
/// path it always did and the fixed colourways cannot be changed by this feature at all.
///
/// `x01` is the element's horizontal position across the display, which is what turns a hue cycle
/// into a WAVE - and on a spectrum display that position is frequency, so the rainbow doubles as a
/// frequency legend.
pub fn tint(
    t: &Theme,
    x01: f32,
    time_s: f32,
    hot: bool,
    fallback: &str,
    alpha: f32,
) -> canvas::Rgba {
    match crate::themes::rainbow_hsv(t, x01, time_s, hot) {
        Some((h, s, v)) => canvas::Rgba::from_hsv(h, s, v, alpha),
        None => canvas::Rgba::from_hex(fallback, alpha),
    }
}

/// Every family the renderer can dispatch on.
///
/// Shared with the theme tests rather than duplicated there: `family_for` falls back to
/// `segmented` for anything unrecognised, so a theme carrying a typo'd or unimplemented
/// family name would silently render as the wrong meter instead of failing. This list is
/// what lets that be asserted, and adding a family is a one-line change in one place.
pub const KNOWN_FAMILIES: [&str; 13] = [
    "segmented", "scope", "vu", "vapor", "tube", "nixie", "waterfall", "reel", "patchbay", "radar", "pantone", "chroma", "fluid",
];

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
        "tube" => Box::new(tube::Tube::default()),
        "nixie" => Box::new(nixie::Nixie::default()),
        "waterfall" => Box::new(waterfall::Waterfall::default()),
        "reel" => Box::new(reel::Reel::default()),
        "patchbay" => Box::new(patchbay::Patchbay::default()),
        "radar" => Box::new(radar::Radar::default()),
        "fluid" => Box::new(fluid::Fluid::default()),
        "pantone" => Box::new(pantone::Pantone::default()),
        "chroma" => Box::new(chroma::Chroma::default()),
        "vu" => Box::new(vu::Vu::default()),
        _ => Box::new(segmented::Segmented::default()),
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use crate::themes::builtin;

    /// Every shipped colourway must render at every plausible overlay size without panicking.
    ///
    /// The overlay canvas is sized from the LIVE Widgets-button rect, which moves and resizes
    /// as the weather text changes - x positions of 1385, 1416 and 1425 have all been observed
    /// on one machine, and a different DPI or a Windows 10 chevron fallback gives different
    /// dimensions again. Each family was only ever eyeballed at 190x60, so this is the guard
    /// that a user selecting a theme cannot take the process down with an out-of-bounds write
    /// or a divide-by-zero in geometry that assumed the reference size.
    #[test]
    fn every_colourway_renders_at_every_plausible_overlay_size() {
        let sizes = [
            (190, 60), // the reference
            (150, 48), // smaller widget / lower DPI
            (240, 72), // 150% DPI
            (456, 60), // the planned wide variant
            (96, 40),  // an unusually narrow rect
            (40, 24),  // pathologically small
            (12, 12),  // degenerate
            (1, 1),    // the limit case
        ];
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = (i as f32 / 63.0).min(1.0);
        }
        d.peaks = d.levels;
        for (i, v) in d.waveform.iter_mut().enumerate() {
            *v = ((i as f32 / 32.0).sin()) * 0.6;
        }
        d.rms_l = 0.09;
        d.rms_r = 0.055;

        for theme in builtin::all() {
            for (w, h) in sizes {
                let mut family = family_for(&theme.family);
                let mut c = Canvas::new(w, h);
                // Several frames, because the stateful families allocate persistence buffers
                // on the first frame and reuse them on later ones.
                for _ in 0..3 {
                    family.draw(&mut c, &theme, &d);
                }
                assert_eq!(
                    c.bits().len(),
                    (w.max(0) * h.max(0)) as usize,
                    "{} at {w}x{h} changed the canvas size",
                    theme.id
                );
            }
        }
    }

    /// The same, but at silence and with a poisoned spectrum, since a NaN reaching a geometry
    /// calculation is how the vaporwave scroll phase was permanently corrupted.
    #[test]
    fn every_colourway_survives_silence_and_a_poisoned_spectrum() {
        for theme in builtin::all() {
            for spoil in [0usize, 1, 2] {
                let mut d = FrameData::default();
                match spoil {
                    0 => {}
                    1 => {
                        d.levels[0] = f32::NAN;
                        d.peaks[5] = f32::NAN;
                        d.waveform[10] = f32::NAN;
                        d.rms_l = f32::NAN;
                        d.dt_ms = f32::NAN;
                    }
                    _ => {
                        d.levels[3] = f32::INFINITY;
                        d.waveform[7] = f32::NEG_INFINITY;
                        d.rms_r = f32::INFINITY;
                        d.dt_ms = 0.0;
                    }
                }
                let mut family = family_for(&theme.family);
                let mut c = Canvas::new(190, 60);
                for _ in 0..4 {
                    family.draw(&mut c, &theme, &d);
                }
            }
        }
    }
}

#[cfg(test)]
mod wide_dump {
    use super::*;
    use crate::themes::builtin;

    /// Dumps one colourway per family at the wide size, for eyeballing.
    /// Run: cargo test --release dump_wide -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_wide() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let (w, h) = (380, 60);
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            let x = i as f32 / 63.0;
            *v = (0.18 + 0.72 * (x * 9.0).sin().abs()) * (1.0 - x * 0.4);
        }
        d.peaks = d.levels;
        for (i, v) in d.waveform.iter_mut().enumerate() {
            let t = i as f32 / 256.0;
            *v = 0.45 * ((t * std::f32::consts::TAU * 3.0).sin() + 0.4 * (t * std::f32::consts::TAU * 7.0).sin());
        }
        d.rms_l = 0.09;
        d.rms_r = 0.055;

        for id in ["vfd-ice", "p1-green", "vu-cream", "vapor-sunset", "tube-soviet"] {
            let theme = builtin::all().into_iter().find(|t| t.id == id).unwrap();
            let mut fam = family_for(&theme.family);
            let mut c = Canvas::new(w, h);
            for _ in 0..8 {
                fam.draw(&mut c, &theme, &d);
            }
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for ch in [px.r, px.g, px.b] {
                        out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("wide-{id}.rgba")), &out).unwrap();
        }
        println!("wrote 5 wide dumps ({w}x{h}) to {}", dir.display());
    }
}

#[cfg(test)]
mod opacity {
    use super::*;
    use crate::themes::builtin;

    /// No family may leave a see-through pixel inside its own panel, at any level.
    ///
    /// The panel is what stands between the meter and the Windows weather widget underneath it, and
    /// the overlay is composited with UpdateLayeredWindow using per-pixel alpha - so a pixel with
    /// alpha < 255 inside the panel is a hole that the weather text shows through. That is not a
    /// theoretical concern: it shipped twice. First when `panel_alpha` was below 1.0, and then again
    /// in the segmented family, whose hot-core gap re-cut used `punch_rect` - which writes ZERO,
    /// not the panel colour - so loud bars punched a scatter of transparent pixels through the
    /// panel.
    ///
    /// Swept across levels precisely because that second bug was LEVEL-DEPENDENT: the hot core only
    /// exists on a loud bar, so the holes appeared "occasionally, not constantly" and a single-level
    /// test would have missed them entirely.
    #[test]
    fn no_family_leaves_a_transparent_pixel_inside_its_panel() {
        let (w, h) = (190, 60);
        let mut worst: Option<(String, f32, i32, i32, u8)> = None;
        let mut offenders = std::collections::BTreeMap::new();

        for theme in builtin::all() {
            let mut fam = family_for(&theme.family);
            for step in 0..=10 {
                let level = step as f32 / 10.0;
                let mut d = FrameData::default();
                // Slightly uneven, so bars differ and the hot core appears on some and not others.
                for (i, v) in d.levels.iter_mut().enumerate() {
                    *v = (level * (0.75 + 0.25 * ((i % 7) as f32 / 6.0))).clamp(0.0, 1.0);
                }
                d.peaks = d.levels;
                d.rms_l = level;
                d.rms_r = level * 0.8;
                for (i, v) in d.waveform.iter_mut().enumerate() {
                    *v = level * ((i as f32 / 20.0).sin());
                }
                let mut c = Canvas::new(w, h);
                // Several frames: the stateful families settle, and peak-holds engage.
                for _ in 0..6 {
                    fam.draw(&mut c, &theme, &d);
                }
                // Inset well clear of the rounded corners and the 1px bezel rows, so this is about
                // the panel's interior rather than its antialiased edge.
                for y in 6..(h - 8) {
                    for x in 6..(w - 6) {
                        let a = c.get(x, y).a;
                        if a < 255 {
                            *offenders.entry(theme.id.clone()).or_insert(0u32) += 1;
                            if worst.as_ref().map(|q| a < q.4).unwrap_or(true) {
                                worst = Some((theme.id.clone(), level, x, y, a));
                            }
                        }
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these colourways leave see-through pixels inside the panel, which the weather widget \
             shows through: {offenders:?}; worst {worst:?}"
        );
    }
}

#[cfg(test)]
mod rainbow_dump {
    use super::*;
    use crate::themes::builtin;
    /// Run: cargo test --release dump_rainbow -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_rainbow() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let mut n = 0;
        for theme in builtin::all().into_iter().filter(|t| t.rainbow > 0.0) {
            for (tag, time_s) in [("t0", 0.0f32), ("t1", 3.0)] {
                let mut fam = family_for(&theme.family);
                let mut c = Canvas::new(190, 60);
                let mut d = FrameData::default();
                for (i, v) in d.levels.iter_mut().enumerate() {
                    let x = i as f32 / 63.0;
                    *v = (0.22 + 0.5 * (x * 8.0).sin().abs()) * (1.0 - x * 0.25);
                }
                d.peaks = d.levels;
                d.rms_l = 0.10;
                d.rms_r = 0.065;
                for (i, v) in d.waveform.iter_mut().enumerate() {
                    *v = 0.4 * ((i as f32 / 26.0).sin() + 0.4 * (i as f32 / 9.0).sin());
                }
                d.time_s = time_s;
                for _ in 0..8 {
                    fam.draw(&mut c, &theme, &d);
                }
                let mut out = Vec::new();
                for y in 0..60 {
                    for x in 0..190 {
                        let px = c.get(x, y);
                        let a = px.a as f32 / 255.0;
                        for ch in [px.r, px.g, px.b] {
                            out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                        }
                        out.push(255);
                    }
                }
                std::fs::write(dir.join(format!("rgb-{}-{tag}.rgba", theme.id)), &out).unwrap();
                n += 1;
            }
        }
        println!("wrote {n} rainbow dumps");
    }
}

#[cfg(test)]
mod newfam_dump {
    use super::*;
    use crate::themes::builtin;
    /// Run: cargo test --release dump_new_families -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_new_families() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let mut n = 0;
        for fam in ["nixie", "waterfall", "reel", "patchbay", "radar"] {
            let theme = builtin::all().into_iter().find(|t| t.family == fam).unwrap();
            let mut f = family_for(fam);
            let mut c = Canvas::new(190, 60);
            // 150 frames of varying spectrum: the history families need a filled buffer and the
            // rotating ones need to be past their start pose.
            for k in 0..150 {
                let mut d = FrameData::default();
                let t = k as f32 / 150.0;
                for (i, v) in d.levels.iter_mut().enumerate() {
                    let x = i as f32 / 63.0;
                    *v = (0.20 + 0.45 * ((x * 7.0 + t * 9.0).sin().abs())) * (1.0 - x * 0.35);
                }
                d.peaks = d.levels;
                d.rms_l = 0.10;
                d.rms_r = 0.07;
                for (i, v) in d.waveform.iter_mut().enumerate() {
                    *v = 0.4 * ((i as f32 / 24.0).sin());
                }
                d.dt_ms = 16.7;
                d.time_s = k as f32 * 0.0167;
                f.draw(&mut c, &theme, &d);
            }
            let mut out = Vec::new();
            for y in 0..60 {
                for x in 0..190 {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for ch in [px.r, px.g, px.b] {
                        out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("new-{fam}.rgba")), &out).unwrap();
            n += 1;
        }
        println!("wrote {n} new-family dumps");
    }
}

#[cfg(test)]
mod newest_dump {
    use super::*;
    use crate::themes::builtin;
    /// Run: cargo test --release dump_newest -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_newest() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let want = [
            "radar-p1", "pantone-spectrum", "pantone-misregister", "pantone-barcode",
            "chroma-spectrum", "chroma-barcode", "chroma-misreg", "vfd-pantone",
        ];
        let mut n = 0;
        for id in want {
            let Some(theme) = builtin::all().into_iter().find(|t| t.id == id) else { continue };
            let mut f = family_for(&theme.family);
            let mut c = Canvas::new(190, 60);
            for k in 0..220 {
                let mut d = FrameData::default();
                let t = k as f32 / 220.0;
                for (i, v) in d.levels.iter_mut().enumerate() {
                    let x = i as f32 / 63.0;
                    *v = (0.18 + 0.5 * ((x * 6.0 + t * 11.0).sin().abs())) * (1.0 - x * 0.3);
                }
                d.peaks = d.levels;
                d.rms_l = 0.24;
                d.rms_r = 0.19;
                for (i, v) in d.waveform.iter_mut().enumerate() {
                    *v = 0.4 * ((i as f32 / 24.0).sin());
                }
                d.dt_ms = 16.7;
                d.time_s = k as f32 * 0.0167;
                f.draw(&mut c, &theme, &d);
            }
            let mut out = Vec::new();
            for y in 0..60 {
                for x in 0..190 {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for ch in [px.r, px.g, px.b] {
                        out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("new2-{id}.rgba")), &out).unwrap();
            n += 1;
        }
        println!("wrote {n} dumps");
    }
    /// Every family's flourish at its peak, beside the same frame with the flourish off.
    ///
    /// Run: cargo test --release dump_flourishes -- --ignored --nocapture
    ///
    /// Writes PAIRS deliberately. A flourish is only interesting as a difference, and judging one from
    /// a single frame is how a subtle effect gets called broken and an overpowering one gets called
    /// fine. The `-off` frame is the same audio, same colourway, same frame index, with
    /// `flourish = 0`.
    ///
    /// The frame chosen is a few ticks AFTER the firing frame, not the firing frame: that frame is
    /// full-scale across every band, so most displays are already saturated by the music there and the
    /// flourish is invisible. See `dsp::flourish::firing_sequence`.
    #[test]
    #[ignore]
    fn dump_flourishes() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let seq = crate::dsp::flourish::firing_sequence(crate::dsp::bands::NUM_BANDS);
        // One representative colourway per family, and a quiet frame to hold after the hit.
        let picks = [
            ("segmented", "vfd-ice"),
            ("vu", "hifi-white"),
            ("waterfall", "waterfall-heat"),
        ];
        let mut n = 0;
        for (family, theme_id) in picks {
            for (tag, strength) in [("on", crate::themes::DEFAULT_FLOURISH), ("off", 0.0)] {
                let mut theme = crate::themes::builtin::all()
                    .into_iter()
                    .find(|t| t.id == theme_id)
                    .unwrap_or_else(|| panic!("no colourway {theme_id}"));
                theme.flourish = strength;
                let mut f = family_for(family);
                let mut c = Canvas::new(190, 60);
                for row in &seq {
                    let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
                    for (i, v) in d.levels.iter_mut().enumerate() {
                        *v = row.get(i).copied().unwrap_or(0.0);
                    }
                    d.peaks = d.levels;
                    // A plausible stereo reading for the dial families, which read rms not bands.
                    d.rms_l = 0.06;
                    d.rms_r = 0.05;
                    f.draw(&mut c, &theme, &d);
                }
                // Four quiet frames: the envelope is still near full, the audio has dropped away.
                for _ in 0..4 {
                    let d = FrameData { dt_ms: 16.7, rms_l: 0.02, rms_r: 0.02, ..FrameData::default() };
                    f.draw(&mut c, &theme, &d);
                }
                let mut out = Vec::new();
                for y in 0..60 {
                    for x in 0..190 {
                        let px = c.get(x, y);
                        let a = px.a as f32 / 255.0;
                        for ch in [px.r, px.g, px.b] {
                            out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                        }
                        out.push(255);
                    }
                }
                std::fs::write(dir.join(format!("flourish-{family}-{tag}.rgba")), &out).unwrap();
                n += 1;
            }
        }
        println!("wrote {n} flourish dumps (on/off pairs) to {}", dir.display());
    }

}
