use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// ~300ms integration at 60fps. Slow on purpose: a fast needle looks broken.
pub const VU_SMOOTHING: f32 = 0.085;
const OVERLOAD_AT: f32 = 0.76;

#[derive(Default)]
pub struct Vu {
    l: f32,
    r: f32,
    pk_l: f32,
    pk_r: f32,
}

impl Family for Vu {
    fn id(&self) -> &'static str {
        "vu"
    }
    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // Ballistics live on the family, not the canvas, so a resize does not
        // reset the needles.
        self.l += (d.rms_l.clamp(0.0, 1.0) - self.l) * VU_SMOOTHING;
        self.r += (d.rms_r.clamp(0.0, 1.0) - self.r) * VU_SMOOTHING;
        self.pk_l = (self.pk_l - 0.004).max(self.l);
        self.pk_r = (self.pk_r - 0.004).max(self.r);

        c.clear();
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&t.panel, t.panel_alpha));

        // Warm backlight pooling from the bottom.
        for y in (h / 3)..(h - 4) {
            let f = (y - h / 3) as f32 / (h - 4 - h / 3).max(1) as f32;
            c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, 0.22 * f));
        }

        let ink = Rgba::from_hex(&t.lit, 0.72);
        let over = Rgba::from_hex(t.overload_hex(), 0.85);
        let dial_w = w / 2 - 3;

        // Arcs, ticks and needles are drawn onto their own transparent layer,
        // not straight onto `c`. `Canvas::bloom` composites its halo UNDER
        // whatever is already on the canvas, and by this point `c` is the
        // fully opaque panel (panel_alpha 1.0 on every dial theme) - blooming
        // `c` directly, as the brief's own code did, means every dial pixel
        // already has dst.a == 255, so `blend_over`'s `sa == 255` fast path
        // (or the near-255 accumulated alpha for partial-alpha ink) leaves
        // the halo invisible everywhere inside the panel. Building the ink on
        // its own layer and `draw_over`-ing the bloomed result onto the
        // opaque panel is the same fix already applied to `segmented.rs` and
        // `scope.rs` for exactly this reason.
        let mut dial = Canvas::new(w, h);

        for (idx, (level, peak)) in [(self.l, self.pk_l), (self.r, self.pk_r)].iter().enumerate() {
            let cx = 2 + idx as i32 * (w / 2) + dial_w / 2;
            let cy = h - 4;
            let radius = (dial_w as f32 * 0.60) as i32;
            let (a0, a1) = (-std::f32::consts::PI * 0.78, -std::f32::consts::PI * 0.22);

            // Printed arc, with the overload segment in its own colour.
            for step in 0..=60 {
                let f = step as f32 / 60.0;
                let ang = a0 + (a1 - a0) * f;
                let col = if f >= OVERLOAD_AT { over } else { ink };
                let px = cx + (ang.cos() * radius as f32) as i32;
                let py = cy + (ang.sin() * radius as f32) as i32;
                dial.fill_rect(px, py, 1, 1, col);
            }

            // Tick marks, longer at the ends and centre.
            for k in 0..=6 {
                let ang = a0 + (a1 - a0) * k as f32 / 6.0;
                let big = k == 0 || k == 3 || k == 6;
                let inner = radius - if big { 5 } else { 3 };
                for rr in inner..=radius {
                    let px = cx + (ang.cos() * rr as f32) as i32;
                    let py = cy + (ang.sin() * rr as f32) as i32;
                    dial.fill_rect(px, py, 1, 1, ink);
                }
            }

            // Ghost peak needle.
            let pang = a0 + (a1 - a0) * peak.clamp(0.0, 1.0);
            for rr in (radius / 3)..radius {
                let px = cx + (pang.cos() * rr as f32) as i32;
                let py = cy + (pang.sin() * rr as f32) as i32;
                dial.fill_rect(px, py, 1, 1, Rgba::from_hex("#ffffff", 0.32));
            }

            // Live needle, red past the overload point.
            let ang = a0 + (a1 - a0) * level.clamp(0.0, 1.0);
            let needle = if *level > OVERLOAD_AT {
                Rgba::from_hex(t.overload_hex(), 1.0)
            } else {
                Rgba::from_hex(&t.hot, 1.0)
            };
            for rr in 0..(radius as f32 * 0.95) as i32 {
                let px = cx + (ang.cos() * rr as f32) as i32;
                let py = cy + (ang.sin() * rr as f32) as i32;
                dial.fill_rect(px, py, 1, 1, needle);
            }
            dial.fill_rect(cx - 1, cy - 1, 2, 2, needle);
        }

        // `Canvas::bloom` keeps the original (crisp) pixel on top of the
        // blurred halo it composites underneath, so blooming `dial` in place
        // preserves the sharp arc/needle lines while adding the glow -
        // `draw_over` then blends that combined result onto the opaque panel.
        dial.bloom(t.bloom as i32, 0.7);
        c.draw_over(&dial);

        // The arc's own radius reaches y = cy - radius at its apex (angle
        // -90 deg), which on a 190x60 canvas is 1px above the panel's top
        // edge (row 2) - a real containment bug independent of bloom. Bloom
        // then spreads that leak (and any other near-edge ink) further past
        // the panel's rounded corners. Clip back to the same rect
        // `rounded_rect` drew above, exactly as `segmented.rs`/`scope.rs` do
        // after their own bloom step.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);

        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::golden::canvas_to_ascii;
    use crate::themes::builtin;

    fn level(l: f32, r: f32) -> FrameData {
        FrameData { rms_l: l, rms_r: r, ..FrameData::default() }
    }

    /// Every dial theme ships `panel_alpha: 1.0`, so `Canvas::blend_over` drives
    /// `.a` to 255 at every in-bounds pixel the instant the opaque panel is
    /// painted - regardless of whether any arc/needle ink was drawn on top.
    /// The brief's own version of this test summed `.a`, which is therefore
    /// vacuous: it would pass identically even with the dial-drawing loop
    /// deleted entirely. Luminance is what actually distinguishes lit ink
    /// from bare (panel + backlight) canvas - the same fix already applied to
    /// `segmented.rs` and `scope.rs`'s own alpha-vs-luminance traps.
    fn lum(p: Rgba) -> f32 {
        0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32
    }

    #[test]
    fn draws_two_dials_with_printed_arcs() {
        let mut c = Canvas::new(190, 60);
        Vu::default().draw(&mut c, &builtin::vu_cream(), &level(0.0, 0.0));
        // Both halves must contain ink clearly brighter than the bare
        // panel-plus-backlight gradient at this row (measured ~28 for
        // vu_cream; arc ink measures ~170).
        let left = (5..90).map(|x| lum(c.get(x, 30))).fold(0.0f32, f32::max);
        let right = (100..185).map(|x| lum(c.get(x, 30))).fold(0.0f32, f32::max);
        assert!(left > 80.0 && right > 80.0, "expected a dial in each half (left {left}, right {right})");
    }

    #[test]
    fn needles_move_slowly_toward_the_target() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        v.draw(&mut c, &builtin::vu_cream(), &level(1.0, 1.0));
        assert!(v.l < 0.2, "one frame must not swing the needle far, got {}", v.l);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(1.0, 1.0));
        }
        assert!(v.l > 0.9, "should converge after ~1.3s, got {}", v.l);
    }

    #[test]
    fn the_two_channels_are_independent() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.9, 0.1));
        }
        assert!(v.l > v.r + 0.5, "L {} and R {} must differ", v.l, v.r);
    }

    #[test]
    fn peak_needles_lag_behind_on_the_way_down() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.9, 0.9));
        }
        for _ in 0..10 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.0, 0.0));
        }
        assert!(v.pk_l > v.l, "peak needle must hold above the live needle");
    }

    #[test]
    fn the_red_dial_flips_its_overload_arc_to_white() {
        // Red-on-red would be invisible - the one colourway needing a behavioural change.
        assert_eq!(builtin::vu_red().overload_hex(), "#ffffff");
        assert_ne!(builtin::vu_cream().overload_hex(), "#ffffff");
    }

    #[test]
    fn state_survives_a_canvas_resize() {
        let mut v = Vu::default();
        let mut a = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut a, &builtin::vu_cream(), &level(0.8, 0.8));
        }
        let before = v.l;
        let mut b = Canvas::new(120, 60);
        v.draw(&mut b, &builtin::vu_cream(), &level(0.8, 0.8));
        assert!((v.l - before).abs() < 0.05, "needle position must not jump on resize");
    }

    #[test]
    fn bloom_halo_does_not_leak_outside_the_panel_bezel() {
        // The dial arc's own radius reaches its apex at y = cy - radius,
        // which on a 190x60 canvas lands at row 1 - one row above the
        // panel's top edge (row 2, see `rounded_rect(1, 2, w-2, h-4, 4, ...)`
        // in `draw`) - a real containment bug independent of bloom. Rows 0,
        // 1, 58 and 59 all sit outside the panel and must stay fully
        // transparent even at a driving level that pushes the needle toward
        // vertical (the steepest, highest-reaching angle).
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_red(), &level(1.0, 1.0));
        }
        for y in [0, 1, 58, 59] {
            for x in 0..190 {
                let p = c.get(x, y);
                assert_eq!(p.a, 0, "row {y} col {x} must stay blank outside the panel bezel, got {p:?}");
            }
        }
    }

    #[test]
    fn golden_vu_cream() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.65, 0.4));
        }
        let expected = include_str!("../../tests/golden/vu-cream.txt");
        // `matches_golden` normalises line endings - a bare `assert_eq!` on the
        // string broke every golden test once already on a branch merge where
        // git checked the committed file out as CRLF while `canvas_to_ascii`
        // emits LF, with byte-identical content otherwise.
        assert!(
            crate::render::golden::matches_golden(&c, expected),
            "golden mismatch - if this change is intended, overwrite \
             tests/golden/vu-cream.txt and eyeball the diff:\n{}",
            canvas_to_ascii(&c)
        );
    }

    #[test]
    #[ignore]
    fn regenerate_golden() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.65, 0.4));
        }
        std::fs::write("tests/golden/vu-cream.txt", canvas_to_ascii(&c)).unwrap();
    }
}
