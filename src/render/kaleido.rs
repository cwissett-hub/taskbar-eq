//! The kaleidoscope family: mirrored, repeating psychedelic symmetry.
//!
//! Asked for as "a kaleidoscope type thing for more psychedelic visuals".
//!
//! # Why this is a FRIEZE and not a rosette
//!
//! A kaleidoscope normally means rosette symmetry - 6- or 8-fold rotation about one centre. That does not
//! fit this panel, and the reason has already killed two designs in this project. At 380x60 the aspect
//! ratio is 6.3:1, so a single centred disc is 60px across and uses 16% of the width; the rest is empty.
//! It is the same failure the isometric attempt hit, where the x axis fed both width and height and 56
//! rows ran out after ~64px of width, and it is not fixable by tuning.
//!
//! So the symmetry GROUP changes rather than the design shrinking. The symmetry groups of an infinite
//! strip are the seven frieze groups, and those are the groups a 6:1 letterbox actually has. This family
//! uses two perpendicular families of mirror lines - vertical ones every `cell_w` pixels and horizontal
//! ones every `cell_h` - which is the frieze group conventionally written pmm2. At the shipped cell size
//! the horizontal family has exactly one member, down the panel's middle, so the result is a row of
//! complete four-fold rosettes marching across the panel at full height: it reads as a kaleidoscope AND
//! fills the strip. A real kaleidoscope's tube view, unrolled along its length, is a frieze; this is not
//! a compromise shape.
//!
//! Both axes are written as general repeats rather than the single reflection the steady pattern needs,
//! because the flourish subdivides both - see the note on it below.
//!
//! # RADIUS IS FREQUENCY, which is what makes it a meter
//!
//! The band a pixel reads is chosen by its distance from the nearest rosette centre: bass at the centre,
//! treble at the rim. So every rosette is a full radial spectrum, and the symmetry is not decoration
//! sitting on top of a meter - the symmetry IS the meter. A loud bass passage swells the centres, a
//! bright hi-hat lights the rims, and the pattern reorganises with the music rather than merely pulsing.
//!
//! That also keeps the house rule: level is POSITION, never brightness. `tube.rs:54-60` measured a driven
//! element 1.46 dL* brighter than its neighbour as invisible against a ~2.3 dL* threshold, so the level
//! here moves where the facet EDGES fall, not how bright they are.
//!
//! # Hard facets, not a smooth field
//!
//! Values are quantised into `FACETS` flat steps. A smooth radial gradient reads as a lens flare; a
//! kaleidoscope is coloured glass, and glass has edges. Quantising is also what makes the mirror lines
//! visible, which is the whole point - a symmetry nobody can see is not a symmetry.
//!
//! # The flourish: THE MIRRORS MULTIPLY
//!
//! Every family here has one. This family's is the event only a kaleidoscope can have: the fold count
//! doubles, so twice as many rosettes crowd into the strip, and the spin surges while it holds.
//!
//! A fold count is discrete, so switching it instantly is correct rather than jarring - the project's
//! finding about steps reads on GEOMETRIC quantities, where a one-frame jump in a position or a size is a
//! snap. A change of topology has no in-between state to pass through, and pretending otherwise (crossing
//! two fold counts over half a second) would read as a dissolve, not as a kaleidoscope turning.
//!
//! # The cost, and the trap in it
//!
//! A naive implementation computes the fold arithmetic per panel pixel per frame: 22,800 pixels, each
//! wanting an `atan2` and a `sqrt`. That is the trap. Everything geometric here depends only on the
//! canvas size, so it is precomputed once into tables and rebuilt only when the size changes:
//!
//! - `band`, `ang`, `r01` are per CELL pixel, and the cell is a quarter of one rosette - about 28x28,
//!   or 784 entries, not 22,800.
//! - `map` is per panel pixel and holds the cell index that pixel mirrors to, so the per-frame work is
//!   a table lookup and a copy.
//!
//! Both fold counts are built up front, because building one mid-flourish would put a table rebuild on
//! the exact frame that is already doing the most work.
//!
//! Per frame that leaves one `sin` per cell pixel (~784) and one lookup per panel pixel. The tables are
//! keyed on `(w, h)` and rebuilt on mismatch - a table sized for the old width indexing a new buffer is
//! either a panic or a silently wrong image, so the dimensions are checked every frame rather than
//! trusting a resize notification this family never receives.

