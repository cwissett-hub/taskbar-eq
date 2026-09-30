//! The Blingee family: a Blingee GIF, circa 2007 - sparkly Y2K "gangsta" bling, glitter everywhere.
//!
//! # The meter - rhinestone bars
//!
//! A word cannot show 64 levels at once; gem columns can. The 64 bands fold into 24 COLUMNS, each a
//! stack of small faceted rhinestones (5x5 on a 6 px pitch; 4x4 on a 5 px pitch below 48 rows): a
//! bright centre pixel, a lighter top-left facet, a darker bottom-right, a 1 px dark outline with the
//! corners cut so each reads round. A column is lit bottom-up to its level in `lit`, the top lit gem
//! in `hot`; each empty setting above is a single dim socket pixel at `t.ghost`. A white `+` GLINT runs up
//! each lit column once per ~0.8 s (per-column phase), and a `hot` peak-hold gem hangs above the
//! stack, falling at `peak_fall`. On a wide panel each column is two gems across, so the bars read
//! as bars rather than strings of beads.
//!
//! `FrameData.levels` arrive already smoothed by main's `Smoother` (the theme's ballistics), so the
//! columns read them directly - no second attack/decay here. Only the peak hold is this family's.
//!
//! # The decoration - the Blingee identity
//!
//! - **Glitter panel.** ~4 % of interior pixels (positions hashed once per panel size and kept in a
//!   preallocated list) twinkle in `lit` / `hot` / white, brightness `0.3 + 0.7 |sin(t r + p)|` at
//!   2-6 Hz; the density rises with rms to 8 %. A few are big sparkles that grow a `+` when bright.
//!   The glitter lies over the empty settings too, under the lit gems.
//! - **Glitter text.** One phrase (`BLING BLING`, `~*UR MINE*~`, ...) centred in the upper third in
//!   2x `font3x5`, ringed in `edge` over a 1 px dark outline and filled with a moving diagonal glitter
//!   gradient (`lit` -> white -> `hot`) with white sparkle pixels, swapping every fourth strong onset.
//!   Dropped below 52 rows or when it will not fit.
//! - **Stamps.** Up to three of our own bitmaps - a sparkle burst, a `$`, a crown, a heart with a star
//!   - pop in on strong onsets at random upper-half positions (one frame at 60 %, then full size) and
//!   fade over 600 ms. No logos.
//!
//! # The flourish - a flash
//!
//! On a rare bass hit: one frame of white flash (alpha 0.85 over the panel); then for 500 ms every gem
//! turns diamond white, a chrome sweep (a diagonal bright band) crosses left to right over 400 ms, and
//! a 15x15 spinning 4-point sparkle star turns over the loudest column.
//!
//! # Colourways
//!
//! Colour comes from the theme; the one per-id knob is `bling-gold`'s diamond gems (pale diamond
//! stones faceted in gold, a gold top gem), matched on the id. All mixes go through
//! `Rgba::lerp_linear` / the canvas's linear-light blend.
//!
//! The panel is opaque (painted first) and the clip to the rounded rect runs last on every path. The
//! frame allocates nothing after the first at a given size: the glitter list is rebuilt only when
//! (w, h) changes, the stamps are a fixed pool, and all text goes through `&'static str`s.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use crate::dsp::bands::NUM_BANDS;
use crate::dsp::flourish::{Envelope, Trigger};
use crate::dsp::onset::Flux;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::font3x5;
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// The glitter phrases, cycled every fourth strong onset. Every character has a `font3x5` glyph -
/// `every_phrase_char_has_a_glyph` is the guard.
pub const PHRASES: [&str; 8] =
    ["BLING BLING", "ICED OUT", "4 REAL", "XOXO", "~*UR MINE*~", "$$$", "HOTTIE", "LUV U 4EVA"];

/// Gem columns (bands folded 64 -> 24).
const COLUMNS: usize = 24;
/// Gem pitch at or above `PITCH_TALL_MIN_H` rows, and below it.
const PITCH_TALL: i32 = 6;
const PITCH_SHORT: i32 = 5;
const PITCH_TALL_MIN_H: i32 = 48;
/// Most gems side by side in one column (a wide panel's columns are two stones across).
const MAX_ACROSS: i32 = 2;

/// Glitter: this percentage of interior pixels are candidate specks, split into `GLITTER_TIERS`
/// tiers; `GLITTER_REST_TIERS` of them show at rest (4 %), all of them at full rms (8 %).
const GLITTER_CANDIDATE_PCT: u64 = 8;
const GLITTER_TIERS: u64 = 8;
const GLITTER_REST_TIERS: f32 = 4.0;
/// A big sparkle grows its `+` arms above this brightness.
const SPARKLE_ARM_AT: f32 = 0.8;
const SPARKLE_LONG_AT: f32 = 0.93;
/// One speck in this many is a big sparkle.
const SPARKLE_BIG_ONE_IN: u64 = 6;

/// A glint runs up each lit column once per this many seconds.
const GLINT_PERIOD_S: f32 = 0.8;

/// The glitter phrase needs at least this many rows.
const TEXT_MIN_H: i32 = 52;
/// The glitter gradient's diagonal period in px, and its drift in px/s.
const GRAD_N: usize = 48;
const GRAD_PX_PER_S: f32 = 30.0;
/// Letter pitch of the 2x phrase: a 6 px glyph plus room for each letter's own outline rings.
const LETTER_PITCH: i32 = 10;
/// The phrase swaps every this-many strong onsets.
const PHRASE_SWAP_EVERY: u32 = 4;

/// The stamp pool, lifetime, pop-in scale, and the panel height at which they draw at 2x.
const STAMPS: usize = 3;
const STAMP_LIFE_MS: f32 = 600.0;
const STAMP_POP_SCALE: f32 = 0.6;
const STAMP_BIG_MIN_H: i32 = 52;

