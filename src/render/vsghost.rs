//! The Virtual Self "Ghost Voices" family: a glitch terminal whose star is a big, glitching kaomoji face.
//!
//! The sibling of `vswings` - the same early-2000s Virtual Self world, but the cold, digital half of it
//! rather than the chrome-and-cherub one. Fidelity pass 2 rethought it: the old piano roll filled the
//! panel and the "ghost" (a tiny phrase, faint rays) was all but invisible behind it, so it read as
//! "bars over a mostly invisible graphic". Now the graphic is the point and the meter is a strip.
//!
//! # The face
//!
//! A kaomoji - `( ^_^ )`, `( >_< )`, `( o_o )`, `( -_- )`, `( ^o^ )`, `( ~_~ )`, `( ._. )` - drawn in
//! `font3x5` cells scaled up as far as the panel allows (5x at 380x60, so the face is 25 of the 60 rows,
//! ~40 %; 3x at 190x48 and at 128x44, where the width - at most ~2/3 of the interior - and not the
//! height is the limit; the spec's "drops to 1x" was read as "drops a size", and 1x was unreadable),
//! centred above the ticker. It is shaded like chrome - `hot` at the top of each glyph to `lit` at the foot, through
//! linear light - over a drop shadow in `edge`, the RGB-split ghost of a glitched CRT. The face cycles
//! on every fourth strong onset, on its own counter, so it stays calm at 160 BPM.
//!
//! It reacts to the music:
//! - the EYES open with the bass: each face's own resting eyes (`^`, `>` `<`, `-`, `~`, `.`) become a
//!   small ring and then a big ring as the bass rises (the brief's `-` -> `o` -> `O`), and brighten
//!   toward `hot`;
//! - the MOUTH opens with the mids: `_` -> a small ring -> a big ring;
//! - two to four horizontal SLICES of the face are shifted sideways every frame by `bass * 1.5 * scale`
//!   px (6 px at the 4x reference of the brief, ~7 px at 5x), a continuous bass-driven datamosh.
//!
//! The eyes and mouth are private 3x5 FEATURE shapes, not `font3x5` letters: that font is single-case,
//! so `o` and `O` are the same glyph and cannot show "opening", and its `>` is a solid PLAY triangle,
//! not a chevron. The brackets come from `font3x5`. Every kaomoji character still has a `font3x5`
//! glyph (`^` and `<` were added there), so the faces can be typed as text elsewhere.
//!
//! Either side of the face, faint GHOST VOICES - little kaomoji, `VOICE`, `ERR`, `01101` - murmur in
//! `font3x5` at a low mix of `edge`, blinking with the onset count (only at 4x and up; on a small panel
//! the margins are too narrow and they would crowd the face). The old light rays are gone: they were
//! the "mostly invisible graphic".
//!
//! The face is composed into a small preallocated buffer (rebuilt only when the panel size changes) and
//! copied onto the panel row by row with each row's slice offset, so `draw` allocates nothing.
//!
//! # The meter
//!
//! 32 thin TICKER BARS along the bottom quarter, low frequencies on the left: each a narrow column of
//! 1 px ticks on a 2 px lattice - the old piano roll, compressed - lit bottom-up in `lit` to the band
//! level, with a `hot` peak tick that hangs and falls at the theme's `peak_fall` and a faint resting
//! lattice so the strip is always there. Above them the PHRASES (`GHOST VOICES // EON BREAK // ...`)
//! crawl right to left in `font3x5`, each letter brightening with the treble band under it. The crawl
//! only ever draws whole glyphs inside its window (no wrap, nothing past the edges) - a character-cell
//! marquee, like a terminal's.
//!
//! The levels drive the bars directly: `d.levels` has already been through the theme's ballistics in
//! `Ticker::tick`, so re-smoothing them here would be a second low-pass pass (v0.3.2 removed exactly
//! that). The peak hold is a decaying running max, not a smoother.
//!
//! # The flourish
//!
//! The datamosh slam, applied to the face's band of the panel: 3-5 horizontal slices of the band are
//! shifted by random offsets (wrapping across the interior) and laid back in negative for a few
//! frames; on the firing frame the whole band strobes to the panel's negative with the face in
//! negative over it. The negative is the colourway's own (bright ice for dark, the darkest ink for
//! bright), not `255 - c`, which turned cobalt orange. The frame after the envelope ends is clean.
//!
//! # The panel is opaque, colour through linear light
//!
//! An opaque panel (black, or white for the inverse colourway) first, the rounded clip last, no bloom.
//! Every in-between colour (chrome ramp, shadow, resting lattice, dim ticker letters) is a
//! precomputed opaque `lerp_linear` mix, so nothing on the panel needs a per-pixel blend.

use crate::dsp::bands::NUM_BANDS;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::font3x5;
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// The faces, cycled on every fourth strong onset. Each is `( E M E )`: the brackets are `font3x5`
/// glyphs, the eyes (positions 2 and 4) and the mouth (position 3) are `feature` shapes.
const FACES: [&str; 7] = ["( ^_^ )", "( >_< )", "( o_o )", "( -_- )", "( ^o^ )", "( ~_~ )", "( ._. )"];

/// Cells in a face string (all `FACES` are this long).
const FACE_CELLS: i32 = 7;

/// The faint background fragments either side of the face - see `draw_voices`.
const VOICES: [&str; 12] =
    ["(^_^)", "(~_~)", "(o_o)", "(-_-)", "GHOST", "VOICE", "01101", "> RUN", "ERR", "A.I.", "0X2A", "(._.)"];

/// The ticker's crawl: the ghost voices, one after another, looping.
const TICKER: &str = "GHOST VOICES // EON BREAK // A.I.NGEL // PARTICLE ARTS // UTOPIA // ";

/// The ticker's crawl speed, px/s. Constant, so the crawl is a texture and not a second meter.
const TICKER_PX_S: f32 = 22.0;

/// The meter: 32 bars, each the max of two adjacent bands.
const BARS: usize = 32;

/// Display gain from level to bar fill, so a realistic 0.6-0.8 level reaches most of the strip.
const FILL_GAIN: f32 = 1.25;

/// Tick pitch in the bars (1 px tick, 1 px gap).
const TICK_PITCH: i32 = 2;

