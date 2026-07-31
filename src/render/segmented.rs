use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::{Texture, Theme};

const BAR_W: i32 = 5;
const GAP: i32 = 2;
const SEG_H: i32 = 3;
const SEG_GAP: i32 = 1;
const PAD_X: i32 = 5;
const PAD_Y: i32 = 6;

/// How much brighter than "energy-preserving" the halo is driven. 1.0 would make
/// a wide radius look identical in intensity to a narrow one - only more spread -
/// because the box blur normalises by kernel size. Above 1.0 is what actually
/// reads as phosphor glow. Chosen by eye against the live taskbar.
const GLOW_GAIN: f32 = 3.4;

/// Couples strength to radius. `Canvas::bloom` is a box blur and normalises by
/// kernel size, so raising the radius alone makes the halo FAINTER, not stronger -
/// the opposite of the intent, and a trap for every theme that follows. Scaling by
/// radius squared cancels the normalisation; GLOW_GAIN then overdrives it.
fn bloom_strength(bloom: f32) -> f32 {
    const REF_RADIUS: f32 = 9.0;
    let r = bloom.max(1.0);
    ((r / REF_RADIUS).powi(2) * 0.85 * GLOW_GAIN).clamp(0.6, 12.0)
}

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

        // 4. lit columns FIRST, so that only they feed the bloom.
        let lit_of = |b: i32| (sample(&d.levels, b) * nseg as f32).round() as i32;
        for b in 0..nbars {
            for k in 0..lit_of(b).min(nseg) {
                let frac = (k + 1) as f32 / nseg as f32;
                let y = PAD_Y + usable_h - (k + 1) * seg_pitch;
                c.fill_rect(ox + b * pitch, y, BAR_W, SEG_H, Rgba::from_hex(t.lit_at(frac), 1.0));
            }
        }

        // 5. bloom - the LIT marks only.
        //
        // Radius and strength must be coupled: Canvas::bloom is a box blur and
        // normalises by kernel size, so a wider radius spreads the same energy over
        // more pixels and the halo gets FAINTER. Raising `bloom` alone therefore dims
        // the glow, the opposite of the intent. See bloom_strength().
        //
        // And this must happen BEFORE the dormant grid is drawn. Blooming the ghost
        // grid too drove it to full alpha at these gain levels, so the idle grid glowed
        // as brightly as a lit segment and the meter read as permanently full.
        let radius = t.bloom.round().max(0.0) as i32;
        c.bloom(radius, bloom_strength(t.bloom));

        // 6. dormant ghost grid - zone-coloured when zones are present. Drawn after
        // the bloom so it stays crisp and dim, and only in the UNLIT part of each bar
        // so it never tints a lit segment.
        if t.ghost > 0.0 {
            for b in 0..nbars {
                let lit = lit_of(b).min(nseg);
                for k in lit..nseg {
                    let frac = (k + 1) as f32 / nseg as f32;
                    let col = Rgba::from_hex(t.lit_at(frac), t.ghost);
                    let y = PAD_Y + usable_h - (k + 1) * seg_pitch;
                    c.fill_rect(ox + b * pitch, y, BAR_W, SEG_H, col);
                }
            }
        }

        // 7. hot core: a narrower brighter rect, not a gradient
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

        // 8. punch the segment gaps back out
        for k in 1..=nseg {
            c.punch_row(PAD_Y + usable_h - k * seg_pitch + SEG_H, SEG_GAP);
        }

        // 9. peak-hold caps
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

        // 10. bezel
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

    /// Perceived brightness. Use this rather than the alpha channel: the panel is
    /// ~0.96 opaque, so alpha is near-saturated everywhere on it and an alpha
    /// assertion silently measures the panel instead of the mark.
    fn lum(p: crate::render::canvas::Rgba) -> f32 {
        0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32
    }

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
        assert!(lum(top) > 150.0, "top segment should be lit at full level");
    }

    #[test]
    fn half_level_lights_the_bottom_but_not_the_top() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
        let bottom = c.get(first_bar_x() + 2, 60 - PAD_Y - 2);
        let top = c.get(first_bar_x() + 2, PAD_Y + 1);
        // Assert on BRIGHTNESS, not alpha. The panel is 0.96 opaque, so every pixel
        // on it has alpha ~245 whether or not a segment is lit - an alpha-based
        // assertion here passes on the panel alone and tests nothing.
        assert!(lum(bottom) > 120.0, "bottom must be lit, got lum {}", lum(bottom));
        assert!(
            lum(top) < lum(bottom) * 0.5,
            "top must be clearly dimmer than bottom at half level (top {}, bottom {})",
            lum(top),
            lum(bottom)
        );
    }

    #[test]
    fn segment_gaps_are_punched_through_to_transparent() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
        // Walk a lit column and confirm the luminance is not monotonic - the
        // gaps must interrupt it. This is what distinguishes a segmented meter
        // from a solid bar.
        // Must sample inside a real bar. The geometry CENTRES the bars, so the
        // first one starts at first_bar_x() (8 at 190px wide), not at PAD_X - and
        // PAD_X + 2 lands in the left margin, where the column is uniform and this
        // test silently measures nothing.
        let x = first_bar_x() + 2;
        let mut transitions = 0;
        let mut prev = lum(c.get(x, PAD_Y)) > 90.0;
        for y in PAD_Y..(60 - PAD_Y) {
            let now = lum(c.get(x, y)) > 90.0;
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
