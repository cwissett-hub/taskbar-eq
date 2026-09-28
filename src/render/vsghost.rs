//! The Virtual Self "Ghost Voices" family: a glitch terminal where the spectrum is a rolling score.
//!
//! The sibling of `vswings` - the same early-2000s Virtual Self world, but the cold, digital half of it
//! rather than the chrome-and-cherub one. The meter is a PIANO ROLL: every band is a narrow column of
//! stacked ticks, a MIDI grid, and a band's level lights its ticks from the bottom up. The peak tick
//! stays lit and decays, so a loud transient leaves a bright mark hanging above the settled level. On an
//! onset the whole set of columns scrolls one column sideways, so the panel reads as a score rolling
//! past rather than a static bar row.
//!
//! # Why a piano roll is a meter and not just an ornament
//!
//! A column of lit ticks IS a segmented bar - the segmented VFD's reading, drawn in a MIDI editor's
//! clothes. Reading the panel is reading the height of each column, exactly as a bar row is read, and
//! the horizontal axis is frequency, low on the left. The resting GRID is drawn faintly in `ghost` so
//! the score is always visible even in silence, and it deliberately fills only the lower two-thirds:
//! that leaves the top third as headroom the loudest transients punch up into, with nothing but the
//! resting phrase there until the music reaches for it. A glance still resolves "where is the energy"
//! the way a straight bar row does.
//!
//! # The panel is opaque
//!
//! Like every other family this one paints an opaque panel (black, or white for the inverse colourway)
//! that covers the Windows weather widget while music plays. Everything - grid, ticks, rays, phrase - is
//! drawn over that panel, and the datamosh flourish composites over it too.
//!
//! # The three quiet parts around the meter
//!
//! - a short ROMANISED phrase in a private 3x5 font at the top-left, in `hot`, swapped on every fourth
//!   strong onset - a line of the "ghost voices" the family is named for. The font is private to this
//!   file (like `vswings`'s title font) because the shared `canvas::glyph_3x5` deliberately omits half
//!   the alphabet as ambiguous at 3x5; the phrases here are fixed and known, so their letters can be
//!   drawn unambiguously. The kaomoji in the design brief cannot be drawn - the 3x5 cell has no room for
//!   `˘ ω ´ •` - so they are the ASCII substitutes `(^_^)` and `(>_<)`, and a test asserts every
//!   character of every phrase has a glyph.
//! - two to four translucent light RAYS rising from the bottom edge, whose angle drifts with the
//!   spectral centroid and whose intensity follows RMS, in `edge`. Subtle on purpose - they are the
//!   atmosphere behind the score, not a second meter.
//! - the FLOURISH is a datamosh: three to five horizontal slices of the composed frame are shifted
//!   sideways by random offsets (wrapping) and colour-inverted for a few frames, with a hard white
//!   strobe on the very first frame. The inversion is `255 - c` per channel - the effect itself, not a
//!   colour mix - and the frame after the envelope ends is clean.
//!
//! # Colour goes through linear light, and no rainbow
//!
//! There is no rainbow colourway here: the identity is a cold terminal - white, cobalt/electric, an
//! inverse (black on white) and a violet/pink set. Ticks are `lit`, peaks and phrase are `hot`, mixed
//! through the shared linear-light blend when they are translucent (the ghost grid, the rays), never by
//! averaging sRGB bytes.

use crate::dsp::bands::NUM_BANDS;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// The romanised phrases and kaomoji, cycled on strong onsets. Only characters the private 3x5 font
/// (`glyph`) can draw - the brief's kaomoji are the ASCII substitutes it names, because the 3x5 cell
/// cannot render `˘ ω ´ •`. `every_phrase_character_has_a_glyph` is the guard on that.
const PHRASES: [&str; 7] =
    ["(^_^)", "(>_<)", "GHOST VOICES", "EON BREAK", "A.I.NGEL", "PARTICLE ARTS", "UTOPIA"];

/// Column geometry: a 2px-wide column with a 1px gap, so the pitch is 3px.
const COL_W: i32 = 2;
const COL_PITCH: i32 = 3;

/// Tick geometry within a column: a 1px-tall tick with a 1px gap, so the pitch is 2px.
const TICK_PITCH: i32 = 2;

/// The panel is at least this large before the family draws its meter - smaller, and it sheds rather
/// than smudging, exactly as the other families do.
const MIN_W: i32 = 60;
const MIN_H: i32 = 18;

