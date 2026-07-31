#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba { r: 0, g: 0, b: 0, a: 0 };

    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Rgba { r, g, b, a }
    }

    /// Parses "#RRGGBB" (leading '#' optional) and applies `alpha` in 0.0..=1.0.
    /// Returns TRANSPARENT on malformed input rather than panicking - theme files
    /// are user-authored and must never crash the app.
    pub fn from_hex(hex: &str, alpha: f32) -> Self {
        let h = hex.trim_start_matches('#');
        // Require ASCII before trusting byte-offset slicing below: len() is a
        // byte count, and a non-ASCII (multi-byte UTF-8) string could have
        // exactly 6 bytes while landing the h[i..i+2] slices off a char
        // boundary, which panics rather than falling through to TRANSPARENT.
        if !h.is_ascii() || h.len() != 6 {
            return Rgba::TRANSPARENT;
        }
        let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        match (p(0), p(2), p(4)) {
            (Some(r), Some(g), Some(b)) => {
                Rgba::new(r, g, b, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
            }
            _ => Rgba::TRANSPARENT,
        }
    }
}

pub struct Canvas {
    w: i32,
    h: i32,
    px: Vec<u32>,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        Canvas { w, h, px: vec![0u32; (w.max(0) * h.max(0)) as usize] }
    }

    pub fn width(&self) -> i32 {
        self.w
    }
    pub fn height(&self) -> i32 {
        self.h
    }
    pub fn bits(&self) -> &[u32] {
        &self.px
    }

    pub fn clear(&mut self) {
        self.px.iter_mut().for_each(|p| *p = 0);
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            None
        } else {
            Some((y * self.w + x) as usize)
        }
    }

    /// Packs to premultiplied 0xAARRGGBB, which is BGRA in DIB memory order.
    fn pack(c: Rgba) -> u32 {
        let a = c.a as u32;
        let pm = |v: u8| ((v as u32 * a + 127) / 255) & 0xff;
        (a << 24) | (pm(c.r) << 16) | (pm(c.g) << 8) | pm(c.b)
    }

    fn unpack(p: u32) -> Rgba {
        let a = (p >> 24) as u32;
        if a == 0 {
            return Rgba::TRANSPARENT;
        }
        let un = |v: u32| ((v * 255 + a / 2) / a).min(255) as u8;
        Rgba::new(
            un((p >> 16) & 0xff),
            un((p >> 8) & 0xff),
            un(p & 0xff),
            a as u8,
        )
    }

    // Pixel read-back has no production consumer yet - `main`'s render loop
    // only ever writes a Canvas and hands it to the overlay. It is exercised
    // by the render test suite (including golden-file generation via
    // `render::golden::canvas_to_ascii`) and is the basis for any future
    // self-sampling verification, so it stays public rather than test-only.
    #[allow(dead_code)]
    pub fn get(&self, x: i32, y: i32) -> Rgba {
        match self.idx(x, y) {
            Some(i) => Self::unpack(self.px[i]),
            None => Rgba::TRANSPARENT,
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        if c.a == 0 {
            return;
        }
        let packed = Self::pack(c);
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in x.max(0)..(x + w).min(self.w) {
                let i = (yy * self.w + xx) as usize;
                self.px[i] = if c.a == 255 {
                    packed
                } else {
                    Self::blend_over(self.px[i], packed)
                };
            }
        }
    }

    /// Source-over on premultiplied values.
    fn blend_over(dst: u32, src: u32) -> u32 {
        let sa = src >> 24;
        if sa == 255 {
            return src;
        }
        let inv = 255 - sa;
        let ch = |sh: u32| {
            let s = (src >> sh) & 0xff;
            let d = (dst >> sh) & 0xff;
            (s + (d * inv + 127) / 255).min(255)
        };
        let a = (sa + (((dst >> 24) & 0xff) * inv + 127) / 255).min(255);
        (a << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
    }

    pub fn rounded_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: Rgba) {
        // Degrade gracefully on non-positive dimensions instead of handing
        // i32::clamp a negative upper bound: w.min(h) / 2 can be negative
        // (e.g. w=190, h=-4 -> -2), and clamp's min <= max assert is
        // unconditional (not debug-only), so it panics in release too.
        if w <= 0 || h <= 0 {
            return;
        }
        let r = r.max(0).min(w.min(h) / 2);
        for yy in 0..h {
            // Shrink the span near the top and bottom to round the corners.
            let dy = if yy < r {
                r - yy
            } else if yy >= h - r {
                yy - (h - r - 1)
            } else {
                0
            };
            let inset = if dy > 0 {
                let f = (r * r - dy * dy).max(0) as f32;
                r - f.sqrt().round() as i32
            } else {
                0
            };
            self.fill_rect(x + inset, y + yy, w - inset * 2, 1, c);
        }
    }

    pub fn punch_row(&mut self, y: i32, h: i32) {
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in 0..self.w {
                self.px[(yy * self.w + xx) as usize] = 0;
            }
        }
    }

    /// Separable box blur of the current contents, composited *under* the
    /// original so lit elements keep their crisp edge and gain a halo.
    pub fn bloom(&mut self, radius: i32, strength: f32) {
        if radius <= 0 || strength <= 0.0 {
            return;
        }
        let (w, h) = (self.w, self.h);
        let src = self.px.clone();
        let mut tmp = vec![0u32; src.len()];

        let blur = |input: &[u32], out: &mut [u32], horizontal: bool| {
            for y in 0..h {
                for x in 0..w {
                    let (mut a, mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
                    for d in -radius..=radius {
                        let (sx, sy) = if horizontal { (x + d, y) } else { (x, y + d) };
                        if sx < 0 || sy < 0 || sx >= w || sy >= h {
                            continue;
                        }
                        let p = input[(sy * w + sx) as usize];
                        a += p >> 24;
                        r += (p >> 16) & 0xff;
                        g += (p >> 8) & 0xff;
                        b += p & 0xff;
                        n += 1;
                    }
                    let n = n.max(1);
                    out[(y * w + x) as usize] =
                        ((a / n) << 24) | ((r / n) << 16) | ((g / n) << 8) | (b / n);
                }
            }
        };

        blur(&src, &mut tmp, true);
        let mut halo = vec![0u32; src.len()];
        blur(&tmp, &mut halo, false);

        for i in 0..self.px.len() {
            let hp = halo[i];
            let scale = |v: u32| ((v as f32 * strength).min(255.0)) as u32;
            let scaled = (scale(hp >> 24) << 24)
                | (scale((hp >> 16) & 0xff) << 16)
                | (scale((hp >> 8) & 0xff) << 8)
                | scale(hp & 0xff);
            self.px[i] = Self::blend_over(scaled, src[i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_canvas_is_fully_transparent() {
        let c = Canvas::new(190, 60);
        assert_eq!(c.width(), 190);
        assert_eq!(c.height(), 60);
        assert_eq!(c.bits().len(), 190 * 60);
        assert!(c.bits().iter().all(|&p| p == 0));
    }

    #[test]
    fn hex_parsing_handles_the_real_theme_colours() {
        assert_eq!(Rgba::from_hex("#8fe4ff", 1.0), Rgba::new(0x8f, 0xe4, 0xff, 255));
        assert_eq!(Rgba::from_hex("8fe4ff", 1.0), Rgba::new(0x8f, 0xe4, 0xff, 255));
        assert_eq!(Rgba::from_hex("#3ddc5a", 0.5), Rgba::new(0x3d, 0xdc, 0x5a, 128));
    }

    #[test]
    fn hex_parsing_never_panics_on_bad_input() {
        // Theme files are user-authored; malformed colour must degrade, not crash.
        for bad in [
            "",
            "#",
            "#12345",
            "#gggggg",
            "not a colour",
            "#1234567",
            // 6 bytes but only 5 chars ('a','ü','a','a','a') - byte offset 2
            // lands inside the 2-byte 'ü', which must not panic on the slice.
            "a\u{FC}aaa",
        ] {
            assert_eq!(Rgba::from_hex(bad, 1.0), Rgba::TRANSPARENT, "input {bad:?}");
        }
    }

    #[test]
    fn opaque_fill_round_trips_exactly() {
        let mut c = Canvas::new(10, 10);
        c.fill_rect(2, 3, 4, 5, Rgba::new(0x8f, 0xe4, 0xff, 255));
        assert_eq!(c.get(2, 3), Rgba::new(0x8f, 0xe4, 0xff, 255));
        assert_eq!(c.get(5, 7), Rgba::new(0x8f, 0xe4, 0xff, 255));
    }

    #[test]
    fn fill_respects_its_bounds() {
        let mut c = Canvas::new(10, 10);
        c.fill_rect(2, 3, 4, 5, Rgba::new(255, 255, 255, 255));
        assert_eq!(c.get(1, 3), Rgba::TRANSPARENT, "left of rect");
        assert_eq!(c.get(6, 3), Rgba::TRANSPARENT, "right of rect");
        assert_eq!(c.get(2, 2), Rgba::TRANSPARENT, "above rect");
        assert_eq!(c.get(2, 8), Rgba::TRANSPARENT, "below rect");
    }

    #[test]
    fn fill_clips_instead_of_panicking() {
        let mut c = Canvas::new(10, 10);
        c.fill_rect(-5, -5, 20, 20, Rgba::new(255, 0, 0, 255));
        assert_eq!(c.get(0, 0), Rgba::new(255, 0, 0, 255));
        assert_eq!(c.get(9, 9), Rgba::new(255, 0, 0, 255));
    }

    #[test]
    fn stored_pixels_are_premultiplied_bgra() {
        // UpdateLayeredWindow with AC_SRC_ALPHA demands premultiplied alpha.
        // White at 50% alpha must store as ~0x80808080, not 0x80FFFFFF.
        let mut c = Canvas::new(4, 4);
        c.fill_rect(0, 0, 4, 4, Rgba::new(255, 255, 255, 128));
        let p = c.bits()[0];
        let (a, r, g, b) = (p >> 24, (p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
        assert_eq!(a, 128, "alpha preserved");
        for (name, v) in [("r", r), ("g", g), ("b", b)] {
            assert!(
                (127..=129).contains(&v),
                "{name} must be premultiplied to ~128, got {v}"
            );
        }
    }

    #[test]
    fn punch_row_clears_a_full_width_band() {
        let mut c = Canvas::new(10, 10);
        c.fill_rect(0, 0, 10, 10, Rgba::new(255, 255, 255, 255));
        c.punch_row(4, 2);
        assert_eq!(c.get(0, 4), Rgba::TRANSPARENT);
        assert_eq!(c.get(9, 5), Rgba::TRANSPARENT);
        assert_eq!(c.get(0, 3), Rgba::new(255, 255, 255, 255), "row above survives");
        assert_eq!(c.get(0, 6), Rgba::new(255, 255, 255, 255), "row below survives");
    }

    #[test]
    fn rounded_rect_omits_its_corners() {
        let mut c = Canvas::new(20, 20);
        c.rounded_rect(0, 0, 20, 20, 5, Rgba::new(255, 255, 255, 255));
        assert_eq!(c.get(0, 0), Rgba::TRANSPARENT, "corner must be cut");
        assert_eq!(c.get(10, 10), Rgba::new(255, 255, 255, 255), "centre filled");
        assert_eq!(c.get(10, 0), Rgba::new(255, 255, 255, 255), "top edge filled");
    }

    #[test]
    fn rounded_rect_does_not_panic_on_negative_dimensions() {
        // w.min(h) / 2 can go negative (e.g. w=190, h=-4 -> -2), which must
        // not be handed to i32::clamp as an upper bound - clamp's min <= max
        // assert is unconditional and panics in release builds too.
        let mut c = Canvas::new(200, 60);
        c.rounded_rect(0, 0, 190, -4, 5, Rgba::new(255, 255, 255, 255));
        c.rounded_rect(0, 0, -4, 190, 5, Rgba::new(255, 255, 255, 255));
        c.rounded_rect(0, 0, 0, 0, 5, Rgba::new(255, 255, 255, 255));
        assert!(c.bits().iter().all(|&p| p == 0), "no pixels drawn");
    }

    #[test]
    fn bloom_spreads_light_outward_without_erasing_the_source() {
        let mut c = Canvas::new(21, 21);
        c.fill_rect(10, 10, 1, 1, Rgba::new(255, 255, 255, 255));
        c.bloom(3, 1.0);
        assert!(c.get(10, 10).a > 200, "source stays bright");
        assert!(c.get(12, 10).a > 0, "light spread sideways");
        assert_eq!(c.get(20, 20).a, 0, "but not across the whole canvas");
    }
}
