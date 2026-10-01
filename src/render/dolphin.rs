//! The dolphins car-stereo family: one coarse dot-matrix LCD, edge to edge.
//!
//! Asked for as the 1990s/2000s aftermarket head unit - Sony Xplod, Pioneer, JVC - "with a dolphin
//! arcing across a low-res display while a spectrum analyser runs underneath" (see
//! `docs/theme-backlog.md` item 1, which is the owner's own spec and the authority for this family).
//!
//! # The three decisions worth knowing
//!
//! **The panel IS the display.** No bezel, no buttons, no fascia. At this size those would spend rows
//! the meter needs, and the object being imitated is the DISPLAY, not the stereo around it.
//!
//! **The unlit dots are drawn.** That faint lattice of dark wells is the whole trick - it is what makes
//! this read as a dot-matrix display rather than as floating squares. `Theme::ghost` sets its alpha,
//! the same field `segmented` uses for its dormant bars. Measured for the shipped colourways the well
//! sits +9.1 to +10.9 dL* above the panel, which is 4x the ~2.3 dL* that `tube.rs:58` established as
//! the floor of visibility here - so it is genuinely visible - while staying at 1.20-1.25:1 contrast
//! against the panel, far below the 3.0:1 this project calls "lit". Visible, and unmistakably off.
//!
//! **Level is POSITION, never brightness.** Column height in dots. `tube.rs:54-60` measured a driven
//! element 1.46 dL* brighter than its idle neighbour and it was invisible; every family since encodes
//! level as position. The three brightness levels here carry STATE (lit / peak-cap / unlit), not
//! magnitude.
//!
//! # Why 14 rows and not 12
//!
//! The brief said "edge to edge" and "12 dot rows at a 60px panel", and those contradict each other:
//! edge to edge at a 4px pitch is 15 rows, and 12 implies keeping `segmented`'s 6px pad - a bezel by
//! another name. Resolved by deriving the grid from the PANEL INTERIOR that every family already
//! respects, `rounded_rect(1, 2, w - 2, h - 4, ..)`: 56 usable pixels at h=60, so 14 dot rows.
//!
//! That is not pedantry, it buys the leap. At 12 rows there was exactly ONE dot row (4px) of clear air
//! above a 5-row sprite's apex; at 14 there are enough for the 8-row 2x hero to rise out of the sea
//! (fidelity pass 2, below), and the spectrum still gets 7 rows - inside the "6-8 segments" the brief
//! asked for.
//!
//! # The dolphin
//!
//! Never a flip. A horizontal flip gives a dolphin climbing while travelling the wrong way; a VERTICAL
//! flip gives the right attitude but puts the dorsal fin on the belly, which reads instantly as an
//! upside-down fish. The attitude comes from a column shear instead (see `MAX_SLOPE`), which keeps the
//! fin on top by construction.
//!
//! Speed tracks loudness, by the owner's decision. Bounded below by aliasing rather than by taste:
//! `reel.rs` measured that motion past half a feature pitch per frame appears to run BACKWARDS, so at
//! a 4px pitch the dolphin may not cross more than half a dot cell per frame. Over a 95-column panel
//! that makes ~1.6s the fastest honest loop; `LOOP_FAST_S` sits above it.

//!
//! # Fidelity pass 2: the hero leap
//!
//! The owner's note was that this family "needs some polish" and did not pop beside blossom and
//! vaporwave. What it lacked was a hero and a world, so both were added without touching the meter:
//!
//! - **The dolphin is 2x** - a new 26x8-cell sprite (a swept-back fin, a melon, a beak, a pale belly
//!   and an eye) rather than the 11x5 one at 2x2 dots, because at this cell count there is room for
//!   the features a doubled lump would not have. It flies a real parabola, PITCHED along its own
//!   tangent - nose up on the way out, level at the apex, nose down on the way in - which the old
//!   sprite could not do (see `BODY`). The leap height follows the bass. The sea occludes it: a
//!   cell of the dolphin only shows above the lit column in front of it, so it rises OUT of the
//!   spectrum and goes back INTO it. Below `HERO_MIN_ROWS` dot rows it falls back to the 1x sprite.
//! - **A re-entry splash** from fixed pools: droplets thrown up under gravity and a ring of dots
//!   spreading along the surface.
//! - **A sky**: a sun (warm colourways) or a crescent moon (cool ones) setting on the horizon with a
//!   glow ring, its glittering reflection column down the sea, and a few twinkling stars.
//! - **Travelling crests**: a dot one row above every few columns' level line, scrolling with time,
//!   each with a dim ring of neighbour glow. Not `bloom` - this panel is opaque and `bloom` both
//!   allocated every frame and cost about 1 ms of the old 1.4 ms frame.
//!
//! The static world - the panel, the lattice with its sky and sea shading, the horizon and the sun -
//! is baked once per (size, colourway) and copied in each frame, which is what pays for the rest.
//! `draw` does not allocate once a size has been seen.

use crate::dsp::bands::NUM_BANDS;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Lit dot size and its pitch, in pixels. The 1px difference is the dark well.
///
/// Deliberately chunkier than the VFD family's 5px bars, per the brief: this display's pixels are
/// meant to be visibly discrete.
const DOT: i32 = 3;
const PITCH: i32 = 4;

/// Rows given to the spectrum, and the alpha the peak-hold cap is drawn at.
///
/// Seven is inside the brief's "often only 6-8 segments tall". `CAP_ALPHA` is one constant for every
/// colourway: composited it puts the cap 26-29 dL* below the lit dot and still clears 4.0:1 against
/// every shipped panel, so a cap can neither be mistaken for a lit dot nor vanish into the substrate.
const SPEC_ROWS: i32 = 7;
const CAP_ALPHA: f32 = 0.60;

/// Below this the family SHEDS rather than smudges - the convention `nixie` and `patchbay` follow.
///
/// A dot-matrix display with four rows or eight columns is not a smaller version of this family, it is
/// an unreadable grid. Under these it draws the panel and stops.
const MIN_ROWS: i32 = 8;
const MIN_COLS: i32 = 14;

/// The level window, taken from `vapor`'s MEASURED p10-p90 of real music rather than invented.
///
/// A mapping over 0..1 renders dead here, and normalising against the frame's loudest band is provably
/// inert - that band already sits at p50 0.819, so the normaliser settles near 1.1x. Four attempts in
/// the vaporwave family failed that way before this window was measured.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// Loop duration at silence and at full drive, in seconds. See the module docs for the aliasing floor.
const LOOP_SLOW_S: f32 = 5.6;
const LOOP_FAST_S: f32 = 2.2;

/// The 1x sprite, kept for panels under `HERO_MIN_ROWS`: body F, chosen from four rendered at true
/// dot scale, with THREE TAIL PHASES.
///
/// The history matters, because two earlier attempts were rejected for the same reason and a third
/// would have been. A solid 9x5 lump read as "a lump with something sticking out". Replacing it with a
/// thin arcing back read as "a worm" - correctly: it was a one-to-two cell diagonal. What was missing
/// both times was a BODY: a thick middle, a distinct head, a fin above and a fluke separated from the
/// body by a narrower peduncle. Eleven cells wide instead of nine buys exactly that.
///
/// The three phases are the FLUKE moving up and down. Cycled on a timer, that is the movement asked
/// for - a dolphin that flicks its tail rather than a rigid decal sliding across the display.
///
/// At 1x the attitude stays LEVEL through the arc: at eleven cells there is no room for a pitched
/// variant that keeps the fin, the melon and the fluke all legible. The 2x `HERO` pitches (see `MAX_SLOPE`).
const SPRITE_W: i32 = 11;
const SPRITE_H: i32 = 5;
const PHASES: usize = 3;
const BODY: [[&str; SPRITE_H as usize]; PHASES] = [
    [
        "....##.....",
        "##..######.",
        ".#.########",
        "##..######.",
        "......##...",
    ],
    [
        "##..##.....",
        ".#..######.",
        "##.########",
        "....######.",
        "......##...",
    ],
    [
        "....##.....",
        "....######.",
        "##.########",
        ".#..######.",
        "##....##...",
    ],
];