/// How long the datamosh runs, in milliseconds. ~4 frames at 16.7ms, per the design brief.
const MOSH_MS: f32 = 70.0;

/// The onset net that scrolls the score and swaps the phrase - the same permissive flux net the
/// flourish uses to find candidates.
const SCROLL_ONSET_RATIO: f32 = 2.8;
const SCROLL_ONSET_REFRACTORY_MS: f32 = 200.0;

/// Display gain from level to tick fill, so a realistic 0.6-0.8 level reaches most of the column.
const FILL_GAIN: f32 = 1.25;

/// Rays: how many, and how wide at the tip in pixels.
const RAYS: usize = 3;
const RAY_HALF_W: f32 = 5.0;

// The fill-smoothing ballistics and the peak-hold decay come from the theme's `ballistics`.

pub struct Vsghost {
    /// Fires the datamosh on a rare, exceptional hit - see `dsp::flourish`.
    flourish: crate::dsp::flourish::Trigger,
    /// The datamosh's one-shot decay envelope.
    mosh: crate::dsp::flourish::Envelope,
    /// The onset that scrolls the score and swaps the phrase. A local detector, because the score
    /// scrolls far more often than the flourish fires.
    onset: crate::dsp::onset::Flux,
    /// Strong onsets counted, so the phrase swaps on every fourth.
    onset_count: u32,
    /// Horizontal scroll of the columns, in pixels, advanced one column per onset and wrapped.
    scroll_px: f32,
    /// Which phrase is showing.
    phrase_idx: usize,
    /// A phrase forced by a test, set once - see `set_phrase_for_test`.
    phrase_override: Option<String>,
    /// Smoothed per-band fill, so a column does not jitter frame to frame.
    fill: [f32; NUM_BANDS],
    /// Peak-hold fill per band, decaying at `peak_fall`.
    peak_ticks: [f32; NUM_BANDS],
    /// Smoothed ray angles, in radians from vertical.
    ray_angles: [f32; RAYS],
    /// The datamosh's frame copy, sized once per panel size and reused - see the allocation note.
    scratch: Option<Canvas>,
    /// splitmix64 state, for the datamosh's slice choices.
    rng: u64,
}

impl Default for Vsghost {
    fn default() -> Self {
        Vsghost {
            flourish: Default::default(),
            mosh: Default::default(),
            onset: Default::default(),
            onset_count: 0,
            scroll_px: 0.0,
            phrase_idx: 0,
            phrase_override: None,
            fill: [0.0; NUM_BANDS],
            peak_ticks: [0.0; NUM_BANDS],
            ray_angles: [0.0; RAYS],
            scratch: None,
            rng: 0x853c_49e6_748f_ea9b,
        }
    }
}

impl Vsghost {
    /// Forces a phrase, for `long_phrase_is_clipped`. The one allocation is here, in the test hook,
    /// once - never in `draw`.
    #[cfg(test)]
    pub fn set_phrase_for_test(&mut self, s: &str) {
        self.phrase_override = Some(s.to_string());
    }

    /// One draw of splitmix64.
    fn next_rng(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The phrase in the private 3x5 font, top-left at `(x0, y0)`, in `col`, clipped so it never draws
    /// past `x0 + maxw`. Pre-truncation by width rather than wrapping: a phrase too long for the panel
    /// simply stops at the right edge, and unknown characters advance the cursor and draw nothing.
    fn draw_phrase(c: &mut Canvas, x0: i32, y0: i32, s: &str, maxw: i32, col: Rgba) {
        let mut cx = x0;
        for ch in s.chars() {
            if cx + 3 > x0 + maxw {
                break;
            }
            if let Some(rows) = glyph(ch.to_ascii_uppercase()) {
                for (dy, row) in rows.iter().enumerate() {
                    for dx in 0..3 {
                        if row & (0b100 >> dx) != 0 {
                            c.fill_rect(cx + dx, y0 + dy as i32, 1, 1, col);
                        }
                    }
                }
            }
            cx += 4;
        }
    }
}

/// One glyph of the private phrase font, five rows of three bits with bit 2 leftmost. `None` for any
/// character not in a phrase, so a stray character degrades to a gap rather than a wrong glyph. Covers
/// exactly the letters, digit-free, and punctuation the `PHRASES` use.
fn glyph(ch: char) -> Option<[u8; 5]> {
    Some(match ch {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b111, 0b101, 0b111, 0b100, 0b100],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        '^' => [0b010, 0b101, 0b000, 0b000, 0b000],
        '_' => [0b000, 0b000, 0b000, 0b000, 0b111],
        '>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        '<' => [0b001, 0b010, 0b100, 0b010, 0b001],
        ' ' => [0, 0, 0, 0, 0],
        _ => return None,
    })
}