/// The largest face scale; a face row buffer holds `5 * MAX_SCALE` rows plus the shadow.
const MAX_SCALE: i32 = 6;
const MAX_FACE_ROWS: usize = (5 * MAX_SCALE + MAX_SCALE) as usize;

/// The panel is at least this large before the family draws - smaller, and it sheds rather than
/// smudging, exactly as the other families do.
const MIN_W: i32 = 60;
const MIN_H: i32 = 18;

/// How long the datamosh runs, in milliseconds. ~4 frames at 16.7ms.
const MOSH_MS: f32 = 70.0;

/// The onset net that cycles the face - a permissive flux net.
const SWAP_ONSET_RATIO: f32 = 2.8;
const SWAP_ONSET_REFRACTORY_MS: f32 = 120.0;

/// Bass (eyes, slices), mid (mouth) and treble (ticker letters) band ranges.
const BASS: std::ops::Range<usize> = 0..8;
const MIDS: std::ops::Range<usize> = 12..36;
const TREBLE: std::ops::Range<usize> = 36..NUM_BANDS;

/// Eye opening thresholds on the bass, mouth thresholds on the mids: below the first the feature
/// rests, then a small ring, then a big ring.
const EYE_OPEN: [f32; 2] = [0.4, 0.7];
const MOUTH_OPEN: [f32; 2] = [0.22, 0.45];

/// Where everything sits, from the panel size alone. All bounds are exclusive at the high end.
#[derive(Clone, Copy, Debug)]
struct Layout {
    ix0: i32,
    iy0: i32,
    ix1: i32,
    iy1: i32,
    /// The face's cell scale (1 glyph pixel = `scale` x `scale` px) and its drop-shadow offset.
    scale: i32,
    shadow: i32,
    /// The face's top-left and size (without the shadow).
    face_x: i32,
    face_y: i32,
    face_w: i32,
    face_h: i32,
    /// The ticker's text row (5 px tall), if the panel is tall enough for one.
    ticker_y: Option<i32>,
    /// The bar strip's rows.
    bar_y0: i32,
    bar_y1: i32,
}

fn layout(w: i32, h: i32) -> Layout {
    let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
    let iw = (ix1 - ix0).max(1);
    let ih = (iy1 - iy0).max(1);
    // The bottom quarter, a whole number of ticks.
    let mut bar_h = ((ih as f32 * 0.25).round() as i32).max(TICK_PITCH * 2);
    bar_h -= bar_h % TICK_PITCH;
    let bar_y1 = iy1;
    let bar_y0 = bar_y1 - bar_h;
    // The ticker sits on the bars with a 2 px gap, when there is room for it and a face above.
    let ticker_y = if ih >= 30 { Some(bar_y0 - 2 - 5) } else { None };
    let face_bottom = ticker_y.unwrap_or(bar_y0) - 2;
    let region_h = (face_bottom - iy0).max(0);
    // As large as the height allows (with a little air) and no wider than ~2/3 of the interior.
    let by_h = (region_h - 3) / 5;
    let by_w = (iw as f32 * 0.68) as i32 / (FACE_CELLS * 4 - 1);
    let scale = by_h.min(by_w).clamp(1, MAX_SCALE);
    let shadow = (scale / 2).max(1);
    let face_w = (FACE_CELLS * 4 - 1) * scale;
    let face_h = 5 * scale;
    let face_x = ix0 + (iw - face_w - shadow) / 2;
    let face_y = iy0 + ((region_h - face_h - shadow) / 2).max(0);
    Layout { ix0, iy0, ix1, iy1, scale, shadow, face_x, face_y, face_w, face_h, ticker_y, bar_y0, bar_y1 }
}

/// The private eye/mouth shapes, five rows of three bits (bit 2 leftmost) like `font3x5`. Resting
/// shapes are per character; `RING_S`/`RING_L` are the opened states.
const RING_S: [u8; 5] = [0b111, 0b101, 0b111, 0b000, 0b000];
const RING_L: [u8; 5] = [0b111, 0b101, 0b101, 0b111, 0b000];
/// The mouth opens lower and smaller than the eyes, so `( O o O )` still reads as two eyes over a
/// mouth rather than three rings in a row.
const MOUTH_S: [u8; 5] = [0b000, 0b000, 0b000, 0b101, 0b111];
const MOUTH_L: [u8; 5] = [0b000, 0b000, 0b111, 0b101, 0b111];

/// A face character's resting feature shape. `None` for a character that is not a feature. Eyes sit
/// in the upper rows, mouths on the baseline.
fn feature(ch: char) -> Option<[u8; 5]> {
    Some(match ch {
        '^' => [0b010, 0b101, 0b000, 0b000, 0b000],
        '>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        '<' => [0b001, 0b010, 0b100, 0b010, 0b001],
        '-' => [0b000, 0b111, 0b000, 0b000, 0b000],
        '~' => [0b110, 0b011, 0b000, 0b000, 0b000],
        '.' => [0b000, 0b010, 0b000, 0b000, 0b000],
        'o' => RING_S,
        'O' => RING_L,
        '_' => [0b000, 0b000, 0b000, 0b000, 0b111],
        _ => return None,
    })
}

/// How open a feature is: 0 rests, 1 small ring, 2 big ring.
fn openness(level: f32, thresholds: [f32; 2]) -> u8 {
    if level >= thresholds[1] {
        2
    } else if level >= thresholds[0] {
        1
    } else {
        0
    }
}

/// The shape of an eye `ch` at `open`. A face whose resting eye is already a ring never closes it.
fn eye_shape(ch: char, open: u8) -> [u8; 5] {
    match open {
        2 => RING_L,
        1 => RING_S,
        _ => feature(ch).unwrap_or(RING_S),
    }
}

/// The shape of a mouth `ch` at `open`; a resting `o` mouth is already the small ring.
fn mouth_shape(ch: char, open: u8) -> [u8; 5] {
    match open {
        2 => MOUTH_L,
        1 => MOUTH_S,
        _ => match ch {
            'o' => MOUTH_S,
            c => feature(c).unwrap_or([0, 0, 0, 0, 0b111]),
        },
    }
}

/// The voices' colour: a low mix of `edge` into the panel - lower on a dark panel, where linear light
/// makes a small mix read bright, than on the light (inverse) one.
fn voice_colour(t: &Theme) -> Rgba {
    let panel = Rgba::from_hex(&t.panel, 1.0);
    let edge = Rgba::from_hex(&t.edge, 1.0);
    let dark = luma(panel) < 128.0;
    Rgba::lerp_linear(panel, edge, if dark { 0.1 } else { 0.3 })
}