use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// How many flat colour steps a rosette is quantised into. See the module note on facets.
///
/// Four rather than more: at ~28px of radius, five or more steps put facet edges within a pixel or two
/// of each other near the centre, where the radial bands are already narrowest, and the shape turns back
/// into a gradient. Four holds its edges everywhere.
const FACETS: f32 = 4.0;

/// Petals per rosette, and colour rings across its radius.
///
/// Six petals because it is the count that reads as a kaleidoscope rather than as a star (4) or a blur
/// (12+) - and because at 28px of radius, twelve petals put adjacent petal edges under 2px apart at half
/// radius, which aliases into a moire rather than resolving.
const PETALS: f32 = 6.0;
const HUE_RINGS: f32 = 1.6;

/// How fast the pattern turns, in radians per second, at silence and at full drive.
///
/// The floor is not zero: a kaleidoscope that stops dead reads as a freeze rather than as quiet, the same
/// reason the blossom family's wind has a floor. The ceiling is bounded by aliasing - the reel family
/// measured that motion past half a feature pitch per frame appears to run BACKWARDS, and a petal edge at
/// full radius travels `r * omega` px/s, so at r=28 and 60fps this keeps the fastest edge under 1.5px per
/// frame.
const SPIN_CALM: f32 = 0.10;
const SPIN_WILD: f32 = 0.55;

/// How fast the spin follows the music, per millisecond. Slow on purpose: a pattern that tracked every
/// transient would read as jitter rather than as motion.
const SPIN_FOLLOW: f32 = 0.005;

/// The flourish: how long the extra mirrors hold, how much faster it spins while they do, and the
/// envelope level below which the fold count drops back.
///
/// The switch-back threshold is well above zero so the coarse pattern returns while there is still some
/// spin surge left - the mirrors go first and the motion settles afterwards, which reads as the tube
/// being turned and let go. Dropping both together reads as a freeze frame.
const SHATTER_MS: f32 = 1500.0;
const SHATTER_SPIN: f32 = 3.6;
const SHATTER_SWITCH: f32 = 0.22;

/// The smallest a fold cell may be. Below this a rosette is smaller than its own facet steps.
///
/// This is what stops the flourish on a narrow panel: halving an already-small cell would produce a
/// pattern finer than the quantisation that is supposed to make it legible, so on a panel that cannot
/// afford the fine fold the family keeps the coarse one and the flourish shows only as the spin surge.
const MIN_CELL: i32 = 9;

/// The level window: `vapor`'s MEASURED p10-p90 of real music.
///
/// Not a 0..1 mapping, which renders dead, and not normalised against the frame's loudest band, which is
/// provably inert at p50 0.819.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// The smallest panel this family will draw on.
const MIN_W: i32 = 60;
const MIN_H: i32 = 24;

/// How much of a rosette's radius the bass end occupies.
///
/// Not 1.0: mapping the full radius across every band puts the top octaves in the outermost 2px, where
/// there is no room for a facet. 0.82 keeps the highest bands inside the rim.
const BAND_REACH: f32 = 0.82;

/// One fold configuration: the quarter-rosette tables plus the panel-to-cell mirror map.
#[derive(Default)]
struct Folds {
    cell_w: i32,
    cell_h: i32,
    /// Per cell pixel: which band it reads, its angle from the rosette centre, its normalised radius.
    band: Vec<u16>,
    ang: Vec<f32>,
    r01: Vec<f32>,
    /// Per panel-field pixel: the cell index it mirrors to. This is the frieze mapping.
    map: Vec<u32>,
    /// Per cell pixel, this frame's resolved colour. Reused rather than reallocated.
    cell: Vec<Rgba>,
}

