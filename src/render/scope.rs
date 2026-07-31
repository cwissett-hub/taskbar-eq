use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

#[derive(Default)]
pub struct Scope {
    trace: Option<Canvas>,
    trail: Option<Canvas>,
}

impl Scope {
    /// Draws the waveform polyline into `buf` after decaying what was there.
    ///
    /// Deliberately does NOT bloom `buf` itself. `Canvas::bloom` composites its
    /// halo *underneath* whatever in the buffer isn't already fully opaque, so
    /// calling it every frame directly on the persisted buffer keeps adding
    /// another layer of halo into the still-translucent areas on top of
    /// whatever halo the previous frame already baked in - measured making the
    /// summed luminance at a fixed row GROW frame over frame (4499 -> 5701 over
    /// one decay step) instead of decay. Blooming happens once, on a disposable
    /// clone, at compose time in `draw` - see there.
    fn stroke_into(buf: &mut Canvas, d: &FrameData, colour: Rgba, fade: f32) {
        // Decay what is already there. Scaling alpha keeps the buffer transparent,
        // which is what lets the panel show through the trail.
        let decay = (1.0 - fade.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        let (w, h) = (buf.width(), buf.height());
        for y in 0..h {
            for x in 0..w {
                let p = buf.get(x, y);
                if p.a == 0 {
                    continue;
                }
                let a = (p.a as f32 * decay) as u8;
                // `fill_rect` always blends source-over onto whatever is already
                // there (see Canvas::blend_over), and it no-ops entirely when the
                // source alpha is 0 (Canvas::fill_rect's own early return). So
                // writing the decayed colour straight over the still-fully-opaque
                // old pixel blends toward the OLD (undecayed) value instead of
                // replacing it - alpha barely moves and the trace never fades.
                // `punch_rect` writes zero unconditionally, so clearing first and
                // then filling the decayed alpha onto a genuinely transparent
                // pixel makes the write an overwrite, not a blend.
                buf.punch_rect(x, y, 1, 1);
                if a > 2 {
                    buf.fill_rect(x, y, 1, 1, Rgba::new(p.r, p.g, p.b, a));
                }
            }
        }

        // Stroke the new trace: one vertical span per column, joining consecutive
        // samples so a steep slope stays continuous instead of dotting.
        let mid = h / 2;
        let amp = (h as f32 * 0.38) as i32;
        let x0 = 5;
        let span = (w - 10).max(1);
        let mut prev_y: Option<i32> = None;
        for px in 0..span {
            let i = (px as usize * 255) / span.max(1) as usize;
            let y = mid - (d.waveform[i.min(255)] * amp as f32) as i32;
            let y = y.clamp(0, h - 1);
            let (lo, hi) = match prev_y {
                Some(p) if p < y => (p, y),
                Some(p) => (y, p),
                None => (y, y),
            };
            buf.fill_rect(x0 + px, lo, 1, (hi - lo + 1).max(1), colour);
            prev_y = Some(y);
        }
    }
}

impl Family for Scope {
    fn id(&self) -> &'static str {
        "scope"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // Reallocate the persistence buffers when the widget rect changes width.
        let stale = self
            .trace
            .as_ref()
            .map(|b| b.width() != w || b.height() != h)
            .unwrap_or(true);
        if stale {
            self.trace = Some(Canvas::new(w, h));
            self.trail = None;
        }
        if t.dual.is_some() && self.trail.is_none() {
            self.trail = Some(Canvas::new(w, h));
        } else if t.dual.is_none() {
            self.trail = None;
        }

        // Slow trail first (drawn underneath), then the fast trace.
        if let (Some((trail_hex, trail_fade)), Some(trail)) = (t.dual.clone(), self.trail.as_mut()) {
            Self::stroke_into(trail, d, Rgba::from_hex(&trail_hex, 1.0), trail_fade);
        }
        if let Some(trace) = self.trace.as_mut() {
            Self::stroke_into(trace, d, Rgba::from_hex(&t.lit, 1.0), t.fade);
        }