/// The 2x hero: `#` back, `+` belly, `o` eye. Facing right, the way it travels.
///
/// Drawn at true dot scale and iterated by eye, like `BODY`. What makes it a DOLPHIN rather than the
/// shark the first draft was: the fin is swept BACK (tip behind its base), the forehead is a rounded
/// melon that steps down to a short beak, and the belly is pale. The fluke is a fork, and its first
/// `FLUKE_COLS` columns are shifted up or down a row by the tail phase - the same three-phase beat as
/// `BODY` without a mask per phase.
const HERO_W: i32 = 26;
const HERO_H: i32 = 8;
const HERO: [&str; HERO_H as usize] = [
    ".........##...............",
    "..........####............",
    "##.......############.....",
    ".##..###############o##...",
    "..######################..",
    ".##...+++++++++++++++++###",
    "##........++++++++++......",
    "............++............",
];
const FLUKE_COLS: i32 = 5;
/// The hero needs this many dot rows; below it the 1x sprite flies instead. Ten is 128x44's grid -
/// the smallest shipped panel - so every shipped size gets the hero.
const HERO_MIN_ROWS: i32 = 10;

/// How long one tail phase is held, in milliseconds.
///
/// 130ms is about 7.7 phases a second, so the fluke beats a little under 3Hz through the 3-phase cycle
/// - a real dolphin's tail beat. Also comfortably above the aliasing floor: the sprite moves less than
/// half a dot cell per frame at every loop speed, so nothing appears to run backwards.
const PHASE_MS: f32 = 130.0;

/// The leap. A traverse holds as many leaps as fit `LEAP_SPAN` columns each, so a wide panel sees the
/// dolphin porpoise twice per crossing while a narrow one sees one big arc.
const LEAP_SPAN: f32 = 70.0;
/// The quietest leap's height as a fraction of the loudest, which puts the hero's top at row 0.
const LEAP_MIN_K: f32 = 0.55;
/// The steepest the hero pitches, as rows of shear per column (about 17 degrees). Steeper and the
/// stair-steps along the back start to read as a broken spine.
const MAX_SLOPE: f32 = 0.3;
/// The apex highlight. The spec says one frame; at 60fps one frame is under the threshold of noticing,
/// so it is held for three.
const APEX_MS: f32 = 50.0;
/// How long a flourish leap is held at full height. Longer than most flourishes because it is a whole
/// leap, not a flash.
const LEAP_MS: f32 = 1800.0;

/// The splash: a fixed pool of droplets and of surface rings - no allocation when one fires.
const DROPS: usize = 10;
const DROPS_NORMAL: usize = 8;
const RINGS: usize = 2;
/// Droplet gravity and launch speed, in rows/s(^2) on the 14-row grid (scaled to the grid's height):
/// the throw peaks 4-9 rows up and lands again in about a second.
const GRAVITY: f32 = 40.0;
const DROP_VY: (f32, f32) = (17.0, 27.0);
const RING_SPEED: f32 = 11.0;
const RING_MS: f32 = 650.0;

/// Wave crests: one dot every `CREST_GAP` columns, scrolling right at a speed that rises with drive,
/// with a dim ring of `CREST_GLOW` around each.
const CREST_GAP: i32 = 7;
const CREST_SPEED: f32 = 4.0;
const CREST_SPEED_DRIVE: f32 = 6.0;
const CREST_GLOW: f32 = 0.22;

/// The sky: up to this many stars, hashed per size, and the twinkle's floor and swing.
const MAX_STARS: usize = 8;
const STAR_FLOOR: f32 = 0.12;
const STAR_SWING: f32 = 0.5;
/// The reflection column's peak alpha (sun; the moon's is `MOON_K` of it) and its glitter rate in Hz.
const GLITTER_A: f32 = 0.55;
const MOON_K: f32 = 0.7;
const GLITTER_HZ: f32 = 7.0;
/// The disc's glow on the sky wells: peak alpha and reach in cells beyond its rim.
const GLOW_A: f32 = 0.38;
const GLOW_R: f32 = 4.0;

/// The rasterising buffer for the pitched hero: its width and height plus the shear's reach
/// (`MAX_SLOPE` x half its width, rounded up, plus the tail flick) and a keyline border.
const BUF: usize = 30;
const PITCH_PAD: i32 = 5;

#[derive(Clone, Copy, Default)]
struct Drop {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    live: bool,
}

#[derive(Clone, Copy, Default)]
struct Ring {
    x: f32,
    r: f32,
    life: f32,
}

#[derive(Clone, Copy, Default)]
struct Star {
    col: i32,
    row: i32,
    phase: f32,
    rate: f32,
}

/// The grid the renderer derives from the panel interior. One function, so the bake, the draw and the
/// tests can never disagree about where a row is.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
struct Grid {
    rows: i32,
    cols: i32,
    ox: i32,
    oy: i32,
    spec_rows: i32,
    water_row: i32,
}

fn grid(w: i32, h: i32) -> Grid {
    let rows = (h - 4) / PITCH;
    let cols = (w - 2) / PITCH;
    let ox = 1 + ((w - 2) - cols * PITCH) / 2;
    let oy = 2 + ((h - 4) - rows * PITCH) / 2;
    let spec_rows = SPEC_ROWS.min(rows - 3);
    let water_row = rows - spec_rows - 1;
    Grid { rows, cols, ox, oy, spec_rows, water_row }
}

/// Where the sun (or moon) sits, in cell units, and its radius. Low on the horizon, right of centre.
fn sun_at(g: Grid, moon: bool) -> (f32, f32, f32) {
    let r = (g.water_row as f32 * 0.45).clamp(1.6, 2.7);
    let cx = (g.cols as f32 * 0.74).round();
    // The sun is SETTING: its centre just above the horizon, so the horizon cuts its lower half. The
    // moon rides a little higher, so its crescent shows whole where the sky has the rows for it.
    let cy = if moon { (g.water_row as f32 - r - 0.4).max(r - 0.4) } else { g.water_row as f32 - r * 0.55 };
    (cx, cy, r)
}

/// Whether a cell is part of the disc. The moon is the disc less a second disc offset up and right.
fn in_disc(col: i32, row: i32, sun: (f32, f32, f32), moon: bool) -> bool {
    let (cx, cy, r) = sun;
    let (dx, dy) = (col as f32 - cx, row as f32 - cy);
    let inside = dx * dx + dy * dy <= r * r;
    if !moon {
        return inside;
    }
    let (bx, by) = (dx - r * 1.2, dy + r * 0.2);
    inside && bx * bx + by * by > r * r
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn hash01(x: u32) -> f32 {
    (hash(x) >> 8) as f32 / (1u32 << 24) as f32
}

fn with_alpha(c: Rgba, a: f32) -> Rgba {
    let a = if a.is_finite() { a.clamp(0.0, 1.0) } else { 0.0 };
    Rgba { a: (a * 255.0).round() as u8, ..c }
}

/// FNV-1a over everything the baked backdrop depends on. No allocation.
fn bake_key(t: &Theme, w: i32, h: i32) -> u64 {
    let mut k: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        k ^= b as u64;
        k = k.wrapping_mul(0x0100_0000_01b3);
    };
    for s in [t.id.as_str(), t.panel.as_str(), t.lit.as_str(), t.hot.as_str()] {
        for b in s.bytes() {
            eat(b);
        }
        eat(0);
    }
    for b in t
        .ghost
        .to_bits()
        .to_le_bytes()
        .into_iter()
        .chain(t.panel_alpha.to_bits().to_le_bytes())
        .chain(w.to_le_bytes())
        .chain(h.to_le_bytes())
    {
        eat(b);
    }
    k | 1
}

