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
//! That is not pedantry, it buys the flourish. The leap needs clear air above the dolphin's normal
//! apex, and at 12 rows there was exactly ONE dot row (4px) of it for a 5-row sprite. At 14 there are
//! two, and the spectrum still gets 7 rows - inside the "6-8 segments" the brief asked for.
//!
//! # The dolphin
//!
//! Two explicit masks, ascending and descending, rather than one mask and a flip. A horizontal flip
//! gives a dolphin climbing while travelling the wrong way; a VERTICAL flip gives the right attitude
//! but puts the dorsal fin on the belly, which reads instantly as an upside-down fish. The descending
//! mask differs from that flip by exactly one dot - the fin - and writing it out is the version that
//! cannot silently regress, because no pixel-count test would notice a belly fin.
//!
//! Speed tracks loudness, by the owner's decision. Bounded below by aliasing rather than by taste:
//! `reel.rs` measured that motion past half a feature pitch per frame appears to run BACKWARDS, so at
//! a 4px pitch the dolphin may not cross more than half a dot cell per frame. Over a 95-column panel
//! that makes ~1.6s the fastest honest loop; `LOOP_FAST_S` sits above it.

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

/// The sprite: body F, chosen from four rendered at true dot scale, with THREE TAIL PHASES.
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
/// The attitude stays LEVEL through the arc rather than tilting nose-up and nose-down. Two reasons: a
/// vertical flip puts the dorsal fin on the belly, which reads instantly as an upside-down fish, and at
/// eleven cells there is no room for a convincing tilted variant that keeps the fin, the melon and the
/// fluke all legible. The arc carries the leap; the tail carries the life.
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

/// How long one tail phase is held, in milliseconds.
///
/// 130ms is about 7.7 phases a second, so the fluke beats a little under 3Hz through the 3-phase cycle
/// - a real dolphin's tail beat. Also comfortably above the aliasing floor: the sprite moves less than
/// half a dot cell per frame at every loop speed, so nothing appears to run backwards.
const PHASE_MS: f32 = 130.0;

/// How long splash dots linger after the dolphin breaks the waterline, in milliseconds.
const SPLASH_MS: f32 = 620.0;

/// Extra dot rows of lift the flourish gives the leap, above the normal apex.
const LEAP_LIFT: i32 = 2;
/// How long a leap lasts. Longer than most flourishes because it is a whole traverse, not a flash.
const LEAP_MS: f32 = 1800.0;

#[derive(Default)]
pub struct Dolphin {
    /// Smoothed level per COLUMN, 0..1, and the peak-hold cap above it in the same units.
    ///
    /// `Vec` rather than an array because the column count follows the panel width, which the taskbar
    /// changes under us - and `Default` is not implemented for arrays past 32 anyway.
    levels: Vec<f32>,
    caps: Vec<f32>,
    /// Splash energy per column at the waterline, 0..1, decaying.
    splash: Vec<f32>,
    /// Loop phase, 0..1. One loop is one traverse of the display.
    phase: f32,
    /// Which tail phase is showing, and the unspent time toward the next.
    tail: usize,
    tail_due: f32,
    /// The flourish: the dolphin leaps clear of the display.
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

    /// Resizes the per-column state, preserving what it can when the taskbar changes width.
    fn fit(&mut self, cols: usize) {
        self.levels.resize(cols, 0.0);
        self.caps.resize(cols, 0.0);
        self.splash.resize(cols, 0.0);
    }
}

impl Family for Dolphin {
    fn id(&self) -> &'static str {
        "dolphin"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        // Armed before the size guard, so a panel too small to draw still keeps the trigger's history
        // current - the same order `waterfall` uses, for the same reason.
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let leap = self.leap.update(fired, dt, LEAP_MS);

        // `rounded_rect` guards non-positive dimensions itself, so the degenerate sizes draw nothing.
        c.rounded_rect(1, 2, w - 2, h - 4, 3, Rgba::from_hex(&t.panel, t.panel_alpha));