/// A colour's brightness in sRGB codes, 0..255 - which side of the light/dark line a panel is on.
fn luma(c: Rgba) -> f32 {
    0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32
}

/// A stateless 64-bit mix (splitmix64's finaliser), for the voices' fixed positions.
fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Mean of a band range, sanitised to 0..1.
fn band_mean(levels: &[f32; NUM_BANDS], r: std::ops::Range<usize>) -> f32 {
    let n = r.len().max(1) as f32;
    let s: f32 = levels[r].iter().map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }).sum();
    s / n
}

pub struct Vsghost {
    /// Fires the datamosh on a rare, exceptional hit - see `dsp::flourish`.
    flourish: crate::dsp::flourish::Trigger,
    /// The datamosh's one-shot decay envelope.
    mosh: crate::dsp::flourish::Envelope,
    /// The onset that counts toward a face swap.
    onset: crate::dsp::onset::Flux,
    /// Strong onsets counted, so the face swaps on every fourth.
    onset_count: u32,
    /// Which face is showing.
    face_idx: usize,
    /// A ticker text forced by a test, set once - see `set_phrase_for_test`.
    phrase_override: Option<String>,
    /// The ticker's crawl position, px.
    ticker_px: f32,
    /// Peak-hold fill per bar, decaying at `peak_fall`.
    peak: [f32; BARS],
    /// Each face row's sideways slice offset this frame, px (0 for a row not in a slice).
    row_off: [i32; MAX_FACE_ROWS],
    /// This frame's eye and mouth openness, kept for the tests.
    eye_open: u8,
    mouth_open: u8,
    /// The face, composed each frame into this buffer before being copied on with the slice offsets;
    /// sized once per panel size.
    face_buf: Option<Canvas>,
    /// The datamosh's frame copy, sized once per panel size and reused.
    scratch: Option<Canvas>,
    /// splitmix64 state, for the slices.
    rng: u64,
}

impl Default for Vsghost {
    fn default() -> Self {
        Vsghost {
            flourish: Default::default(),
            mosh: Default::default(),
            onset: Default::default(),
            onset_count: 0,
            face_idx: 0,
            phrase_override: None,
            ticker_px: 0.0,
            peak: [0.0; BARS],
            row_off: [0; MAX_FACE_ROWS],
            eye_open: 0,
            mouth_open: 0,
            face_buf: None,
            scratch: None,
            rng: 0x853c_49e6_748f_ea9b,
        }
    }
}

impl Vsghost {
    /// Forces the ticker text, for `long_phrase_is_clipped`. The one allocation is here, in the test
    /// hook, once - never in `draw`.
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

    /// The crawl: `text` looping right to left in `font3x5`, scrolled `scroll` px, inside the window
    /// `x0 .. x0 + maxw` on the 5 px row at `y0`. Only WHOLE glyphs that sit entirely inside the window
    /// are drawn - nothing past either edge, never a glyph cut mid-column, never a second line. Each
    /// glyph's colour comes from `col(glyph_x)`.
    fn draw_ticker(c: &mut Canvas, x0: i32, y0: i32, maxw: i32, text: &str, scroll: f32, col: impl Fn(i32) -> Rgba) {
        let bytes = text.as_bytes();
        let n = bytes.len() as i32;
        if n == 0 || maxw < 3 {
            return;
        }
        let period = n * 4;
        let off = if scroll.is_finite() { (scroll.rem_euclid(period as f32)) as i32 } else { 0 };
        // The first cell that could reach the window, then every cell across it.
        let first = off / 4;
        let mut cx = x0 - (off - first * 4);
        let mut k = first;
        while cx < x0 + maxw {
            if cx >= x0 && cx + 3 <= x0 + maxw {
                let ch = bytes[(k % n) as usize] as char;
                if let Some(rows) = font3x5::glyph(ch) {
                    let colour = col(cx);
                    for (dy, row) in rows.iter().enumerate() {
                        for dx in 0..3 {
                            if row & (0b100 >> dx) != 0 {
                                c.fill_rect(cx + dx, y0 + dy as i32, 1, 1, colour);
                            }
                        }
                    }
                }
            }
            cx += 4;
            k += 1;
        }
    }
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
        let l = layout(w, h);

        // ---- the opaque panel ----
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        let panel_o = Rgba::from_hex(&t.panel, 1.0);
        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let sens = if t.sensitivity.is_finite() { t.sensitivity.max(0.0) } else { 1.0 };

        let bass = (band_mean(&d.levels, BASS) * sens).clamp(0.0, 1.0);
        let mids = (band_mean(&d.levels, MIDS) * sens).clamp(0.0, 1.0);

        // ---- onset: count toward the face swap ----
        if self.onset.update(&d.levels, dt, SWAP_ONSET_RATIO, SWAP_ONSET_REFRACTORY_MS) {
            self.onset_count = self.onset_count.wrapping_add(1);
            if self.onset_count.is_multiple_of(4) {
                self.face_idx = (self.face_idx + 1) % FACES.len();
            }
        }

        // ---- the ticker bars (the meter) ----
        self.draw_bars(c, t, d, &l, panel_o, lit, hot, sens);

        // ---- the ticker crawl ----
        self.ticker_px += TICKER_PX_S * dt / 1000.0;
        if !self.ticker_px.is_finite() || self.ticker_px > 1.0e6 {
            self.ticker_px = 0.0;
        }
        if let Some(ty) = l.ticker_y {
            // Resting letters are a strong mix of `lit` into the panel (stronger on the light panel,
            // where linear light makes a mix read pale); the treble lifts them to a bright ink.
            let dark = luma(panel_o) < 128.0;
            let dim = Rgba::lerp_linear(panel_o, lit, if dark { 0.65 } else { 0.85 });
            let bright = if dark { Rgba::lerp_linear(hot, Rgba::new(255, 255, 255, 255), 0.4) } else { hot };
            let (ix0, iw) = (l.ix0, l.ix1 - l.ix0);
            let levels = &d.levels;
            let col = |gx: i32| {
                // The treble band under this letter, across the treble range left to right.
                let f = ((gx - ix0) as f32 / iw.max(1) as f32).clamp(0.0, 0.999);
                let b = TREBLE.start + (f * TREBLE.len() as f32) as usize;
                let v = levels[b.min(NUM_BANDS - 1)];
                let v = if v.is_finite() { (v * sens * 1.6).clamp(0.0, 1.0) } else { 0.0 };
                Rgba::lerp_linear(dim, bright, v)
            };
            let text: &str = self.phrase_override.as_deref().unwrap_or(TICKER);
            Self::draw_ticker(c, l.ix0 + 1, ty, iw - 2, text, self.ticker_px, col);
        }