impl Family for Vsghost {
    fn id(&self) -> &'static str {
        "vsghost"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        if w < MIN_W || h < MIN_H {
            return; // shed rather than smudge
        }
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 250.0) } else { 16.7 };

        // ---- the opaque panel ----
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);

        // The interior the meter lives in. `ix1`/`iy1` are exclusive bounds.
        let (ix0, iy0) = (2i32, 2i32);
        let (ix1, iy1) = (w - 2, h - 2);
        let iw = ix1 - ix0;
        let ih = iy1 - iy0;

        // The grid fills only the lower two-thirds; the top third is headroom the loud transients
        // punch into. See the module note - it is also what makes the resting grid and the loud peaks
        // separable in the level test.
        let ghost_top = iy0 + ih / 3;

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let ghost = Rgba::from_hex(&t.lit, t.ghost.clamp(0.0, 1.0));

        // ---- onset: scroll the score, count for the phrase swap ----
        let onset = self.onset.update(&d.levels, dt, SCROLL_ONSET_RATIO, SCROLL_ONSET_REFRACTORY_MS);
        if onset {
            self.scroll_px += COL_PITCH as f32;
            self.onset_count = self.onset_count.wrapping_add(1);
            if self.onset_count.is_multiple_of(4) && self.phrase_override.is_none() {
                self.phrase_idx = (self.phrase_idx + 1) % PHRASES.len();
            }
        }
        if !self.scroll_px.is_finite() {
            self.scroll_px = 0.0;
        }
        let scroll = self.scroll_px.rem_euclid(iw as f32);

        // ---- the piano roll ----
        let attack = t.ballistics.attack.clamp(0.0, 1.0);
        let decay = t.ballistics.decay.clamp(0.0, 1.0);
        let peak_fall = t.ballistics.peak_fall.max(0.0);
        let num_cols = (iw / COL_PITCH).max(1);

        for k in 0..num_cols {
            let band = ((k as i64 * NUM_BANDS as i64) / num_cols as i64).min(NUM_BANDS as i64 - 1) as usize;
            let level_i = if d.levels[band].is_finite() { d.levels[band].clamp(0.0, 1.0) } else { 0.0 };
            let target = (level_i * FILL_GAIN * t.sensitivity.max(0.0)).clamp(0.0, 1.0);
            let cur = self.fill[band];
            let kf = if target > cur { attack } else { decay };
            let mut f = cur + (target - cur) * kf;
            if !f.is_finite() {
                f = 0.0;
            }
            self.fill[band] = f;
            // Peak hold, decaying slowly.
            let mut pk = (self.peak_ticks[band] - peak_fall).max(f);
            if !pk.is_finite() {
                pk = f;
            }
            self.peak_ticks[band] = pk;

            // The column's on-screen x, wrapped by the scroll. Drawn twice when it straddles the
            // right edge so the wrap is seamless.
            let base = (k * COL_PITCH) as f32 + scroll;
            let x = ix0 + base.rem_euclid(iw as f32).round() as i32;
            for xoff in [0i32, -iw] {
                let cx = x + xoff;
                if cx + COL_W <= ix0 || cx >= ix1 {
                    continue;
                }
                self.draw_column(c, cx, iy0, iy1, ghost_top, f, pk, lit, hot, ghost);
            }
        }

        // ---- the rays ----
        self.draw_rays(c, t, d, (ix0, iy0, ix1, iy1));

        // ---- the phrase ----
        let phrase: &str = self.phrase_override.as_deref().unwrap_or(PHRASES[self.phrase_idx]);
        Self::draw_phrase(c, ix0 + 1, iy0, phrase, iw - 2, hot);

        // ---- the flourish: a datamosh over the composed frame ----
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let mosh = self.mosh.update(fired, dt, MOSH_MS);
        if mosh > 0.0 {
            self.datamosh(c, fired, (ix0, iy0, ix1, iy1));
        }

        // Keep nothing on the rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

