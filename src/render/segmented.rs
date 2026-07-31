use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::{Texture, Theme};

const BAR_W: i32 = 5;
const GAP: i32 = 2;
const SEG_H: i32 = 3;
const SEG_GAP: i32 = 1;
const PAD_X: i32 = 5;
const PAD_Y: i32 = 6;

pub struct Segmented;

impl Family for Segmented {
    fn id(&self) -> &'static str {
        "segmented"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        // 1. panel
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&t.panel, t.panel_alpha));

        // 2. panel texture
        match t.texture {
            Texture::Glass => {
                for y in 2..(h / 2) {
                    let a = 0.09 * (1.0 - (y - 2) as f32 / (h / 2 - 2) as f32);
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex("#bef0ff", a));
                }
            }
            Texture::Scanlines => {
                let mut y = 2;
                while y < h - 4 {
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, 0.045));
                    y += 2;
                }
            }
            Texture::Filament => {
                for y in (h / 2)..(h - 4) {
                    let a = 0.20 * (y - h / 2) as f32 / (h / 2 - 4) as f32;
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, a));
                }
            }
            Texture::Haze => {
                for y in 2..(h - 4) {
                    let dy = ((y - h / 2) as f32 / (h as f32 * 0.5)).abs();
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, 0.13 * (1.0 - dy).max(0.0)));
                }
            }
            Texture::Grille => {
                let mut x = 1;
                while x < w - 1 {
                    c.fill_rect(x, 2, 1, h - 4, Rgba::from_hex("#ffffff", 0.028));
                    x += 3;
                }
            }
            Texture::None_ => {}
        }

        // 3. geometry from the live rect
        let usable_w = w - PAD_X * 2;
        let usable_h = h - PAD_Y * 2;
        let pitch = BAR_W + GAP;
        let nbars = (usable_w / pitch).max(1);
        let ox = PAD_X + (usable_w - nbars * pitch + GAP) / 2;
        let seg_pitch = SEG_H + SEG_GAP;
        let nseg = (usable_h / seg_pitch).max(1);

        let sample = |arr: &[f32], b: i32| -> f32 {
            let lo = (b as usize * arr.len()) / nbars as usize;
            let hi = (((b + 1) as usize * arr.len()) / nbars as usize).max(lo + 1);
            arr[lo..hi.min(arr.len())].iter().copied().fold(0.0f32, f32::max)
        };

        // 4. dormant ghost grid - zone-coloured when zones are present
        if t.ghost > 0.0 {
            for k in 0..nseg {
                let frac = (k + 1) as f32 / nseg as f32;
                let col = Rgba::from_hex(t.lit_at(frac), t.ghost);
                let y = PAD_Y + usable_h - (k + 1) * seg_pitch;
                for b in 0..nbars {
                    c.fill_rect(ox + b * pitch, y, BAR_W, SEG_H, col);
                }
            }
        }

        // 5. lit columns - one fill per bar (or per zone), then bloom, then punch
        for b in 0..nbars {
            let lit = (sample(&d.levels, b) * nseg as f32).round() as i32;
            for k in 0..lit.min(nseg) {
                let frac = (k + 1) as f32 / nseg as f32;
                let y = PAD_Y + usable_h - (k + 1) * seg_pitch;
                c.fill_rect(ox + b * pitch, y, BAR_W, SEG_H, Rgba::from_hex(t.lit_at(frac), 1.0));
            }
        }

        c.bloom(t.bloom.round() as i32, 0.85);

        // 6. hot core: a narrower brighter rect, not a gradient
        if t.zones.is_empty() {
            for b in 0..nbars {
                let lit = (sample(&d.levels, b) * nseg as f32).round() as i32;
                if lit <= 0 {
                    continue;
                }
                let hh = lit.min(nseg) * seg_pitch - SEG_GAP;
                c.fill_rect(
                    ox + b * pitch + (BAR_W as f32 * 0.28) as i32,
                    PAD_Y + usable_h - hh,
                    (BAR_W as f32 * 0.44).ceil() as i32,
                    hh,
                    Rgba::from_hex(&t.hot, 0.55),
                );
            }
        }

        // 7. punch the segment gaps back out
        for k in 1..=nseg {
            c.punch_row(PAD_Y + usable_h - k * seg_pitch + SEG_H, SEG_GAP);
        }

        // 8. peak-hold caps
        for b in 0..nbars {
            let pk = (sample(&d.peaks, b) * nseg as f32).round() as i32;
            if pk <= 0 {
                continue;
            }
            let frac = pk as f32 / nseg as f32;
            c.fill_rect(
                ox + b * pitch,
                PAD_Y + usable_h - pk * seg_pitch,
                BAR_W,
                1,
                Rgba::from_hex(t.hot_at(frac), 1.0),
            );
        }

        // 9. bezel
        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
        c.fill_rect(1, 2, 1, h - 4, e);
        c.fill_rect(w - 2, 2, 1, h - 4, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::bands::NUM_BANDS;
    use crate::render::golden::canvas_to_ascii;
    use crate::themes::builtin;

    fn frame(level: f32) -> FrameData {
        FrameData {
            levels: [level; NUM_BANDS],
            peaks: [level; NUM_BANDS],
            waveform: [0.0; 256],
            rms_l: level,
            rms_r: level,
        }
    }

    #[test]
    fn silence_still_draws_the_panel_and_ghost_grid() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.0));
        // Panel is opaque enough to be visible.
        assert!(c.get(95, 30).a > 100, "panel must be drawn even in silence");
        // Ghost grid present but dim.
        let ascii = canvas_to_ascii(&c);
        assert!(ascii.contains('.') || ascii.contains(':'), "expected a dim ghost grid");
        assert!(!ascii.contains('@'), "nothing should be fully lit in silence");
    }

    /// x of the first bar's left edge. The brief's original tests sampled at
    /// `PAD_X + 2`, which assumes the bar grid starts exactly at PAD_X. It
    /// doesn't: `draw` centres the 25-bar grid inside the usable width, and
    /// for 190x60 that centring offset is 3px (measured: ox=8, not 5), so
    /// `PAD_X + 2` (=7) landed one pixel to the *left* of the first bar - in
    /// the unlit margin, not "2px into" it. A live probe there read alpha
    /// 199 (bloom bleeding just past the edge) against 255 one column to the
    /// right: an easy-to-miss near pass/fail at the >200 threshold.
    /// Recomputing ox from the real geometry (mirroring `draw`'s own
    /// formula) keeps the sample point correct if the constants ever change.
    fn first_bar_x() -> i32 {
        let usable_w = 190 - PAD_X * 2;
        let pitch = BAR_W + GAP;
        let nbars = usable_w / pitch;
        PAD_X + (usable_w - nbars * pitch + GAP) / 2
    }

    #[test]
    fn full_level_lights_the_top_of_the_bars() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
        let top = c.get(first_bar_x() + 2, PAD_Y + 1);
        assert!(top.a > 200, "top segment should be lit at full level");
    }

    #[test]
    fn half_level_lights_the_bottom_but_not_the_top() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
        let bottom = c.get(first_bar_x() + 2, 60 - PAD_Y - 2);
        let top = c.get(first_bar_x() + 2, PAD_Y + 1);
        assert!(bottom.a > 150, "bottom must be lit");
        assert!(top.a < bottom.a, "top must be dimmer than bottom at half level");
    }

    #[test]
    fn segment_gaps_are_punched_through_to_transparent() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
        // Walk a lit column and confirm the luminance is not monotonic - the
        // gaps must interrupt it. This is what distinguishes a segmented meter
        // from a solid bar.
        let x = PAD_X + 2;
        let mut transitions = 0;
        let mut prev = c.get(x, PAD_Y).a > 128;
        for y in PAD_Y..(60 - PAD_Y) {
            let now = c.get(x, y).a > 128;
            if now != prev {
                transitions += 1;
            }
            prev = now;
        }
        assert!(transitions >= 6, "expected several segment gaps, saw {transitions}");
    }

    #[test]
    fn bar_count_matches_the_geometry() {
        // (190 - 10) / 7 = 25 bars at the measured widget width.
        let usable = 190 - PAD_X * 2;
        assert_eq!(usable / (BAR_W + GAP), 25);
    }

    #[test]
    fn nothing_is_drawn_outside_the_canvas() {
        // Narrow rect - must clip, not panic.
        let mut c = Canvas::new(40, 20);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
        assert_eq!(c.bits().len(), 40 * 20);
    }

    #[test]
    fn golden_vfd_ice_at_half_level() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
        let actual = canvas_to_ascii(&c);
        let expected = include_str!("../../tests/golden/vfd-ice.txt");
        assert_eq!(
            actual, expected,
            "golden mismatch - if this change is intended, overwrite \
             tests/golden/vfd-ice.txt and eyeball the diff"
        );
    }

    #[test]
    #[ignore]
    fn regenerate_golden() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
        std::fs::write("tests/golden/vfd-ice.txt", canvas_to_ascii(&c)).unwrap();
    }
}