        // ---- the ghost voices: faint terminal fragments either side of the face ----
        self.draw_voices(c, &l, voice_colour(t));

        // ---- the face ----
        self.eye_open = openness(bass, EYE_OPEN);
        self.mouth_open = openness(mids, MOUTH_OPEN);
        self.compose_face(&l, panel_o, lit, hot, edge, bass);
        // The slices: 2-4 bands of rows, shifted by the bass.
        let rows = (l.face_h + l.shadow).min(MAX_FACE_ROWS as i32);
        self.row_off.iter_mut().for_each(|o| *o = 0);
        let amp = (bass * 1.5 * l.scale as f32).round() as i32;
        let nslices = 2 + (self.next_rng() % 3) as i32;
        for _ in 0..nslices {
            // Thin slices (1..=scale rows), so even four of them leave most of the face in place.
            let sh = 1 + (self.next_rng() % l.scale as u64) as i32;
            let sy = (self.next_rng() % (rows.max(1) as u64)) as i32;
            let sign = if self.next_rng() & 1 == 0 { 1 } else { -1 };
            // 60-100 % of the bass amplitude, so the slices do not all move in lockstep.
            let k = 0.6 + (self.next_rng() % 1000) as f32 / 2500.0;
            let shift = sign * (amp as f32 * k).round() as i32;
            for y in sy..(sy + sh).min(rows) {
                self.row_off[y as usize] = shift;
            }
        }
        self.paste_face(c, &l, None);

        // ---- the flourish: a datamosh slam over the face's band ----
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let mosh = self.mosh.update(fired, dt, MOSH_MS);
        if mosh > 0.0 {
            self.datamosh(c, fired, &l, t);
        }

        // Keep nothing on the rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

impl Vsghost {
    /// The ghost voices: faint `font3x5` fragments - little kaomoji, words, a prompt - on the text rows
    /// either side of the face, at positions hashed from the row and side. Each one blinks on or off
    /// with the strong-onset count, so the margins murmur with the music without ever competing with
    /// the face (they are drawn at a low mix of `edge` into the panel).
    fn draw_voices(&self, c: &mut Canvas, l: &Layout, col: Rgba) {
        if l.scale < 4 {
            return; // on a small panel the margins are too narrow: the voices would crowd the face
        }
        let fx0 = l.face_x - 8;
        let fx1 = l.face_x + l.face_w + l.shadow + 8;
        let bottom = l.ticker_y.unwrap_or(l.bar_y0) - 3;
        let mut row = 0u64;
        let mut y = l.iy0 + 1;
        while y + 5 <= bottom {
            for side in 0..2u64 {
                let (zx0, zx1) = if side == 0 { (l.ix0 + 2, fx0) } else { (fx1, l.ix1 - 2) };
                let hsh = mix64(row * 2 + side + 0x51);
                let text = VOICES[(hsh % VOICES.len() as u64) as usize];
                let tw = font3x5::width(text);
                let room = zx1 - zx0 - tw;
                if room < 0 {
                    continue;
                }
                // Blink: each voice is off on about one onset-count step in three.
                if mix64(hsh ^ self.onset_count as u64).is_multiple_of(3) {
                    continue;
                }
                let x = zx0 + ((hsh >> 16) % (room as u64 + 1)) as i32;
                font3x5::draw(c, x, y, text, col);
            }
            row += 1;
            y += 7;
        }
    }

    /// The 32 ticker bars: a faint resting lattice, `lit` ticks bottom-up to the level, a `hot` peak.
    #[allow(clippy::too_many_arguments)]
    fn draw_bars(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData, l: &Layout, panel: Rgba, lit: Rgba, hot: Rgba, sens: f32) {
        let ghost = Rgba::lerp_linear(panel, lit, t.ghost.clamp(0.0, 1.0));
        let peak_fall = if t.ballistics.peak_fall.is_finite() { t.ballistics.peak_fall.max(0.0) } else { 0.03 };
        let iw = (l.ix1 - l.ix0) as f32;
        let pitch = iw / BARS as f32;
        let bw = ((pitch * 0.5).round() as i32).max(1);
        let bh = l.bar_y1 - l.bar_y0;
        let per_band = NUM_BANDS / BARS;
        for k in 0..BARS {
            let mut v = 0.0f32;
            for b in k * per_band..(k + 1) * per_band {
                let x = d.levels[b];
                if x.is_finite() {
                    v = v.max(x);
                }
            }
            let f = (v.clamp(0.0, 1.0) * FILL_GAIN * sens).clamp(0.0, 1.0);
            let mut pk = (self.peak[k] - peak_fall).max(f);
            if !pk.is_finite() {
                pk = f;
            }
            self.peak[k] = pk;
            let x = l.ix0 + (k as f32 * pitch + (pitch - bw as f32) * 0.5).round() as i32;
            let lit_ticks = (f * (bh / TICK_PITCH) as f32).round() as i32;
            let peak_tick = (pk * (bh / TICK_PITCH) as f32).round() as i32;
            let mut i = 0;
            let mut y = l.bar_y1 - 1;
            while y >= l.bar_y0 {
                let colour = if i < lit_ticks {
                    lit
                } else if i + 1 == peak_tick && pk > f + 0.02 {
                    hot
                } else {
                    ghost
                };
                c.fill_rect(x, y, bw, 1, colour);
                y -= TICK_PITCH;
                i += 1;
            }
            // A peak above the strip still shows, on its top tick.
            if peak_tick > bh / TICK_PITCH && pk > f + 0.02 {
                c.fill_rect(x, l.bar_y0 + (bh - 1) % TICK_PITCH, bw, 1, hot);
            }
        }
    }