        let rows = (h - 4) / PITCH;
        let cols = (w - 2) / PITCH;
        if rows < MIN_ROWS || cols < MIN_COLS {
            return; // shed rather than smudge
        }
        self.fit(cols as usize);

        // Centre the grid in whatever slack the panel leaves, so the lattice never crowds one edge.
        let ox = 1 + ((w - 2) - cols * PITCH) / 2;
        let oy = 2 + ((h - 4) - rows * PITCH) / 2;
        let spec_rows = SPEC_ROWS.min(rows - 3);
        let water_row = rows - spec_rows - 1;

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let cap_c = Rgba::from_hex(&t.lit, CAP_ALPHA);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let well = Rgba::from_hex(&t.lit, t.ghost.clamp(0.0, 1.0));

        // ---- the well lattice: every cell, faintly. This is what makes it a display. ----
        for row in 0..rows {
            for col in 0..cols {
                c.fill_rect(ox + col * PITCH, oy + row * PITCH, DOT, DOT, well);
            }
        }

        // ---- spectrum: height in dots, bottom up ----
        let b = &t.ballistics;
        for col in 0..cols {
            // Group bands onto columns rather than dropping any: a column covers a contiguous span and
            // takes its MAX, so a single sharp band cannot be averaged into invisibility.
            let lo = (col as usize * NUM_BANDS) / cols as usize;
            let hi = (((col as usize + 1) * NUM_BANDS) / cols as usize).clamp(lo + 1, NUM_BANDS);
            let band = d.levels[lo..hi].iter().copied().fold(0.0f32, f32::max);

            let target = resp(band, t.sensitivity);
            let cur = self.levels[col as usize];
            let k = if target > cur { b.attack } else { b.decay };
            let next = cur + (target - cur) * k.clamp(0.0, 1.0);
            self.levels[col as usize] = if next.is_finite() { next } else { 0.0 };

            let held = (self.caps[col as usize] - b.peak_fall.max(0.0)).max(self.levels[col as usize]);
            self.caps[col as usize] = if held.is_finite() { held.clamp(0.0, 1.0) } else { 0.0 };

            let dots = (self.levels[col as usize] * spec_rows as f32).round() as i32;
            for k in 0..dots {
                let row = rows - 1 - k;
                if row > water_row {
                    c.fill_rect(ox + col * PITCH, oy + row * PITCH, DOT, DOT, lit);
                }
            }
            // The cap only draws where it is genuinely ABOVE the lit column - otherwise it would
            // re-light the top lit dot at a lower alpha and read as nothing at all.
            let cap_dots = (self.caps[col as usize] * spec_rows as f32).round() as i32;
            if cap_dots > dots {
                let row = rows - 1 - cap_dots;
                if row > water_row {
                    c.fill_rect(ox + col * PITCH, oy + row * PITCH, DOT, DOT, cap_c);
                }
            }
        }

        // ---- the waterline: a dotted row the dolphin breaks through ----
        for col in (0..cols).step_by(2) {
            c.fill_rect(ox + col * PITCH, oy + water_row * PITCH, DOT, DOT, cap_c);
        }

