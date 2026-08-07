//! The track-name banner: a transient readout that appears when the track changes.
//!
//! THE REASON THIS NEEDS NO PER-THEME TUNING is that it is not part of any theme. A permanent readout
//! would have to find room in thirteen families with completely different visual languages - segments,
//! a scope trace, VU dials, a liquid tank - and would need placing and styling in each. A TRANSIENT one
//! does not: it owns the panel for about two seconds and then gives it back, so it never has to
//! coexist with anything.
//!
//! What it does take from the theme is its COLOURS - `lit`, `hot`, `panel` and `edge`, which all 88
//! colourways already define - so it comes out cyan on VFD ice, green on P1 and yellow on Pantone
//! without a line of per-theme code. `tint` is used rather than the raw hex, so a rainbow or ink
//! colourway drives the banner too.
//!
//! IT DIMS RATHER THAN HIDES, and it dims by blending toward the panel colour rather than by reducing
//! alpha. That is not a style choice: alpha in this app is what the taskbar shows through, so fading
//! the meter out would let the weather widget bleed into the banner. It is the same hazard that
//! `punch_rect` created in the segmented family, and the same rule applies - anything that looks like
//! a gap must be PAINTED, never punched.

use super::canvas::{Canvas, Rgba};
use super::text::{render as render_text, TextMask};
use super::tint;
use crate::themes::Theme;

/// Rise, hold and fall, in milliseconds.
///
/// The rise is short enough to read as a pop rather than a fade - that was the brief - and the fall is
/// slower so the meter comes back without a jolt. The hold is what a title actually needs to be read
/// at a glance; much less and a long title cannot finish scrolling, much more and it stops feeling
/// like a notification.
const RISE_MS: f32 = 140.0;
const HOLD_MS: f32 = 2200.0;
const FALL_MS: f32 = 520.0;

/// How far the meter is pushed toward the panel colour at full banner opacity.
///
/// Not 1.0 on purpose. The meter staying faintly visible behind the text is what stops the banner
/// feeling like the app has been replaced by a dialog for two seconds. Reviewed at 0.78, which left
/// the meter almost invisible on the darker colourways; 0.66 keeps it reading as the same app.
const DIM: f32 = 0.66;

/// Text height as a fraction of the panel interior.
///
/// Reviewed by eye twice, and the second pass corrected a mistake in the first: the sizes were being
/// judged against a 190px panel when the actual display is configured to 380. Measured widths for real
/// titles ("Encore Une Fois - Original Edit" is the worst of the current playlist) against the 372px a
/// 380 panel leaves:
///
/// | size  | worst title | fits 182px | fits 372px |
/// |-------|-------------|------------|------------|
/// | 13px  | 212px       | no         | yes        |
/// | 19px  | 291px       | no         | yes, 81px spare |
/// | 24px  | 359px       | no         | only just |
///
/// 19px is as large as it can be while still leaving room for a title half again as long as anything
/// in that sample. Nothing fits 182px at any readable size, which is fine - that is what the marquee
/// is for, and a narrow panel is the case where scrolling is unavoidable rather than fidgety.
const TEXT_FRACTION: f32 = 0.34;

pub struct Banner {
    mask: TextMask,
    /// Milliseconds since it appeared.
    age: f32,
}

impl Banner {
    /// Rasterises `text` for a panel of this interior height. `None` if there is nothing to show.
    pub fn new(text: &str, interior_h: i32) -> Option<Banner> {
        let px = ((interior_h as f32 * TEXT_FRACTION).round() as i32).clamp(9, 22);
        let mask = render_text(text, px)?;
        Some(Banner { mask, age: 0.0 })
    }

    /// Advances time. Returns false once it has finished and should be dropped.
    pub fn advance(&mut self, dt_ms: f32) -> bool {
        let dt = if dt_ms.is_finite() { dt_ms.clamp(0.0, 250.0) } else { 16.0 };
        self.age += dt;
        self.age < RISE_MS + HOLD_MS + FALL_MS
    }

    /// 0..1 envelope.
    fn opacity(&self) -> f32 {
        if self.age < RISE_MS {
            let t = (self.age / RISE_MS).clamp(0.0, 1.0);
            // Smoothstep in, so it arrives without a hard edge on the first frame.
            t * t * (3.0 - 2.0 * t)
        } else if self.age < RISE_MS + HOLD_MS {
            1.0
        } else {
            let t = ((self.age - RISE_MS - HOLD_MS) / FALL_MS).clamp(0.0, 1.0);
            1.0 - t * t
        }
    }