/// The flash flourish: its length, the fired frame's white alpha, the chrome sweep's crossing time
/// and band width, and the sparkle star's arm length (15x15 = centre + 7 each way).
const FLASH_MS: f32 = 500.0;
const FLASH_ALPHA: f32 = 0.85;
const SWEEP_MS: f32 = 400.0;
const SWEEP_W: i32 = 10;
const STAR_R: f32 = 7.0;
const STAR_TURNS_PER_S: f32 = 1.5;
/// The flourish only fires on a BASS hit (the `sesh` pattern).
const FLOURISH_BASS_MIN: f32 = 0.6;

/// A strong onset needs the low bands over this; the permissive flux net `sesh` uses.
const BASS_ONSET: f32 = 0.55;
const ONSET_RATIO: f32 = 2.8;
const ONSET_REFRACTORY_MS: f32 = 200.0;

/// Our own stamp bitmaps. `.` is empty, `L` lit, `H` hot, `W` white; the dark outline is derived.
const STAMP_SPARKLE: [&str; 7] = ["...W...", "...W...", ".H.W.H.", "WWWWWWW", ".H.W.H.", "...W...", "...W..."];
const STAMP_DOLLAR: [&str; 7] = ["..W..", ".LLLL", "L.W..", ".LLL.", "..W.L", "LLLL.", "..W.."];
const STAMP_CROWN: [&str; 5] = ["W..W..W", "L.LHL.L", "LLLLLLL", "LHLWLHL", "LLLLLLL"];
const STAMP_HEART: [&str; 6] = [".HH.HH.", "HHHWHHH", "HHWWWHH", ".HHWHH.", "..HHH..", "...H..."];

fn stamp_rows(kind: u8) -> &'static [&'static str] {
    match kind % 4 {
        0 => &STAMP_SPARKLE,
        1 => &STAMP_DOLLAR,
        2 => &STAMP_CROWN,
        _ => &STAMP_HEART,
    }
}

/// Where the gems go, for a panel size. Shared by `draw` and the tests (which read the painted gems).
#[derive(Clone, Copy, PartialEq, Default)]
struct Geom {
    pitch: i32,
    gem: i32,
    across: i32,
    rows: i32,
    /// Left x of each column's first gem.
    col_x: [i32; COLUMNS],
    /// Interior bottom (exclusive).
    iy1: i32,
}

impl Geom {
    fn col_w(&self) -> i32 {
        self.across * self.pitch - 1
    }
    fn col_cx(&self, col: usize) -> i32 {
        self.col_x[col] + self.col_w() / 2
    }
    /// Top y of gem `k` (0 = the bottom gem).
    fn gem_y(&self, k: i32) -> i32 {
        self.iy1 - (k + 1) * self.pitch
    }
    #[cfg(test)]
    fn gem_centre(&self, col: usize, k: i32) -> (i32, i32) {
        (self.col_x[col] + self.gem / 2, self.gem_y(k) + self.gem / 2)
    }
}

fn geom(w: i32, h: i32) -> Geom {
    let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
    let (iw, ih) = (ix1 - ix0, iy1 - iy0);
    let pitch = if h >= PITCH_TALL_MIN_H { PITCH_TALL } else { PITCH_SHORT };
    let slot = iw.max(0) as f32 / COLUMNS as f32;
    let across = (((slot - 1.0) / pitch as f32).floor() as i32).clamp(1, MAX_ACROSS);
    let rows = ((ih - 1) / pitch).max(0);
    let col_w = across * pitch - 1;
    let mut col_x = [0i32; COLUMNS];
    for (c, x) in col_x.iter_mut().enumerate() {
        *x = ix0 + ((2 * c as i32 + 1) * iw) / (2 * COLUMNS as i32) - col_w / 2;
    }
    Geom { pitch, gem: pitch - 1, across, rows, col_x, iy1 }
}

/// One glitter speck, hashed once per panel size.
#[derive(Clone, Copy)]
struct Speck {
    x: i16,
    y: i16,
    phase: f32,
    rate: f32,
    tier: u8,
    /// 0 `lit`, 1 `hot`, 2 white.
    kind: u8,
    big: bool,
}

#[derive(Clone, Copy, Default)]
struct Stamp {
    kind: u8,
    cx: i32,
    cy: i32,
    age_ms: f32,
    frames: u32,
    alive: bool,
}

/// A gem's five tones, all opaque.
#[derive(Clone, Copy)]
struct GemPal {
    outline: Rgba,
    light: Rgba,
    base: Rgba,
    dark: Rgba,
    centre: Rgba,
}

const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 255 };
const BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 255 };

impl GemPal {
    /// A stone of `base`, its dark facet toward `shade`.
    fn of(base: Rgba, shade: Rgba, outline: Rgba) -> GemPal {
        GemPal {
            outline,
            light: Rgba::lerp_linear(base, WHITE, 0.55),
            base,
            dark: Rgba::lerp_linear(base, shade, 0.45),
            centre: Rgba::lerp_linear(base, WHITE, 0.95),
        }
    }
}

pub struct Bling {
    /// Fires the flash on a rare bass hit. `pub(crate)` so the tests can force it.
    pub(crate) flourish: Trigger,
    flash: Envelope,
    onset: Flux,
    strong_count: u32,
    phrase_idx: usize,
    /// Real elapsed seconds (wrapped at an hour), for the twinkle, glint and gradient.
    elapsed_s: f32,
    /// Peak-hold level per column, falling at `peak_fall`.
    peak: [f32; COLUMNS],
    specks: Vec<Speck>,
    speck_dim: (i32, i32),
    geom: Geom,
    geom_dim: (i32, i32),
    stamps: [Stamp; STAMPS],
    rng: u64,
    /// Test hooks: draw the stamps alone; pop a stamp on the next frame.
    stamps_only: bool,
    force_stamp: bool,
}