    /// Composes the face (shadow, then chrome-shaded glyphs) into `face_buf` at its origin. The buffer
    /// is (re)made only when the panel size changes.
    #[allow(clippy::too_many_arguments)]
    fn compose_face(&mut self, l: &Layout, panel: Rgba, lit: Rgba, hot: Rgba, edge: Rgba, bass: f32) {
        let (bw, bh) = (l.face_w + l.shadow, l.face_h + l.shadow);
        let mut buf = self
            .face_buf
            .take()
            .filter(|b| b.width() == bw && b.height() == bh)
            .unwrap_or_else(|| Canvas::new(bw, bh));
        buf.clear();
        let s = l.scale;
        let face = FACES[self.face_idx % FACES.len()];
        let shadow = Rgba::lerp_linear(panel, edge, 0.6);
        // The chrome ramp: `hot` at the top glyph row to `lit` at the foot.
        let mut ramp = [hot; 5];
        for (r, slot) in ramp.iter_mut().enumerate() {
            *slot = Rgba::lerp_linear(hot, lit, r as f32 / 4.0);
        }
        // Eyes brighten toward `hot` - and past it toward white on the loudest kicks, on a dark panel;
        // on the light (inverse) panel white would wash them out, so they go to full `hot` there.
        let eye_hi = if luma(panel) < 128.0 { Rgba::lerp_linear(hot, Rgba::new(255, 255, 255, 255), 0.5 * bass) } else { hot };
        // A CRT scanline through every glyph pixel row once the cells are big enough to carry one.
        let scan_rows = if s >= 3 { 1 } else { 0 };
        for pass in 0..2 {
            for (i, ch) in face.chars().enumerate() {
                let rows = match i {
                    2 | 4 => eye_shape(ch, self.eye_open),
                    3 => mouth_shape(ch, self.mouth_open),
                    _ => match font3x5::glyph(ch) {
                        Some(r) => r,
                        None => continue,
                    },
                };
                let is_eye = i == 2 || i == 4;
                let gx = i as i32 * 4 * s;
                for (dy, row) in rows.iter().enumerate() {
                    let col = if pass == 0 {
                        shadow
                    } else if is_eye {
                        Rgba::lerp_linear(ramp[dy], eye_hi, 0.35 + 0.65 * bass)
                    } else {
                        ramp[dy]
                    };
                    let off = if pass == 0 { l.shadow } else { 0 };
                    let (body, scan) = if pass == 0 { (s, 0) } else { (s - scan_rows, scan_rows) };
                    let scan_col = Rgba::lerp_linear(col, panel, 0.45);
                    for dx in 0..3 {
                        if row & (0b100 >> dx) != 0 {
                            let (x, y) = (gx + dx * s + off, dy as i32 * s + off);
                            buf.fill_rect(x, y, s, body, col);
                            if scan > 0 {
                                buf.fill_rect(x, y + body, s, scan, scan_col);
                            }
                        }
                    }
                }
            }
        }
        self.face_buf = Some(buf);
    }

    /// Copies the composed face onto the panel, each row shifted by its slice offset, clamped to the
    /// interior. With `neg`, each pixel goes through the palette negative instead (the strobe frame).
    fn paste_face(&mut self, c: &mut Canvas, l: &Layout, neg: Option<&[Rgba; NEG_STEPS]>) {
        let Some(buf) = self.face_buf.as_ref() else { return };
        for y in 0..buf.height() {
            let dy = l.face_y + y;
            if dy < l.iy0 || dy >= l.iy1 {
                continue;
            }
            let off = self.row_off.get(y as usize).copied().unwrap_or(0);
            for x in 0..buf.width() {
                let p = buf.get(x, y);
                if p.a == 0 {
                    continue;
                }
                let dx = l.face_x + x + off;
                if dx < l.ix0 || dx >= l.ix1 {
                    continue;
                }
                let p = match neg {
                    Some(lut) => lut[neg_index(p)],
                    None => p,
                };
                c.fill_rect(dx, dy, 1, 1, p);
            }
        }
    }