/// The dolphin's back: between `lit` and `hot`, nearer `hot`. A colour nothing else on the panel uses,
/// which is also what lets a test find the dolphin among the sun, the stars and the splash.
fn body_colour(t: &Theme) -> Rgba {
    Rgba::lerp_linear(Rgba::from_hex(&t.lit, 1.0), Rgba::from_hex(&t.hot, 1.0), 0.7)
}

/// White water: the splash's colour, the brightest thing on the sea so it reads against the pale
/// crests - and, like `body_colour`, one nothing else uses.
fn spray_colour(t: &Theme) -> Rgba {
    Rgba::lerp_linear(Rgba::from_hex(&t.hot, 1.0), Rgba::new(255, 255, 255, 255), 0.5)
}

/// A cool colourway gets a moon rather than a sun: ice under a sun reads as a mistake.
fn is_moon(t: &Theme) -> bool {
    let lit = Rgba::from_hex(&t.lit, 1.0);
    lit.b > lit.r
}

#[derive(Default)]
pub struct Dolphin {
    /// Level per COLUMN, 0..1, and the peak-hold cap above it in the same units.
    ///
    /// The level is `d.levels` through the window, NOT smoothed again here: main.rs already runs every
    /// band through the theme's ballistics before `FrameData` is built, and this family used to apply
    /// them a second time (see the fidelity-pass-2 report). The cap's fall is still this family's.
    levels: Vec<f32>,
    caps: Vec<f32>,
    /// The top lit row of each column - the sea surface the dolphin rises out of. `rows` = no water.
    surf: Vec<i32>,
    /// Loop phase, 0..1. One loop is one traverse of the display.
    phase: f32,
    /// Which tail phase is showing, and the unspent time toward the next.
    tail: usize,
    tail_due: f32,
    /// This leap's height in rows (ratcheted up by the bass during the climb, frozen on the way down),
    /// the previous frame's position in the leap, and whether the dolphin was above the water then.
    leap_h: f32,
    prev_q: f32,
    prev_above: bool,
    /// The apex highlight's remaining time, ms.
    apex_ms: f32,
    /// Re-entries so far: seeds the splash and lets a test find the moment one happened.
    entries: u32,
    drops: [Drop; DROPS],
    rings: [Ring; RINGS],
    /// Crest scroll offset, columns.
    crest_off: f32,
    /// The baked static world and what it was baked for.
    backdrop: Option<Canvas>,
    baked: u64,
    stars: [Star; MAX_STARS],
    n_stars: usize,
    /// The flourish: a full-height leap with a bigger splash.
    flourish: crate::dsp::flourish::Trigger,
    leap: crate::dsp::flourish::Envelope,
}