impl Default for Bling {
    fn default() -> Self {
        Bling {
            flourish: Default::default(),
            flash: Default::default(),
            onset: Default::default(),
            strong_count: 0,
            phrase_idx: 0,
            elapsed_s: 0.0,
            peak: [0.0; COLUMNS],
            specks: Vec::new(),
            speck_dim: (0, 0),
            geom: Geom::default(),
            geom_dim: (0, 0),
            stamps: [Stamp::default(); STAMPS],
            rng: 0x2545_f491_4f6c_dd1d,
            stamps_only: false,
            force_stamp: false,
        }
    }
}

/// A deterministic 2D hash (the glitter positions, the text sparkle).
fn hash2(x: i32, y: i32) -> u64 {
    let mut z = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xD1B5_4A32_D192_ED03);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A finite level, clamped 0..1.
fn lvl(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn with_alpha(c: Rgba, a: f32) -> Rgba {
    Rgba { a: (a.clamp(0.0, 1.0) * 255.0).round() as u8, ..c }
}

impl Bling {
    /// Draws the stamps alone (no glitter, gems or text), for the stamp test.
    #[cfg(test)]
    fn stamps_only_for_test(&mut self) {
        self.stamps_only = true;
    }

    /// Pops a stamp on the next frame, as a strong onset would.
    #[cfg(test)]
    fn force_stamp_for_test(&mut self) {
        self.force_stamp = true;
    }

    /// One draw of splitmix64.
    fn next_rng(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Rebuilds the glitter list and the gem geometry when the panel size changes.
    fn resize(&mut self, w: i32, h: i32, bbox: (i32, i32, i32, i32)) {
        if self.geom_dim != (w, h) {
            self.geom = geom(w, h);
            self.geom_dim = (w, h);
        }
        if self.speck_dim != (w, h) {
            let (ix0, iy0, ix1, iy1) = bbox;
            self.specks.clear();
            for y in iy0..iy1 {
                for x in ix0..ix1 {
                    let hsh = hash2(x, y);
                    if hsh % 100 >= GLITTER_CANDIDATE_PCT {
                        continue;
                    }
                    let kind = match (hsh >> 44) % 20 {
                        0..=8 => 0,
                        9..=14 => 1,
                        _ => 2,
                    };
                    self.specks.push(Speck {
                        x: x as i16,
                        y: y as i16,
                        phase: ((hsh >> 20) & 0xffff) as f32 / 65536.0 * TAU,
                        // |sin| repeats at rate/PI Hz: 2..6 Hz.
                        rate: PI * (2.0 + 4.0 * ((hsh >> 36) & 0xff) as f32 / 255.0),
                        tier: ((hsh >> 8) % GLITTER_TIERS) as u8,
                        kind,
                        big: (hsh >> 52).is_multiple_of(SPARKLE_BIG_ONE_IN),
                    });
                }
            }
            self.speck_dim = (w, h);
        }
    }

    /// Pops a stamp into a free slot (or the oldest), at a random upper-half position.
    fn spawn_stamp(&mut self, bbox: (i32, i32, i32, i32), scale: i32) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let (r0, r1, r2) = (self.next_rng(), self.next_rng(), self.next_rng());
        let mut slot = 0;
        let mut oldest = -1.0f32;
        for (i, s) in self.stamps.iter().enumerate() {
            if !s.alive {
                slot = i;
                break;
            }
            if s.age_ms > oldest {
                oldest = s.age_ms;
                slot = i;
            }
        }
        let half = 4 * scale; // half a stamp, roughly
        let xs = (ix1 - ix0 - 2 * half).max(1) as u64;
        let ys = ((iy1 - iy0) / 2 - half).max(1) as u64;
        self.stamps[slot] = Stamp {
            kind: (r0 % 4) as u8,
            cx: ix0 + half + (r1 % xs) as i32,
            cy: iy0 + half + (r2 % ys) as i32,
            age_ms: 0.0,
            frames: 0,
            alive: true,
        };
    }
}

impl Family for Bling {
    fn id(&self) -> &'static str {
        "bling"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // ---- the opaque panel ----
        let panel = Rgba::from_hex(&t.panel, 1.0);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        let (ix0, iy0, ix1, iy1) = (3i32, 4i32, w - 3, h - 4);
        let (iw, ih) = (ix1 - ix0, iy1 - iy0);
        if iw < 8 || ih < 8 {
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return;
        }
        let bbox = (ix0, iy0, ix1, iy1);
        self.resize(w, h, bbox);

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        self.elapsed_s = (self.elapsed_s + dt / 1000.0).rem_euclid(3600.0);
        if !self.elapsed_s.is_finite() {
            self.elapsed_s = 0.0;
        }
        let e = self.elapsed_s;
        let rms = lvl((d.rms_l + d.rms_r) * 0.5);
        let bass = d.levels[0..8].iter().map(|&v| lvl(v)).sum::<f32>() / 8.0;

        // ---- fold 64 -> 24 columns; the levels are ALREADY smoothed, so only the peak hold here ----
        let g = self.geom;
        let peak_fall = t.ballistics.peak_fall.max(0.0) * dt / 16.667;
        let mut col_level = [0.0f32; COLUMNS];
        let mut loudest = 0usize;
        for (col, cl) in col_level.iter_mut().enumerate() {
            let lo = col * NUM_BANDS / COLUMNS;
            let hi = ((col + 1) * NUM_BANDS / COLUMNS).max(lo + 1);
            let sum: f32 = d.levels[lo..hi].iter().map(|&v| lvl(v)).sum();
            *cl = sum / (hi - lo) as f32;
            let pk = (self.peak[col] - peak_fall).max(*cl);
            self.peak[col] = if pk.is_finite() { pk.clamp(0.0, 1.0) } else { *cl };
        }
        for col in 1..COLUMNS {
            if col_level[col] > col_level[loudest] {
                loudest = col;
            }
        }

        // ---- onsets: stamps pop, the phrase swaps ----
        let onset = self.onset.update(&d.levels, dt, ONSET_RATIO, ONSET_REFRACTORY_MS);
        let strong = onset && bass > BASS_ONSET;
        let stamp_scale = if h >= STAMP_BIG_MIN_H { 2 } else { 1 };
        if strong {
            self.strong_count = self.strong_count.wrapping_add(1);
            if self.strong_count.is_multiple_of(PHRASE_SWAP_EVERY) {
                self.phrase_idx = (self.phrase_idx + 1) % PHRASES.len();
            }
        }
        if strong || std::mem::take(&mut self.force_stamp) {
            self.spawn_stamp(bbox, stamp_scale);
        }

        // ---- flourish: a bass-hit flash ----
        let triggered = self.flourish.update(&d.levels, dt, t.flourish);
        let bass_gate = triggered && bass >= FLOURISH_BASS_MIN;
        #[cfg(test)]
        let fired = bass_gate || (triggered && self.flourish.was_forced());
        #[cfg(not(test))]
        let fired = bass_gate;
        let env = self.flash.update(fired, dt, FLASH_MS);
        if fired {
            // Frame 1: the panel washed white, nothing else.
            c.fill_rect(ix0, iy0, iw, ih, with_alpha(WHITE, FLASH_ALPHA));
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return;
        }
        let diamond = env > 0.0;
        let since_ms = (1.0 - env) * FLASH_MS;

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let dark = Rgba::lerp_linear(panel, BLACK, 0.6);

        if !self.stamps_only {
            // ---- empty settings, then the glitter over them ----
            let ghost_k = (t.ghost * 2.0).clamp(0.05, 0.4);
            let lit_pal = if t.id == "bling-gold" {
                // Diamond stones faceted in gold.
                GemPal::of(Rgba::lerp_linear(hot, WHITE, 0.45), lit, dark)
            } else {
                GemPal::of(lit, BLACK, dark)
            };
            let top_pal = if t.id == "bling-gold" { GemPal::of(lit, BLACK, dark) } else { GemPal::of(hot, lit, dark) };
            let diamond_pal = GemPal::of(Rgba::lerp_linear(lit, WHITE, 0.8), Rgba::lerp_linear(lit, WHITE, 0.3), dark);
            let socket = Rgba::lerp_linear(panel, lit, ghost_k);

            let mut lit_n = [0i32; COLUMNS];
            for col in 0..COLUMNS {
                lit_n[col] = ((col_level[col] * g.rows as f32) + 0.35).floor().clamp(0.0, g.rows as f32) as i32;
                // An empty setting is only its socket: one dim centre pixel at `ghost`, so the column
                // grid is hinted without a field of dull stones fighting the lit ones.
                for k in lit_n[col]..g.rows {
                    for j in 0..g.across {
                        let (sx, sy) = (g.col_x[col] + j * g.pitch + g.gem / 2, g.gem_y(k) + g.gem / 2);
                        c.fill_rect(sx, sy, 1, 1, socket);
                    }
                }
            }

            let glitter = [lit, hot, WHITE];
            let tiers = (GLITTER_REST_TIERS + (GLITTER_TIERS as f32 - GLITTER_REST_TIERS) * rms).round() as u8;
            for s in &self.specks {
                if s.tier >= tiers {
                    continue;
                }
                let b = 0.3 + 0.7 * (e * s.rate + s.phase).sin().abs();
                let (x, y) = (s.x as i32, s.y as i32);
                c.fill_rect(x, y, 1, 1, with_alpha(glitter[s.kind as usize], b));
                if s.big && b > SPARKLE_ARM_AT {
                    let arm = with_alpha(WHITE, (b - SPARKLE_ARM_AT) / (1.0 - SPARKLE_ARM_AT) * 0.8);
                    c.fill_rect(x - 1, y, 1, 1, arm);
                    c.fill_rect(x + 1, y, 1, 1, arm);
                    c.fill_rect(x, y - 1, 1, 1, arm);
                    c.fill_rect(x, y + 1, 1, 1, arm);
                    if b > SPARKLE_LONG_AT {
                        // The brightest moment of a big sparkle: the arms reach two pixels.
                        let tip = with_alpha(WHITE, (b - SPARKLE_LONG_AT) / (1.0 - SPARKLE_LONG_AT) * 0.6);
                        c.fill_rect(x - 2, y, 1, 1, tip);
                        c.fill_rect(x + 2, y, 1, 1, tip);
                        c.fill_rect(x, y - 2, 1, 1, tip);
                        c.fill_rect(x, y + 2, 1, 1, tip);
                    }
                }
            }

            // ---- the rhinestone columns: lit gems, the top gem hot, the peak gem, the glint ----
            let peak_pal = GemPal::of(hot, lit, dark);
            for col in 0..COLUMNS {
                let n = lit_n[col];
                for k in 0..n {
                    let pal = if diamond {
                        &diamond_pal
                    } else if k == n - 1 {
                        &top_pal
                    } else {
                        &lit_pal
                    };
                    for j in 0..g.across {
                        draw_gem(c, g.col_x[col] + j * g.pitch, g.gem_y(k), g.gem, pal);
                    }
                }
                let pk = ((self.peak[col] * g.rows as f32) + 0.35).floor() as i32 - 1;
                if pk >= n && pk < g.rows && self.peak[col] > 0.05 {
                    let pal = if diamond { &diamond_pal } else { &peak_pal };
                    for j in 0..g.across {
                        draw_gem(c, g.col_x[col] + j * g.pitch, g.gem_y(pk), g.gem, pal);
                    }
                }
                if n > 0 {
                    // Per-column phase, so the glints run up the columns out of step.
                    let phase = (hash2(col as i32, 7) % 1000) as f32 / 1000.0;
                    let f = (e / GLINT_PERIOD_S + phase).fract();
                    let y_bot = iy1 - 2 - g.gem / 2;
                    let y_top = g.gem_y(n - 1) + g.gem / 2;
                    let gy = y_bot - (f * (y_bot - y_top) as f32).round() as i32;
                    let gx = g.col_cx(col);
                    let arm = Rgba::lerp_linear(WHITE, lit, 0.25);
                    c.fill_rect(gx - 1, gy, 3, 1, arm);
                    c.fill_rect(gx, gy - 1, 1, 3, arm);
                    c.fill_rect(gx, gy, 1, 1, WHITE);
                }
            }

            // ---- the glitter phrase ----
            let phrase = PHRASES[self.phrase_idx];
            let tw = phrase_width(phrase);
            if h >= TEXT_MIN_H && tw + 8 <= iw {
                let mut grad = [lit; GRAD_N];
                // lit -> hot -> a white shimmer -> hot -> lit, mixed in linear light.
                for (i, gc) in grad.iter_mut().enumerate() {
                    let u = i as f32 / GRAD_N as f32;
                    *gc = if u < 0.4 {
                        Rgba::lerp_linear(lit, hot, u / 0.4)
                    } else if u < 0.5 {
                        Rgba::lerp_linear(hot, WHITE, (u - 0.4) / 0.1)
                    } else if u < 0.6 {
                        Rgba::lerp_linear(WHITE, hot, (u - 0.5) / 0.1)
                    } else {
                        Rgba::lerp_linear(hot, lit, (u - 0.6) / 0.4)
                    };
                }
                let x = ix0 + (iw - tw) / 2;
                let y = iy0 + 3;
                let scroll = (e * GRAD_PX_PER_S) as i32;
                let spark = (e * 12.0) as i32;
                draw_phrase(c, x, y, phrase, &grad, scroll, spark, edge, dark);
            }
        }

        // ---- the stamps ----
        for s in self.stamps.iter_mut() {
            if !s.alive {
                continue;
            }
            if s.age_ms >= STAMP_LIFE_MS {
                s.alive = false;
                continue;
            }
            let f = if s.frames == 0 { STAMP_POP_SCALE } else { 1.0 };
            // Eased so the stamp stays punchy, then gone by `STAMP_LIFE_MS`.
            let a = 1.0 - (s.age_ms / STAMP_LIFE_MS).powi(2);
            draw_stamp(c, s, stamp_scale as f32 * f, a, [lit, hot, WHITE], dark);
            s.frames += 1;
            s.age_ms += dt;
        }

        // ---- the flash's tail: chrome sweep and the spinning star ----
        if diamond && !self.stamps_only {
            if since_ms < SWEEP_MS {
                let span = (iw + ih + SWEEP_W) as f32;
                let pos = ix0 - ih - SWEEP_W + (since_ms / SWEEP_MS * span) as i32;
                let sheen = with_alpha(WHITE, 0.35);
                let core = with_alpha(WHITE, 0.55);
                for y in iy0..iy1 {
                    let x = pos + (iy1 - 1 - y);
                    let (x0, x1) = (x.max(ix0), (x + SWEEP_W).min(ix1));
                    if x1 > x0 {
                        c.fill_rect(x0, y, x1 - x0, 1, sheen);
                    }
                    let (c0, c1) = ((x + 4).max(ix0), (x + 7).min(ix1));
                    if c1 > c0 {
                        c.fill_rect(c0, y, c1 - c0, 1, core);
                    }
                }
            }
            let n = ((col_level[loudest] * g.rows as f32) + 0.35).floor().max(1.0) as i32;
            let cx = g.col_cx(loudest);
            let cy = (g.gem_y(n - 1) + g.gem / 2).clamp(iy0 + STAR_R as i32, iy1 - 1 - STAR_R as i32);
            let ang = since_ms / 1000.0 * TAU * STAR_TURNS_PER_S;
            let a = (env * 2.0).min(1.0);
            let (long, short) = (with_alpha(WHITE, a), with_alpha(hot, a));
            for k in 0..4 {
                let th = ang + k as f32 * FRAC_PI_2;
                let (ex, ey) = ((th.cos() * STAR_R).round() as i32, (th.sin() * STAR_R).round() as i32);
                c.line(cx, cy, cx + ex, cy + ey, long);
                let th = th + FRAC_PI_4;
                let (sx, sy) = ((th.cos() * 3.0).round() as i32, (th.sin() * 3.0).round() as i32);
                c.line(cx, cy, cx + sx, cy + sy, short);
            }
            c.fill_rect(cx - 1, cy - 1, 3, 3, long);
        }

        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

/// One faceted rhinestone, `size` 5 (inner 3x3) or 4 (inner 2x2), top-left at (x, y): a dark
/// outline ring with the corners cut, a light top-left facet, a bright centre, a dark bottom-right.
fn draw_gem(c: &mut Canvas, x: i32, y: i32, size: i32, p: &GemPal) {
    let s = size;
    // The ring, corners cut.
    c.fill_rect(x + 1, y, s - 2, 1, p.outline);
    c.fill_rect(x + 1, y + s - 1, s - 2, 1, p.outline);
    c.fill_rect(x, y + 1, 1, s - 2, p.outline);
    c.fill_rect(x + s - 1, y + 1, 1, s - 2, p.outline);
    let (ix, iy) = (x + 1, y + 1);
    if s >= 5 {
        c.fill_rect(ix, iy, 1, 1, p.light);
        c.fill_rect(ix + 1, iy, 2, 1, p.base);
        c.fill_rect(ix, iy + 1, 1, 2, p.base);
        c.fill_rect(ix + 1, iy + 1, 1, 1, p.centre);
        c.fill_rect(ix + 2, iy + 1, 1, 2, p.dark);
        c.fill_rect(ix + 1, iy + 2, 1, 1, p.dark);
    } else {
        c.fill_rect(ix, iy, 1, 1, p.centre);
        c.fill_rect(ix + 1, iy, 1, 1, p.light);
        c.fill_rect(ix, iy + 1, 1, 1, p.base);
        c.fill_rect(ix + 1, iy + 1, 1, 1, p.dark);
    }
}

/// The drawn width of a phrase at 2x and `LETTER_PITCH` (no trailing gap, rings excluded).
fn phrase_width(text: &str) -> i32 {
    (text.chars().count() as i32 * LETTER_PITCH - (LETTER_PITCH - 6)).max(0)
}

/// The glitter phrase: 2x `font3x5`, each glyph pixel a 2x2 block, ringed in `edge` over a 1 px dark
/// outline, filled from the diagonal glitter gradient with white sparkle pixels.
#[allow(clippy::too_many_arguments)]
fn draw_phrase(
    c: &mut Canvas,
    x: i32,
    y: i32,
    text: &str,
    grad: &[Rgba; GRAD_N],
    scroll: i32,
    spark: i32,
    edge: Rgba,
    dark: Rgba,
) {
    for pass in 0..3 {
        let mut cx = x;
        for ch in text.chars() {
            if let Some(rows) = font3x5::glyph(ch) {
                for (dy, row) in rows.iter().enumerate() {
                    for dx in 0..3 {
                        if row & (0b100 >> dx) == 0 {
                            continue;
                        }
                        let (px, py) = (cx + dx * 2, y + dy as i32 * 2);
                        match pass {
                            0 => c.fill_rect(px - 2, py - 2, 6, 6, edge),
                            1 => c.fill_rect(px - 1, py - 1, 4, 4, dark),
                            _ => {
                                let col = if hash2(px + spark * 131, py).is_multiple_of(7) {
                                    WHITE
                                } else {
                                    grad[(px + py - scroll).rem_euclid(GRAD_N as i32) as usize]
                                };
                                c.fill_rect(px, py, 2, 2, col);
                            }
                        }
                    }
                }
            }
            cx += LETTER_PITCH;
        }
    }
}

/// One stamp at `scale` (nearest-neighbour), faded to `alpha`, with a dark 1 px outline derived from
/// its shape. `cols` = [lit, hot, white].
fn draw_stamp(c: &mut Canvas, s: &Stamp, scale: f32, alpha: f32, cols: [Rgba; 3], dark: Rgba) {
    let rows = stamp_rows(s.kind);
    let (bw, bh) = (rows[0].len() as i32, rows.len() as i32);
    let ow = ((bw as f32 * scale).round() as i32).max(1);
    let oh = ((bh as f32 * scale).round() as i32).max(1);
    let (x0, y0) = (s.cx - ow / 2, s.cy - oh / 2);
    let sample = |ox: i32, oy: i32| -> u8 {
        if ox < 0 || oy < 0 || ox >= ow || oy >= oh {
            return b'.';
        }
        rows[(oy * bh / oh) as usize].as_bytes()[(ox * bw / ow) as usize]
    };
    let outline = with_alpha(dark, alpha);
    let [l, hh, wh] = cols.map(|col| with_alpha(col, alpha));
    for oy in -1..=oh {
        for ox in -1..=ow {
            let col = match sample(ox, oy) {
                b'L' => l,
                b'H' => hh,
                b'W' => wh,
                _ => {
                    let near = (-1..=1).any(|dy| (-1..=1).any(|dx| sample(ox + dx, oy + dy) != b'.'));
                    if !near {
                        continue;
                    }
                    outline
                }
            };
            c.fill_rect(x0 + ox, y0 + oy, 1, 1, col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    const IDS: [&str; 4] = ["bling-pink", "bling-gold", "bling-ice", "bling-myspace"];

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    fn frames(fam: &mut Bling, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
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
    /// Draws `n` frames with only the given bands at `level` (the rest silent).
    fn frames_bands(
        fam: &mut Bling,
        t: &Theme,
        w: i32,
        h: i32,
        bands: std::ops::Range<usize>,
        level: f32,
        n: usize,
    ) -> Canvas {
        let mut c = Canvas::new(w, h);
        let mut d = FrameData::default();
        for v in d.levels[bands].iter_mut() {
            *v = level;
        }
        d.peaks = d.levels;
        d.rms_l = level * 0.5;
        d.rms_r = level * 0.5;
        d.dt_ms = 16.7;
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, t, &d);
        }
        c
    }
    /// How many gems of column `col` are LIT, read off the canvas: a lit gem's centre pixel is the
    /// near-white facet (far brighter than the panel), an empty setting's centre is dim.
    fn lit_gems(c: &Canvas, t: &Theme, col: usize) -> i32 {
        let g = geom(c.width(), c.height());
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let mut n = 0;
        for k in 0..g.rows {
            let (x, y) = g.gem_centre(col, k);
            let px = c.get(x, y);
            let d = (px.r as i32 - panel.r as i32) + (px.g as i32 - panel.g as i32) + (px.b as i32 - panel.b as i32);
            if d > 300 {
                n += 1;
            }
        }
        n
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"bling"));
        assert_eq!(crate::render::family_for("bling").id(), "bling");
        assert_eq!(crate::themes::family_label("bling"), "Blingee: bling bling");
        let ids: Vec<String> =
            crate::themes::builtin::all().iter().filter(|t| t.family == "bling").map(|t| t.id.clone()).collect();
        assert_eq!(ids, IDS.map(String::from).to_vec());
    }

    /// Silence still sparkles. At 380x60 the phrase is up too; at 190x48 (no phrase, no gems lit)
    /// the glitter alone has to carry it.
    #[test]
    fn rest_frame_is_not_empty() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (190, 48)] {
                let c = frames(&mut Bling::default(), &t, w, h, 0.0, 10);
                let n = lit(&c, &t);
                assert!(n as f32 >= 0.025 * (w * h) as f32, "{id} {w}x{h}: only {n} px at rest");
            }
        }
    }

    /// Bass-only: the leftmost columns stand tall, the rightmost stay (nearly) empty - the gem
    /// columns ARE the spectrum, read off the painted pixels.
    #[test]
    fn gem_columns_are_the_meter() {
        for (w, h) in [(380, 60), (190, 48), (128, 44)] {
            let t = theme("bling-pink");
            let mut fam = Bling::default();
            let c = frames_bands(&mut fam, &t, w, h, 0..8, 0.9, 20);
            for col in 0..2 {
                let n = lit_gems(&c, &t, col);
                assert!(n >= 4, "{w}x{h}: bass column {col} has only {n} lit gems");
            }
            for col in COLUMNS - 2..COLUMNS {
                let n = lit_gems(&c, &t, col);
                assert!(n <= 1, "{w}x{h}: treble column {col} has {n} lit gems on a bass-only input");
            }
        }
    }

    /// At rest the glitter still twinkles: two frames 100 ms apart differ in at least 30 pixels. At
    /// 190x48 so the phrase (whose gradient also moves) is dropped and at rest there are no glints or
    /// stamps: only the glitter can change.
    #[test]
    fn glitter_twinkles() {
        let t = theme("bling-pink");
        let mut fam = Bling::default();
        let a = frames(&mut fam, &t, 190, 48, 0.0, 20);
        let b = frames(&mut fam, &t, 190, 48, 0.0, 6); // 6 x 16.7 = 100 ms later
        let mut diff = 0;
        for y in 4..44 {
            for x in 3..187 {
                let (pa, pb) = (a.get(x, y), b.get(x, y));
                let d = (pa.r as i32 - pb.r as i32).abs()
                    + (pa.g as i32 - pb.g as i32).abs()
                    + (pa.b as i32 - pb.b as i32).abs();
                if d > 24 {
                    diff += 1;
                }
            }
        }
        assert!(diff >= 30, "only {diff} pixels changed in 100 ms at rest");
    }

    /// A forced strong onset pops a stamp (drawn alone, the other layers suppressed); 700 ms later it
    /// has faded out completely.
    #[test]
    fn stamps_pop_on_strong_onsets_and_fade() {
        let t = theme("bling-gold");
        let mut fam = Bling::default();
        fam.stamps_only_for_test();
        let _ = frames(&mut fam, &t, 380, 60, 0.0, 3);
        fam.force_stamp_for_test();
        // Total paint over the panel interior: the channel deltas summed, so a fade shows as a falling number.
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let intensity = |c: &Canvas| -> i64 {
            let mut s = 0i64;
            for y in 4..c.height() - 4 {
                for x in 3..c.width() - 3 {
                    let px = c.get(x, y);
                    s += ((px.r as i32 - panel.r as i32).abs()
                        + (px.g as i32 - panel.g as i32).abs()
                        + (px.b as i32 - panel.b as i32).abs()) as i64;
                }
            }
            s
        };
        let c = frames(&mut fam, &t, 380, 60, 0.0, 3); // ~50 ms
        let n = lit(&c, &t);
        assert!(n >= 20, "no stamp popped: {n} px");
        let early = intensity(&c);
        let c = frames(&mut fam, &t, 380, 60, 0.0, 15); // ~300 ms
        let mid = intensity(&c);
        assert!(lit(&c, &t) > 0 && (mid as f64) < 0.9 * early as f64, "not fading: {early} -> {mid}");
        let c = frames(&mut fam, &t, 380, 60, 0.0, 24); // ~700 ms
        let n = lit(&c, &t);
        assert_eq!(n, 0, "the stamp is still there after 700 ms: {n} px");
    }

    #[test]
    fn every_phrase_char_has_a_glyph() {
        for p in PHRASES {
            for ch in p.chars() {
                assert!(font3x5::glyph(ch).is_some(), "{p:?} {ch:?}");
            }
        }
    }

    /// The fired frame of the flash flourish is the panel washed white - before any gem or glitter.
    #[test]
    fn flash_flourish_whites_the_panel_first() {
        for id in IDS {
            let t = theme(id);
            let mut fam = Bling::default();
            let _ = frames(&mut fam, &t, 380, 60, 0.3, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 380, 60, 0.3, 1);
            let (mut white, mut total) = (0, 0);
            for y in 4..56 {
                for x in 3..377 {
                    let px = c.get(x, y);
                    total += 1;
                    if px.r >= 200 && px.g >= 200 && px.b >= 200 {
                        white += 1;
                    }
                }
            }
            assert!(white as f32 >= 0.95 * total as f32, "{id}: only {white}/{total} interior px flashed white");
            // And after the flash it is a meter again, not a white panel.
            let c = frames(&mut fam, &t, 380, 60, 0.3, 40);
            let mut still_white = 0;
            for y in 4..56 {
                for x in 3..377 {
                    let px = c.get(x, y);
                    if px.r >= 200 && px.g >= 200 && px.b >= 200 {
                        still_white += 1;
                    }
                }
            }
            assert!((still_white as f32) < 0.3 * total as f32, "{id}: {still_white} px still white after the flash");
        }
    }

    /// 190x48 and 128x44 with a forced flourish and stamp: no panic, something drawn, and nothing
    /// outside the panel (rows 0-1 and h-2..h, columns 0 and w-1 stay transparent).
    #[test]
    fn fits_the_narrow_panel_and_flourish_does_not_panic() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(190, 48), (128, 44)] {
                let mut fam = Bling::default();
                let _ = frames(&mut fam, &t, w, h, 0.7, 5);
                fam.force_stamp_for_test();
                fam.flourish.force_next();
                for n in [1usize, 5, 30] {
                    let c = frames(&mut fam, &t, w, h, 0.7, n);
                    assert!(lit(&c, &t) > 0, "{id} {w}x{h}");
                    for x in 0..w {
                        for y in [0, 1, h - 2, h - 1] {
                            assert_eq!(c.get(x, y).a, 0, "{id} {w}x{h}: paint outside the panel at {x},{y}");
                        }
                    }
                    for y in 0..h {
                        assert_eq!(c.get(0, y).a, 0, "{id} {w}x{h}: paint left of the panel at 0,{y}");
                        assert_eq!(c.get(w - 1, y).a, 0, "{id} {w}x{h}: paint right of the panel at {y}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_four_colourways_are_visibly_different() {
        let (w, h) = (380i32, 60i32);
        let canvases: Vec<Canvas> =
            IDS.iter().map(|id| frames(&mut Bling::default(), &theme(id), w, h, 0.85, 30)).collect();
        let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
        let interior = ((ix1 - ix0) * (iy1 - iy0)) as f32;
        let mut too_similar = Vec::new();
        for a in 0..IDS.len() {
            for b in (a + 1)..IDS.len() {
                let mut diff = 0usize;
                for y in iy0..iy1 {
                    for x in ix0..ix1 {
                        let (pa, pb) = (canvases[a].get(x, y), canvases[b].get(x, y));
                        let d = (pa.r as i32 - pb.r as i32).abs()
                            + (pa.g as i32 - pb.g as i32).abs()
                            + (pa.b as i32 - pb.b as i32).abs();
                        if d > 24 {
                            diff += 1;
                        }
                    }
                }
                let frac = diff as f32 / interior;
                if frac < 0.15 {
                    too_similar.push(format!("{} vs {}: {:.1}%", IDS[a], IDS[b], frac * 100.0));
                }
            }
        }
        assert!(too_similar.is_empty(), "colourway pairs differ in <15% of interior pixels: {too_similar:?}");
    }

    /// Dumps for the eye test - composited over `#202020` like every other family's dump. Each file
    /// is `<name>.<w>x<h>.rgba`.
    ///
    /// Run: cargo test --release dump_bling -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_bling() {
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
            std::fs::write(dir.join(format!("{name}.{}x{}.rgba", c.width(), c.height())), &out).unwrap();
        };
        let frame = |level: f32, t_s: f32, beat: bool| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                let shape = (1.0 - f).powf(1.2) * 0.7 + 0.12;
                let wob = 1.0 + 0.3 * (t_s * 2.4 + f * 6.0).sin();
                let kick = if beat { 1.0 + 0.9 * (1.0 - f) } else { 1.0 };
                *v = ((shape * wob * kick) * level).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d.rms_l = level * 0.6;
            d.rms_r = level * 0.6;
            d
        };
        // Beats every 24 frames, the kick held three frames so the onset net sees it.
        let beat = |k: usize| k % 24 < 3;
        let run = |id: &str, w: i32, h: i32, level: f32, n: usize| {
            let t = theme(id);
            let mut fam = Bling::default();
            let mut c = Canvas::new(w, h);
            for k in 0..n {
                c.clear();
                fam.draw(&mut c, &t, &frame(level, k as f32 * 0.0167, beat(k)));
            }
            (fam, c, t)
        };
        for id in IDS {
            let short = &id["bling-".len()..];
            for (tag, level) in [("calm", 0.28f32), ("loud", 0.85)] {
                let (_, c, _) = run(id, 380, 60, level, 124);
                write(format!("bling-{short}-{tag}"), &c);
            }
            // Flourish: settle, fire (the white flash frame), then ~150 ms in (diamond gems, the
            // chrome sweep crossing, the spinning star).
            let (mut fam, mut c, t) = run(id, 380, 60, 0.7, 120);
            fam.flourish.force_next();
            c.clear();
            fam.draw(&mut c, &t, &frame(0.7, 120.0 * 0.0167, false));
            write(format!("bling-{short}-flash"), &c);
            for k in 121..130 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167, false));
            }
            write(format!("bling-{short}-flourish"), &c);
        }
        for (w, h) in [(190, 48), (128, 44)] {
            let (_, c, _) = run("bling-pink", w, h, 0.85, 124);
            write(format!("bling-pink-loud-{w}x{h}"), &c);
        }
        println!("wrote bling dumps to {}", dir.display());
    }

    /// Per-frame cost, steady (with stamps kept popping) and during the flash flourish, at 380x60.
    ///
    /// Run: cargo test --release probe_bling_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_bling_cost() {
        for id in IDS {
            let t = theme(id);
            let mut fam = Bling::default();
            let mut c = Canvas::new(380, 60);
            let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                *v = 0.6 * (1.0 - i as f32 / 96.0);
            }
            d.peaks = d.levels;
            d.rms_l = 0.6;
            d.rms_r = 0.6;
            for _ in 0..60 {
                fam.draw(&mut c, &t, &d);
            }
            let n = 600;
            let t0 = std::time::Instant::now();
            for k in 0..n {
                if k % 12 == 0 {
                    fam.force_stamp_for_test();
                }
                d.time_s = k as f32 * 0.0167;
                fam.draw(&mut c, &t, &d);
            }
            let steady = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
            fam.flourish.force_next();
            let m = 30; // the 500 ms flash
            let t1 = std::time::Instant::now();
            for k in n..(n + m) {
                d.time_s = k as f32 * 0.0167;
                fam.draw(&mut c, &t, &d);
            }
            let flourish = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
            println!("{id}: steady {steady:.3} ms/frame, flourish {flourish:.3} ms/frame at 380x60");
        }
    }
}