impl Vsghost {
    /// One column of the piano roll: faint `ghost` ticks in the lower two-thirds so the grid is always
    /// there, `lit` ticks from the bottom up to the fill, and a `hot` peak tick where the peak hangs.
    #[allow(clippy::too_many_arguments)]
    fn draw_column(
        &self,
        c: &mut Canvas,
        cx: i32,
        iy0: i32,
        iy1: i32,
        ghost_top: i32,
        fill: f32,
        peak: f32,
        lit: Rgba,
        hot: Rgba,
        ghost: Rgba,
    ) {
        let ih = (iy1 - iy0) as f32;
        // Where the lit column and the peak tick reach, as a y coordinate (higher fill = smaller y).
        let lit_y = iy1 - (fill.clamp(0.0, 1.0) * ih).round() as i32;
        let peak_y = iy1 - (peak.clamp(0.0, 1.0) * ih).round() as i32;
        // Ticks on a fixed vertical lattice, so the grid does not crawl with the fill.
        let mut y = iy1 - 1;
        while y >= iy0 {
            let colour = if y >= lit_y {
                Some(lit)
            } else if y >= ghost_top {
                Some(ghost)
            } else {
                None
            };
            if let Some(col) = colour {
                c.fill_rect(cx, y, COL_W, 1, col);
            }
            y -= TICK_PITCH;
        }
        // The peak tick, above the lit column, in hot.
        if peak > fill + 0.02 {
            let py = peak_y.max(iy0);
            c.fill_rect(cx, py, COL_W, 1, hot);
        }
    }

