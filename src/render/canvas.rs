#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba { r: 0, g: 0, b: 0, a: 0 };

    /// Builds a colour from hue/saturation/value, hue in turns (0..1, wrapping).
    ///
    /// Needed because a rainbow colourway's colour cannot be a hex string in a theme file - it
    /// changes every frame and varies across the display - so it has to be computed. Hue in turns
    /// rather than degrees so a phase can wrap with `fract()` without a conversion either side.
    pub fn from_hsv(hue_turns: f32, sat: f32, val: f32, alpha: f32) -> Rgba {
        let h = if hue_turns.is_finite() { hue_turns.rem_euclid(1.0) } else { 0.0 } * 6.0;
        let s = sat.clamp(0.0, 1.0);
        let v = val.clamp(0.0, 1.0);
        let c = v * s;
        let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
        let m = v - c;
        let (r, g, b) = match h as i32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        Rgba::new(
            ((r + m) * 255.0).round() as u8,
            ((g + m) * 255.0).round() as u8,
            ((b + m) * 255.0).round() as u8,
            (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        )
    }

    /// Builds a colour from OKLCh - perceptual lightness, chroma, and hue in turns.
    ///
    /// **Why a second colour space exists here at all.** `from_hsv` above is what the chroma field
    /// used, and an HSV sweep at full saturation and value has two defects that are measurable rather
    /// than matters of taste, and that were reported as "the colours are not the most pleasing":
    ///
    /// - **Hue steps are not perceptually even.** HSV spends a huge span on yellow-green and compresses
    ///   the blues, so an evenly spaced sweep does not look evenly spaced.
    /// - **Lightness swings wildly.** Pure yellow sits near L* 97 and pure blue near L* 32, so a field
    ///   painted across the hue circle flickers in brightness from one side to the other. That is also
    ///   what forced this family to opt out of the project's 3:1 contrast rule: pure blue on a
    ///   near-black panel manages only 2.36:1, and no panel colour fixes it.
    ///
    /// OKLab is near enough perceptually uniform that constant `l` really does look like constant
    /// lightness and equal hue steps really do look equal. Holding `l` fixed across a ramp fixes both
    /// defects at once - and because every hue then carries the same luminance, the contrast floor
    /// stops depending on hue.
    ///
    /// Out-of-gamut combinations are REDUCED IN CHROMA, never clipped per channel: clipping a channel
    /// shifts the hue, which would undo the evenness this is here to provide. See `oklch_max_chroma`.
    pub fn from_oklch(l: f32, chroma: f32, hue_turns: f32, alpha: f32) -> Rgba {
        let l = if l.is_finite() { l.clamp(0.0, 1.0) } else { 0.0 };
        let h = if hue_turns.is_finite() { hue_turns.rem_euclid(1.0) } else { 0.0 };
        let c = if chroma.is_finite() { chroma.max(0.0) } else { 0.0 };
        let c = c.min(Self::oklch_max_chroma(l, h));
        let (r, g, b) = Self::oklch_to_linear(l, c, h);
        Rgba::new(
            Self::encode_srgb(r),
            Self::encode_srgb(g),
            Self::encode_srgb(b),
            (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        )
    }

    /// Largest chroma that stays inside sRGB at this lightness and hue.
    ///
    /// Bisection rather than an analytic boundary: the sRGB gamut in OKLab is a lumpy solid with no
    /// closed form, and 18 halvings resolve chroma to about 1e-6, far finer than an 8-bit channel can
    /// show. Called once per stripe per frame at most, so the cost is irrelevant.
    ///
    /// This is what makes "maximum chroma" a well-defined instruction. The chroma field's whole
    /// identity is full chroma, and at a FIXED lightness full chroma is a different number for every
    /// hue - much lower for blue than for yellow. Asking for more than the gamut holds and clipping
    /// would silently desaturate some hues and shift others.
    pub fn oklch_max_chroma(l: f32, hue_turns: f32) -> f32 {
        let inside = |c: f32| {
            let (r, g, b) = Self::oklch_to_linear(l, c, hue_turns);
            let ok = |v: f32| (-0.0005..=1.0005).contains(&v);
            ok(r) && ok(g) && ok(b)
        };
        if !inside(0.0) {
            return 0.0;
        }
        // 0.45 is past the most chromatic colour sRGB holds in OKLab, so the bracket always contains
        // the boundary.
        let (mut lo, mut hi) = (0.0f32, 0.45f32);
        for _ in 0..18 {
            let mid = (lo + hi) * 0.5;
            if inside(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// OKLCh to LINEAR sRGB, unclamped so the caller can test gamut membership.
    fn oklch_to_linear(l: f32, chroma: f32, hue_turns: f32) -> (f32, f32, f32) {
        let hr = hue_turns * std::f32::consts::TAU;
        let (a, b) = (chroma * hr.cos(), chroma * hr.sin());
        // Bjorn Ottosson's OKLab -> LMS' -> linear sRGB, coefficients as published.
        let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
        let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
        let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
        let (lc, mc, sc) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
        (
            4.076_741_7 * lc - 3.307_711_6 * mc + 0.230_969_94 * sc,
            -1.268_438 * lc + 2.609_757_4 * mc - 0.341_319_38 * sc,
            -0.004_196_086_3 * lc - 0.703_418_6 * mc + 1.707_614_7 * sc,
        )
    }

    /// OKLab lightness of the most chromatic sRGB colour at this hue.
    ///
    /// "Natural" lightness: what the hue wants to be. Yellow comes out near 0.97 and blue near 0.45,
    /// which is the whole reason an HSV ramp flickers - and also why a perfectly FLAT perceptual ramp
    /// turns yellow into olive, since holding yellow down to a mid lightness is exactly what olive is.
    /// `ChromaParams::lightness_tilt` uses this to give a little of that back.
    pub fn oklch_natural_l(hue_turns: f32) -> f32 {
        let c = Rgba::from_hsv(hue_turns, 1.0, 1.0, 1.0);
        Self::oklab_l_of(c)
    }

    /// OKLab lightness of an sRGB colour.
    pub fn oklab_l_of(c: Rgba) -> f32 {
        let d = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let (r, g, b) = (d(c.r), d(c.g), d(c.b));
        let l = (0.412_221_47 * r + 0.536_332_54 * g + 0.051_445_995 * b).cbrt();
        let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
        let s = (0.088_302_46 * r + 0.281_718_84 * g + 0.629_978_5 * b).cbrt();
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s
    }

    /// Linear light to an 8-bit sRGB channel.
    fn encode_srgb(v: f32) -> u8 {
        let v = v.clamp(0.0, 1.0);
        let e = if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (e.clamp(0.0, 1.0) * 255.0).round() as u8
    }

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

#[derive(Clone)]
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

    /// Zeroes every pixel that falls OUTSIDE the given rounded rect, leaving
    /// pixels inside untouched. This is `rounded_rect`'s own corner math run
    /// in reverse, so the clipped region exactly matches what `rounded_rect`
    /// would have drawn for the same `(x, y, w, h, r)` - not just its bounding
    /// box. Intended to run right after an operation (like `bloom`) that can
    /// spread pixels beyond a shape drawn earlier, e.g. so a glow halo cannot
    /// leak past the panel it is supposed to be contained behind.
    pub fn clip_to_rounded_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32) {
        // Degenerate rect: nothing is "inside", so clip everything. Mirrors
        // rounded_rect's own guard - w.min(h) / 2 can go negative and
        // i32::clamp's min <= max assert is unconditional, panicking in
        // release too, so this must be checked before computing `r` below.
        if w <= 0 || h <= 0 {
            self.clear();
            return;
        }
        let r = r.max(0).min(w.min(h) / 2);
        for yy in 0..self.h {
            let ly = yy - y;
            if ly < 0 || ly >= h {
                // Fully outside the rect vertically: the whole row is clipped.
                for xx in 0..self.w {
                    self.px[(yy * self.w + xx) as usize] = 0;
                }
                continue;
            }
            // Same corner-rounding distance as rounded_rect: shrink the
            // in-bounds span near the top and bottom edges.
            let dy = if ly < r {
                r - ly
            } else if ly >= h - r {
                ly - (h - r - 1)
            } else {
                0
            };
            let inset = if dy > 0 {
                let f = (r * r - dy * dy).max(0) as f32;
                r - f.sqrt().round() as i32
            } else {
                0
            };
            let lo = x + inset;
            let hi = x + w - inset;
            for xx in 0..self.w {
                if xx < lo || xx >= hi {
                    self.px[(yy * self.w + xx) as usize] = 0;
                }
            }
        }
    }

    /// Zeroes alpha across the FULL canvas width. Kept as a primitive, but note
    /// the trap it caused: using it for segment gaps erased the panel too, leaving
    /// transparent stripes with the taskbar showing through. Punch within a mark's
    /// own width via `punch_rect` unless you really do mean the whole row.
    #[allow(dead_code)]
    pub fn punch_row(&mut self, y: i32, h: i32) {
        self.punch_rect(0, y, self.w, h);
    }

    /// Like `punch_row`, but bounded in x too instead of always spanning the
    /// full canvas width. Needed where only a narrow column - not an entire
    /// row - should be erased, e.g. re-cutting a segment gap through a
    /// narrow "hot core" highlight without also erasing a bloom halo that
    /// has since spread into the same row further out in x.
    pub fn punch_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in x.max(0)..(x + w).min(self.w) {
                self.px[(yy * self.w + xx) as usize] = 0;
            }
        }
    }

    /// Separable box blur of the current contents, composited *under* the
    /// original so lit elements keep their crisp edge and gain a halo.
    /// Composites `src` OVER self, source-over. Needed because `bloom` blends its
    /// halo UNDER whatever is already on the canvas - which means an opaque panel
    /// hides the halo completely. Blooming the lit marks on their own transparent
    /// layer and then compositing that layer over the panel is the only way to get a
    /// halo that is actually visible on an opaque background.
    pub fn draw_over(&mut self, src: &Canvas) {
        debug_assert_eq!(self.w, src.w);
        debug_assert_eq!(self.h, src.h);
        let n = self.px.len().min(src.px.len());
        for i in 0..n {
            let s = src.px[i];
            if s >> 24 == 0 {
                continue;
            }
            self.px[i] = Self::blend_over(self.px[i], s);
        }
    }

    /// One 3x5 glyph, as five rows of three bits with bit 2 leftmost. `None` for anything
    /// not in the set.
    ///
    /// A hand-rolled bitmap font rather than a text API because there is no text API available
    /// here: the canvas is a raw premultiplied-BGRA buffer composited with UpdateLayeredWindow,
    /// and GDI text at this size would be anti-aliased into grey mush against a dark panel.
    /// 3x5 is the smallest cell in which these particular glyphs stay unambiguous, and the
    /// panel is only 60px tall - a taller font would not fit under the dial arc.
    ///
    /// Deliberately covers only the characters the meter labels use. An unsupported character
    /// advances the cursor and draws nothing, so a future label with a stray character degrades
    /// to a gap rather than a panic or a wrong glyph.
    fn glyph_3x5(ch: char) -> Option<[u8; 5]> {
        Some(match ch.to_ascii_uppercase() {
            'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
            'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
            'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
            'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
            'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
            'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
            'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
            'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
            // Added for the radar warning receiver's threat designators, which annotate a contact
            // the way a real RWR does - numerals for the SA-series, letters for the named systems.
            //
            // Only letters that stay UNAMBIGUOUS in a 3x5 cell are here, which is the rule this font
            // was built on. Three obvious candidates are deliberately absent: 'S' is pixel-identical
            // to '5' at this size, 'T' differs from 'I' only in its bottom row, and 'N' has no
            // 3-wide form that does not read as 'D' or 'M'. A designator that can be misread is worse
            // than one that is not offered, so those systems are simply not in the default table.
            'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
            'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
            'F' => [0b111, 0b100, 0b111, 0b100, 0b100],
            'P' => [0b111, 0b101, 0b111, 0b100, 0b100],
            '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
            '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
            '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
            '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
            '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
            '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
            '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
            '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
            '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
            '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
            ' ' => [0, 0, 0, 0, 0],
            _ => return None,
        })
    }

    /// Draws `text` in the 3x5 font with its top-left at (x, y). 4px cell pitch.
    ///
    /// Returns the width drawn, so a caller can centre or right-align without duplicating the
    /// pitch arithmetic.
    pub fn text_3x5(&mut self, x: i32, y: i32, text: &str, c: Rgba) -> i32 {
        let mut cx = x;
        for ch in text.chars() {
            if let Some(rows) = Self::glyph_3x5(ch) {
                for (dy, row) in rows.iter().enumerate() {
                    for dx in 0..3 {
                        if row & (0b100 >> dx) != 0 {
                            self.fill_rect(cx + dx, y + dy as i32, 1, 1, c);
                        }
                    }
                }
            }
            cx += 4;
        }
        (cx - x - 1).max(0)
    }

    /// Width `text` would occupy in the 3x5 font, without drawing it.
    pub fn text_3x5_width(text: &str) -> i32 {
        (text.chars().count() as i32 * 4 - 1).max(0)
    }

    /// Scales every pixel's alpha by `k`, for the reveal/hide fade.
    ///
    /// Scaling all four premultiplied channels by the same factor preserves the
    /// r,g,b <= a invariant, so this cannot produce the opaque-dark pixels that
    /// independent per-channel scaling caused in `bloom`.
    pub fn scale_alpha(&mut self, k: f32) {
        let k = k.clamp(0.0, 1.0);
        if k >= 1.0 {
            return;
        }
        if k <= 0.0 {
            self.clear();
            return;
        }
        for px in self.px.iter_mut() {
            if *px == 0 {
                continue;
            }
            let sc = |v: u32| ((v as f32 * k).round() as u32).min(255);
            let (a, r, g, b) = (*px >> 24, (*px >> 16) & 0xff, (*px >> 8) & 0xff, *px & 0xff);
            *px = (sc(a) << 24) | (sc(r) << 16) | (sc(g) << 8) | sc(b);
        }
    }

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
            // Scale while PRESERVING the premultiplied invariant (r,g,b <= a).
            //
            // Scaling the four channels independently and clamping each at 255 breaks
            // it: for any real pixel alpha is the largest channel, so alpha saturates
            // FIRST while r,g,b are still below 255. The result is an opaque pixel with
            // dark colour - i.e. a black wash wherever the halo is strongest, which is
            // exactly the "black box around everything" this produced. Clamp by the
            // single limiting factor instead, so the colour keeps its hue and only its
            // brightness changes.
            let (ha, hr, hg, hb) = (hp >> 24, (hp >> 16) & 0xff, (hp >> 8) & 0xff, hp & 0xff);
            let peak = ha.max(hr).max(hg).max(hb) as f32;
            let k = if peak * strength > 255.0 && peak > 0.0 {
                255.0 / peak
            } else {
                strength
            };
            let scale = |v: u32| ((v as f32 * k).round().min(255.0)) as u32;
            let scaled = (scale(ha) << 24) | (scale(hr) << 16) | (scale(hg) << 8) | scale(hb);
            self.px[i] = Self::blend_over(scaled, src[i]);
        }
    }

    /// Shared per-pixel source-over blend for primitives (the line and the
    /// gradients) that vary colour pixel-by-pixel rather than filling a whole
    /// span at once with `fill_rect`. Out-of-bounds coordinates are silently
    /// dropped via `idx`, matching every other primitive's clip-don't-panic
    /// behaviour.
    fn blend_px(&mut self, x: i32, y: i32, c: Rgba) {
        if c.a == 0 {
            return;
        }
        if let Some(i) = self.idx(x, y) {
            let packed = Self::pack(c);
            self.px[i] = if c.a == 255 { packed } else { Self::blend_over(self.px[i], packed) };
        }
    }

    /// 1px Bresenham line, deliberately without anti-aliasing: the grid
    /// family snaps coordinates to get crisp lines, and an anti-aliased 1px
    /// diagonal at 60px tall reads as grey mush. Clips to the canvas rather
    /// than wrapping - every plotted point goes through `blend_px`'s bounds
    /// check, so endpoints (or the whole segment) may sit entirely
    /// off-canvas without panicking.
    //
    // No production consumer yet - this is a primitive for the vaporwave
    // grid family (docs/superpowers/specs/2026-07-31-vaporwave-grid-family-design.md),
    // which is separate work and must not be wired in here. Exercised by the
    // unit tests below.
    #[allow(dead_code)]
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) {
        if c.a == 0 {
            return;
        }
        // i64 for the deltas: x1-x0 can approach i32's full range, and
        // doubling it for the Bresenham step (2*err) would overflow i32.
        let (mut x, mut y) = (x0 as i64, y0 as i64);
        let (x1, y1) = (x1 as i64, y1 as i64);
        let dx = (x1 - x).abs();
        let dy = (y1 - y).abs();
        let sx: i64 = if x1 >= x { 1 } else { -1 };
        let sy: i64 = if y1 >= y { 1 } else { -1 };
        let mut err = dx - dy;
        loop {
            self.blend_px(x as i32, y as i32, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Scanline fill of a closed polygon - the edge from the last point back
    /// to the first is implicit, so callers do not repeat the first point.
    /// Uses even-odd parity: everything this ever fills (hidden-line ground
    /// fills, the sun's body) is a simple, non-self-intersecting polygon, for
    /// which even-odd and non-zero winding agree, and even-odd is the
    /// simpler scanline to get right. Horizontal edges are skipped when
    /// building each scanline's crossing list (they contribute nothing to
    /// parity), and each edge's y-range is treated half-open `[low, high)` so
    /// a vertex shared between two edges is never counted twice.
    ///
    /// Consequence of the half-open range: the row at the polygon's lowest y
    /// (its topmost vertex) is always inclusive, but the row at its highest y
    /// (its bottommost vertex) is always exclusive - the same "inclusive near
    /// edge, exclusive far edge" convention `fill_rect(x, y, w, h)` already
    /// uses (which stops at row `y + h - 1`, not `y + h`). This is why
    /// `fill_poly` reproduces `fill_rect` pixel-for-pixel when a rectangle is
    /// authored with corners `(x, y)`..`(x + w, y + h)`: both conventions
    /// exclude the far edge in the same place. A caller closing a ground fill
    /// down to the literal bottom of the canvas must therefore place that
    /// bottom edge at `y = height` (one past the last row), exactly as they
    /// would size a `fill_rect`'s `h` - placing it at `height - 1` leaves the
    /// last row unfilled. See
    /// `fill_poly_flat_bottom_must_close_one_past_the_last_desired_row` below.
    // No production consumer yet - see `line`'s note above.
    #[allow(dead_code)]
    pub fn fill_poly(&mut self, points: &[(i32, i32)], c: Rgba) {
        if points.len() < 3 || c.a == 0 {
            return;
        }
        let n = points.len();
        let miny = points.iter().map(|p| p.1).min().unwrap();
        let maxy = points.iter().map(|p| p.1).max().unwrap();
        let y0 = miny.max(0);
        let y1 = maxy.min(self.h - 1);
        let mut xs: Vec<i32> = Vec::new();
        for y in y0..=y1 {
            xs.clear();
            for i in 0..n {
                let (ax, ay) = points[i];
                let (bx, by) = points[(i + 1) % n];
                if ay == by {
                    continue; // horizontal edge: no parity contribution
                }
                let (lo, hi, lo_x, hi_x) =
                    if ay < by { (ay, by, ax, bx) } else { (by, ay, bx, ax) };
                if y < lo || y >= hi {
                    continue;
                }
                let t = (y - lo) as f32 / (hi - lo) as f32;
                xs.push((lo_x as f32 + t * (hi_x - lo_x) as f32).round() as i32);
            }
            xs.sort_unstable();
            let mut i = 0;
            while i + 1 < xs.len() {
                let x = xs[i];
                let w = xs[i + 1] - x;
                if w > 0 {
                    self.fill_rect(x, y, w, 1, c);
                }
                i += 2;
            }
        }
    }

    /// 4x4 Bayer matrix (values 0..16) used to spread quantisation rounding
    /// across a spatial pattern instead of always rounding the same way.
    const BAYER_4X4: [[u8; 4]; 4] =
        [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

    /// Per-pixel rounding threshold in (0, 1). With dithering off this is a
    /// flat 0.5 (plain round-to-nearest); with it on, a 4x4 tile of the Bayer
    /// matrix gives each pixel its own threshold so a run of pixels that
    /// would otherwise all quantise to the same flat value instead spreads
    /// the rounding error into a stipple.
    fn dither_threshold(ordered_dither: bool, x: i32, y: i32) -> f32 {
        if !ordered_dither {
            return 0.5;
        }
        let bx = x.rem_euclid(4) as usize;
        let by = y.rem_euclid(4) as usize;
        (Self::BAYER_4X4[by][bx] as f32 + 0.5) / 16.0
    }

    /// Rounds a straight 0.0..=255.0 channel value against `threshold`
    /// instead of always against 0.5. A value with zero fractional part
    /// never crosses any threshold in (0, 1), so an exact stop colour is
    /// unaffected by dithering regardless of this setting - only in-between
    /// values change.
    fn quantize_channel(v: f32, threshold: f32) -> u8 {
        let v = v.clamp(0.0, 255.0);
        let base = v.floor();
        let frac = v - base;
        (if frac >= threshold { base + 1.0 } else { base }).min(255.0) as u8
    }

    /// Straight-colour-space interpolation across `stops` (positions assumed
    /// sorted ascending) at `t`, extrapolated flat beyond the first/last
    /// stop. Returns straight (non-premultiplied) channels as f32 so callers
    /// can dither before rounding and premultiplying on store - interpolating
    /// already-premultiplied values would darken the midpoints, since a
    /// translucent stop's colour would bleed toward black as its own alpha
    /// shrinks, rather than staying the same hue and fading.
    fn sample_stops(stops: &[(f32, Rgba)], t: f32) -> (f32, f32, f32, f32) {
        let as_f32 = |c: Rgba| (c.r as f32, c.g as f32, c.b as f32, c.a as f32);
        if stops.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        if stops.len() == 1 || t <= stops[0].0 {
            return as_f32(stops[0].1);
        }
        let last = stops.len() - 1;
        if t >= stops[last].0 {
            return as_f32(stops[last].1);
        }
        for i in 0..last {
            let (p0, c0) = stops[i];
            let (p1, c1) = stops[i + 1];
            if t >= p0 && t <= p1 {
                let (r0, g0, b0, a0) = as_f32(c0);
                let (r1, g1, b1, a1) = as_f32(c1);
                let f = (t - p0) / (p1 - p0).max(f32::EPSILON);
                return (r0 + (r1 - r0) * f, g0 + (g1 - g0) * f, b0 + (b1 - b0) * f, a0 + (a1 - a0) * f);
            }
        }
        as_f32(stops[last].1)
    }

    /// Fills `[x, x+w) x [y, y+h)` with a vertical gradient through `stops`.
    /// Interpolated in straight colour space per scanline (see
    /// `sample_stops`), then premultiplied on store via `blend_px`.
    ///
    /// `ordered_dither` breaks up flat quantisation bands with a 4x4 Bayer
    /// pattern - needed because the vaporwave sky is only ~29px tall across
    /// 3 stops, which is few enough rows that plain rounding can flatten
    /// several consecutive rows to the exact same colour (see
    /// `vertical_gradient_ordered_dither_breaks_up_a_flat_quantisation_band`).
    // No production consumer yet - see `line`'s note above.
    #[allow(dead_code)]
    pub fn vertical_gradient(&mut self, x: i32, y: i32, w: i32, h: i32, stops: &[(f32, Rgba)], ordered_dither: bool) {
        if w <= 0 || h <= 0 || stops.is_empty() {
            return;
        }
        for row in 0..h {
            let t = if h == 1 { 0.0 } else { row as f32 / (h - 1) as f32 };
            let (r, g, b, a) = Self::sample_stops(stops, t);
            let py = y + row;
            for col in 0..w {
                let px = x + col;
                let th = Self::dither_threshold(ordered_dither, px, py);
                self.blend_px(
                    px,
                    py,
                    Rgba::new(
                        Self::quantize_channel(r, th),
                        Self::quantize_channel(g, th),
                        Self::quantize_channel(b, th),
                        Self::quantize_channel(a, th),
                    ),
                );
            }
        }
    }

    /// Radial gradient centred at `(cx, cy)`. Pixels at or inside `r_inner`
    /// take `stops`' first colour exactly; pixels beyond `r_outer` are left
    /// untouched entirely - not filled with the last stop - so a halo
    /// genuinely fades to nothing rather than filling its bounding box.
    /// Between the two radii, position 0.0..=1.0 maps linearly across the
    /// annulus and is looked up the same way `vertical_gradient` looks up
    /// `stops`.
    // No production consumer yet - see `line`'s note above.
    #[allow(dead_code)]
    pub fn radial_gradient(&mut self, cx: i32, cy: i32, r_inner: i32, r_outer: i32, stops: &[(f32, Rgba)]) {
        if r_outer <= 0 || stops.is_empty() {
            return;
        }
        let r_inner = r_inner.max(0) as f32;
        let r_outer = r_outer as f32;
        let span = (r_outer - r_inner).max(f32::EPSILON);
        // `as i32` on a float is a saturating cast (stable since Rust 1.45),
        // so a centre at an extreme like i32::MIN cannot panic here even
        // though `cx as f32 - r_outer` may itself be far outside i32's range.
        let x0 = (cx as f32 - r_outer).floor().max(0.0) as i32;
        let x1 = ((cx as f32 + r_outer).ceil() as i32).min(self.w - 1);
        let y0 = (cy as f32 - r_outer).floor().max(0.0) as i32;
        let y1 = ((cy as f32 + r_outer).ceil() as i32).min(self.h - 1);
        if x0 > x1 || y0 > y1 {
            return;
        }
        for py in y0..=y1 {
            for px in x0..=x1 {
                // Cast to f32 before subtracting, not after: `px - cx` as an
                // i32 subtraction overflows and panics when cx is near
                // i32::MIN (0 - i32::MIN does not fit in i32), which a
                // far-off-canvas centre can genuinely produce.
                let dx = px as f32 - cx as f32;
                let dy = py as f32 - cy as f32;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > r_outer {
                    continue;
                }
                let (r, g, b, a) = if dist <= r_inner {
                    let c = stops[0].1;
                    (c.r as f32, c.g as f32, c.b as f32, c.a as f32)
                } else {
                    Self::sample_stops(stops, ((dist - r_inner) / span).clamp(0.0, 1.0))
                };
                self.blend_px(
                    px,
                    py,
                    Rgba::new(
                        r.clamp(0.0, 255.0).round() as u8,
                        g.clamp(0.0, 255.0).round() as u8,
                        b.clamp(0.0, 255.0).round() as u8,
                        a.clamp(0.0, 255.0).round() as u8,
                    ),
                );
            }
        }
    }


    /// Offsets the RED and BLUE channels horizontally in opposite directions, leaving green -
    /// and alpha - exactly where they were. The mis-registered colour plates of a badly printed
    /// page, or a display with a failing cable.
    ///
    /// `shift` is in pixels: red moves `shift` to the RIGHT and blue `shift` to the LEFT, so the
    /// total separation between the two fringes is `2 * shift`. A negative value swaps which side
    /// each plate lands on. A flat field is unchanged; the effect exists only at EDGES, which is
    /// what makes it read as misregistration rather than as a tint.
    ///
    /// Three properties, each of which guards a rule this project has already broken once:
    ///
    /// - **Resampled in STRAIGHT colour, never premultiplied.** `bloom` once scaled premultiplied
    ///   channels independently of alpha and produced opaque-DARK pixels, because for any real
    ///   pixel alpha is the largest channel and saturates first. Moving a premultiplied `r` from a
    ///   bright neighbour into a fainter pixel is the same class of bug and breaks `r <= a`
    ///   outright. So each pixel is unpacked to straight colour, takes its r/b from the
    ///   neighbours' STRAIGHT colour, and is repacked - `pack` re-premultiplies, so the invariant
    ///   holds by construction instead of by clamping. (The unpack/repack round trip is exact at
    ///   alpha 255 and can move a channel by one at partial alpha, which is why this belongs at
    ///   the end of a frame rather than inside an accumulating loop.)
    /// - **Alpha never moves.** Every destination keeps its own alpha bit-for-bit. So this cannot
    ///   punch a hole in an opaque panel - the overlay is composited with per-pixel alpha over the
    ///   Windows weather widget, so a pixel below 255 inside the panel is a hole the forecast
    ///   shows through - and it cannot make an off-panel pixel visible either. That is what makes
    ///   it safe as the LAST step of a family, AFTER `clip_to_rounded_rect`, which is also the
    ///   only place it does anything: on a transparent ink layer there is nothing either side of a
    ///   mark for the plates to fringe against, so the effect needs the opaque panel already
    ///   underneath it.
    /// - **A fully transparent source contributes nothing.** `unpack` returns black at alpha 0, so
    ///   sampling from outside the panel would import "colour" that is really just absence and lay
    ///   a dark cyan and a dark red band down the panel's own left and right edges - `shift`
    ///   columns of fringe belonging to no mark at all. Those pixels keep their own channel.
    pub fn chromatic_aberration(&mut self, shift: i32) {
        let (w, h) = (self.w, self.h);
        if shift == 0 || w <= 0 || h <= 0 {
            return;
        }
        // Bounded before it reaches any address arithmetic: `x + shift` at an i32::MAX shift
        // overflows, which panics in debug and wraps in release. Anything beyond the canvas width
        // samples nothing anyway, so clamping cannot change the result.
        let shift = shift.clamp(-w, w);
        let src = self.px.clone();
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let i = (row + x) as usize;
                let here = src[i];
                if here >> 24 == 0 {
                    continue; // transparent stays transparent
                }
                let own = Self::unpack(here);
                let plate = |sx: i32| -> Option<Rgba> {
                    if sx < 0 || sx >= w {
                        return None;
                    }
                    let p = src[(row + sx) as usize];
                    if p >> 24 == 0 {
                        None
                    } else {
                        Some(Self::unpack(p))
                    }
                };
                let r = plate(x + shift).map(|c| c.r).unwrap_or(own.r);
                let b = plate(x - shift).map(|c| c.b).unwrap_or(own.b);
                self.px[i] = Self::pack(Rgba::new(r, own.g, b, own.a));
            }
        }
    }

    /// Gradient over an ELLIPSE, so the vertical and horizontal extents can differ.
    ///
    /// A strict generalisation of `radial_gradient`: with `rx == ry` it produces byte-identical
    /// output, which is asserted by a test. It exists because a meter needs to be able to grow the
    /// LIT AREA and not only its brightness - position is resolved far more readily than intensity
    /// at these sizes, and a light column growing from a fixed baseline reads as a profile at a
    /// glance where ten brightnesses have to be compared pairwise.
    ///
    /// Distance is measured in normalised ellipse units, so `1.0` is the boundary regardless of
    /// aspect. The inner stop is applied within one pixel of the centre, matching
    /// `radial_gradient`'s `r_inner = 1` behaviour.
    pub fn elliptical_gradient(&mut self, cx: i32, cy: i32, rx: f32, ry: f32, stops: &[(f32, Rgba)]) {
        if stops.is_empty() || rx <= 0.0 || ry <= 0.0 {
            return;
        }
        // Bounds in f32 before casting, for the same reason radial_gradient does it: a centre far
        // off-canvas makes the i32 arithmetic overflow.
        let x0 = (cx as f32 - rx).floor().max(0.0) as i32;
        let x1 = ((cx as f32 + rx).ceil() as i32).min(self.w - 1);
        let y0 = (cy as f32 - ry).floor().max(0.0) as i32;
        let y1 = ((cy as f32 + ry).ceil() as i32).min(self.h - 1);
        if x0 > x1 || y0 > y1 {
            return;
        }
        // Matches radial_gradient's inner radius of 1 expressed in normalised units.
        let inner = (1.0 / rx.max(1.0)).min(0.999);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let dx = (px as f32 - cx as f32) / rx;
                let dy = (py as f32 - cy as f32) / ry;
                let e = (dx * dx + dy * dy).sqrt();
                if e > 1.0 {
                    continue;
                }
                let (r, g, b, a) = if e <= inner {
                    let c = stops[0].1;
                    (c.r as f32, c.g as f32, c.b as f32, c.a as f32)
                } else {
                    Self::sample_stops(stops, ((e - inner) / (1.0 - inner)).clamp(0.0, 1.0))
                };
                self.blend_px(
                    px,
                    py,
                    Rgba::new(
                        r.clamp(0.0, 255.0).round() as u8,
                        g.clamp(0.0, 255.0).round() as u8,
                        b.clamp(0.0, 255.0).round() as u8,
                        a.clamp(0.0, 255.0).round() as u8,
                    ),
                );
            }
        }
    }

    /// Scanline fill of a full disc. `r <= 0` draws nothing; a centre far
    /// off-canvas draws nothing but does not panic, since each row still
    /// goes through `fill_rect`'s own clipping.
    // No production consumer yet - see `line`'s note above.
    #[allow(dead_code)]
    pub fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, c: Rgba) {
        self.fill_circle_rows(cx, cy, r, c, -r, r);
    }

    /// Scanline fill of the UPPER half only (rows `cy - r ..= cy`), i.e. a
    /// dome sitting on the horizontal line through the centre. This is the
    /// shape the sun actually needs, since it sits on the horizon rather
    /// than floating as a full circle.
    // No production consumer yet - see `line`'s note above.
    #[allow(dead_code)]
    pub fn fill_semicircle_upper(&mut self, cx: i32, cy: i32, r: i32, c: Rgba) {
        self.fill_circle_rows(cx, cy, r, c, -r, 0);
    }

    fn fill_circle_rows(&mut self, cx: i32, cy: i32, r: i32, c: Rgba, dy0: i32, dy1: i32) {
        if r <= 0 {
            return;
        }
        for dy in dy0..=dy1 {
            // Cast to f32 before squaring - r*r/dy*dy as native i32 multiplication
            // overflows for |r| beyond roughly 46,340 (sqrt(i32::MAX)). Same class
            // of bug as radial_gradient's (px - cx) overflow; fixed the same way.
            let half = ((r as f32 * r as f32 - dy as f32 * dy as f32).max(0.0)).sqrt().round() as i32;
            self.fill_rect(cx - half, cy + dy, half * 2 + 1, 1, c);
        }
    }

    /// Zeroes everything OUTSIDE the given plain (non-rounded) rect - the
    /// grid family needs to confine its sky/ground backdrop to the panel.
    /// Delegates to `clip_to_rounded_rect` at `r = 0`: that function's corner
    /// inset is only non-zero within `r` rows of the top/bottom edge, so at
    /// `r = 0` it is always zero and the rounded clip degenerates exactly to
    /// a plain rect clip - sharing the loop instead of re-deriving it avoids
    /// the trap `clip_to_rounded_rect` itself exists to prevent: a clip
    /// shape that doesn't exactly match what was drawn.
    // No production consumer yet - see `line`'s note above.
    #[allow(dead_code)]
    pub fn clip_outside_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.clip_to_rounded_rect(x, y, w, h, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks the premultiplied invariant against the *stored* (packed)
    /// pixels via `bits()`, not `get()` - `get()` deliberately unpacks back
    /// to straight colour (so e.g. 50% white round-trips to r=255, not 128),
    /// and straight colour has no r<=a relationship to preserve.
    fn assert_invariant(c: &Canvas, label: &str) {
        for &p in c.bits() {
            let (a, r, g, b) = (p >> 24, (p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
            assert!(
                r <= a && g <= a && b <= a,
                "{label} broke the premultiplied invariant: a={a} r={r} g={g} b={b}"
            );
        }
    }

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
    fn punch_rect_only_clears_its_own_columns() {
        let mut c = Canvas::new(10, 10);
        c.fill_rect(0, 0, 10, 10, Rgba::new(255, 255, 255, 255));
        c.punch_rect(3, 4, 2, 2);
        assert_eq!(c.get(3, 4), Rgba::TRANSPARENT);
        assert_eq!(c.get(4, 5), Rgba::TRANSPARENT);
        assert_eq!(c.get(2, 4), Rgba::new(255, 255, 255, 255), "left of the punched rect survives");
        assert_eq!(c.get(5, 4), Rgba::new(255, 255, 255, 255), "right of the punched rect survives");
        assert_eq!(c.get(3, 3), Rgba::new(255, 255, 255, 255), "above the punched rect survives");
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
    fn clip_to_rounded_rect_erases_everything_outside_the_shape() {
        let mut c = Canvas::new(20, 20);
        c.fill_rect(0, 0, 20, 20, Rgba::new(255, 255, 255, 255));
        c.clip_to_rounded_rect(2, 2, 16, 16, 4);
        assert_eq!(c.get(0, 0), Rgba::TRANSPARENT, "outside the rect entirely");
        assert_eq!(c.get(2, 2), Rgba::TRANSPARENT, "corner must be cut, matching rounded_rect");
        assert_eq!(c.get(10, 10), Rgba::new(255, 255, 255, 255), "centre survives");
        assert_eq!(c.get(2, 10), Rgba::new(255, 255, 255, 255), "left edge (not a corner) survives");
    }

    #[test]
    fn clip_to_rounded_rect_matches_what_rounded_rect_would_have_drawn() {
        // The clip and the draw share the same corner math, so clipping a
        // fully-lit canvas to (x,y,w,h,r) must reproduce exactly the pixels
        // rounded_rect(x,y,w,h,r) would have drawn from a blank canvas.
        let (x, y, w, h, r) = (1, 2, 190 - 2, 60 - 4, 4);
        let mut drawn = Canvas::new(190, 60);
        drawn.rounded_rect(x, y, w, h, r, Rgba::new(255, 255, 255, 255));

        let mut clipped = Canvas::new(190, 60);
        clipped.fill_rect(0, 0, 190, 60, Rgba::new(255, 255, 255, 255));
        clipped.clip_to_rounded_rect(x, y, w, h, r);

        assert_eq!(drawn.bits(), clipped.bits());
    }

    #[test]
    fn clip_to_rounded_rect_on_degenerate_rect_clears_everything() {
        let mut c = Canvas::new(10, 10);
        c.fill_rect(0, 0, 10, 10, Rgba::new(255, 255, 255, 255));
        c.clip_to_rounded_rect(0, 0, -4, 190, 5);
        assert!(c.bits().iter().all(|&p| p == 0), "degenerate rect clips everything");
    }

    #[test]
    fn chromatic_aberration_moves_red_right_and_blue_left_with_green_staying_put() {
        let mut c = bar_on_panel();
        c.chromatic_aberration(2);
        // Two columns LEFT of the bar now carries the red plate on its own: red was pulled in
        // from x+2 (inside the bar) while green and blue are still the panel's.
        assert_eq!(c.get(7, 2), Rgba::new(255, 7, 10, 255), "a lone red plate 2px left of the bar");
        // ...and two columns RIGHT carries the blue plate on its own.
        assert_eq!(c.get(13, 2), Rgba::new(7, 7, 255, 255), "a lone blue plate 2px right of the bar");
        // Green is the reference plate: the bar's own extent in green must not have moved at all,
        // which is what stops this reading as a plain sideways smear.
        for x in 9..12 {
            assert_eq!(c.get(x, 2).g, 255, "green must stay registered at x={x}");
        }
        assert_eq!(c.get(8, 2).g, 7, "and must not spread left");
        assert_eq!(c.get(12, 2).g, 7, "or right");
        // The bar's own edges therefore go yellow (red present, blue gone) and cyan (the
        // reverse) - the actual misprint signature, not just a coloured halo.
        assert_eq!(c.get(9, 2), Rgba::new(255, 255, 10, 255), "left edge of the bar goes yellow");
        assert_eq!(c.get(11, 2), Rgba::new(7, 255, 255, 255), "right edge goes cyan");
    }

    #[test]
    fn chromatic_aberration_never_changes_alpha_anywhere() {
        // The rule this is most likely to break: the overlay is composited with per-pixel alpha
        // over the Windows weather widget, so any pixel it drops below 255 inside the panel is a
        // hole the forecast shows through. Mixed alpha on purpose - fully transparent margins, a
        // translucent wash, and opaque marks.
        for shift in [-5i32, -1, 1, 2, 3, 9, 400] {
            let mut c = Canvas::new(24, 6);
            c.fill_rect(2, 1, 20, 4, Rgba::new(7, 7, 10, 255));
            c.fill_rect(4, 1, 3, 4, Rgba::new(255, 40, 200, 255));
            c.fill_rect(14, 2, 4, 2, Rgba::new(0, 255, 255, 90));
            let before: Vec<u32> = c.bits().iter().map(|p| p >> 24).collect();
            c.chromatic_aberration(shift);
            let after: Vec<u32> = c.bits().iter().map(|p| p >> 24).collect();
            assert_eq!(before, after, "shift {shift} altered alpha somewhere");
        }
    }

    #[test]
    fn chromatic_aberration_never_breaks_the_premultiplied_invariant() {
        // The failure mode being avoided is exactly `bloom`'s: move a premultiplied channel from a
        // bright neighbour into a fainter pixel and r > a, which renders as an opaque dark blot.
        for shift in [-7i32, -2, 1, 4, 11] {
            let mut c = Canvas::new(20, 6);
            c.fill_rect(0, 0, 20, 6, Rgba::new(255, 255, 255, 12));
            c.fill_rect(5, 1, 3, 4, Rgba::new(255, 10, 200, 255));
            c.fill_rect(9, 1, 3, 4, Rgba::new(0, 0, 0, 40));
            c.chromatic_aberration(shift);
            assert_invariant(&c, &format!("chromatic_aberration({shift})"));
        }
    }

    #[test]
    fn chromatic_aberration_does_not_import_colour_from_a_transparent_neighbour() {
        // `unpack` returns BLACK for alpha 0, so naively sampling off the panel would lay a dark
        // band down the panel's own edges - `shift` columns of fringe that belong to no mark.
        let mut c = Canvas::new(8, 1);
        c.fill_rect(3, 0, 5, 1, Rgba::new(255, 255, 255, 255));
        c.chromatic_aberration(2);
        assert_eq!(
            c.get(3, 0),
            Rgba::new(255, 255, 255, 255),
            "the leftmost opaque pixel must keep its own blue rather than sampling the void"
        );
        assert_eq!(c.get(2, 0), Rgba::TRANSPARENT, "and the void itself stays transparent");
        // The right-hand plate does still fall off the canvas edge, and that is the same case:
        // keep your own channel rather than going dark.
        assert_eq!(c.get(7, 0), Rgba::new(255, 255, 255, 255), "nor off the canvas edge");
    }

    #[test]
    fn chromatic_aberration_is_a_no_op_at_zero_and_survives_degenerate_canvases() {
        let mut c = bar_on_panel();
        let before = c.bits().to_vec();
        c.chromatic_aberration(0);
        assert_eq!(c.bits(), &before[..], "shift 0 must not touch a pixel");

        // A shift far wider than the canvas: every sample is off-canvas, so every pixel keeps its
        // own colour. Must not panic on the `x + shift` arithmetic.
        let mut wide = bar_on_panel();
        wide.chromatic_aberration(i32::MAX);
        assert_eq!(wide.bits(), &before[..], "an enormous shift samples nothing");
        let mut neg = bar_on_panel();
        neg.chromatic_aberration(i32::MIN);
        assert_eq!(neg.bits(), &before[..], "and neither does an enormous negative one");

        for (w, h) in [(1, 1), (0, 0), (1, 60), (190, 1)] {
            let mut tiny = Canvas::new(w, h);
            tiny.fill_rect(0, 0, w, h, Rgba::new(255, 255, 255, 255));
            tiny.chromatic_aberration(3);
            assert_eq!(tiny.bits().len(), (w.max(0) * h.max(0)) as usize, "{w}x{h} changed size");
        }
    }

    #[test]
    fn chromatic_aberration_does_not_leak_across_rows() {
        // Both plates are sampled from the SAME row. A row-crossing index bug would show up as
        // colour bleeding vertically, which at 60px tall reads as a smear rather than a misprint.
        let mut c = Canvas::new(9, 3);
        c.fill_rect(0, 0, 9, 3, Rgba::new(7, 7, 10, 255));
        c.fill_rect(4, 1, 1, 1, Rgba::new(255, 255, 255, 255));
        c.chromatic_aberration(2);
        for x in 0..9 {
            assert_eq!(c.get(x, 0), Rgba::new(7, 7, 10, 255), "row 0 must be untouched at x={x}");
            assert_eq!(c.get(x, 2), Rgba::new(7, 7, 10, 255), "row 2 must be untouched at x={x}");
        }
        assert_eq!(c.get(2, 1).r, 255, "the red plate landed in the mark's own row");
    }


    /// An opaque dark field with a 3px white bar in the middle of it, which is the shape the
    /// effect is actually applied to: marks on an already-opaque panel.
    fn bar_on_panel() -> Canvas {
        let mut c = Canvas::new(21, 5);
        c.fill_rect(0, 0, 21, 5, Rgba::new(7, 7, 10, 255));
        c.fill_rect(9, 0, 3, 5, Rgba::new(255, 255, 255, 255));
        c
    }

    #[test]
    fn from_hsv_hits_the_primaries_and_wraps() {
        let full = |h: f32| Rgba::from_hsv(h, 1.0, 1.0, 1.0);
        assert_eq!(full(0.0), Rgba::new(255, 0, 0, 255), "0 turns = red");
        assert_eq!(full(1.0 / 3.0), Rgba::new(0, 255, 0, 255), "1/3 = green");
        assert_eq!(full(2.0 / 3.0), Rgba::new(0, 0, 255, 255), "2/3 = blue");
        // Wrapping matters: the hue is an accumulating phase, so it goes past 1 and negative.
        assert_eq!(full(1.0), full(0.0), "a full turn must return to the start");
        assert_eq!(full(-1.0 / 3.0), full(2.0 / 3.0), "negative hues must wrap, not clamp");
        // NaN must not produce a garbage colour - the phase is derived from frame timing.
        assert_eq!(full(f32::NAN), full(0.0), "a non-finite hue must fall back, not corrupt");
    }

    #[test]
    fn from_hsv_desaturates_toward_white_and_never_breaks_premultiplication() {
        let grey = Rgba::from_hsv(0.5, 0.0, 1.0, 1.0);
        assert_eq!(grey, Rgba::new(255, 255, 255, 255), "zero saturation is white at full value");
        // Rgba holds STRAIGHT rgba - premultiplication happens when a pixel is written - so the
        // invariant to check here is that alpha round-trips and no channel is produced out of
        // range, not the r,g,b <= a relation that applies to stored pixels.
        for h in 0..24 {
            for &(sat, val, a) in &[(1.0f32, 1.0f32, 1.0f32), (0.68, 1.0, 0.5), (0.3, 0.6, 0.25)] {
                let c = Rgba::from_hsv(h as f32 / 24.0, sat, val, a);
                assert_eq!(
                    c.a,
                    (a * 255.0).round() as u8,
                    "alpha must pass through untouched at h={h}"
                );
                let peak = c.r.max(c.g).max(c.b);
                assert_eq!(
                    peak,
                    (val * 255.0).round() as u8,
                    "the brightest channel must equal value at h={h}, sat={sat}"
                );
            }
        }
    }

    #[test]
    fn an_elliptical_gradient_with_equal_radii_is_exactly_a_radial_one() {
        // The whole safety argument for replacing radial_gradient in the valve family is that the
        // ellipse is a STRICT generalisation - so with equal radii it must be byte-identical, not
        // merely similar. If this drifts, the approved valve look drifts with it.
        let stops = [
            (0.0, Rgba::new(0xff, 0xd9, 0xa0, 240)),
            (0.40, Rgba::new(0xff, 0x8a, 0x2a, 170)),
            (1.0, Rgba::new(0xff, 0x8a, 0x2a, 0)),
        ];
        for r in [3i32, 7, 12, 25] {
            let mut a = Canvas::new(64, 64);
            a.radial_gradient(30, 34, 1, r, &stops);
            let mut b = Canvas::new(64, 64);
            b.elliptical_gradient(30, 34, r as f32, r as f32, &stops);
            assert_eq!(a.bits(), b.bits(), "radii {r}: the ellipse must reduce to the circle");
        }
    }

    #[test]
    fn an_elliptical_gradient_grows_only_the_axis_it_is_told_to() {
        // The point of the primitive: a taller ry must extend the light vertically WITHOUT
        // fattening it sideways, or a valve's glow would bleed into its neighbour as it lights.
        let stops = [(0.0, Rgba::new(255, 255, 255, 255)), (1.0, Rgba::new(255, 255, 255, 0))];
        let extent = |rx: f32, ry: f32| -> (i32, i32) {
            let mut c = Canvas::new(64, 64);
            c.elliptical_gradient(32, 40, rx, ry, &stops);
            let (mut wide, mut tall) = (0, 0);
            for y in 0..64 {
                for x in 0..64 {
                    if c.get(x, y).a > 8 {
                        wide = wide.max((x - 32).abs());
                        tall = tall.max((y - 40).abs());
                    }
                }
            }
            (wide, tall)
        };
        let (w1, h1) = extent(10.0, 6.0);
        let (w2, h2) = extent(10.0, 20.0);
        assert!(h2 > h1 + 8, "ry must extend it vertically: {h1} -> {h2}");
        assert_eq!(w1, w2, "rx unchanged must leave the horizontal extent identical");
    }

    #[test]
    fn text_3x5_draws_inside_its_reported_box_and_nothing_outside() {
        let mut c = Canvas::new(40, 12);
        let white = Rgba::new(255, 255, 255, 255);
        let w = c.text_3x5(2, 3, "LR", white);
        assert_eq!(w, Canvas::text_3x5_width("LR"), "reported width must match the helper");
        // Every lit pixel must lie inside the advertised box, or callers cannot lay labels out.
        for y in 0..12 {
            for x in 0..40 {
                if c.get(x, y).a > 0 {
                    assert!(
                        (2..2 + w + 1).contains(&x) && (3..8).contains(&y),
                        "glyph pixel at ({x},{y}) is outside the reported 3x5 box"
                    );
                }
            }
        }
    }

    #[test]
    fn every_label_the_meters_use_is_actually_in_the_font() {
        // The failure this guards is silent: an unsupported character draws NOTHING, so a label
        // would just be missing on the panel with no error anywhere.
        for label in ["L", "R", "LO", "HI", "MID", "1", "2", "3", "4", "5", "6"] {
            let mut c = Canvas::new(40, 12);
            c.text_3x5(1, 1, label, Rgba::new(255, 255, 255, 255));
            let lit = c.bits().iter().filter(|p| **p != 0).count();
            assert!(lit > 0, "label {label:?} drew nothing - a character is missing from the font");
        }
    }

    #[test]
    fn an_unsupported_character_leaves_a_gap_and_still_advances() {
        let mut c = Canvas::new(40, 12);
        // '@' is not in the set. It must advance the cursor and draw nothing - so the R lands in
        // the third cell rather than sliding into the second.
        c.text_3x5(1, 1, "L@R", Rgba::new(255, 255, 255, 255));

        // The middle cell must be completely empty.
        for x in 5..8 {
            for y in 1..6 {
                assert_eq!(
                    c.get(x, y).a, 0,
                    "the unsupported glyph's cell must be blank, but ({x},{y}) is lit"
                );
            }
        }
        // And the third cell must hold the R, proving the cursor advanced past the gap rather
        // than the string being silently compacted.
        let third_cell_lit = (9..12).any(|x| (1..6).any(|y| c.get(x, y).a > 0));
        assert!(third_cell_lit, "'R' should have been drawn in the third cell");
        assert_eq!(Canvas::text_3x5_width("L@R"), 11);
    }

    #[test]
    fn scale_alpha_fades_without_breaking_the_premultiplied_invariant() {
        for k in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let mut c = Canvas::new(8, 8);
            c.fill_rect(0, 0, 8, 8, Rgba::new(0x8f, 0xe4, 0xff, 255));
            c.scale_alpha(k);
            for i in 0..c.bits().len() {
                let p = c.bits()[i];
                let (a, r, g, b) = (p >> 24, (p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
                assert!(r <= a && g <= a && b <= a, "invariant broken at k={k}");
            }
            let a = c.bits()[0] >> 24;
            let want = (255.0 * k).round() as u32;
            assert!(
                a.abs_diff(want) <= 1,
                "alpha should scale to ~{want} at k={k}, got {a}"
            );
        }
    }

    #[test]
    fn scale_alpha_of_zero_clears_and_of_one_is_a_no_op() {
        let mut c = Canvas::new(4, 4);
        c.fill_rect(0, 0, 4, 4, Rgba::new(10, 20, 30, 200));
        let before = c.bits().to_vec();
        c.scale_alpha(1.0);
        assert_eq!(c.bits(), &before[..], "k=1 must not touch the pixels");
        c.scale_alpha(0.0);
        assert!(c.bits().iter().all(|&p| p == 0), "k=0 must clear");
    }

    #[test]
    fn bloom_never_breaks_the_premultiplied_invariant() {
        // Scaling alpha past r/g/b produces opaque dark pixels - a black wash. Any
        // strength, however extreme, must keep r,g,b <= a.
        for strength in [0.5f32, 2.0, 8.0, 40.0] {
            let mut c = Canvas::new(21, 21);
            c.fill_rect(9, 9, 3, 3, Rgba::new(0x8f, 0xe4, 0xff, 255));
            c.bloom(6, strength);
            for y in 0..21 {
                for x in 0..21 {
                    let i = (y * 21 + x) as usize;
                    let p = c.bits()[i];
                    let (a, r, g, b) = (p >> 24, (p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
                    assert!(
                        r <= a && g <= a && b <= a,
                        "premultiplied invariant broken at ({x},{y}) strength {strength}: a={a} r={r} g={g} b={b} - this is what makes a halo go black"
                    );
                }
            }
        }
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

    // ---- line ----

    #[test]
    fn line_draws_a_horizontal_segment() {
        let mut c = Canvas::new(10, 10);
        let red = Rgba::new(255, 0, 0, 255);
        c.line(1, 5, 6, 5, red);
        for x in 1..=6 {
            assert_eq!(c.get(x, 5), red, "x={x}");
        }
        assert_eq!(c.get(0, 5), Rgba::TRANSPARENT, "left of the segment");
        assert_eq!(c.get(7, 5), Rgba::TRANSPARENT, "right of the segment");
    }

    #[test]
    fn line_draws_a_vertical_segment() {
        let mut c = Canvas::new(10, 10);
        let red = Rgba::new(255, 0, 0, 255);
        c.line(4, 1, 4, 6, red);
        for y in 1..=6 {
            assert_eq!(c.get(4, y), red, "y={y}");
        }
        assert_eq!(c.get(4, 0), Rgba::TRANSPARENT);
        assert_eq!(c.get(4, 7), Rgba::TRANSPARENT);
    }

    #[test]
    fn line_draws_both_diagonal_directions() {
        let mut c = Canvas::new(10, 10);
        let red = Rgba::new(255, 0, 0, 255);
        c.line(0, 0, 3, 3, red);
        for i in 0..=3 {
            assert_eq!(c.get(i, i), red, "down-right diagonal at {i}");
        }

        let mut c2 = Canvas::new(10, 10);
        c2.line(3, 0, 0, 3, red);
        for i in 0..=3 {
            assert_eq!(c2.get(3 - i, i), red, "down-left diagonal at {i}");
        }
    }

    #[test]
    fn line_single_point_draws_exactly_one_pixel() {
        let mut c = Canvas::new(10, 10);
        let red = Rgba::new(255, 0, 0, 255);
        c.line(5, 5, 5, 5, red);
        assert_eq!(c.get(5, 5), red);
        let lit = c.bits().iter().filter(|&&p| p != 0).count();
        assert_eq!(lit, 1, "exactly one pixel must be touched");
    }

    #[test]
    fn line_clips_a_segment_that_crosses_the_canvas_without_panicking() {
        let mut c = Canvas::new(10, 10);
        let red = Rgba::new(255, 0, 0, 255);
        // Both endpoints are off-canvas but the diagonal passes through it.
        c.line(-5, -5, 15, 15, red);
        for i in 0..10 {
            assert_eq!(c.get(i, i), red, "diagonal should still cross the visible canvas at {i}");
        }
    }

    #[test]
    fn line_entirely_offcanvas_draws_nothing_and_does_not_panic() {
        let mut c = Canvas::new(10, 10);
        let red = Rgba::new(255, 0, 0, 255);
        c.line(-20, -20, -5, -5, red);
        assert!(c.bits().iter().all(|&p| p == 0), "nothing on-canvas to draw");
    }

    #[test]
    fn line_produces_a_connected_run_for_a_shallow_diagonal() {
        let mut c = Canvas::new(12, 5);
        let red = Rgba::new(255, 0, 0, 255);
        c.line(0, 0, 10, 3, red);
        let mut ys = Vec::new();
        for x in 0..=10 {
            let mut hit = None;
            for y in 0..5 {
                if c.get(x, y) == red {
                    assert!(hit.is_none(), "more than one lit pixel in column {x}");
                    hit = Some(y);
                }
            }
            ys.push(hit.unwrap_or_else(|| panic!("no gap allowed: column {x} has no lit pixel")));
        }
        for i in 1..ys.len() {
            let step = (ys[i] - ys[i - 1]).abs();
            assert!(step <= 1, "gap between column {} and {}: y jumped by {}", i - 1, i, step);
        }
    }

    // ---- fill_poly ----

    #[test]
    fn fill_poly_matches_fill_rect_on_a_rectangle_shaped_polygon() {
        let white = Rgba::new(255, 255, 255, 255);
        let mut drawn = Canvas::new(20, 20);
        drawn.fill_rect(3, 4, 10, 8, white);

        let mut polyed = Canvas::new(20, 20);
        polyed.fill_poly(&[(3, 4), (13, 4), (13, 12), (3, 12)], white);

        assert_eq!(drawn.bits(), polyed.bits());
    }

    #[test]
    fn fill_poly_with_fewer_than_three_points_draws_nothing() {
        let white = Rgba::new(255, 255, 255, 255);
        for pts in [vec![], vec![(1, 1)], vec![(1, 1), (5, 5)]] {
            let mut c = Canvas::new(10, 10);
            c.fill_poly(&pts, white);
            assert!(c.bits().iter().all(|&p| p == 0), "{} points must draw nothing", pts.len());
        }
    }

    #[test]
    fn fill_poly_fills_a_concave_l_shape_correctly() {
        // (0,0)-(6,0)-(6,3)-(3,3)-(3,6)-(0,6): an L, concave at (3,3).
        let white = Rgba::new(255, 255, 255, 255);
        let mut c = Canvas::new(10, 10);
        c.fill_poly(&[(0, 0), (6, 0), (6, 3), (3, 3), (3, 6), (0, 6)], white);
        assert_eq!(c.get(5, 1), white, "inside the bottom bar of the L");
        assert_eq!(c.get(1, 4), white, "inside the left bar of the L");
        assert_eq!(c.get(5, 4), Rgba::TRANSPARENT, "inside the notch - must stay empty");
    }

    #[test]
    fn fill_poly_entirely_offcanvas_does_not_panic() {
        let white = Rgba::new(255, 255, 255, 255);
        let mut c = Canvas::new(10, 10);
        c.fill_poly(&[(-50, -50), (-40, -50), (-40, -40), (-50, -40)], white);
        assert!(c.bits().iter().all(|&p| p == 0));
    }

    #[test]
    fn fill_poly_clips_a_polygon_that_straddles_the_canvas_edge() {
        let white = Rgba::new(255, 255, 255, 255);
        let mut c = Canvas::new(10, 10);
        c.fill_poly(&[(-5, -5), (5, -5), (5, 5), (-5, 5)], white);
        assert_eq!(c.get(0, 0), white, "the overlapping corner is filled");
        assert_eq!(c.get(9, 9), Rgba::TRANSPARENT, "far corner is outside the polygon");
    }

    #[test]
    fn fill_poly_handles_a_triangle_with_a_horizontal_base_without_double_filling() {
        // Apex at the top, horizontal base at the bottom - the base edge is
        // skipped as a crossing, so the fill must come entirely from the two
        // slanted sides and should narrow strictly toward the apex.
        let white = Rgba::new(255, 255, 255, 255);
        let mut c = Canvas::new(12, 6);
        c.fill_poly(&[(0, 5), (10, 5), (5, 0)], white);
        let width_at = |c: &Canvas, y: i32| (0..12).filter(|&x| c.get(x, y) == white).count();
        let near_apex = width_at(&c, 1);
        let near_base = width_at(&c, 4);
        assert!(near_apex > 0, "must draw something near the apex");
        assert!(near_base > near_apex, "must be wider near the base ({near_base}) than near the apex ({near_apex})");
    }

    #[test]
    fn fill_poly_flat_bottom_row_at_the_literal_max_y_is_excluded_matching_fill_rect() {
        // Documents the half-open convention explicitly: a flat-bottomed
        // polygon whose bottom vertices sit AT the last row you want filled
        // (rather than one past it) leaves that literal row unfilled - the
        // same "exclusive far edge" fill_rect(x, y, w, h) already has (it
        // stops at row y + h - 1, not y + h). This is not a bug: it is what
        // makes fill_poly reproduce fill_rect exactly on a rectangle-shaped
        // polygon (see the test above), which the brief calls out as the
        // strongest correctness check available for this primitive.
        let white = Rgba::new(255, 255, 255, 255);
        let width_at = |c: &Canvas, y: i32| (0..12).filter(|&x| c.get(x, y) == white).count();

        // Base vertices AT y=5 (the last row of a 6-row canvas): row 5 is
        // excluded, exactly like fill_rect(_, _, _, h=5) would exclude it.
        let mut at_last_row = Canvas::new(12, 6);
        at_last_row.fill_poly(&[(0, 5), (10, 5), (5, 0)], white);
        assert_eq!(width_at(&at_last_row, 5), 0, "the literal max-y row is excluded, same as fill_rect's far edge");

        // Base vertices AT y=6 (one PAST the last desired row): now row 5
        // is filled, because it is no longer the polygon's own max-y row.
        let mut one_past = Canvas::new(12, 6);
        one_past.fill_poly(&[(0, 6), (10, 6), (5, 0)], white);
        assert!(width_at(&one_past, 5) > 0, "closing one row past the target, like fill_rect's h, fills the target row");
    }

    // ---- vertical_gradient ----

    #[test]
    fn vertical_gradient_hits_exact_endpoint_colours() {
        let top = Rgba::new(0xff, 0xf6, 0xd0, 255);
        let bottom = Rgba::new(0xff, 0x5f, 0x93, 255);
        let mut c = Canvas::new(5, 29);
        c.vertical_gradient(0, 0, 5, 29, &[(0.0, top), (1.0, bottom)], false);
        assert_eq!(c.get(2, 0), top, "position 0.0 must be exact");
        assert_eq!(c.get(2, 28), bottom, "position 1.0 must be exact");
    }

    #[test]
    fn vertical_gradient_middle_stop_lands_near_its_authored_position() {
        let top = Rgba::new(0xff, 0xf6, 0xd0, 255);
        let mid = Rgba::new(0xff, 0xd7, 0x6e, 255);
        let bottom = Rgba::new(0xff, 0x5f, 0x93, 255);
        let mut c = Canvas::new(1, 29);
        c.vertical_gradient(0, 0, 1, 29, &[(0.0, top), (0.35, mid), (1.0, bottom)], false);
        let row = (0.35f32 * 28.0).round() as i32;
        let got = c.get(0, row);
        assert!(
            (got.g as i32 - mid.g as i32).abs() <= 2,
            "row {row} (t=0.35) should be close to the middle stop, got {got:?}"
        );
    }

    #[test]
    fn vertical_gradient_respects_bounds_and_leaves_the_rest_transparent() {
        let mut c = Canvas::new(10, 10);
        c.vertical_gradient(2, 2, 4, 4, &[(0.0, Rgba::new(255, 255, 255, 255)), (1.0, Rgba::new(0, 0, 0, 255))], false);
        assert_eq!(c.get(0, 0), Rgba::TRANSPARENT, "outside the rect");
        assert_eq!(c.get(2, 2), Rgba::new(255, 255, 255, 255));
    }

    #[test]
    fn vertical_gradient_ordered_dither_breaks_up_a_flat_quantisation_band() {
        let stops = [(0.0, Rgba::new(0, 0, 0, 255)), (1.0, Rgba::new(255, 0, 0, 255))];

        let mut flat = Canvas::new(4, 3);
        flat.vertical_gradient(0, 0, 4, 3, &stops, false);
        let row: Vec<u8> = (0..4).map(|x| flat.get(x, 1).r).collect();
        assert_eq!(row, vec![128, 128, 128, 128], "without dithering row 1 quantises flat");

        let mut dithered = Canvas::new(4, 3);
        dithered.vertical_gradient(0, 0, 4, 3, &stops, true);
        let row: Vec<u8> = (0..4).map(|x| dithered.get(x, 1).r).collect();
        assert!(
            row.iter().any(|&v| v != row[0]),
            "with dithering row 1 must not be perfectly flat, got {row:?}"
        );
    }

    #[test]
    fn vertical_gradient_never_breaks_the_premultiplied_invariant() {
        let translucent = Rgba::new(255, 10, 200, 40);
        let mut c = Canvas::new(20, 20);
        c.vertical_gradient(-5, -5, 30, 30, &[(0.0, translucent), (1.0, Rgba::new(0, 0, 0, 10))], true);
        assert_invariant(&c, "vertical_gradient");
    }

    // ---- radial_gradient ----

    #[test]
    fn radial_gradient_hits_first_stop_exactly_inside_r_inner() {
        let inner_c = Rgba::new(255, 255, 200, 255);
        let outer_c = Rgba::new(50, 0, 0, 255);
        let mut c = Canvas::new(40, 40);
        c.radial_gradient(20, 20, 3, 15, &[(0.0, inner_c), (1.0, outer_c)]);
        assert_eq!(c.get(20, 20), inner_c, "centre is inside r_inner");
        assert_eq!(c.get(22, 20), inner_c, "distance 2 <= r_inner=3");
    }

    #[test]
    fn radial_gradient_hits_last_stop_exactly_at_r_outer() {
        let inner_c = Rgba::new(255, 255, 200, 255);
        let outer_c = Rgba::new(50, 0, 0, 255);
        let mut c = Canvas::new(40, 40);
        c.radial_gradient(20, 20, 3, 15, &[(0.0, inner_c), (1.0, outer_c)]);
        assert_eq!(c.get(35, 20), outer_c, "exactly at r_outer along the x axis");
    }

    #[test]
    fn radial_gradient_draws_nothing_beyond_r_outer_not_the_last_stop() {
        let inner_c = Rgba::new(255, 255, 200, 255);
        let outer_c = Rgba::new(50, 0, 0, 255);
        let mut c = Canvas::new(40, 40);
        c.radial_gradient(20, 20, 3, 15, &[(0.0, inner_c), (1.0, outer_c)]);
        assert_eq!(c.get(39, 39), Rgba::TRANSPARENT, "outside r_outer must stay untouched, not filled with outer_c");
    }

    #[test]
    fn radial_gradient_centre_far_offcanvas_does_not_panic() {
        let mut c = Canvas::new(10, 10);
        c.radial_gradient(-100_000, -100_000, 3, 15, &[(0.0, Rgba::new(255, 255, 255, 255)), (1.0, Rgba::new(0, 0, 0, 255))]);
        assert!(c.bits().iter().all(|&p| p == 0), "no overlap with the canvas");

        let mut c2 = Canvas::new(10, 10);
        c2.radial_gradient(i32::MIN, i32::MIN, 3, i32::MAX, &[(0.0, Rgba::new(255, 255, 255, 255))]);
        // Must not panic; overlap behaviour is unspecified at this extreme.
        let _ = c2.bits();
    }

    #[test]
    fn radial_gradient_never_breaks_the_premultiplied_invariant() {
        let translucent = Rgba::new(255, 10, 200, 40);
        let mut c = Canvas::new(40, 40);
        c.radial_gradient(20, 20, -10, 100, &[(0.0, translucent), (1.0, Rgba::new(0, 0, 0, 10))]);
        assert_invariant(&c, "radial_gradient");
    }

    // ---- fill_circle / fill_semicircle_upper ----

    #[test]
    fn fill_circle_draws_a_symmetric_disc() {
        let white = Rgba::new(255, 255, 255, 255);
        let mut c = Canvas::new(21, 21);
        c.fill_circle(10, 10, 5, white);
        assert_eq!(c.get(10, 10), white, "centre");
        assert_eq!(c.get(10, 5), white, "top of the disc");
        assert_eq!(c.get(10, 15), white, "bottom of the disc");
        assert_eq!(c.get(5, 10), white, "left of the disc");
        assert_eq!(c.get(15, 10), white, "right of the disc");
        assert_eq!(c.get(10, 0), Rgba::TRANSPARENT, "well outside the disc");
    }

    #[test]
    fn fill_circle_non_positive_radius_draws_nothing() {
        let white = Rgba::new(255, 255, 255, 255);
        for r in [0, -1, -100] {
            let mut c = Canvas::new(10, 10);
            c.fill_circle(5, 5, r, white);
            assert!(c.bits().iter().all(|&p| p == 0), "r={r} must draw nothing");
        }
    }

    #[test]
    fn fill_semicircle_upper_only_fills_the_top_half() {
        let white = Rgba::new(255, 255, 255, 255);
        let mut c = Canvas::new(21, 21);
        c.fill_semicircle_upper(10, 10, 5, white);
        assert_eq!(c.get(10, 6), white, "above centre, inside the dome");
        assert_eq!(c.get(10, 10), white, "the diameter row itself is included");
        assert_eq!(c.get(10, 14), Rgba::TRANSPARENT, "below centre must stay empty");
    }

    #[test]
    fn fill_circle_centre_far_offcanvas_does_not_panic() {
        let white = Rgba::new(255, 255, 255, 255);
        let mut c = Canvas::new(10, 10);
        c.fill_circle(-1_000_000, -1_000_000, 50, white);
        assert!(c.bits().iter().all(|&p| p == 0));
    }

    #[test]
    fn fill_circle_huge_radius_does_not_overflow() {
        // r*r as native i32 multiplication overflows above ~46,340 (sqrt(i32::MAX));
        // this must go through f32 before squaring, mirroring radial_gradient's
        // (px - cx) overflow fix. Must not panic, on-canvas rows must still fill.
        // (fill_circle_rows loops dy over -r..=r, so r itself must stay small enough
        // to run in a unit test - the point here is exercising the r*r multiply
        // above the i32 overflow threshold, not an astronomically large radius.)
        let white = Rgba::new(255, 255, 255, 255);
        let mut c = Canvas::new(10, 10);
        c.fill_circle(5, 5, 50_000, white);
        assert_eq!(c.get(5, 5), white, "centre still filled despite the huge radius");
    }

    // ---- clip_outside_rect ----

    #[test]
    fn clip_outside_rect_zeroes_everything_outside_but_keeps_corners() {
        let white = Rgba::new(255, 255, 255, 255);
        let mut c = Canvas::new(10, 10);
        c.fill_rect(0, 0, 10, 10, white);
        c.clip_outside_rect(2, 2, 4, 4);
        assert_eq!(c.get(0, 0), Rgba::TRANSPARENT, "outside the rect");
        assert_eq!(c.get(2, 2), white, "unlike a rounded clip, the plain corner survives");
        assert_eq!(c.get(5, 5), white, "still inside the rect - x,y in [2,6) x [2,6)");
        assert_eq!(c.get(6, 6), Rgba::TRANSPARENT, "just past the rect's far edge");
    }

    #[test]
    fn clip_outside_rect_matches_what_fill_rect_would_have_drawn() {
        let white = Rgba::new(255, 255, 255, 255);
        let (x, y, w, h) = (3, 4, 10, 8);
        let mut drawn = Canvas::new(20, 20);
        drawn.fill_rect(x, y, w, h, white);

        let mut clipped = Canvas::new(20, 20);
        clipped.fill_rect(0, 0, 20, 20, white);
        clipped.clip_outside_rect(x, y, w, h);

        assert_eq!(drawn.bits(), clipped.bits());
    }

    // ---- cross-primitive invariant sweep, per the task's "extreme inputs" requirement ----

    #[test]
    fn every_new_primitive_preserves_the_premultiplied_invariant_at_extreme_inputs() {
        let translucent = Rgba::new(255, 10, 200, 40);

        let mut c = Canvas::new(40, 40);
        c.line(-5, -5, 45, 45, translucent);
        assert_invariant(&c, "line");

        let mut c = Canvas::new(40, 40);
        c.fill_poly(&[(-5, -5), (45, -5), (45, 45), (-5, 45)], translucent);
        assert_invariant(&c, "fill_poly");

        let mut c = Canvas::new(40, 40);
        c.fill_circle(20, 20, 1000, translucent);
        assert_invariant(&c, "fill_circle");

        let mut c = Canvas::new(40, 40);
        c.fill_semicircle_upper(20, 20, 1000, translucent);
        assert_invariant(&c, "fill_semicircle_upper");
    }

    // Diagnostic only, like `sweep_edge_glow`/`sweep_glow_strength` below in
    // this crate - not a pinned golden, does not run in `cargo test`. Renders
    // the real vaporwave-sky shape (3 stops, ~29 rows of a 190-wide, 60-tall
    // canvas) with and without ordered dithering and dumps both to stdout,
    // which is how the "is dithering needed at this size" call in the task
    // report was actually made rather than assumed. Run with
    // `cargo test canvas::tests::render_sky_gradient_for_visual_inspection -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn render_sky_gradient_for_visual_inspection() {
        let stops = [
            (0.0, Rgba::new(0xff, 0xf6, 0xd0, 255)),
            (0.35, Rgba::new(0xff, 0xd7, 0x6e, 255)),
            (1.0, Rgba::new(0xff, 0x5f, 0x93, 255)),
        ];
        let horizon = (60.0f32 * 0.48).round() as i32; // 29
        for (label, dither) in [("WITHOUT dithering", false), ("WITH ordered dithering", true)] {
            let mut c = Canvas::new(190, 60);
            c.vertical_gradient(0, 0, 190, horizon, &stops, dither);
            println!("--- sky gradient, {label} ({horizon} rows) ---");
            print!("{}", super::super::golden::canvas_to_ascii(&c));
            let mut prev: Option<Rgba> = None;
            let mut max_run = 1;
            let mut run = 1;
            for row in 0..horizon {
                let p = c.get(0, row);
                if Some(p) == prev {
                    run += 1;
                    max_run = max_run.max(run);
                } else {
                    run = 1;
                }
                prev = Some(p);
                println!("row {row:2}: {p:?}");
            }
            println!("longest run of identical consecutive rows: {max_run}");
        }
    }
}