    /// The datamosh slam over the face's band: copy the band, then shift 3-5 slices of it sideways
    /// (wrapping across the interior) and lay them back in NEGATIVE. On the firing frame the band
    /// strobes to the negative of the panel and the face is laid over it in negative too.
    ///
    /// The negative is the palette's, not `255 - c`: the old per-channel inversion turned cobalt and
    /// violet into orange and yellow, off the Virtual Self palette. Here a pixel's brightness picks a
    /// colour on the ramp from a bright ice ink (for dark input) to the darkest of panel/`lit` (for
    /// bright input) - still a hard inversion of light and dark, in the colourway's own colours. The
    /// ramp is a small precomputed table, so the slices cost a lookup per pixel. Every write is
    /// clamped to the interior.
    fn datamosh(&mut self, c: &mut Canvas, fired: bool, l: &Layout, t: &Theme) {
        let (w, h) = (c.width(), c.height());
        let (ix0, ix1) = (l.ix0, l.ix1);
        let iw = ix1 - ix0;
        let y0 = (l.face_y - 2).max(l.iy0);
        let y1 = (l.face_y + l.face_h + l.shadow + 2).min(l.iy1);
        let bh = y1 - y0;
        if iw <= 0 || bh <= 0 {
            return;
        }
        let lut = neg_lut(t);
        let mut scr = self
            .scratch
            .take()
            .filter(|s| s.width() == w && s.height() == h)
            .unwrap_or_else(|| Canvas::new(w, h));
        scr.copy_region(c, (ix0, y0), (ix0, y0), iw, bh);

        if fired {
            // The one-frame strobe: the band goes to the panel's negative, the face to its own.
            let panel = Rgba::from_hex(&t.panel, 1.0);
            c.fill_rect(ix0, y0, iw, bh, lut[neg_index(panel)]);
            self.paste_face(c, l, Some(&lut));
        }
        let nslices = 3 + (self.next_rng() % 3) as i32; // 3..=5
        for _ in 0..nslices {
            let sh = 1 + (self.next_rng() % (2 * l.scale as u64).max(1)) as i32;
            let room = (bh - sh).max(1) as u64;
            let sy = y0 + (self.next_rng() % room) as i32;
            let shift = (self.next_rng() % iw as u64) as i32;
            let sy1 = (sy + sh).min(y1);
            for y in sy..sy1 {
                for x in ix0..ix1 {
                    let sx = ix0 + (x - ix0 - shift).rem_euclid(iw);
                    c.fill_rect(x, y, 1, 1, lut[neg_index(scr.get(sx, y))]);
                }
            }
        }
        self.scratch = Some(scr);
    }
}

/// Steps in the palette-negative table.
const NEG_STEPS: usize = 16;

/// The palette negative: `NEG_STEPS` colours from the bright ink (index 0, for the darkest input) to
/// the dark ink (the last index, for the brightest input). See `Vsghost::datamosh`.
fn neg_lut(t: &Theme) -> [Rgba; NEG_STEPS] {
    let panel = Rgba::from_hex(&t.panel, 1.0);
    let lit = Rgba::from_hex(&t.lit, 1.0);
    let hot = Rgba::from_hex(&t.hot, 1.0);
    let dark = if luma(panel) <= luma(lit) { panel } else { lit };
    let bright = Rgba::lerp_linear(hot, Rgba::new(255, 255, 255, 255), 0.55);
    let mut lut = [bright; NEG_STEPS];
    for (i, slot) in lut.iter_mut().enumerate() {
        *slot = Rgba::lerp_linear(bright, dark, i as f32 / (NEG_STEPS - 1) as f32);
    }
    lut
}

/// A pixel's slot in the negative table, by its brightness.
fn neg_index(p: Rgba) -> usize {
    ((luma(p) / 255.0 * (NEG_STEPS - 1) as f32).round() as usize).min(NEG_STEPS - 1)
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
    /// `n` frames of an explicit spectrum.
    fn frames_of(fam: &mut Vsghost, t: &Theme, w: i32, h: i32, levels: [f32; NUM_BANDS], n: usize) -> Canvas {
        let mut c = Canvas::new(w, h);
        let mut d = FrameData { levels, peaks: levels, dt_ms: 16.7, ..FrameData::default() };
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, t, &d);
        }
        c
    }
    /// A spectrum with the bass at `bass`, the mids at `mids`, everything else silent.
    fn spectrum(bass: f32, mids: f32) -> [f32; NUM_BANDS] {
        let mut l = [0.0f32; NUM_BANDS];
        for b in BASS {
            l[b] = bass;
        }
        for b in MIDS {
            l[b] = mids;
        }
        l
    }
    /// Pixels the family DREW OVER the panel - opaque and differing from the panel colour by more than a
    /// rounding margin. The panel is opaque, so "lit" is paint on top of it, not raw alpha.
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
    /// Face paint: anything over the panel in the rows above the ticker except the faint voices.
    fn face_px(c: &Canvas, t: &Theme) -> Vec<(i32, i32)> {
        let l = layout(c.width(), c.height());
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let voice = voice_colour(t);
        let top = l.ticker_y.unwrap_or(l.bar_y0) - 1;
        let mut v = Vec::new();
        for y in 0..top {
            for x in 0..c.width() {
                let p = c.get(x, y);
                if drew_over_panel(p, panel) && p != voice {
                    v.push((x, y));
                }
            }
        }
        v
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"vsghost"));
        assert_eq!(crate::render::family_for("vsghost").id(), "vsghost");
        assert_ne!(crate::themes::family_label("vsghost"), "vsghost");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "vsghost").count(), 4);
    }

    /// MIGRATED (fidelity pass 2): was "loud paint reaches the top third of the panel" on the piano
    /// roll; the meter is now the ticker bars, so it is "loud lights the bars' upper half, quiet does
    /// not" - counted in `lit`/`hot` exactly, so the resting lattice does not count.
    #[test]
    fn ticks_light_upward_with_level() {
        let t = theme("vsghost-white");
        let l = layout(380, 48);
        let quiet = frames(&mut Vsghost::default(), &t, 380, 48, 0.2, 30);
        let loud = frames(&mut Vsghost::default(), &t, 380, 48, 0.8, 30);
        let (lit_c, hot_c) = (Rgba::from_hex(&t.lit, 1.0), Rgba::from_hex(&t.hot, 1.0));
        let mid = (l.bar_y0 + l.bar_y1) / 2;
        let upper = |c: &Canvas| {
            (l.bar_y0..mid)
                .flat_map(|y| (0..380).map(move |x| (x, y)))
                .filter(|&(x, y)| c.get(x, y) == lit_c || c.get(x, y) == hot_c)
                .count()
        };
        assert!(upper(&loud) > 0, "loud never reached the bars' upper half");
        assert!(upper(&loud) > upper(&quiet) * 2, "bars' upper half: quiet {} loud {}", upper(&quiet), upper(&loud));
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("vsghost-white");
        let c = frames(&mut Vsghost::default(), &t, 380, 48, 0.0, 10);
        assert!(
            lit(&c, &t) as f32 >= 0.02 * (380 * 48) as f32,
            "face + lattice + ticker should show: {}",
            lit(&c, &t)
        );
    }

    /// MIGRATED (fidelity pass 2): the phrase is now the ticker crawl, so "clipped" means: it stays on
    /// its own 5 px row (never a second line), inside its window, and only whole glyphs are drawn -
    /// at every scroll position, not just one.
    #[test]
    fn long_phrase_is_clipped() {
        let t = theme("vsghost-white");
        let (w, h) = (190, 48);
        let l = layout(w, h);
        let ty = l.ticker_y.expect("190x48 has a ticker");
        // In situ: the same family state with the long phrase vs an empty one differs ONLY on the
        // ticker's row, inside its window. Everything else (face, bars, rng) is identical.
        let mut with = Vsghost::default();
        with.set_phrase_for_test("a very long phrase that cannot possibly fit in one hundred and ninety pixels of taskbar");
        let mut without = Vsghost::default();
        without.set_phrase_for_test("");
        let a = frames(&mut with, &t, w, h, 0.3, 5);
        let b = frames(&mut without, &t, w, h, 0.3, 5);
        let mut diff = 0;
        for y in 0..h {
            for x in 0..w {
                if a.get(x, y) != b.get(x, y) {
                    diff += 1;
                    assert!((ty..ty + 5).contains(&y), "phrase paint off its row (wrapped?) at {x},{y}");
                    assert!(x > l.ix0 && x < l.ix1 - 1, "phrase paint outside its window at {x},{y}");
                }
            }
        }
        // 1. NOT vacuous: the phrase actually draws.
        assert!(diff > 0, "the phrase did not draw at all");
        // 2. Whole glyphs, inside the window, at every scroll offset: a string of solid `O`s (whose
        // top row is `111`) must leave only runs of exactly 3 px on that row, and nothing at or past
        // either edge. Removing the whole-glyph guard in `draw_ticker` fails this.
        let (x0, maxw) = (3i32, 40i32);
        let solid = Rgba::new(255, 0, 0, 255);
        for scroll in 0..16 {
            let mut u = Canvas::new(80, 20);
            Vsghost::draw_ticker(&mut u, x0, 4, maxw, "OOOOOOOOOOOOOOOOOOOO", scroll as f32 * 0.75, |_| solid);
            let (mut lo, mut hi) = (i32::MAX, -1);
            for y in 0..20 {
                for x in 0..80 {
                    if u.get(x, y) == solid {
                        assert!((4..9).contains(&y), "scroll {scroll}: a second line at y {y}");
                        lo = lo.min(x);
                        hi = hi.max(x);
                    }
                }
            }
            assert!(hi >= 0, "scroll {scroll}: drew nothing");
            assert!(lo >= x0 && hi < x0 + maxw, "scroll {scroll}: drew {lo}..={hi} outside {x0}..{}", x0 + maxw);
            let mut run = 0;
            for x in 0..=80 {
                if x < 80 && u.get(x, 4) == solid {
                    run += 1;
                } else {
                    assert!(run == 0 || run == 3, "scroll {scroll}: a cut glyph (run {run}) ending at x {x}");
                    run = 0;
                }
            }
        }
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

    /// MIGRATED (fidelity pass 2): the private phrase font is gone - the ticker and the face brackets
    /// use the shared `font3x5`. Every ticker and kaomoji character must have a `font3x5` glyph, and
    /// every eye/mouth character a feature shape.
    #[test]
    fn every_phrase_character_has_a_glyph() {
        for ch in TICKER.chars().chain(VOICES.iter().flat_map(|v| v.chars())) {
            assert!(font3x5::glyph(ch).is_some(), "no font3x5 glyph for {ch:?} in the ticker");
        }
        for f in FACES {
            assert_eq!(f.chars().count() as i32, FACE_CELLS, "{f:?}");
            for (i, ch) in f.chars().enumerate() {
                assert!(font3x5::glyph(ch).is_some(), "no font3x5 glyph for {ch:?} in {f:?}");
                if (2..=4).contains(&i) {
                    assert!(feature(ch).is_some(), "no feature shape for {ch:?} in {f:?}");
                }
            }
        }
        for ch in ['(', ')', '^', '_', '>', '<', 'o', 'O', '-', '~', '.'] {
            assert!(font3x5::glyph(ch).is_some(), "no font3x5 glyph for {ch:?}");
        }
    }

    #[test]
    fn the_face_is_large_and_centred() {
        let t = theme("vsghost-cobalt");
        let (w, h) = (380, 60);
        let c = frames_of(&mut Vsghost::default(), &t, w, h, [0.0; NUM_BANDS], 5);
        let px = face_px(&c, &t);
        assert!(!px.is_empty(), "no face");
        let (x0, x1) = (px.iter().map(|p| p.0).min().unwrap(), px.iter().map(|p| p.0).max().unwrap());
        let (y0, y1) = (px.iter().map(|p| p.1).min().unwrap(), px.iter().map(|p| p.1).max().unwrap());
        assert!(layout(w, h).scale >= 2, "the face is not drawn at 2x or more");
        assert!(x0 as f32 >= 0.2 * w as f32 && x1 as f32 <= 0.8 * w as f32, "face {x0}..={x1} not in the centre 60 %");
        assert!(((x0 + x1) / 2 - w / 2).abs() <= 4, "face centre {} vs {}", (x0 + x1) / 2, w / 2);
        assert!((y1 - y0 + 1) as f32 >= 0.33 * h as f32, "face only {} px tall", y1 - y0 + 1);
        assert!((x1 - x0 + 1) as f32 >= 0.25 * w as f32, "face only {} px wide", x1 - x0 + 1);
    }

    #[test]
    fn the_eyes_open_with_the_bass() {
        let t = theme("vsghost-white");
        let mut low = Vsghost::default();
        let a = frames_of(&mut low, &t, 380, 60, spectrum(0.1, 0.0), 5);
        let mut high = Vsghost::default();
        let b = frames_of(&mut high, &t, 380, 60, spectrum(0.9, 0.0), 5);
        assert_eq!(low.eye_open, 0);
        assert_eq!(high.eye_open, 2);
        assert_eq!(high.mouth_open, 0, "the bass must not open the mouth");
        assert_ne!(eye_shape('^', 0), eye_shape('^', 2));
        // The opened eyes are bigger on screen (slices move pixels, they do not remove them).
        let (na, nb) = (face_px(&a, &t).len(), face_px(&b, &t).len());
        assert!(nb as f32 > na as f32 * 1.2, "face px at bass 0.1: {na}, at 0.9: {nb}");
    }

    #[test]
    fn the_mouth_opens_with_the_mids() {
        let t = theme("vsghost-white");
        let mut low = Vsghost::default();
        let a = frames_of(&mut low, &t, 380, 60, spectrum(0.0, 0.1), 5);
        let mut high = Vsghost::default();
        let b = frames_of(&mut high, &t, 380, 60, spectrum(0.0, 0.9), 5);
        assert_eq!(low.mouth_open, 0);
        assert_eq!(high.mouth_open, 2);
        assert_eq!(high.eye_open, 0, "the mids must not open the eyes");
        let (na, nb) = (face_px(&a, &t).len(), face_px(&b, &t).len());
        assert!(nb > na, "face px at mids 0.1: {na}, at 0.9: {nb}");
        // And it is the MOUTH cell that grew: compare the middle cell's paint.
        let l = layout(380, 60);
        let cell = 4 * l.scale;
        let mx0 = l.face_x + 3 * cell;
        let in_mouth = |v: &Vec<(i32, i32)>| v.iter().filter(|p| p.0 >= mx0 && p.0 < mx0 + cell).count();
        assert!(in_mouth(&face_px(&b, &t)) > in_mouth(&face_px(&a, &t)) * 2);
    }

    #[test]
    fn slices_glitch_with_the_bass() {
        let t = theme("vsghost-white");
        let l = layout(380, 60);
        // The per-row leftmost face pixel, which is the left bracket's edge on every bracket row.
        let leftmost = |c: &Canvas| {
            let (panel, voice) = (Rgba::from_hex(&t.panel, 1.0), voice_colour(&t));
            (l.face_y..l.face_y + l.face_h)
                .map(|y| (0..380).find(|&x| drew_over_panel(c.get(x, y), panel) && c.get(x, y) != voice))
                .collect::<Vec<_>>()
        };
        let mut quiet = Vsghost::default();
        let rest = frames_of(&mut quiet, &t, 380, 60, spectrum(0.0, 0.0), 1);
        assert!(quiet.row_off.iter().all(|&o| o == 0), "slices moved at bass 0: {:?}", quiet.row_off);
        let rest_left = leftmost(&rest);
        let mut loud = Vsghost::default();
        let mut moved_frames = 0;
        let mut c = Canvas::new(380, 60);
        let lv = spectrum(0.9, 0.0);
        for k in 0..10 {
            let d = FrameData { levels: lv, peaks: lv, dt_ms: 16.7, time_s: k as f32 * 0.0167, ..FrameData::default() };
            loud.draw(&mut c, &t, &d);
            assert!(loud.row_off.iter().any(|&o| o != 0), "frame {k}: no slice moved at bass 0.9");
            if leftmost(&c) != rest_left {
                moved_frames += 1;
            }
        }
        assert!(moved_frames >= 8, "the face's rows visibly shifted on only {moved_frames}/10 frames");
        // And at bass 0 the face really is unshifted, every frame.
        for _ in 0..5 {
            let c = frames_of(&mut quiet, &t, 380, 60, spectrum(0.0, 0.0), 1);
            assert_eq!(leftmost(&c), rest_left);
        }
    }

    #[test]
    fn the_ticker_bars_are_the_meter() {
        let t = theme("vsghost-white");
        let l = layout(380, 60);
        let mut lv = [0.0f32; NUM_BANDS];
        for v in lv.iter_mut().take(16) {
            *v = 0.8;
        }
        let c = frames_of(&mut Vsghost::default(), &t, 380, 60, lv, 10);
        let lit_c = Rgba::from_hex(&t.lit, 1.0);
        let count = |xs: std::ops::Range<i32>| {
            (l.bar_y0..l.bar_y1).flat_map(|y| xs.clone().map(move |x| (x, y))).filter(|&(x, y)| c.get(x, y) == lit_c).count()
        };
        let q = (l.ix1 - l.ix0) / 4;
        let (left, right) = (count(l.ix0..l.ix0 + q), count(l.ix1 - q..l.ix1));
        assert!(left > 100, "the bass bars did not light: {left}");
        assert_eq!(right, 0, "silent treble bars lit: {right}");
        // And the bars are only in the bottom quarter.
        assert!((l.bar_y1 - l.bar_y0) as f32 <= 0.26 * 60.0);
    }

    #[test]
    fn the_face_fits_the_smallest_panel() {
        for id in ["vsghost-white", "vsghost-inverse"] {
            let t = theme(id);
            let (w, h) = (128, 44);
            let l = layout(w, h);
            assert!(l.scale < layout(380, 60).scale, "the face did not drop a size");
            let mut fam = Vsghost::default();
            let c = frames(&mut fam, &t, w, h, 0.9, 10);
            let px = face_px(&c, &t);
            assert!(px.len() > 80, "{id}: face too small to read: {}", px.len());
            for &(x, y) in &px {
                assert!(x >= l.ix0 && x < l.ix1 && y >= l.iy0, "{id}: face paint at {x},{y}");
            }
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, w, h, 0.9, 1);
            assert_eq!(c.get(0, 0).a, 0, "{id}: flourish painted the corner");
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
            std::fs::write(dir.join(format!("{name}_{}x{}.rgba", c.width(), c.height())), &out).unwrap();
        };
        // A shaped spectrum that beats, with an onset every so often so the face swaps between dumps.
        let frame = |level: f32, t_s: f32, beat: f32| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                let shape = (1.0 - f).powf(1.2) * 0.7 + 0.12;
                let wob = 1.0 + 0.3 * (t_s * 2.4 + f * 6.0).sin();
                let kick = 1.0 + beat * 0.8 * (1.0 - f);
                *v = ((shape * wob * kick) * level).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d.rms_l = level * 0.6;
            d.rms_r = level * 0.6;
            d
        };
        // The kick decays over ~6 frames, like a real one through the ballistics.
        let beat_at = |k: i32| {
            let p = k.rem_euclid(24);
            (1.0 - p as f32 / 6.0).max(0.0)
        };
        for (w, h) in [(380, 60), (128, 44)] {
            for id in ["vsghost-white", "vsghost-cobalt", "vsghost-inverse", "vsghost-violet"] {
                let t = theme(id);
                let tag = &t.id["vsghost-".len()..];
                for (name, level, last) in [("calm", 0.28f32, 130), ("loud", 0.85, 98)] {
                    let mut fam = Vsghost::default();
                    let mut c = Canvas::new(w, h);
                    for k in 0..=last {
                        c.clear();
                        fam.draw(&mut c, &t, &frame(level, k as f32 * 0.0167, beat_at(k)));
                    }
                    write(format!("vsghost-{tag}-{name}"), &c);
                }
                // Flourish: settle, fire, capture the strobe frame and a mid-decay slice frame.
                let mut fam = Vsghost::default();
                let mut c = Canvas::new(w, h);
                for k in 0..120 {
                    c.clear();
                    fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167, beat_at(k)));
                }
                fam.flourish.force_next();
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, 120.0 * 0.0167, 1.0));
                write(format!("vsghost-{tag}-flourish"), &c);
                for k in 121..123 {
                    c.clear();
                    fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167, beat_at(k)));
                }
                write(format!("vsghost-{tag}-flourish-decay"), &c);
                for k in 123..135 {
                    c.clear();
                    fam.draw(&mut c, &t, &frame(0.3, k as f32 * 0.0167, 0.0));
                }
                write(format!("vsghost-{tag}-clean"), &c);
            }
        }
    }

    /// The per-frame cost at 380x60, steady and through the flourish.
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
        println!("vsghost steady: {:.3} ms/frame at 380x60", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
        // Flourish frames: force one every 5 frames so most measured frames are inside the envelope.
        let t0 = std::time::Instant::now();
        for k in 0..n {
            if k % 5 == 0 {
                fam.flourish.force_next();
            }
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        println!("vsghost flourish: {:.3} ms/frame at 380x60", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }
}