/// Level through the measured window. Position, so this is the only thing carrying magnitude.
fn resp(level: f32, sensitivity: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    let x = ((level - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0);
    (x.powf(LEVEL_GAMMA) * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

impl Dolphin {
    /// Mean of the band levels through the window - what drives the dolphin's speed.
    fn drive(d: &FrameData, sensitivity: f32) -> f32 {
        let n = d.levels.len().max(1);
        let sum: f32 = d.levels.iter().map(|v| resp(*v, sensitivity)).sum();
        (sum / n as f32).clamp(0.0, 1.0)
    }

    /// The bottom eighth of the spectrum through the window - what sets the leap height.
    fn bass(d: &FrameData, sensitivity: f32) -> f32 {
        let n = (NUM_BANDS / 8).max(1);
        let sum: f32 = d.levels[..n].iter().map(|v| resp(*v, sensitivity)).sum();
        (sum / n as f32).clamp(0.0, 1.0)
    }

    /// Resizes the per-column state, preserving what it can when the taskbar changes width.
    fn fit(&mut self, cols: usize) {
        self.levels.resize(cols, 0.0);
        self.caps.resize(cols, 0.0);
        self.surf.resize(cols, 0);
    }

    /// Bakes the static world for this size and colourway. The only place `draw` allocates.
    fn bake(&mut self, t: &Theme, w: i32, h: i32, g: Grid) {
        let key = bake_key(t, w, h);
        if self.baked == key && self.backdrop.is_some() {
            return;
        }
        self.baked = key;
        let moon = is_moon(t);
        let sun = sun_at(g, moon);
        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let ghost = t.ghost.clamp(0.0, 1.0);
        let mut bd = Canvas::new(w, h);
        bd.rounded_rect(1, 2, w - 2, h - 4, 3, Rgba::from_hex(&t.panel, t.panel_alpha));

        // ---- the well lattice: every cell, faintly. This is what makes it a display. ----
        //
        // Shaded for depth without leaving the "off" range: the sky deepens upward to a night at the
        // top (the glow-and-depth families all sit on a dark ground, and a flat mid-tone lattice read as
        // noise beside them), the sea's wells hold a little light near the horizon and fade with depth,
        // and around the disc the wells pick up its glow, falling off smoothly (stepped rings read as a
        // box around the disc at this resolution).
        let wr = g.water_row.max(1) as f32;
        for row in 0..g.rows {
            for col in 0..g.cols {
                let a = if row < g.water_row {
                    ghost * (0.08 + 0.97 * (row as f32 / wr).powf(1.3))
                } else if row == g.water_row {
                    ghost * 1.1
                } else {
                    ghost * (1.15 - 0.45 * (row - g.water_row) as f32 / g.spec_rows.max(1) as f32)
                };
                let (x, y) = (g.ox + col * PITCH, g.oy + row * PITCH);
                bd.fill_rect(x, y, DOT, DOT, with_alpha(lit, a));
                if row < g.water_row {
                    let (dx, dy) = (col as f32 - sun.0, row as f32 - sun.1);
                    let d = (dx * dx + dy * dy).sqrt() - sun.2;
                    if d > 0.0 && d < GLOW_R {
                        let f = 1.0 - d / GLOW_R;
                        bd.fill_rect(x, y, DOT, DOT, with_alpha(hot, GLOW_A * f * f * if moon { 0.6 } else { 1.0 }));
                    }
                }
            }
        }

        // ---- the horizon: a dotted row, molten where the sun sits on it ----
        let cap_c = Rgba::from_hex(&t.lit, CAP_ALPHA);
        for col in 0..g.cols {
            let near = (col as f32 - sun.0).abs();
            let (x, y) = (g.ox + col * PITCH, g.oy + g.water_row * PITCH);
            if !moon && near <= sun.2 + 0.5 {
                bd.fill_rect(x, y, DOT, DOT, Rgba::lerp_linear(lit, hot, 0.5));
            } else if col % 2 == 0 {
                bd.fill_rect(x, y, DOT, DOT, cap_c);
            }
        }

        // ---- the sun (or moon) ----
        for row in 0..g.water_row {
            for col in 0..g.cols {
                if !in_disc(col, row, sun, moon) {
                    continue;
                }
                let (dx, dy) = (col as f32 - sun.0, row as f32 - sun.1);
                let rim = (dx * dx + dy * dy).sqrt() > sun.2 - 0.9;
                let c = if rim { Rgba::lerp_linear(hot, lit, 0.65) } else { hot };
                bd.fill_rect(g.ox + col * PITCH, g.oy + row * PITCH, DOT, DOT, c);
            }
        }
        self.backdrop = Some(bd);

        // ---- the stars: hashed per size, in the upper sky, clear of the sun ----
        let top_rows = (g.water_row - 2).max(1);
        let want = (g.cols / 12).clamp(3, MAX_STARS as i32) as usize;
        let seed = (w as u32).wrapping_mul(2_654_435_761) ^ (h as u32).wrapping_mul(40_503);
        self.n_stars = 0;
        let mut k = 0u32;
        while self.n_stars < want && k < 64 {
            let col = (hash01(seed ^ k.wrapping_mul(7919)) * g.cols as f32) as i32;
            let row = (hash01(seed ^ k.wrapping_mul(104_729) ^ 0x5bd1) * top_rows as f32) as i32;
            k += 1;
            if (col as f32 - sun.0).abs() < sun.2 + 2.5 || col < 1 || col >= g.cols - 1 {
                continue;
            }
            // Not shoulder to shoulder: a pair of adjacent stars reads as one smudge.
            if self.stars[..self.n_stars].iter().any(|s| (s.col - col).abs() < 3 && s.row == row) {
                continue;
            }
            self.stars[self.n_stars] = Star {
                col,
                row,
                phase: hash01(seed ^ k ^ 0x9e37) * std::f32::consts::TAU,
                rate: 1.3 + 2.2 * hash01(seed ^ k ^ 0x7f4a),
            };
            self.n_stars += 1;
        }
    }

    /// Throws a splash at grid column `x`, at the surface row `surface`.
    fn splash(&mut self, x: f32, surface: i32, big: bool, rows: i32) {
        let n = if big { DROPS } else { DROPS_NORMAL };
        let vs = rows as f32 / 14.0;
        let seed = self.entries.wrapping_mul(0x9e37_79b9);
        for (i, drop) in self.drops.iter_mut().enumerate().take(n) {
            let s = seed ^ (i as u32).wrapping_mul(0x85eb_ca6b);
            *drop = Drop {
                x: x + (hash01(s ^ 1) - 0.5) * 4.0,
                y: surface as f32 - 1.0,
                // Mostly backward and up, the way water thrown off a diving body goes; a few forward.
                vx: (-10.0 + 15.0 * hash01(s ^ 2)) * vs,
                vy: -(DROP_VY.0 + (DROP_VY.1 - DROP_VY.0) * hash01(s ^ 3)) * vs,
                live: true,
            };
        }
        let slot = (self.entries as usize) % RINGS;
        self.rings[slot] = Ring { x, r: 0.0, life: 1.0 };
        if big {
            self.rings[(slot + 1) % RINGS] = Ring { x, r: -2.5, life: 1.0 };
        }
        self.entries = self.entries.wrapping_add(1);
    }
}

impl Family for Dolphin {
    fn id(&self) -> &'static str {
        "dolphin"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        let secs = dt / 1000.0;
        let time_s = if d.time_s.is_finite() { d.time_s } else { 0.0 };
        // Armed before the size guard, so a panel too small to draw still keeps the trigger's history
        // current - the same order `waterfall` uses, for the same reason.
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let leap = self.leap.update(fired, dt, LEAP_MS);

        let g = grid(w, h);
        if g.rows < MIN_ROWS || g.cols < MIN_COLS {
            c.clear();
            // `rounded_rect` guards non-positive dimensions itself, so the degenerate sizes draw nothing.
            c.rounded_rect(1, 2, w - 2, h - 4, 3, Rgba::from_hex(&t.panel, t.panel_alpha));
            return; // shed rather than smudge
        }
        self.fit(g.cols as usize);
        self.bake(t, w, h, g);
        // The opaque panel first: the backdrop is panel + lattice + horizon + sun, baked, copied whole.
        if let Some(bd) = self.backdrop.as_ref() {
            c.copy_region(bd, (0, 0), (0, 0), w, h);
        }

        let Grid { rows, cols, ox, oy, spec_rows, water_row } = g;
        let cell = |c: &mut Canvas, col: i32, row: i32, colour: Rgba| {
            if (0..cols).contains(&col) && (0..rows).contains(&row) {
                c.fill_rect(ox + col * PITCH, oy + row * PITCH, DOT, DOT, colour);
            }
        };
        let lit = Rgba::from_hex(&t.lit, 1.0);
        let cap_c = Rgba::from_hex(&t.lit, CAP_ALPHA);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let moon = is_moon(t);
        let sun = sun_at(g, moon);

        // ---- the stars, twinkling ----
        for s in &self.stars[..self.n_stars] {
            let tw = 0.5 + 0.5 * (time_s * s.rate + s.phase).sin();
            cell(c, s.col, s.row, with_alpha(hot, STAR_FLOOR + STAR_SWING * tw * tw * tw));
        }

        // ---- the sea: the spectrum, height in dots, bottom up ----
        let b = &t.ballistics;
        for col in 0..cols {
            // Group bands onto columns rather than dropping any: a column covers a contiguous span and
            // takes its MAX, so a single sharp band cannot be averaged into invisibility.
            let lo = (col as usize * NUM_BANDS) / cols as usize;
            let hi = (((col as usize + 1) * NUM_BANDS) / cols as usize).clamp(lo + 1, NUM_BANDS);
            let band = d.levels[lo..hi].iter().copied().fold(0.0f32, f32::max);
            let i = col as usize;
            self.levels[i] = resp(band, t.sensitivity);

            let held = (self.caps[i] - b.peak_fall.max(0.0)).max(self.levels[i]);
            self.caps[i] = if held.is_finite() { held.clamp(0.0, 1.0) } else { 0.0 };

            let dots = (self.levels[i] * spec_rows as f32).round() as i32;
            self.surf[i] = rows - dots;
            for k in 0..dots {
                cell(c, col, rows - 1 - k, lit);
            }
            // The cap only draws where it is genuinely ABOVE the lit column - otherwise it would
            // re-light the top lit dot at a lower alpha and read as nothing at all.
            let cap_dots = (self.caps[i] * spec_rows as f32).round() as i32;
            if cap_dots > dots {
                let row = rows - 1 - cap_dots;
                if row > water_row {
                    cell(c, col, row, cap_c);
                }
            }
        }

        // ---- the sun's reflection: a glittering column down the sea, over the waves too ----
        let tick = (time_s * GLITTER_HZ).floor() as i32 as u32;
        let peak = GLITTER_A * if moon { MOON_K } else { 1.0 };
        for row in (water_row + 1)..rows {
            let depth = (row - water_row) as f32;
            let half = sun.2 * 0.6 + depth * 0.45;
            let c0 = (sun.0 - half).floor() as i32;
            let c1 = (sun.0 + half).ceil() as i32;
            for col in c0..=c1 {
                let off = (col as f32 - sun.0).abs() / half.max(0.5);
                if off > 1.0 {
                    continue;
                }
                let s = hash01((col as u32).wrapping_mul(73_856_093) ^ (row as u32).wrapping_mul(19_349_663) ^ tick.wrapping_mul(83_492_791));
                // Glitter, not a pillar: sparse, and denser on alternate rows so it reads as ripples.
                let dense = if (row - water_row) % 2 == 1 { 0.62 } else { 0.3 };
                if s > dense - 0.3 * off {
                    continue;
                }
                cell(c, col, row, with_alpha(hot, peak * (1.0 - 0.6 * off) * (1.0 - 0.25 * depth / spec_rows as f32)));
            }
        }

        // ---- wave crests: a travelling dot one row above the level line, with a dim neighbour ring ----
        let drive = Self::drive(d, t.sensitivity);
        self.crest_off += secs * (CREST_SPEED + CREST_SPEED_DRIVE * drive);
        if !self.crest_off.is_finite() {
            self.crest_off = 0.0;
        }
        self.crest_off = self.crest_off.rem_euclid(CREST_GAP as f32);
        let shift = self.crest_off.floor() as i32;
        let glow = with_alpha(lit, CREST_GLOW);
        for col in 0..cols {
            if (col - shift).rem_euclid(CREST_GAP) != 0 {
                continue;
            }
            let row = self.surf[col as usize] - 1;
            if row < water_row || self.surf[col as usize] >= rows {
                continue; // a crest needs a wave under it, and stays below the horizon
            }
            for (dc, dr) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (1, 1)] {
                let (nc, nr) = (col + dc, row + dr);
                if (0..cols).contains(&nc) && nr < self.surf[nc as usize] {
                    cell(c, nc, nr, glow);
                }
            }
            cell(c, col, row, hot);
        }

        // ---- the dolphin's flight ----
        let hero = rows >= HERO_MIN_ROWS;
        let (sw, sh) = if hero { (HERO_W, HERO_H) } else { (SPRITE_W, SPRITE_H) };
        let loop_s = LOOP_SLOW_S + (LOOP_FAST_S - LOOP_SLOW_S) * drive;
        self.phase += secs / loop_s.max(0.2);
        if !self.phase.is_finite() {
            self.phase = 0.0;
        }
        while self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        // Enters left of the panel and leaves to the right, so it is never clipped mid-body at an edge.
        let travel = (cols + sw * 2) as f32;
        let leaps = ((travel / LEAP_SPAN).round() as i32).max(1);
        let q = (self.phase * leaps as f32).fract();
        let xc = self.phase * travel - sw as f32 * 0.5;
        let half = (sh - 1) as f32 * 0.5;
        // Fully under the panel's bottom row at take-off and landing; top at row 0 for the loudest leap.
        let base = rows as f32 + half + 0.5;
        let h_max = base - half;
        let target = if leap > 0.01 {
            h_max
        } else {
            h_max * (LEAP_MIN_K + (1.0 - LEAP_MIN_K) * Self::bass(d, t.sensitivity))
        };
        if q < self.prev_q {
            // A new leap: its height is the bass at take-off ...
            self.leap_h = target;
            self.prev_above = false;
        } else if q < 0.5 {
            // ... raised by any kick during the climb, then ballistic on the way down.
            self.leap_h = self.leap_h.max(target);
        }
        if !self.leap_h.is_finite() {
            self.leap_h = 0.0;
        }
        let yc = base - self.leap_h * 4.0 * q * (1.0 - q);
        let span = travel / leaps as f32;
        // Pitch follows the arc's own slope (rows per column): nose up on the climb, level at the apex.
        let slope = if hero { (self.leap_h * 4.0 * (1.0 - 2.0 * q) / span.max(1.0)).clamp(-MAX_SLOPE, MAX_SLOPE) } else { 0.0 };
        if self.prev_q < 0.5 && q >= 0.5 {
            self.apex_ms = APEX_MS;
        } else {
            self.apex_ms = (self.apex_ms - dt).max(0.0);
        }
        self.prev_q = q;

        // Re-entry: the NOSE going back below the surface in front of it, on the way down. The nose
        // rather than the centre, because by the time the centre is under, most of the dolphin is too
        // and the splash would land on an empty sea.
        let nose_x = xc + sw as f32 * 0.5 - 1.0;
        let nose_y = yc + if hero { 5.0 - half } else { 0.0 } - (sw as f32 * 0.5) * slope;
        let nc = nose_x.round() as i32;
        let surface = if (0..cols).contains(&nc) { self.surf[nc as usize] } else { rows };
        let above = nose_y < surface as f32;
        if q > 0.5 && self.prev_above && !above && (0..cols).contains(&nc) {
            self.splash(nose_x, surface, leap > 0.01, rows);
        }
        self.prev_above = above;

        // The tail flicks on its own clock, independent of how fast the dolphin is crossing - a real
        // tail beat does not slow down just because the animal is moving gently.
        self.tail_due += dt;
        if !self.tail_due.is_finite() {
            self.tail_due = 0.0;
        }
        self.tail_due = self.tail_due.min(PHASE_MS * PHASES as f32);
        while self.tail_due >= PHASE_MS {
            self.tail_due -= PHASE_MS;
            self.tail = (self.tail + 1) % PHASES;
        }
        let tail = self.tail.min(PHASES - 1);
        let flick = [0i32, -1, 1][tail];

        // ---- rasterise the (pitched) dolphin into a small cell buffer ----
        //
        // 0 = nothing, 1 = back, 2 = belly, 3 = eye. Index (i, j) is grid cell (x0 + i, y0 + j), with a
        // one-cell border for the keyline and `PITCH_PAD` rows above and below for the pitch.
        //
        // Pitch is a column SHEAR, not a rotation: each sprite column keeps its exact cells and only
        // moves up or down. A rotation resampled at this resolution tore the one-cell fin and beak into
        // loose dots; the shear keeps every feature and still reads as nose-up / nose-down.
        let mut buf = [[0u8; BUF]; BUF];
        let x0 = (xc - sw as f32 * 0.5).round() as i32 - 1;
        let y0 = (yc - half).round() as i32 - 1 - PITCH_PAD;
        let mcx = (sw - 1) as f32 * 0.5;
        for mx in 0..sw {
            let mut off = (-(mx as f32 - mcx) * slope).round() as i32;
            if hero && mx < FLUKE_COLS {
                off += flick;
            }
            for my in 0..sh {
                let ch = if hero { HERO[my as usize].as_bytes()[mx as usize] } else { BODY[tail][my as usize].as_bytes()[mx as usize] };
                let kind = match ch {
                    b'#' => 1,
                    b'+' => 2,
                    b'o' => 3,
                    _ => continue,
                };
                let j = my + off + 1 + PITCH_PAD;
                let i = mx + 1;
                if (0..BUF as i32).contains(&j) && (0..BUF as i32).contains(&i) {
                    buf[j as usize][i as usize] = kind;
                }
            }
        }

        // The water hides what is under it: a dolphin cell shows only ABOVE the lit column in front.
        let surf = &self.surf;
        let dry = |col: i32, row: i32| -> bool {
            (0..cols).contains(&col) && (0..rows).contains(&row) && row < surf[col as usize]
        };
        // KEYLINE FIRST. Without it the dolphin is lit dots on a lit lattice with nothing between
        // them, and it reads as an amorphous cluster - which is exactly how the first render came out.
        // `chroma.rs:19-25` records the fix: a hard dark outline makes a shape legible independently of
        // its own colour. Drawn as opaque panel over every cell ADJACENT to the body, so it erases the
        // lattice, a star or the sun behind it - but never a lit sea dot: the meter wins.
        let key = Rgba::from_hex(&t.panel, 1.0);
        for (j, line) in buf.iter_mut().enumerate() {
            for (i, v) in line.iter_mut().enumerate() {
                if !dry(x0 + i as i32, y0 + j as i32) {
                    *v = 0; // under the water: neither drawn nor outlined
                }
            }
        }
        for j in 1..BUF - 1 {
            for i in 1..BUF - 1 {
                if buf[j][i] != 0 {
                    continue;
                }
                let touches = buf[j - 1][i - 1..=i + 1].iter().any(|v| *v != 0)
                    || buf[j][i - 1] != 0
                    || buf[j][i + 1] != 0
                    || buf[j + 1][i - 1..=i + 1].iter().any(|v| *v != 0);
                let (col, row) = (x0 + i as i32, y0 + j as i32);
                if touches && dry(col, row) {
                    cell(c, col, row, key);
                }
            }
        }
        let apex = self.apex_ms > 0.0;
        // Paler than the sea, so a dolphin rising out of the lit columns is never the same colour as
        // the water it leaves; the belly paler still. The apex and the flourish lift both toward white.
        let white = Rgba::new(255, 255, 255, 255);
        let (back, belly) = if apex {
            (Rgba::lerp_linear(hot, white, 0.75), white)
        } else if leap > 0.01 {
            (Rgba::lerp_linear(hot, white, 0.25), Rgba::lerp_linear(hot, white, 0.6))
        } else {
            (body_colour(t), Rgba::lerp_linear(hot, white, 0.3))
        };
        for (j, line) in buf.iter().enumerate() {
            for (i, v) in line.iter().enumerate() {
                let (col, row) = (x0 + i as i32, y0 + j as i32);
                if *v == 0 {
                    continue;
                }
                let colour = match *v {
                    1 => back,
                    2 => belly,
                    _ => key,
                };
                cell(c, col, row, colour);
            }
        }

        // ---- the splash: droplets under gravity, rings spreading on the surface ----
        let gravity = GRAVITY * rows as f32 / 14.0;
        let spray = spray_colour(t);
        for drop in self.drops.iter_mut().filter(|p| p.live) {
            drop.vy += gravity * secs;
            drop.x += drop.vx * secs;
            drop.y += drop.vy * secs;
            let (col, row) = (drop.x.round() as i32, drop.y.round() as i32);
            let landed = !(0..cols).contains(&col) || row >= rows || (drop.vy > 0.0 && row >= self.surf[col as usize]);
            if landed || !(drop.x.is_finite() && drop.y.is_finite()) {
                drop.live = false;
                continue;
            }
            if row >= 0 {
                cell(c, col, row, spray);
            }
        }
        for ring in self.rings.iter_mut().filter(|r| r.life > 0.0) {
            ring.r += RING_SPEED * secs;
            ring.life -= dt / RING_MS;
            if ring.r < 0.0 || ring.life <= 0.0 {
                continue;
            }
            // A side view of a ring is a crown: the leading edge on the surface, a dimmer trailing
            // edge one row up, both sides spreading out from the entry point.
            for (side, lag, lift, k) in [(-1.0f32, 0.0f32, 1, 1.0f32), (1.0, 0.0, 1, 1.0), (-1.0, 1.5, 2, 0.55), (1.0, 1.5, 2, 0.55)] {
                let r = ring.r - lag;
                if r < 0.0 {
                    continue;
                }
                let col = (ring.x + side * r).round() as i32;
                if (0..cols).contains(&col) {
                    let row = self.surf[col as usize] - lift;
                    if row >= 0 && row < self.surf[col as usize] {
                        cell(c, col, row, with_alpha(spray, ring.life * k));
                    }
                }
            }
        }

        // Clip last: nothing this family draws may land outside the panel's rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    /// A plausible spectrum: bass-heavy and falling, which is what music looks like here.
    fn frame(gain: f32, t_s: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.6) * 0.75 + 0.12;
            let wobble = 1.0 + 0.30 * ((t_s * 3.1 + f * 9.0).sin());
            *v = (shape * wobble * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = 0.30 * gain;
        d.rms_r = 0.27 * gain;
        d
    }

    /// Brightness of a pixel, for probes that ask whether a cell is lit.
    fn lum(px: Rgba) -> f32 {
        let a = px.a as f32 / 255.0;
        (0.2126 * px.r as f32 + 0.7152 * px.g as f32 + 0.0722 * px.b as f32) * a
    }

    /// The geometry the renderer derives, recomputed once for every probe.
    ///
    /// In ONE place deliberately. The reel family had a probe that hard-coded a pixel window, and when
    /// the strip height changed the probe silently sampled the wrong rows and failed a test about a lamp
    /// that was working perfectly.
    fn geom(w: i32, h: i32) -> (i32, i32, i32, i32, i32, i32) {
        let g = grid(w, h);
        (g.rows, g.cols, g.ox, g.oy, g.spec_rows, g.water_row)
    }

    /// Whether a pixel is the dolphin's BACK - `body_colour`, which nothing else on the panel uses: the
    /// sun, the stars and the droplets are `hot` or partial, so this is the probe the sky can no longer
    /// fool the way a brightness threshold now would.
    fn is_body(px: Rgba, t: &Theme) -> bool {
        let l = body_colour(t);
        (px.r as i32 - l.r as i32).abs() + (px.g as i32 - l.g as i32).abs() + (px.b as i32 - l.b as i32).abs() < 30
    }

    /// Whether a pixel is (within `tol`, summed over channels) exactly this colour.
    fn close(px: Rgba, c: Rgba, tol: i32) -> bool {
        (px.r as i32 - c.r as i32).abs() + (px.g as i32 - c.g as i32).abs() + (px.b as i32 - c.b as i32).abs() <= tol
    }

    /// Every band at `level`, the bottom eighth at `bass`: no wobble, so nothing moves but the family.
    fn flat(level: f32, bass: f32, t_s: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = if i < NUM_BANDS / 8 { bass } else { level };
        }
        d.peaks = d.levels;
        d
    }

    fn settled(t: &Theme, gain: f32, frames: usize, w: i32, h: i32) -> (Dolphin, Canvas) {
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(w, h);
        for k in 0..frames {
            fam.draw(&mut c, t, &frame(gain, k as f32 * 0.0167));
        }
        (fam, c)
    }

    #[test]
    fn the_unlit_lattice_is_drawn_and_reads_as_off() {
        // The lattice is the whole illusion, and its requirement is two-sided, so it needs two
        // assertions. A well must be brighter than the panel or there is no lattice at all; and much
        // dimmer than a lit dot or it reads as lit. Deleting the lattice loop fails the first.
        // Drawing the wells at full alpha fails the second.
        let t = builtin::dolphin_sony_amber();
        let (_, _, ox, oy, _, water_row) = geom(190, 60);
        let (_, c) = settled(&t, 0.0, 30, 190, 60);
        let well = lum(c.get(ox + 4 * PITCH, oy + PITCH));
        let panel = lum(c.get(ox + 4 * PITCH + DOT, oy + PITCH + DOT));
        assert!(
            well > panel + 3.0,
            "the unlit well ({well:.1}) must be visibly brighter than the panel ({panel:.1}), or the \
             display reads as floating squares rather than as a dot matrix"
        );
        let (_, hot_c) = settled(&t, 0.9, 30, 190, 60);
        let lit = lum(hot_c.get(ox + 4 * PITCH, oy + (water_row + 2) * PITCH));
        // 2x: a well measures ~65 and a lit dot ~170 (2.6x). Re-based for linear-light blending (task 7): faint, low-alpha marks now carry the light their alpha says, so the "off" state is brighter than it was in gamma space; it was 3x.
        assert!(
            lit > well * 2.0,
            "a lit dot ({lit:.1}) must clearly outrank a well ({well:.1}); at {:.1}x they read as the \
             same state",
            lit / well.max(0.01)
        );
    }

    #[test]
    fn column_height_tracks_level_because_level_is_position() {
        // The house rule this family is built under: magnitude is POSITION, never brightness. Louder
        // music must light MORE ROWS. A constant height fails this, and so does the tempting mistake of
        // encoding level as alpha - which looks plausible on screen and is invisible at this size.
        let t = builtin::dolphin_sony_amber();
        let (_, _, ox, oy, spec_rows, water_row) = geom(190, 60);
        let lit_rows = |gain: f32| -> i32 {
            let (_, c) = settled(&t, gain, 60, 190, 60);
            (0..spec_rows)
                // 110 sits between an unlit well (~65) and a lit dot (~170). Re-based for linear-light blending (task 7): faint, low-alpha marks now carry the light their alpha says, so the "off" state is brighter than it was in gamma space; it was 60.
                .filter(|k| lum(c.get(ox + PITCH, oy + (water_row + 1 + k) * PITCH)) > 110.0)
                .count() as i32
        };
        let quiet = lit_rows(0.25);
        let loud = lit_rows(0.95);
        assert!(
            loud > quiet + 1,
            "louder music must light more rows: {quiet} at 0.25 gain against {loud} at 0.95"
        );
        assert!(loud <= spec_rows, "the column overflowed its band: {loud} rows of {spec_rows}");
    }

    /// Slow (~7.3s in debug); gated out of the default suite. The dolphin arcs and dips through the waterline. Run: `cargo test --release slow_ -- --ignored`.
    #[test]
    #[ignore]
    fn slow_the_dolphin_arcs_and_dips_through_the_waterline() {
        // Above the waterline the dolphin is the ONLY thing drawn, so the topmost lit row up there is
        // its altitude. Two properties: the altitude must vary a lot over a loop (a frozen phase or a
        // flat arc fails), and it must come back down to the water (an arc that never lands fails).
        let t = builtin::dolphin_sony_amber();
        let (_, cols, ox, oy, _, water_row) = geom(380, 60);
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(380, 60);
        let mut tops: Vec<i32> = Vec::new();
        let mut ever_at_water = false;
        for k in 0..400 {
            fam.draw(&mut c, &t, &frame(0.55, k as f32 * 0.0167));
            let mut top = None;
            for row in 0..water_row {
                for col in 0..cols {
                    if is_body(c.get(ox + col * PITCH, oy + row * PITCH), &t) {
                        top = Some(row);
                        break;
                    }
                }
                if top.is_some() {
                    break;
                }
            }
            if let Some(r) = top {
                tops.push(r);
                if r >= water_row - 1 {
                    ever_at_water = true;
                }
            }
        }
        assert!(!tops.is_empty(), "the dolphin never appeared above the waterline at all");
        let hi = *tops.iter().min().unwrap();
        let lo = *tops.iter().max().unwrap();
        assert!(
            lo - hi >= 3,
            "the dolphin barely changed altitude over 400 frames: rows {hi}..{lo}"
        );
        assert!(ever_at_water, "the dolphin never dipped back down to the waterline");
    }

    #[test]
    fn the_dolphin_travels_further_when_the_music_is_louder() {
        // The owner chose speed-tracks-loudness. Ignoring `drive` in the loop duration leaves these two
        // equal, which is the mutation this test exists for.
        let t = builtin::dolphin_sony_amber();
        let (_, cols, ox, oy, _, water_row) = geom(380, 60);
        let reach = |gain: f32| -> i32 {
            let mut fam = Dolphin::default();
            let mut c = Canvas::new(380, 60);
            let mut best = 0;
            for k in 0..120 {
                fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
                for col in 0..cols {
                    for row in 0..water_row {
                        if is_body(c.get(ox + col * PITCH, oy + row * PITCH), &t) {
                            best = best.max(col);
                        }
                    }
                }
            }
            best
        };
        let quiet = reach(0.20);
        let loud = reach(0.95);
        assert!(
            loud > quiet + 4,
            "over the same 120 frames the dolphin reached column {quiet} on quiet music and {loud} on \
             loud - speed is not tracking loudness"
        );
    }

    #[test]
    fn the_keyline_separates_the_sprite_from_the_lattice() {
        // Without the keyline the sprite is lit dots on a lit lattice and reads as an amorphous
        // cluster - which is exactly how the first render came out. The keyline is opaque panel, so a
        // cell adjacent to the body must be DARKER than an ordinary well.
        let t = builtin::dolphin_sony_amber();
        let (_, cols, ox, oy, _, water_row) = geom(380, 60);
        let (_, c) = settled(&t, 0.55, 70, 380, 60);
        // An ordinary sky well: low, at the left, clear of the corner clip, the sun's glow and the stars.
        let well_ref = lum(c.get(ox + 2 * PITCH, oy + (water_row - 1) * PITCH));
        let mut found_body = false;
        let mut found_key = false;
        for row in 1..water_row {
            for col in 1..cols - 1 {
                if is_body(c.get(ox + col * PITCH, oy + row * PITCH), &t) {
                    found_body = true;
                    for (dc, dr) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                        let n = lum(c.get(ox + (col + dc) * PITCH, oy + (row + dr) * PITCH));
                        if n < well_ref * 0.5 {
                            found_key = true;
                        }
                    }
                }
            }
        }
        assert!(found_body, "no sprite was on screen, so the keyline had nothing to be tested against");
        assert!(
            found_key,
            "nothing around the sprite was darker than an ordinary well ({well_ref:.1}), so the \
             keyline is not being drawn and the dolphin has nothing separating it from the lattice"
        );
    }

    /// The topmost row the dolphin's back reaches over `frames` frames, or `rows` if it never shows.
    fn highest_row(t: &Theme, w: i32, h: i32, frames: usize, d: impl Fn(usize) -> FrameData) -> i32 {
        let (rows, cols, ox, oy, _, _) = geom(w, h);
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(w, h);
        let mut best = rows;
        for k in 0..frames {
            fam.draw(&mut c, t, &d(k));
            for row in 0..best {
                if (0..cols).any(|col| is_body(c.get(ox + col * PITCH, oy + row * PITCH), t)) {
                    best = row;
                    break;
                }
            }
        }
        best
    }

    #[test]
    fn the_leap_height_follows_the_bass() {
        // Same mids and treble, different bass: the leap must go visibly higher on the heavy one.
        // Mutation: a constant leap height (drop the `bass` term from `target`) makes these equal.
        let t = builtin::dolphin_sony_amber();
        let low = highest_row(&t, 380, 60, 400, |k| flat(0.3, 0.15, k as f32 * 0.0167));
        let high = highest_row(&t, 380, 60, 400, |k| flat(0.3, 0.85, k as f32 * 0.0167));
        assert!(high < 14, "the dolphin never showed at all on the bass-heavy fixture");
        assert!(
            high + 3 <= low,
            "a heavy bass should throw the dolphin at least 3 rows higher: its back topped out at row \
             {high} with the bass in and row {low} with it out"
        );
    }

    #[test]
    fn re_entry_throws_a_splash() {
        // Within 300 ms of the nose going back in, white water must be in the air: spray-coloured cells
        // at least three rows above the sea surface (above the ring's crown, which stays within two).
        // Mutation: `DROPS_NORMAL = 0` leaves only the ring and fails this.
        let t = builtin::dolphin_sony_amber();
        let (rows, cols, ox, oy, _, _) = geom(380, 60);
        let spray = spray_colour(&t);
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(380, 60);
        let mut k = 0;
        while fam.entries == 0 && k < 600 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            k += 1;
        }
        assert!(fam.entries > 0, "the dolphin never re-entered the water in 600 frames");
        let mut most = 0;
        for j in 0..18 {
            fam.draw(&mut c, &t, &frame(0.6, (k + j) as f32 * 0.0167));
            let mut n = 0;
            for col in 0..cols {
                for row in 0..(fam.surf[col as usize] - 2).min(rows) {
                    if close(c.get(ox + col * PITCH, oy + row * PITCH), spray, 6) {
                        n += 1;
                    }
                }
            }
            most = most.max(n);
        }
        assert!(
            most >= 4,
            "within 300 ms of re-entry at most {most} droplet cells were in the air; the splash is missing"
        );
    }

    #[test]
    fn the_sky_has_a_sun_and_its_reflection() {
        // Two things, both needed: a disc of `hot` above the horizon, and below it on a SILENT sea (so
        // no column lights anything) the wells under the disc brighter than the wells far from it.
        // Mutations: skipping the disc in `bake` fails the first; skipping the reflection loop fails
        // the second.
        let t = builtin::dolphin_sony_amber();
        let (rows, cols, ox, oy, _, water_row) = geom(380, 60);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(380, 60);
        fam.draw(&mut c, &t, &flat(0.0, 0.0, 0.0));
        let mut disc: Vec<i32> = Vec::new();
        for row in 0..water_row {
            for col in 0..cols {
                if close(c.get(ox + col * PITCH, oy + row * PITCH), hot, 6) {
                    disc.push(col);
                }
            }
        }
        assert!(disc.len() >= 6, "only {} cells of `hot` in the sky: there is no sun", disc.len());
        let lo = *disc.iter().min().unwrap();
        let hi = *disc.iter().max().unwrap();
        assert!(hi - lo <= 6, "the hot cells span columns {lo}..{hi}: that is not one disc");
        let sun_col = (lo + hi) / 2;
        // Averaged over a second of glitter, so the test is about the column rather than one frame. The
        // far reference is the right edge: the dolphin starts its crossing at the left.
        let (mut near, mut far) = (0.0f32, 0.0f32);
        for k in 1..60 {
            fam.draw(&mut c, &t, &flat(0.0, 0.0, k as f32 * 0.0167));
            for row in (water_row + 1)..rows {
                for dc in -1..=1 {
                    near += lum(c.get(ox + (sun_col + dc) * PITCH, oy + row * PITCH));
                }
                for col in (cols - 5)..(cols - 2) {
                    far += lum(c.get(ox + col * PITCH, oy + row * PITCH));
                }
            }
        }
        assert!(
            near > far * 1.3,
            "the sea under the sun ({near:.0}) is no brighter than the sea far from it ({far:.0}): \
             there is no reflection"
        );
    }

    #[test]
    fn crests_travel() {
        // On a steady spectrum the crests must still MOVE: the set of columns carrying a crest dot
        // (`hot`, one row above the level line) must shift between frames 200 ms apart. Mutation:
        // `CREST_SPEED = 0` and `CREST_SPEED_DRIVE = 0` freeze them, and only the cells the passing
        // dolphin covers change.
        let t = builtin::dolphin_sony_amber();
        let (_, cols, ox, oy, _, _) = geom(380, 60);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let crests = |fam: &Dolphin, c: &Canvas| -> Vec<i32> {
            (0..cols)
                .filter(|&col| {
                    let row = fam.surf[col as usize] - 1;
                    row >= 0 && close(c.get(ox + col * PITCH, oy + row * PITCH), hot, 6)
                })
                .collect()
        };
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..61 {
            fam.draw(&mut c, &t, &flat(0.55, 0.55, k as f32 * 0.0167));
        }
        let a = crests(&fam, &c);
        for k in 61..73 {
            fam.draw(&mut c, &t, &flat(0.55, 0.55, k as f32 * 0.0167));
        }
        let b = crests(&fam, &c);
        assert!(a.len() >= 6, "only {} crests on the sea: {a:?}", a.len());
        let moved = a.iter().filter(|col| !b.contains(col)).count();
        assert!(
            moved * 10 >= a.len() * 6,
            "only {moved} of {} crests left their column in 200 ms; the crests are not travelling \
             ({a:?} -> {b:?})",
            a.len()
        );
    }

    #[test]
    fn the_hero_flies_at_2x_on_the_smallest_panel() {
        // 128x44 is the smallest shipped panel, and it must still get the 2x hero: at some point in a
        // loop the dolphin's visible cells span more rows than the whole 1x sprite has. A calm sea and
        // a heavy bass, so the leap clears the water. Mutation: `HERO_MIN_ROWS = 11` flies the 1x
        // sprite here and fails.
        let t = builtin::dolphin_sony_amber();
        let (rows, cols, ox, oy, _, _) = geom(128, 44);
        let belly = Rgba::lerp_linear(Rgba::from_hex(&t.hot, 1.0), Rgba::new(255, 255, 255, 255), 0.3);
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(128, 44);
        let mut tallest = 0;
        for k in 0..400 {
            fam.draw(&mut c, &t, &flat(0.1, 0.9, k as f32 * 0.0167));
            let shown: Vec<i32> = (0..rows)
                .filter(|&row| {
                    (0..cols).any(|col| {
                        let px = c.get(ox + col * PITCH, oy + row * PITCH);
                        is_body(px, &t) || close(px, belly, 6)
                    })
                })
                .collect();
            if let (Some(a), Some(b)) = (shown.first(), shown.last()) {
                tallest = tallest.max(b - a + 1);
            }
        }
        assert!(
            tallest > SPRITE_H,
            "at 128x44 the dolphin never stood more than {tallest} rows tall - the 1x sprite, not the hero"
        );
    }

    /// Per-frame cost at 380x60, steady and across a forced flourish's envelope.
    /// Run: cargo test --release probe_dolphin_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_dolphin_cost() {
        let t = builtin::dolphin_sony_amber();
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..60 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            fam.draw(&mut c, &t, &frame(0.6, (60 + k) as f32 * 0.0167));
        }
        let steady = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("dolphin steady: {steady:.3} ms/frame at 380x60");

        fam.flourish.force_next();
        let m = 108;
        let t1 = std::time::Instant::now();
        for k in 0..m {
            fam.draw(&mut c, &t, &frame(0.6, (360 + k) as f32 * 0.0167));
        }
        let flourish = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
        println!("dolphin flourish: {flourish:.3} ms/frame at 380x60");
    }

    /// The frames the fidelity dump lands on (picked from the filmstrip to show the leap, not a dive).
    const FID_CALM: usize = 100;
    const FID_LOUD: usize = 60;
    const FID_FLOURISH: (usize, usize) = (40, 90);

    /// Run: cargo test --release dump_dolphin -- --ignored --nocapture
    ///
    /// Written before any assertion, deliberately: the sprite either reads as a dolphin at nine dots
    /// wide or it does not, and no pixel count can tell me which.
    #[test]
    #[ignore]
    fn dump_dolphin() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: String, c: &Canvas| {
            let mut out = Vec::new();
            for y in 0..c.height() {
                for x in 0..c.width() {
                    let px = c.get(x, y);
                    out.extend_from_slice(&[px.r, px.g, px.b, px.a]);
                }
            }
            std::fs::write(dir.join(format!("{name}.rgba")), &out).unwrap();
        };

        // One settled frame per colourway, at both panel widths.
        for t in builtin::all().into_iter().filter(|t| t.family == "dolphin") {
            for (w, h, tag) in [(190, 60, "190"), (380, 60, "380")] {
                let mut fam = Dolphin::default();
                let mut c = Canvas::new(w, h);
                for k in 0..90 {
                    fam.draw(&mut c, &t, &frame(0.55, k as f32 * 0.0167));
                }
                write(format!("dolphin-{}-{tag}", t.id), &c);
            }
        }

        // The arc, as a filmstrip: the sprite at eight points around one loop.
        let t = builtin::dolphin_sony_amber();
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(380, 60);
        let mut shot = 0;
        for k in 0..260 {
            fam.draw(&mut c, &t, &frame(0.55, k as f32 * 0.0167));
            if k >= 40 && (k - 40) % 24 == 0 && shot < 8 {
                write(format!("dolphin-arc-{shot}"), &c);
                shot += 1;
            }
        }

        // A filmstrip for reading the motion: every 6th frame of one loud loop.
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..240 {
            fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
            if k % 6 == 0 || (96..150).contains(&k) {
                write(format!("dolphin-film-{:03}", k), &c);
            }
        }

        // The fidelity set: calm, loud and the flourish on two colourways, and 128x44.
        for (id, w, h) in [("dolphin-sony-amber", 380, 60), ("dolphin-jvc-ice", 380, 60), ("dolphin-sony-amber", 128, 44)] {
            let t = builtin::all().into_iter().find(|t| t.id == id).unwrap();
            let tag = if w == 380 { id.to_string() } else { format!("{id}-{w}x{h}") };
            for (gain, state, n) in [(0.25f32, "calm", FID_CALM), (0.9, "loud", FID_LOUD)] {
                let mut fam = Dolphin::default();
                let mut c = Canvas::new(w, h);
                for k in 0..n {
                    fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
                }
                write(format!("fid-{tag}-{state}"), &c);
            }
            let mut fam = Dolphin::default();
            let mut c = Canvas::new(w, h);
            for k in 0..FID_FLOURISH.0 {
                fam.draw(&mut c, &t, &frame(0.8, k as f32 * 0.0167));
            }
            fam.flourish.force_next();
            for k in FID_FLOURISH.0..FID_FLOURISH.1 {
                fam.draw(&mut c, &t, &frame(0.8, k as f32 * 0.0167));
            }
            write(format!("fid-{tag}-flourish"), &c);
        }
        // Below `HERO_MIN_ROWS` the 1x sprite flies: 190x40 is nine dot rows.
        let mut fam = Dolphin::default();
        let mut c = Canvas::new(190, 40);
        for k in 0..FID_CALM {
            fam.draw(&mut c, &t, &frame(0.5, k as f32 * 0.0167));
        }
        write("fid-dolphin-1x-190x40".into(), &c);
        println!("wrote dolphin dumps to {}", dir.display());
    }
}