        // Compose: panel, graticule, trail, trace, bezel.
        c.clear();
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&t.panel, t.panel_alpha));

        let grid = Rgba::from_hex(&t.lit, 0.10);
        for k in 1..8 {
            c.fill_rect(1 + (w - 2) * k / 8, 2, 1, h - 4, grid);
        }
        for k in 1..4 {
            c.fill_rect(1, 2 + (h - 4) * k / 4, w - 2, 1, grid);
        }
        c.fill_rect(1, h / 2, w - 2, 1, Rgba::from_hex(&t.lit, 0.20));

        // Trail then trace. Each is bloomed on a disposable clone - never the
        // persisted buffer itself, or the halo would compound frame over frame
        // (see `stroke_into`) - then composited over the already-opaque panel
        // via `draw_over`; blooming in place on `c` would hide the halo
        // entirely once it sits under the opaque panel fill drawn above.
        if let Some(trail) = self.trail.as_ref() {
            let mut glow = trail.clone();
            glow.bloom((t.bloom * 0.8) as i32, 0.9);
            c.draw_over(&glow);
        }
        if let Some(trace) = self.trace.as_ref() {
            let mut glow = trace.clone();
            glow.bloom(t.bloom as i32, 0.9);
            c.draw_over(&glow);
        }

        // The panel above is only inset 1-2px from the canvas edge, but
        // `Canvas::bloom` only clips at the canvas boundary, not the panel's,
        // so the halo just composited via `draw_over` can spread past that
        // thin margin onto the bare transparent/acrylic background outside
        // the rounded "screen" - exactly the bug already fixed once for
        // `segmented.rs` (see its own step 7 and `fix-bloom-containment-report.md`).
        // Must run after the trail/trace composite and before the edge bezel
        // below, mirroring segmented.rs's fix.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);

        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::bands::NUM_BANDS;
    use crate::render::golden::canvas_to_ascii;
    use crate::themes::builtin;

    fn wave(amp: f32) -> FrameData {
        let mut d = FrameData::default();
        for i in 0..256 {
            d.waveform[i] = amp * ((i as f32 / 256.0) * std::f32::consts::TAU * 2.0).sin();
        }
        d.levels = [0.0; NUM_BANDS];
        d
    }

    /// The panel is fully opaque (`panel_alpha: 1.0`), so `.a` is 255 at every
    /// in-bounds pixel regardless of what is drawn on top of the panel -
    /// asserting on `.a` alone is vacuous here. Luminance is what actually
    /// distinguishes a lit trace/halo pixel from bare dark panel.
    fn lum(p: Rgba) -> f32 {
        (0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32) * (p.a as f32 / 255.0)
    }

    #[test]
    fn draws_a_graticule_even_with_no_signal() {
        let mut c = Canvas::new(190, 60);
        Scope::default().draw(&mut c, &builtin::p1_green(), &wave(0.0));
        let ascii = canvas_to_ascii(&c);
        assert!(ascii.contains('.') || ascii.contains(':'), "graticule should be faintly visible");
    }

    #[test]
    fn a_flat_signal_traces_the_centre_line() {
        let mut c = Canvas::new(190, 60);
        Scope::default().draw(&mut c, &builtin::p1_green(), &wave(0.0));
        let mid = lum(c.get(95, 30));
        let top = lum(c.get(95, 8));
        assert!(mid > top, "flat trace must sit on the centre line (mid lum {mid}, top lum {top})");
    }

    #[test]
    fn a_large_signal_reaches_away_from_the_centre() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p1_green(), &wave(1.0));
        // Somewhere in the upper third must be lit by the excursion.
        let lit_high = (0..190).any(|x| lum(c.get(x, 12)) > 100.0);
        assert!(lit_high, "full-scale wave should reach the upper third");
    }

    #[test]
    fn persistence_accumulates_across_frames() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);

        // Frame 1: signal high. Frame 2: flat. The old trace must still be faintly there.
        s.draw(&mut c, &builtin::p1_green(), &wave(1.0));
        let lit_after_one: f32 = (0..190).map(|x| lum(c.get(x, 12))).sum();

        s.draw(&mut c, &builtin::p1_green(), &wave(0.0));
        let lit_after_two: f32 = (0..190).map(|x| lum(c.get(x, 12))).sum();

        // The dark panel and its graticule contribute non-zero luminance at row
        // 12 even with no signal ever drawn (the panel isn't pure black, and a
        // vertical grid line crosses every row). So ">0" against that baseline
        // alone would pass even if persistence were totally broken - it has to
        // be compared against what a scope with NO history at all reads here.
        let mut fresh = Canvas::new(190, 60);
        Scope::default().draw(&mut fresh, &builtin::p1_green(), &wave(0.0));
        let baseline: f32 = (0..190).map(|x| lum(fresh.get(x, 12))).sum();

        assert!(
            lit_after_two > baseline,
            "the previous trace must persist above the bare-panel baseline ({lit_after_two} vs baseline {baseline})"
        );
        assert!(lit_after_two < lit_after_one, "but it must be decaying");
    }

    #[test]
    fn a_high_fade_decays_faster_than_a_low_one() {
        let fast = builtin::p11_blue_violet(); // fade 0.20
        let slow = builtin::scope_amber();     // fade 0.11
        assert!(fast.fade > slow.fade, "test premise: p11 fades faster than amber");

        let residue = |t: &Theme| {
            let mut s = Scope::default();
            let mut c = Canvas::new(190, 60);
            s.draw(&mut c, t, &wave(1.0));
            for _ in 0..8 {
                s.draw(&mut c, t, &wave(0.0));
            }
            (0..190).map(|x| lum(c.get(x, 12))).sum::<f32>()
        };
        assert!(residue(&fast) < residue(&slow), "higher fade must leave less residue");
    }

    #[test]
    fn p7_uses_two_buffers_and_the_others_use_one() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p7_dual(), &wave(0.5));
        assert!(s.trail.is_some(), "P7 must allocate a second buffer for its trail");

        let mut s2 = Scope::default();
        s2.draw(&mut c, &builtin::p1_green(), &wave(0.5));
        assert!(s2.trail.is_none(), "single-layer phosphors must not allocate a trail");
    }

    #[test]
    fn bloom_halo_does_not_leak_outside_the_panel_bezel() {
        // The panel is `rounded_rect(1, 2, w-2, h-4, 4, ...)`, so its row range
        // is 2..=57 on a 60px-tall canvas; rows 0-1 and 58-59 sit outside it,
        // on the bare transparent background. A high bloom radius (p7-dual's
        // 8) composited with a full-amplitude wave without a clip-back spreads
        // a visible halo into those rows - the same containment bug fixed once
        // already for `segmented.rs`. Assert those rows stay fully transparent.
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p7_dual(), &wave(1.0));
        for y in [0, 1, 58, 59] {
            for x in 0..190 {
                let p = c.get(x, y);
                assert_eq!(
                    p.a, 0,
                    "row {y} col {x} must stay blank outside the panel bezel, got {p:?}"
                );
            }
        }
    }

    #[test]
    fn p7_trail_colour_is_distinct_from_its_trace() {
        // The task requires the trail to be "genuinely distinct" from the
        // trace, not just present as a second buffer. Read the private
        // buffers directly (this test module already does so via
        // `p7_uses_two_buffers_and_the_others_use_one`) and check hue, not
        // just that both are lit.
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p7_dual(), &wave(1.0));

        let trace = s.trace.as_ref().expect("trace buffer must exist");
        let trail = s.trail.as_ref().expect("P7 must allocate a trail buffer");

        let lit = |buf: &Canvas| -> Option<Rgba> {
            (0..buf.height())
                .flat_map(|y| (0..buf.width()).map(move |x| (x, y)))
                .map(|(x, y)| buf.get(x, y))
                .find(|p| p.a > 40)
        };

        let trace_px = lit(trace).expect("trace must have a lit pixel");
        let trail_px = lit(trail).expect("trail must have a lit pixel");

        // trace is #e8f4ff (blue-white: b >= g >= r); trail is #cfe86a
        // (yellow-green: g > b > r-ish, definitely g > b). These are cheap,
        // hue-shape checks rather than exact-value checks, so they survive
        // small colour tweaks while still catching "trail is just the trace
        // colour again" (which would fail the trail's `g > b` check).
        assert!(
            trace_px.b >= trace_px.g && trace_px.g >= trace_px.r,
            "trace pixel should read blue-white, got {trace_px:?}"
        );
        assert!(
            trail_px.g > trail_px.b,
            "trail pixel should read yellow-green (g > b), got {trail_px:?}"
        );
        assert_ne!(
            (trace_px.r, trace_px.g, trace_px.b),
            (trail_px.r, trail_px.g, trail_px.b),
            "trail must be a genuinely different colour from the trace"
        );
    }

    #[test]
    fn resizing_the_canvas_does_not_panic() {
        // The widget rect changes width; buffers must be reallocated, not indexed stale.
        let mut s = Scope::default();
        let mut a = Canvas::new(190, 60);
        s.draw(&mut a, &builtin::p1_green(), &wave(0.5));
        let mut b = Canvas::new(120, 60);
        s.draw(&mut b, &builtin::p1_green(), &wave(0.5));
        assert_eq!(b.bits().len(), 120 * 60);
    }

    #[test]
    fn golden_p1_green() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p1_green(), &wave(0.7));
        let expected = include_str!("../../tests/golden/p1-green.txt");
        // `matches_golden` normalises line endings - a bare `assert_eq!` on the
        // string broke every golden test once already on a branch merge where
        // git checked the committed file out as CRLF while `canvas_to_ascii`
        // emits LF, with byte-identical content otherwise.
        assert!(
            crate::render::golden::matches_golden(&c, expected),
            "golden mismatch - if this change is intended, overwrite \
             tests/golden/p1-green.txt and eyeball the diff:\n{}",
            canvas_to_ascii(&c)
        );
    }

    #[test]
    #[ignore]
    fn regenerate_golden() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p1_green(), &wave(0.7));
        std::fs::write("tests/golden/p1-green.txt", canvas_to_ascii(&c)).unwrap();
    }
}