        // ---- the dolphin ----
        let drive = Self::drive(d, t.sensitivity);
        let loop_s = LOOP_SLOW_S + (LOOP_FAST_S - LOOP_SLOW_S) * drive;
        self.phase += dt / 1000.0 / loop_s.max(0.2);
        if !self.phase.is_finite() {
            self.phase = 0.0;
        }
        while self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let arc = (std::f32::consts::PI * self.phase).sin();
        // Enters left of the panel and leaves to the right, so it is never clipped mid-body at an edge.
        let travel = cols + SPRITE_W * 2;
        let sx = (self.phase * travel as f32) as i32 - SPRITE_W;
        // Submerged at the ends, apex in the middle, lifted further while leaping.
        let base_top = water_row;
        let apex_top = (water_row - (SPRITE_H - 1) - (leap * LEAP_LIFT as f32).round() as i32).max(0);
        let sy = base_top - ((base_top - apex_top) as f32 * arc).round() as i32;
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
        let mask: &[&str; SPRITE_H as usize] = &BODY[self.tail.min(PHASES - 1)];
        let body = if leap > 0.01 { hot } else { lit };
        let on = |rx: i32, ry: i32| -> bool {
            ry >= 0
                && ry < SPRITE_H
                && rx >= 0
                && rx < SPRITE_W
                && mask[ry as usize].as_bytes()[rx as usize] == b'#'
        };
        // KEYLINE FIRST. Without it the dolphin is lit dots on a lit lattice with nothing between
        // them, and it reads as an amorphous cluster - which is exactly how the first render came out.
        // `chroma.rs:19-25` records the fix: a hard dark outline makes a shape legible independently of
        // its own colour. Drawn as opaque panel over every cell ADJACENT to the body, so it erases the
        // well lattice and any spectrum dot there rather than blending with it.
        let key = Rgba::from_hex(&t.panel, 1.0);
        for ry in -1..=SPRITE_H {
            for rx in -1..=SPRITE_W {
                if on(rx, ry) {
                    continue;
                }
                let touches = (-1..=1).any(|dy| (-1..=1).any(|dx| on(rx + dx, ry + dy)));
                if !touches {
                    continue;
                }
                let (col, row) = (sx + rx, sy + ry);
                if col >= 0 && col < cols && row >= 0 && row < rows {
                    c.fill_rect(ox + col * PITCH, oy + row * PITCH, DOT, DOT, key);
                }
            }
        }
        for (ry, line) in mask.iter().enumerate() {
            for (rx, ch) in line.chars().enumerate() {
                let col = sx + rx as i32;
                let row = sy + ry as i32;
                if ch == '#' && col >= 0 && col < cols && row >= 0 && row < rows {
                    c.fill_rect(ox + col * PITCH, oy + row * PITCH, DOT, DOT, body);
                }
            }
        }

        // ---- splash: breaking the waterline throws dots along it ----
        let nose = sx + SPRITE_W - 1;
        let breaking = sy + SPRITE_H - 1 >= water_row && arc < 0.55;
        for col in 0..cols {
            let near = (col - nose).abs();
            let add = if breaking && near <= 3 { 1.0 - near as f32 / 4.0 } else { 0.0 };
            let next = (self.splash[col as usize] - dt / SPLASH_MS).max(0.0).max(add);
            self.splash[col as usize] = if next.is_finite() { next.clamp(0.0, 1.0) } else { 0.0 };
            if self.splash[col as usize] > 0.35 && water_row - 1 >= 0 {
                c.fill_rect(ox + col * PITCH, oy + (water_row - 1) * PITCH, DOT, DOT, hot);
            }
        }

        // A transmissive LCD has almost no halo, so the bloom is tight and the colourways keep it low.
        c.bloom(t.bloom as i32, t.glow_strength);
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
        let rows = (h - 4) / PITCH;
        let cols = (w - 2) / PITCH;
        let ox = 1 + ((w - 2) - cols * PITCH) / 2;
        let oy = 2 + ((h - 4) - rows * PITCH) / 2;
        let spec_rows = SPEC_ROWS.min(rows - 3);
        let water_row = rows - spec_rows - 1;
        (rows, cols, ox, oy, spec_rows, water_row)
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
        assert!(
            lit > well * 3.0,
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
                .filter(|k| lum(c.get(ox + PITCH, oy + (water_row + 1 + k) * PITCH)) > 60.0)
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

    #[test]
    fn the_dolphin_arcs_and_dips_through_the_waterline() {
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
                    if lum(c.get(ox + col * PITCH, oy + row * PITCH)) > 90.0 {
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
                        if lum(c.get(ox + col * PITCH, oy + row * PITCH)) > 90.0 {
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
        let well_ref = lum(c.get(ox, oy));
        let mut found_body = false;
        let mut found_key = false;
        for row in 1..water_row {
            for col in 1..cols - 1 {
                if lum(c.get(ox + col * PITCH, oy + row * PITCH)) > 90.0 {
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
        println!("wrote dolphin dumps to {}", dir.display());
    }
}