    /// Draws over whatever the family already put down.
    pub fn draw(&self, c: &mut Canvas, theme: &Theme, time_s: f32) {
        let (w, h) = (c.width(), c.height());
        let (ix, iy, iw, ih) = (1, 2, w - 2, h - 4);
        if iw < 8 || ih < 8 {
            return;
        }
        let op = self.opacity();
        if op <= 0.004 {
            return;
        }

        // ---- dim the meter, by blending toward the panel colour ---------------------------------
        // NOT by scaling alpha: see the module note. Every pixel written here stays fully opaque.
        let panel = Rgba::from_hex(&theme.panel, 1.0);
        let k = DIM * op;
        for y in iy..(iy + ih) {
            for x in ix..(ix + iw) {
                let p = c.get(x, y);
                if p.a == 0 {
                    continue;
                }
                let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round() as u8;
                c.fill_rect(
                    x,
                    y,
                    1,
                    1,
                    Rgba::new(mix(p.r, panel.r), mix(p.g, panel.g), mix(p.b, panel.b), p.a),
                );
            }
        }

        // ---- the text ---------------------------------------------------------------------------
        // On its own layer so it can be bloomed like every other light in this app, then drawn over.
        let mut lit = Canvas::new(w, h);
        let colour = tint(theme, 0.5, time_s, false, &theme.lit, 1.0);
        let top = iy + (ih - self.mask.h) / 2;

        // Scroll only if it does not fit. A title that fits is centred and still, because a marquee on
        // a short title is fidgety for no reason.
        let pad = 3;
        let avail = iw - pad * 2;
        let left = if self.mask.w <= avail {
            ix + (iw - self.mask.w) / 2
        } else {
            // Travels during the hold only, so it is stationary while fading in and out - a title
            // moving as it appears is much harder to catch than one that is already there.
            let over = (self.mask.w - avail) as f32;
            let t = ((self.age - RISE_MS) / HOLD_MS).clamp(0.0, 1.0);
            // Eased, and it lingers at both ends, so the beginning and the end of a long title can
            // both actually be read.
            let e = (t * 1.35 - 0.175).clamp(0.0, 1.0);
            let e = e * e * (3.0 - 2.0 * e);
            ix + pad - (over * e).round() as i32
        };

        for my in 0..self.mask.h {
            let y = top + my;
            if y < iy || y >= iy + ih {
                continue;
            }
            for mx in 0..self.mask.w {
                let x = left + mx;
                if x < ix + pad || x >= ix + iw - pad {
                    continue;
                }
                let cov = self.mask.coverage_at(mx, my) as f32 / 255.0;
                if cov <= 0.01 {
                    continue;
                }
                let a = (cov * op * 255.0).clamp(0.0, 255.0) as u8;
                lit.fill_rect(x, y, 1, 1, Rgba::new(colour.r, colour.g, colour.b, a));
            }
        }

        if theme.bloom > 0.0 {
            let mut glow = lit.clone();
            glow.bloom(theme.bloom as i32, theme.glow_strength.clamp(0.0, 1.0) * 0.7);
            c.draw_over(&glow);
        }
        c.draw_over(&lit);
        // After compositing, exactly as every family does - a halo must not escape the rounded corner.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn lum(p: Rgba) -> f32 {
        0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32
    }

/// Renders the banner over a spread of families so it can be judged by eye.
    /// Run: cargo test --release dump_banner -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_banner() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let picks = [
            "vfd-ice", "p1-green", "vu-copper", "vapor-b", "tube-soviet", "fluid-deep",
            "fluid-pantone", "radar-p1", "pantone-barcode", "nixie-orange",
        ];
        let all = builtin::all();
        let mut n = 0;
        for id in picks {
            let Some(theme) = all.iter().find(|t| t.id == id) else { continue };
            for (tag, age, pw) in [
                ("off", 0.0f32, 380),
                ("mid", RISE_MS + 400.0, 380),
                ("late", RISE_MS + HOLD_MS - 200.0, 380),
                ("narrow", RISE_MS + 400.0, 190),
            ] {
                let mut c = Canvas::new(pw, 60);
                let mut fam = crate::render::family_for(&theme.family);
                // A few frames so the family has something on screen, not a cold first frame.
                for k in 0..90 {
                    fam.draw(
                        &mut c,
                        theme,
                        &crate::render::FrameData { time_s: k as f32 * 0.016, ..Default::default() },
                    );
                }
                if age > 0.0 {
                    let mut b = Banner::new("Hot Dog - Limp Bizkit", 56).expect("banner");
                    while b.age < age {
                        let _ = b.advance(16.0);
                    }
                    b.draw(&mut c, theme, 1.4);
                }
                let mut out = Vec::with_capacity((pw * 60 * 4) as usize);
                for y in 0..60 {
                    for x in 0..pw {
                        let p = c.get(x, y);
                        let a = p.a as f32 / 255.0;
                        for ch in [p.r, p.g, p.b] {
                            out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                        }
                        out.push(255);
                    }
                }
                std::fs::write(dir.join(format!("banner-{id}-{tag}.rgba")), &out).unwrap();
                n += 1;
            }
        }
        println!("wrote {n} banner dumps to {}", dir.display());
    }

        #[test]
    fn the_envelope_rises_holds_and_falls_and_then_reports_it_is_done() {
        let mut b = Banner::new("Hot Dog", 56).expect("banner");
        assert!(b.opacity() < 0.2, "it must start dark, not snap on: {}", b.opacity());
        // Through the rise.
        while b.age < RISE_MS {
            assert!(b.advance(16.0));
        }
        assert!(b.opacity() > 0.9, "not fully up after the rise: {}", b.opacity());
        // Through the hold.
        while b.age < RISE_MS + HOLD_MS - 20.0 {
            assert!(b.advance(16.0));
            assert!(b.opacity() > 0.9, "dipped during the hold: {}", b.opacity());
        }
        // And it finishes rather than living for ever.
        let mut guard = 0;
        while b.advance(16.0) {
            guard += 1;
            assert!(guard < 1000, "the banner never finished");
        }
        assert!(guard > 10, "it finished suspiciously fast");
    }

    #[test]
    fn a_pathological_frame_interval_cannot_skip_the_whole_banner() {
        // The render loop can be handed a huge dt after a stall, and a banner that ages 3 seconds in
        // one frame would never be seen at all.
        let mut b = Banner::new("Hot Dog", 56).expect("banner");
        assert!(b.advance(100_000.0), "one bad frame consumed the entire banner");
        assert!(b.advance(f32::NAN), "a NaN interval killed it");
        assert!(b.age < RISE_MS + HOLD_MS, "aged {}ms from two frames", b.age);
    }

    #[test]
    fn it_writes_no_transparent_pixel_inside_the_panel() {
        // The rule this app has been bitten by: alpha is what the taskbar's weather widget shows
        // through, so anything that looks like a gap must be PAINTED. Dimming by scaling alpha would
        // break it, which is precisely why the dim blends toward the panel colour instead.
        for theme in builtin::all().into_iter().take(24) {
            let mut c = Canvas::new(190, 60);
            let mut fam = crate::render::family_for(&theme.family);
            fam.draw(&mut c, &theme, &crate::render::FrameData::default());
            let mut b = Banner::new("Encore Une Fois - Original Edit", 56).expect("banner");
            let _ = b.advance(200.0);
            b.draw(&mut c, &theme, 1.0);
            for y in 4..56 {
                for x in 4..186 {
                    assert_ne!(
                        c.get(x, y).a,
                        0,
                        "{} left a transparent pixel at ({x},{y}) under the banner",
                        theme.id
                    );
                }
            }
        }
    }

    #[test]
    fn the_text_is_actually_brighter_than_the_dimmed_meter_behind_it() {
        // A banner nobody can read is worse than none. Compares the brightest pixel on the text's own
        // rows against the same rows with no banner.
        let theme = builtin::vfd_ice();
        let mut plain = Canvas::new(190, 60);
        let mut fam = crate::render::family_for(&theme.family);
        let d = crate::render::FrameData::default();
        fam.draw(&mut plain, &theme, &d);

        let mut with = Canvas::new(190, 60);
        let mut fam2 = crate::render::family_for(&theme.family);
        fam2.draw(&mut with, &theme, &d);
        let mut b = Banner::new("Hot Dog", 56).expect("banner");
        let _ = b.advance(200.0);
        b.draw(&mut with, &theme, 1.0);

        let brightest = |c: &Canvas| -> f32 {
            let mut m = 0.0f32;
            for y in 20..40 {
                for x in 4..186 {
                    m = m.max(lum(c.get(x, y)));
                }
            }
            m
        };
        let (a, bb) = (brightest(&plain), brightest(&with));
        assert!(bb > a * 0.9 + 30.0, "banner text peaked at {bb:.0} against a bare meter at {a:.0}");
    }

    #[test]
    fn a_long_title_scrolls_and_a_short_one_does_not() {
        // Two properties in one: a title that fits must not fidget, and one that does not fit must
        // actually move, or its end can never be read.
        let theme = builtin::vfd_ice();
        // Stepped in real frames, not one giant `advance`. `advance` clamps dt to 250ms - deliberately,
        // and `a_pathological_frame_interval_cannot_skip_the_whole_banner` asserts it - so handing it
        // the whole age at once silently left both samples at 250ms and made this test compare a
        // banner with itself.
        let sample = |text: &str, age: f32| -> Vec<u8> {
            let mut c = Canvas::new(190, 60);
            let mut b = Banner::new(text, 56).expect("banner");
            while b.age < age {
                let _ = b.advance(16.0);
            }
            b.draw(&mut c, &theme, 1.0);
            (4..186).map(|x| c.get(x, 30).r).collect()
        };
        let short_a = sample("Hi", RISE_MS + 100.0);
        let short_b = sample("Hi", RISE_MS + HOLD_MS - 100.0);
        assert_eq!(short_a, short_b, "a short title moved during the hold");

        let long_a = sample("Encore Une Fois - Original Edit (Extended)", RISE_MS + 60.0);
        let long_b = sample("Encore Une Fois - Original Edit (Extended)", RISE_MS + HOLD_MS - 60.0);
        assert_ne!(long_a, long_b, "a long title never scrolled, so its end cannot be read");
    }

    #[test]
    fn it_renders_at_every_plausible_panel_size_including_degenerate_ones() {
        let theme = builtin::fluid_deep();
        for (w, h) in [(190, 60), (380, 60), (120, 40), (60, 30), (12, 10), (1, 1)] {
            let mut c = Canvas::new(w, h);
            if let Some(mut b) = Banner::new("Some Track Name", (h - 4).max(1)) {
                let _ = b.advance(200.0);
                b.draw(&mut c, &theme, 1.0);
            }
        }
    }
}