impl Folds {
    fn build(fw: i32, fh: i32, cw: i32, ch: i32) -> Folds {
        let n = (cw.max(1) * ch.max(1)) as usize;
        let mut f = Folds {
            cell_w: cw,
            cell_h: ch,
            band: vec![0; n],
            ang: vec![0.0; n],
            r01: vec![0.0; n],
            map: vec![0; (fw.max(0) * fh.max(0)) as usize],
            cell: vec![Rgba { r: 0, g: 0, b: 0, a: 0 }; n],
        };
        // The rosette centre is the corner where the two mirror families meet: (cw-1, ch-1) in cell
        // coordinates. Radius is measured from there.
        let rmax = (((cw - 1) * (cw - 1) + (ch - 1) * (ch - 1)) as f32).sqrt().max(1.0);
        let nb = crate::dsp::bands::NUM_BANDS as f32;
        for sy in 0..ch {
            for sx in 0..cw {
                let dx = (cw - 1 - sx) as f32;
                let dy = (ch - 1 - sy) as f32;
                let i = (sy * cw + sx) as usize;
                let r = (dx * dx + dy * dy).sqrt();
                f.r01[i] = (r / rmax).clamp(0.0, 1.0);
                // `atan2` of two zeros is 0.0 in Rust rather than NaN, so the exact centre is defined.
                f.ang[i] = dy.atan2(dx);
                f.band[i] = (f.r01[i] / BAND_REACH * nb).floor().clamp(0.0, nb - 1.0) as u16;
            }
        }
        // The frieze mapping. BOTH axes fold the same way: a mirror every `cw` columns and every `ch`
        // rows, each alternating direction. Together that is pmm2.
        //
        // The y fold is written as a general repeat rather than a single reflection about the middle, and
        // that matters for the flourish. With `ch` at half the field height the two are identical - the
        // fold reduces to exactly one mirror down the centre line - so the steady pattern is unchanged.
        // But the fine fold halves `ch` too, and a single centre mirror could not express that: halving
        // only the WIDTH stretched every rosette into a tall ellipse and the doubled pattern rendered as
        // a bright horizontal band instead of as more flowers. Rosettes have to stay round, so both axes
        // subdivide together.
        let fold = |v: i32, c: i32| -> i32 {
            let off = v.rem_euclid(c);
            if (v / c) % 2 == 0 { off } else { c - 1 - off }
        };
        for ly in 0..fh {
            let sy = fold(ly, ch).clamp(0, ch - 1);
            for lx in 0..fw {
                let sx = fold(lx, cw).clamp(0, cw - 1);
                f.map[(ly * fw + lx) as usize] = (sy * cw + sx) as u32;
            }
        }
        f
    }

    fn ok(&self, fw: i32, fh: i32) -> bool {
        !self.cell.is_empty() && self.map.len() == (fw.max(0) * fh.max(0)) as usize
    }
}

#[derive(Default)]
pub struct Kaleido {
    /// The `(w, h)` the tables were built for. Checked every frame - see the module note.
    dims: (i32, i32),
    coarse: Folds,
    fine: Folds,
    /// Accumulated rotation, and the smoothed spin rate driving it.
    phase: f32,
    spin: f32,
    flourish: crate::dsp::flourish::Trigger,
    shatter: crate::dsp::flourish::Envelope,
}

/// The drawable field inside the rounded panel: `(x, y, w, h)`.
///
/// Deliberately the panel's interior rather than the whole canvas, so the pattern cannot square off the
/// corners the panel just rounded. The final clip is belt-and-braces on top of this.
fn field(w: i32, h: i32) -> (i32, i32, i32, i32) {
    (1, 2, w - 2, h - 4)
}

fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Rgba { r: f(a.r, b.r), g: f(a.g, b.g), b: f(a.b, b.b), a: f(a.a, b.a) }
}

impl Kaleido {
    fn rebuild(&mut self, w: i32, h: i32) {
        let (_, _, fw, fh) = field(w, h);
        // A quarter of a rosette. The horizontal mirror runs down the field's middle, so the cell is half
        // the field's height; the cell is square so the rosettes are round rather than stretched, which
        // is the one thing that would give away that this is a strip and not a tube.
        let ch = (fh + 1) / 2;
        self.coarse = Folds::build(fw, fh, ch.max(1), ch.max(1));
        // The flourish's fold count: half the cell in BOTH axes, so there are twice as many rosettes
        // across and two rows of them instead of one, and every rosette stays ROUND. Halving only the
        // width was tried and it stretched each one into a tall ellipse - the doubled pattern rendered
        // as a bright horizontal band rather than as more flowers, which is not what "the mirrors
        // multiply" is supposed to look like.
        let fine = (ch / 2).max(1);
        self.fine = if fine >= MIN_CELL { Folds::build(fw, fh, fine, fine) } else { Folds::default() };
        self.dims = (w, h);
    }
}

impl Family for Kaleido {
    fn id(&self) -> &'static str {
        "kaleido"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        if w < MIN_W || h < MIN_H {
            return; // shed rather than smudge
        }
        if self.dims != (w, h) {
            self.rebuild(w, h);
        }
        let (fx, fy, fw, fh) = field(w, h);
        if !self.coarse.ok(fw, fh) {
            return; // a rebuild that could not produce a usable table
        }

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 250.0) } else { 16.7 };
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let shatter = self.shatter.update(fired, dt, SHATTER_MS);

        // ---- how fast it turns ----
        let mean = d.levels.iter().sum::<f32>() / d.levels.len() as f32;
        let drive = ((mean - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0).powf(LEVEL_GAMMA);
        let want = (SPIN_CALM + (SPIN_WILD - SPIN_CALM) * drive)
            * (1.0 + (SHATTER_SPIN - 1.0) * shatter);
        self.spin += (want - self.spin) * (SPIN_FOLLOW * dt).min(1.0);
        if !self.spin.is_finite() {
            self.spin = SPIN_CALM;
        }
        self.phase += self.spin * dt / 1000.0;
        if !self.phase.is_finite() {
            self.phase = 0.0;
        }
        // Wrapped rather than left to grow: an f32 accumulating for a working day loses the low bits of
        // its fractional part, and the pattern would visibly quantise by the afternoon.
        self.phase = self.phase.rem_euclid(std::f32::consts::TAU);

        // ---- which fold count ----
        let use_fine = shatter > SHATTER_SWITCH && self.fine.ok(fw, fh);
        let phase = self.phase;
        let folds = if use_fine { &mut self.fine } else { &mut self.coarse };

        // ---- resolve one quarter-rosette ----
        let dark = Rgba::from_hex(&t.panel, 1.0);
        let cells = (folds.cell_w * folds.cell_h) as usize;
        for i in 0..cells.min(folds.cell.len()) {
            let lv = ((d.levels[folds.band[i] as usize] - LEVEL_FLOOR) / LEVEL_SPAN)
                .clamp(0.0, 1.0)
                .powf(LEVEL_GAMMA);
            let petal = 0.5 + 0.5 * (folds.ang[i] * PETALS + phase).sin();
            // Quantised into flat facets - see FACETS. The floor is what puts a hard edge between steps.
            let v = ((lv * (0.30 + 0.70 * petal)) * FACETS).floor() / FACETS;
            folds.cell[i] = if v <= 0.0 {
                dark
            } else {
                // Hue walks with angle AND radius, so a rainbow colourway gets colour petals crossed
                // with colour rings rather than one flat wash. On a fixed colourway `tint` returns
                // `t.lit` unchanged and the facet step is carried by the lit-to-hot ramp instead.
                let hue = (folds.ang[i] / std::f32::consts::TAU * PETALS
                    + folds.r01[i] * HUE_RINGS
                    - phase * 0.08)
                    .rem_euclid(1.0);
                let base = crate::render::tint(t, hue, d.time_s, false, &t.lit, 1.0);
                let hot = crate::render::tint(t, hue, d.time_s, true, &t.hot, 1.0);
                lerp(base, hot, v)
            };
        }

        // ---- mirror it across the panel ----
        for ly in 0..fh {
            for lx in 0..fw {
                let idx = folds.map[(ly * fw + lx) as usize] as usize;
                if let Some(col) = folds.cell.get(idx) {
                    c.fill_rect(fx + lx, fy + ly, 1, 1, *col);
                }
            }
        }

        if t.bloom > 0.0 && t.glow_strength > 0.0 {
            c.bloom(t.bloom as i32, t.glow_strength);
        }
        // The pattern is drawn to the field, which is already inside the rounded panel, so this clips
        // nothing today. It is here because bloom spreads outward and every future change to the field
        // arithmetic is one slip away from painting on the corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    /// Rows and columns skipped at the field's edges when checking symmetry.
    ///
    /// The rounded panel's corner radius is 3, and `clip_to_rounded_rect` runs AFTER the pattern is
    /// drawn - so within a few pixels of a corner one side of a mirror pair can be clipped away while
    /// its partner survives. That asymmetry is the clip's, deliberately, not the pattern's. The margin
    /// is 5 rather than 3 to leave room for the bloom that spreads across the clip boundary.
    const EDGE: i32 = 5;

    fn frame(gain: f32, t_s: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / crate::dsp::bands::NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.5) * 0.58 + 0.15;
            let wob = 1.0 + 0.32 * ((t_s * 2.2 + f * 7.0).sin());
            *v = ((shape * wob) * gain).clamp(0.0, 1.0);
        }
        d
    }

    fn render(gain: f32, frames: usize, w: i32, h: i32) -> (Kaleido, Canvas) {
        let t = builtin::kaleido_prism();
        let mut fam = Kaleido::default();
        let mut c = Canvas::new(w, h);
        for k in 0..frames {
            fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
        }
        (fam, c)
    }

    /// Counts how many mirror pairs hold, and how many break, across the field at a given cell width.
    fn mirror_score(c: &Canvas, w: i32, h: i32, cw: i32) -> (i32, i32) {
        let (fx, fy, fw, fh) = field(w, h);
        let (mut ok, mut bad) = (0, 0);
        for ly in EDGE..fh - EDGE {
            for off in 0..cw {
                let left = cw - 1 - off;
                let right = cw + off;
                if left < EDGE || right >= fw - EDGE {
                    continue;
                }
                let a = c.get(fx + left, fy + ly);
                let b = c.get(fx + right, fy + ly);
                if (a.r, a.g, a.b) == (b.r, b.g, b.b) {
                    ok += 1;
                } else {
                    bad += 1;
                }
            }
        }
        (ok, bad)
    }

    /// The load-bearing property: the pattern is actually SYMMETRIC. A kaleidoscope whose mirrors do not
    /// line up is just a texture, and it is the one thing that cannot be judged from a constant.
    ///
    /// Checks both mirror families independently, because they are computed by different arithmetic and
    /// an error in either would leave the other intact.
    ///
    /// Mutation: drop the `(lx / cw) % 2` alternation in the x fold, or the `fh - 1 - ly` reflection in
    /// the y fold. Either turns a mirror into a plain repeat and this fails.
    #[test]
    fn the_pattern_is_mirrored_in_both_axes() {
        let (fam, c) = render(0.62, 40, 380, 60);
        let (fx, fy, fw, fh) = field(380, 60);
        let cw = fam.coarse.cell_w;
        assert!(cw > 8, "the cell is too small to test: {cw}");

        // Horizontal mirror: the field's middle. Row `ly` must match row `fh - 1 - ly`.
        let mut hchecked = 0;
        for ly in EDGE..fh / 2 {
            for lx in EDGE..fw - EDGE {
                let a = c.get(fx + lx, fy + ly);
                let b = c.get(fx + lx, fy + fh - 1 - ly);
                assert_eq!(
                    (a.r, a.g, a.b),
                    (b.r, b.g, b.b),
                    "horizontal mirror broken at lx {lx}, ly {ly}"
                );
                hchecked += 1;
            }
        }
        assert!(hchecked > 5000, "the horizontal mirror probe barely ran: {hchecked}");

        // Vertical mirrors: one between every pair of cells.
        let (ok, bad) = mirror_score(&c, 380, 60, cw);
        assert!(ok > 500, "the vertical mirror probe barely ran: {ok} ok");
        assert_eq!(bad, 0, "vertical mirror broken on {bad} of {} pairs", ok + bad);
    }

    /// Radius is frequency, so the bass must reach the rosette CENTRES and the treble the RIMS. This is
    /// what makes the family a meter rather than an ornament, and it is invisible in any single frame -
    /// it only shows as a difference between two spectra.
    ///
    /// Mutation: make `band` a constant, or drop `BAND_REACH` so every band lands in the outer 2px.
    #[test]
    fn bass_lights_the_centres_and_treble_the_rims() {
        let t = builtin::kaleido_prism();
        let nb = crate::dsp::bands::NUM_BANDS;
        let probe = |bassy: bool| {
            let mut fam = Kaleido::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..40 {
                let mut d = FrameData { dt_ms: 16.7, time_s: k as f32 * 0.0167, ..Default::default() };
                for (i, v) in d.levels.iter_mut().enumerate() {
                    let low = i * 3 < nb;
                    *v = if low == bassy { 0.85 } else { 0.12 };
                }
                fam.draw(&mut c, &t, &d);
            }
            let (fx, fy, _, fh) = field(380, 60);
            let (cw, ch) = (fam.coarse.cell_w, fam.coarse.cell_h);
            let dark = Rgba::from_hex(&t.panel, 1.0);
            let (mut mid, mut rim) = (0, 0);
            for ly in 0..fh {
                for lx in 0..cw * 2 {
                    let px = c.get(fx + lx, fy + ly);
                    if (px.r, px.g, px.b) == (dark.r, dark.g, dark.b) {
                        continue;
                    }
                    let dx = (lx as f32 - (cw as f32 - 0.5)).abs();
                    let dy = (ly as f32 - (ch as f32 - 0.5)).abs();
                    let r = (dx * dx + dy * dy).sqrt() / ch as f32;
                    if r < 0.42 {
                        mid += 1;
                    } else if r > 0.75 {
                        rim += 1;
                    }
                }
            }
            (mid, rim)
        };
        let (bass_mid, bass_rim) = probe(true);
        let (treb_mid, treb_rim) = probe(false);
        assert!(bass_mid > 40 && treb_rim > 40, "the probe found nothing lit: {bass_mid} {treb_rim}");
        assert!(
            bass_mid * treb_rim > treb_mid * bass_rim,
            "radius does not track frequency: bass mid/rim {bass_mid}/{bass_rim}, treble {treb_mid}/{treb_rim}"
        );
    }

    /// Louder music turns the pattern faster. The family's level-as-position mapping.
    ///
    /// Mutation: make `want` a constant, or drop the `drive` term.
    #[test]
    fn louder_music_spins_the_pattern_faster() {
        let (calm, _) = render(0.12, 240, 380, 60);
        let (wild, _) = render(0.95, 240, 380, 60);
        assert!(
            wild.spin > calm.spin * 1.5,
            "spin did not follow level: calm {:.3} wild {:.3}",
            calm.spin,
            wild.spin
        );
    }

    /// The flourish MULTIPLIES THE MIRRORS. Tested by the property that defines it: during the flourish a
    /// mirror exists at HALF the normal spacing, where in the steady pattern there is none.
    ///
    /// That is checked in both directions, because "a mirror appeared" is only interesting if the same
    /// place was not already mirrored - and the fine mirror lines are a superset of the coarse ones, so
    /// a test that only looked for the fine mirror during the flourish would pass on a family that
    /// always drew the fine fold.
    ///
    /// Mutation: drop the `use_fine` switch, or set SHATTER_SWITCH above 1.0 so it can never engage.
    #[test]
    fn the_flourish_doubles_the_mirrors() {
        let t = builtin::kaleido_prism();
        let mut fam = Kaleido::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..60 {
            fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
        }
        let coarse_w = fam.coarse.cell_w;
        let fine_w = fam.fine.cell_w;
        assert!(fine_w >= MIN_CELL && fine_w < coarse_w, "no fine fold built: {fine_w} vs {coarse_w}");

        // Steady state: the fine spacing must NOT be a mirror line.
        let (ok_before, bad_before) = mirror_score(&c, 380, 60, fine_w);
        assert!(
            bad_before > 20,
            "the fine spacing was already mirrored before the flourish: {ok_before} ok, {bad_before} bad"
        );

        // Flourish: it must be.
        fam.flourish.force_next();
        fam.draw(&mut c, &t, &frame(0.62, 1.02));
        for k in 0..6 {
            fam.draw(&mut c, &t, &frame(0.62, 1.04 + k as f32 * 0.0167));
        }
        let (ok_after, bad_after) = mirror_score(&c, 380, 60, fine_w);
        assert!(ok_after > 200, "the flourish probe barely ran: {ok_after}");
        assert_eq!(
            bad_after, 0,
            "the flourish did not add mirrors at the fine spacing: {bad_after} of {} pairs broken",
            ok_after + bad_after
        );

        // And it must let go again, rather than leaving the panel permanently doubled.
        for k in 0..200 {
            fam.draw(&mut c, &t, &frame(0.62, 2.0 + k as f32 * 0.0167));
        }
        let (_, bad_settled) = mirror_score(&c, 380, 60, fine_w);
        assert!(bad_settled > 20, "the flourish never let go: fine mirrors still hold");
    }

    /// Resizing must rebuild the tables. A table sized for the old width indexing a new buffer is either
    /// a panic or a silently wrong image, and this family is resized in production whenever an unrelated
    /// window opens or closes.
    ///
    /// Mutation: skip the `self.dims != (w, h)` check, and the second draw panics or renders garbage.
    #[test]
    fn a_resize_rebuilds_the_tables_without_panicking() {
        let t = builtin::kaleido_prism();
        let mut fam = Kaleido::default();
        for (w, h) in [(380, 60), (190, 60), (380, 60), (64, 30), (240, 44), (380, 60)] {
            let mut c = Canvas::new(w, h);
            for k in 0..8 {
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            }
            let (_, _, fw, fh) = field(w, h);
            assert_eq!(fam.dims, (w, h), "tables not rebuilt for {w}x{h}");
            assert_eq!(fam.coarse.map.len(), (fw * fh) as usize, "map wrong size for {w}x{h}");
        }
    }

    /// A resize DURING a flourish is the case where a stale table would actually be indexed, because the
    /// fine fold is only ever read on those frames.
    #[test]
    fn a_resize_mid_flourish_does_not_panic() {
        let t = builtin::kaleido_prism();
        let mut fam = Kaleido::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..30 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        fam.flourish.force_next();
        fam.draw(&mut c, &t, &frame(0.6, 0.6));
        for (w, h) in [(190, 60), (380, 44), (72, 26), (380, 60)] {
            let mut c2 = Canvas::new(w, h);
            for k in 0..6 {
                fam.draw(&mut c2, &t, &frame(0.6, 1.0 + k as f32 * 0.0167));
            }
        }
    }

    /// Small panels shed rather than smudge, and must not panic on the way out.
    #[test]
    fn tiny_panels_shed() {
        let t = builtin::kaleido_prism();
        for (w, h) in [(1, 1), (8, 8), (59, 23), (60, 12), (12, 60), (0, 0)] {
            let mut fam = Kaleido::default();
            let mut c = Canvas::new(w, h);
            fam.flourish.force_next();
            fam.draw(&mut c, &t, &frame(0.6, 0.1));
            fam.draw(&mut c, &t, &frame(0.6, 0.2));
        }
    }

    /// A hostile frame: NaN and infinity in the levels and the timestep must not reach the canvas or
    /// wedge the accumulated phase.
    #[test]
    fn a_hostile_frame_does_not_poison_the_state() {
        let t = builtin::kaleido_prism();
        let mut fam = Kaleido::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..20 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        let mut bad = frame(0.6, 1.0);
        bad.dt_ms = f32::NAN;
        bad.levels[0] = f32::NAN;
        bad.levels[3] = f32::INFINITY;
        bad.levels[5] = -1.0e30;
        fam.draw(&mut c, &t, &bad);
        fam.draw(&mut c, &t, &frame(0.6, 1.1));
        assert!(fam.phase.is_finite(), "phase went non-finite");
        assert!(fam.spin.is_finite(), "spin went non-finite");
    }

    /// Every colourway renders something, on both panel widths.
    #[test]
    fn every_colourway_draws_on_both_widths() {
        for t in builtin::all().into_iter().filter(|t| t.family == "kaleido") {
            for w in [380, 190] {
                let mut fam = Kaleido::default();
                let mut c = Canvas::new(w, 60);
                for k in 0..30 {
                    fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
                }
                let mut lit = 0;
                for y in 0..60 {
                    for x in 0..w {
                        if c.get(x, y).a > 0 {
                            lit += 1;
                        }
                    }
                }
                assert!(lit > w * 20, "{} drew almost nothing at {w}px: {lit}", t.id);
            }
        }
    }

    /// The per-frame cost, which the precomputed tables exist to bound. Ignored: it is a measurement,
    /// not a gate, and timing assertions on a shared machine are flaky.
    #[test]
    #[ignore]
    fn probe_kaleido_cost() {
        let t = builtin::kaleido_prism();
        let mut fam = Kaleido::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..60 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!(
            "kaleido: {ms:.3} ms/frame at 380x60 (coarse cell {}x{}, fine {}x{})",
            fam.coarse.cell_w, fam.coarse.cell_h, fam.fine.cell_w, fam.fine.cell_h
        );
    }

    #[test]
    #[ignore]
    fn dump_kaleido() {
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
        for t in builtin::all().into_iter().filter(|t| t.family == "kaleido") {
            let mut fam = Kaleido::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..300 {
                fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            }
            write(format!("kaleido-{}", t.id), &c);
        }
        // Quiet against loud, so the spin mapping shows as a difference in the pattern's twist.
        let t = builtin::kaleido_prism();
        for (gain, tag) in [(0.12f32, "calm"), (0.95, "wild")] {
            let mut fam = Kaleido::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..300 {
                fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
            }
            write(format!("kaleido-spin-{tag}"), &c);
        }
        // The flourish, mid-hold, so the doubled mirrors are visible against the steady pattern.
        let mut fam = Kaleido::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..120 {
            fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
        }
        write("kaleido-steady".into(), &c);
        fam.flourish.force_next();
        for k in 0..14 {
            fam.draw(&mut c, &t, &frame(0.62, 2.0 + k as f32 * 0.0167));
        }
        write("kaleido-shatter".into(), &c);
    }
}