    /// The light rays: `RAYS` translucent wedges rising from the bottom edge, their angle drifting
    /// toward the spectral centroid and their alpha following RMS.
    fn draw_rays(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData, bbox: (i32, i32, i32, i32)) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let rms = if d.rms_l.is_finite() { d.rms_l.clamp(0.0, 1.0) } else { 0.0 };
        let a = rms * 0.35;
        if a <= 0.01 {
            // Still advance the smoothed angle so it does not jump when the music returns.
        }
        // Spectral centroid, 0..1 across the bands.
        let (mut num, mut den) = (0.0f32, 0.0f32);
        for (i, &v) in d.levels.iter().enumerate() {
            let v = if v.is_finite() { v.max(0.0) } else { 0.0 };
            num += i as f32 * v;
            den += v;
        }
        let centroid = if den > 1.0e-4 { num / (den * (NUM_BANDS - 1) as f32) } else { 0.5 };
        // Map the centroid to +-35 degrees from vertical.
        let target = (centroid - 0.5) * 2.0 * 35.0f32.to_radians();
        let iw = (ix1 - ix0) as f32;
        let ih = (iy1 - iy0) as f32;
        let col = Rgba::from_hex(&t.edge, a.clamp(0.0, 1.0));
        for j in 0..RAYS {
            // Each ray fans a little around the centroid.
            let spread = (j as f32 - (RAYS as f32 - 1.0) * 0.5) * 12.0f32.to_radians();
            let tgt = target + spread;
            let cur = self.ray_angles[j];
            let mut ang = cur + (tgt - cur) * 0.08;
            if !ang.is_finite() {
                ang = 0.0;
            }
            self.ray_angles[j] = ang;
            if a <= 0.01 {
                continue; // angle kept fresh, but nothing drawn when silent
            }
            let bx = ix0 as f32 + iw * (0.5 + 0.18 * (j as f32 - 1.0));
            let by = iy1 as f32;
            let len = ih * 1.15;
            // A wedge ANCHORED at the bottom edge: a wide base on the bottom line narrowing to a
            // point at the top, so it reads as a beam rising from the bottom rather than hanging.
            let tipx = bx + ang.sin() * len;
            let tipy = by - ang.cos() * len;
            let pts = [
                ((bx - RAY_HALF_W).round() as i32, by.round() as i32),
                ((bx + RAY_HALF_W).round() as i32, by.round() as i32),
                (tipx.round() as i32, tipy.round() as i32),
            ];
            c.fill_poly(&pts, col);
        }
    }

    /// The datamosh: copy the composed frame, then shift 3-5 horizontal slices sideways (wrapping) and
    /// invert their colours, laying the result back over the frame. On the firing frame a hard white
    /// strobe covers the interior. Every write is clamped to the interior, so nothing is drawn off the
    /// canvas and there is no panic at a narrow size.
    fn datamosh(&mut self, c: &mut Canvas, fired: bool, bbox: (i32, i32, i32, i32)) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let (w, h) = (c.width(), c.height());
        let iw = ix1 - ix0;
        let ih = iy1 - iy0;
        if iw <= 0 || ih <= 0 {
            return;
        }
        // Copy the current frame into the reused scratch canvas.
        let mut scr = self
            .scratch
            .take()
            .filter(|s| s.width() == w && s.height() == h)
            .unwrap_or_else(|| Canvas::new(w, h));
        scr.clear();
        scr.draw_over(c);

        let nslices = 3 + (self.next_rng() % 3) as i32; // 3..=5
        for _ in 0..nslices {
            let sh = 1 + (self.next_rng() % 6) as i32; // 1..=6 rows
            let room = (ih - sh).max(1) as u64;
            let sy = iy0 + (self.next_rng() % room) as i32;
            let shift = (self.next_rng() % iw as u64) as i32;
            let y1 = (sy + sh).min(iy1);
            for y in sy..y1 {
                for x in ix0..ix1 {
                    let sx = ix0 + (x - ix0 - shift).rem_euclid(iw);
                    let p = scr.get(sx, y);
                    let inv = Rgba::new(255 - p.r, 255 - p.g, 255 - p.b, 255);
                    c.fill_rect(x, y, 1, 1, inv);
                }
            }
        }
        if fired {
            // The one-frame strobe.
            c.fill_rect(ix0, iy0, iw, ih, Rgba::from_hex("#ffffff", 0.8));
        }
        self.scratch = Some(scr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    fn frames(fam: &mut Vsghost, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
        let mut c = Canvas::new(w, h);
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = level * (1.0 - i as f32 / 96.0);
        }
        d.peaks = d.levels;
        d.rms_l = level;
        d.rms_r = level;
        d.dt_ms = 16.7;
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, t, &d);
        }
        c
    }
    /// Pixels the family DREW OVER the panel - opaque and differing from the panel colour by more than a
    /// rounding margin. The panel is opaque, so "lit" is paint on top of it, not raw alpha - the same
    /// adaptation `vswings`'s tests make.
    fn drew_over_panel(px: Rgba, panel: Rgba) -> bool {
        let d = (px.r as i32 - panel.r as i32).abs()
            + (px.g as i32 - panel.g as i32).abs()
            + (px.b as i32 - panel.b as i32).abs();
        px.a > 8 && d > 24
    }
    fn lit(c: &Canvas, t: &Theme) -> usize {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let mut n = 0;
        for y in 0..c.height() {
            for x in 0..c.width() {
                if drew_over_panel(c.get(x, y), panel) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"vsghost"));
        assert_eq!(crate::render::family_for("vsghost").id(), "vsghost");
        assert_ne!(crate::themes::family_label("vsghost"), "vsghost");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "vsghost").count(), 4);
    }

    #[test]
    fn ticks_light_upward_with_level() {
        let t = theme("vsghost-white");
        let quiet = frames(&mut Vsghost::default(), &t, 380, 48, 0.2, 30);
        let loud = frames(&mut Vsghost::default(), &t, 380, 48, 0.8, 30);
        let panel = Rgba::from_hex(&t.panel, 1.0);
        // Count painted pixels in the TOP third: only loud material should reach it.
        let top = |c: &Canvas| {
            (0..16)
                .flat_map(|y| (0..380).map(move |x| (x, y)))
                .filter(|&(x, y)| drew_over_panel(c.get(x, y), panel))
                .count()
        };
        assert!(top(&loud) > top(&quiet) * 2, "top third: quiet {} loud {}", top(&quiet), top(&loud));
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("vsghost-white");
        let c = frames(&mut Vsghost::default(), &t, 380, 48, 0.0, 10);
        assert!(
            lit(&c, &t) as f32 >= 0.02 * (380 * 48) as f32,
            "grid + phrase should show: {}",
            lit(&c, &t)
        );
    }

    #[test]
    fn long_phrase_is_clipped() {
        let t = theme("vsghost-white");
        let mut fam = Vsghost::default();
        fam.set_phrase_for_test("a very long phrase that cannot possibly fit in one hundred and ninety pixels of taskbar");
        let c = frames(&mut fam, &t, 190, 48, 0.3, 5);
        // No pixel outside the canvas can be written (draw completed without panic); the phrase is
        // pre-truncated by width, never wrapped past the right edge.
        assert!(lit(&c, &t) > 0);
    }

    #[test]
    fn datamosh_stays_inside_the_canvas_at_narrow_size() {
        for id in ["vsghost-white", "vsghost-cobalt", "vsghost-inverse", "vsghost-violet"] {
            let t = theme(id);
            let mut fam = Vsghost::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 12);
            assert!(lit(&c, &t) > 0, "{id}");
        }
    }

    #[test]
    fn every_phrase_character_has_a_glyph() {
        // The kaomoji in the brief cannot be drawn at 3x5, so the phrases use ASCII substitutes; this
        // is the guard that every character of every phrase resolves to a glyph rather than a gap.
        for p in PHRASES {
            for ch in p.chars() {
                assert!(glyph(ch.to_ascii_uppercase()).is_some(), "no glyph for {ch:?} in {p:?}");
            }
        }
    }

    /// Dumps for the eye test - composited over `#202020` like every other family's dump.
    ///
    /// Run: cargo test --release dump_vsghost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_vsghost() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let bg = Rgba::from_hex("#202020", 1.0);
        let write = |name: String, c: &Canvas| {
            let mut out = Vec::with_capacity((c.width() * c.height() * 4) as usize);
            for y in 0..c.height() {
                for x in 0..c.width() {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for (ch, base) in [(px.r, bg.r), (px.g, bg.g), (px.b, bg.b)] {
                        out.push((ch as f32 * a + base as f32 * (1.0 - a)).round() as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("{name}.rgba")), &out).unwrap();
        };
        // A shaped spectrum that beats, and an onset every so often so the score scrolls and the phrase
        // swaps between dumps.
        let frame = |level: f32, t_s: f32, beat: bool| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                let shape = (1.0 - f).powf(1.2) * 0.7 + 0.12;
                let wob = 1.0 + 0.3 * (t_s * 2.4 + f * 6.0).sin();
                let kick = if beat { 1.0 + 0.8 * (1.0 - f) } else { 1.0 };
                *v = ((shape * wob * kick) * level).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d.rms_l = level * 0.6;
            d.rms_r = level * 0.6;
            d
        };
        for id in ["vsghost-white", "vsghost-cobalt", "vsghost-inverse", "vsghost-violet"] {
            let t = theme(id);
            for (tag, level) in [("calm", 0.28f32), ("loud", 0.85)] {
                let mut fam = Vsghost::default();
                let mut c = Canvas::new(380, 60);
                for k in 0..120 {
                    c.clear();
                    fam.draw(&mut c, &t, &frame(level, k as f32 * 0.0167, k % 24 == 0));
                }
                write(format!("vsghost-{}-{tag}", &t.id["vsghost-".len()..]), &c);
            }
            // Flourish: settle, fire, then capture the strobe frame and a mid-decay slice frame.
            let mut fam = Vsghost::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..120 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167, k % 24 == 0));
            }
            fam.flourish.force_next();
            c.clear();
            fam.draw(&mut c, &t, &frame(0.45, 120.0 * 0.0167, false));
            write(format!("vsghost-{}-flourish", &t.id["vsghost-".len()..]), &c);
            for k in 121..124 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.3, k as f32 * 0.0167, false));
            }
            write(format!("vsghost-{}-flourish-decay", &t.id["vsghost-".len()..]), &c);
            // And the frame after the envelope ends must be clean.
            for k in 124..135 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.3, k as f32 * 0.0167, false));
            }
            write(format!("vsghost-{}-clean", &t.id["vsghost-".len()..]), &c);
        }
        // One at the awkward mid size.
        let t = theme("vsghost-cobalt");
        let mut fam = Vsghost::default();
        let mut c = Canvas::new(190, 48);
        for k in 0..120 {
            c.clear();
            fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167, k % 24 == 0));
        }
        write("vsghost-cobalt-190x48".into(), &c);
    }

    /// The per-frame cost, well under the 2ms budget at 380x60.
    ///
    /// Run: cargo test --release probe_vsghost_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_vsghost_cost() {
        let t = theme("vsghost-cobalt");
        let mut fam = Vsghost::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = 0.6 * (1.0 - i as f32 / 96.0);
        }
        d.peaks = d.levels;
        d.rms_l = 0.3;
        for _ in 0..60 {
            fam.draw(&mut c, &t, &d);
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        println!("vsghost: {:.3} ms/frame at 380x60", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }
}
