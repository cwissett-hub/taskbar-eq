//! The 3D Pipes family: the Windows screensaver, in a REAL perspective projection.
//!
//! Asked for directly. An isometric version was built first and rejected - "if we cant do true 3d then
//! I think it's not really worth it" - and it deserved to be: in isometric the x axis feeds both width
//! and height, so a 56-row panel runs out of height after ~64px of width, and one lattice occupied 48px
//! of a 380px panel. The workaround (several small lattices) is fakery. This one has a camera, a divide
//! and a near plane.
//!
//! # The projection, and the numbers that prove it works
//!
//! LEFT-HANDED eye space, camera AT THE ORIGIN, +x right, +y up, +z INTO the screen. Left-handed
//! deliberately: right-handed would make depth negative, the near test need a sign flip and the painter
//! sort need sign games. Here depth is positive, the near test is one comparison, the sort is ascending.
//!
//! ```text
//!   inv = F / z            <- the divide, the whole point
//!   col = cx + X * inv
//!   row = CY - Y * inv     <- minus, because world +y is up and screen rows go down
//! ```
//!
//! One lattice step is one world unit, so `F` is pixels per step at unit depth.
//!
//! **THE CAMERA SITS ABOVE THE LATTICE, NOT INSIDE IT, and that is load-bearing.** `row = CY - F*Y/z`,
//! so a level at `Y = 0` lands on the horizon with EXACTLY ZERO depth separation at every depth -
//! centring the camera vertically in the lattice manufactures the collapse below on purpose. Every level
//! sits at `Y = -(Y_TOP + j)` with `Y_TOP > 0`, strictly below the horizon.
//!
//! **That collapse is measured, not theoretical.** `vapor` at its tuned `persp = 2.07` put SEVEN of
//! sixteen depth lines onto rows 28-29, and it silently disabled that family's occlusion, because two
//! lines sharing an integer row cannot occlude each other. With the constants below, every one of the
//! four levels resolves all EIGHT depth planes onto its own integer row:
//!
//! ```text
//!   level Y     integer rows                distinct  min gap
//!   L0   -4.0   28 22 17 14 12 10  9  8      8 / 8     1.164 px
//!   L1   -5.0   36 28 23 19 16 14 12 11      8 / 8     1.455 px
//!   L2   -6.0   44 34 28 23 20 17 15 13      8 / 8     1.745 px
//!   L3   -7.0   52 41 33 28 24 21 19 16      8 / 8     2.036 px
//! ```
//!
//! A gap of >= 1.0px is SUFFICIENT for two distinct integer rows, so 1.164 is the guarantee plus an f32
//! margin. `F` is pinned by exactly one inequality, because the smallest gap is always on the TOP level
//! at the FAR plane:
//!
//! ```text
//!   F >= g_min / ( Y_TOP * (1/(Z_FAR-1) - 1/Z_FAR) ) = 1.15 / (4 * (1/10 - 1/11)) = 31.6  ->  F = 32
//! ```
//!
//! # The honest price of a divide
//!
//! EIGHT depth planes. Not sixteen, not thirty. A linear projection spends 1 row per depth step; a
//! perspective divide spends about 2.9, because the gap goes as 1/z^2 and the near gaps must therefore
//! be rho^2 = 5.5x the far gaps for the far gaps to reach one pixel. The 48-row budget decomposes as
//! 20.4 rows of depth spread + 24.0 of lattice height + 3.9 of pipe thickness. Each extra vertical level
//! costs 8 rows, which is 2.5 depth planes - the trade in one number.
//!
//! That inefficiency IS the perspective. It is what makes near pipes fat and far pipes thin, which the
//! isometric version could not do at all.
//!
//! # Why the near clip exists even though it never fires
//!
//! `canvas.rs` `line()` is integer Bresenham that breaks only on reaching its endpoint - no off-canvas
//! early-out. A vertex at the eye projects to infinity, `as i32` saturates to 2147483647, and ONE such
//! edge iterates ~2.1e9 times: measured at 294.6ms, eighteen dropped frames.
//!
//! Here the lattice lives at `z` in [4, 11] and the camera never moves, so `z >= Z_NEAR` always holds
//! and the clip is dead code today. It is written anyway, because "the geometry happens to stay in front
//! of the camera" is an invariant nobody will remember the day the camera is made to drift.
//!
//! Two details that are easy to get subtly wrong:
//!
//! - The test is `z >= Z_NEAR`, NOT `!(z < Z_NEAR)`. For a NaN `z` the first is FALSE, which puts the
//!   vertex in the reject bucket; the second is TRUE and leaks a NaN into the divide. Same value,
//!   opposite safety.
//! - Never test `z > 0.0`. The coordinate bound is `F * R / Z_NEAR`, which goes to infinity as
//!   `Z_NEAR -> 0`. A plane at zero is not a near plane, it is the hang with extra steps.
//!
//! **The clip bounds the coordinate; the clamp only bounds the arithmetic.** With `Z_NEAR = 4`, `F = 32`
//! and a half-width under 24 world units, `|col - cx| <= 192px` - far inside `COORD_LIMIT`.
//!
//! # What the music does
//!
//! Pipes wanders by itself and displays nothing; that is the real work, not the geometry. The lazy
//! answer is already ruled out - `tube.rs:54-60` measured a driven element 1.46 dL* brighter than its
//! neighbour as INVISIBLE against a ~2.3 dL* threshold, so "the pipes glow with the bass" cannot work
//! at this size.
//!
//! - **Turns land on onsets**, so every corner of every run is a beat and the shape left behind is the
//!   rhythm. This is the load-bearing mapping: a listener can read tempo off the geometry.
//! - **Growth rate tracks level**, so a busy passage lays pipe faster.
//! - **Each run reads its own slice of the spectrum**, low on the left, so several runs are not several
//!   copies of one signal.
//! - Radius comes from DEPTH, never from level. It has to: that taper is the perspective.

use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// The near plane, in lattice steps. The depth range is `Z_NEAR + 0 ..= Z_NEAR + nz`, and `nz` is
/// FITTED to the panel height rather than fixed - see `Fit`.
const Z_NEAR: f32 = 4.0;

/// Vertical levels (`NY + 1` of them) and how far above the top one the camera sits.
const NY: i32 = 3;
const Y_TOP: f32 = 4.0;


/// Pipe radius as a fraction of a lattice step. Projected: 2.88px at the near plane, 1.05px at the far.
const R_PIPE: f32 = 0.36;

/// The near plane for the clip, and the post-divide coordinate clamp.
const Z_MIN: f32 = 1.0;
const COORD_LIMIT: i32 = 4096;

/// Runs alive at once, and segments each keeps before its tail is dropped.
///
/// THREE, one per hue (`lit`, `hot`, and their midpoint) - fidelity pass 1. Four runs in three hues put
/// two same-hue pipes on the panel, which reads as one pipe that jumped. At any width wide enough for a
/// lattice all three run, so a 128px panel is a lattice of three pipes too rather than one.
const MAX_RUNS: usize = 3;
///
/// The cell budget was 26 when one pipe had the panel to itself. With three sharing it, a run capped at
/// 60% of 26 (15 cells) left most of the lattice empty - measured in the fidelity-pass dumps, three short
/// streaks on a black strip. 44 keeps 26 per run, so three runs lay about 78 cells across the lattice.
const MAX_SEG: usize = 44;

/// Milliseconds per segment at silence and at full drive.
const GROW_SLOW_MS: f32 = 300.0;
const GROW_FAST_MS: f32 = 95.0;

/// The level window, `vapor`'s MEASURED p10-p90 of real music - not a 0..1 mapping, which renders dead,
/// and not normalised against the loudest band, which is provably inert at p50 0.819.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// The flourish: a SURGE, not a reset.
///
/// It used to call `restart()` on every run, which cleared every segment - so the whole picture vanished
/// in one frame and grew back from nothing. Reported as "jarring movement... like they're resetting",
/// and that is exactly what it was: the screensaver's own behaviour, which is fine on a full screen once
/// every few minutes and is a hard cut on a 60px strip every thirty seconds.
///
/// Now the runs simply lay pipe much faster and run hot while it lasts. Nothing is destroyed, so there is
/// no discontinuity anywhere - the picture only ever grows and its tail only ever retreats one segment at
/// a time, which it was already doing continuously.
const RESET_MS: f32 = 1400.0;

/// How much faster pipe is laid during the surge. 3x is visible without outrunning the aliasing bound.
const SURGE_RATE: f32 = 3.0;

/// How many segments a run keeps before its tail dissolves, segment by segment - 60% of `MAX_SEG`,
/// the cell budget. Shorter than the old flush-at-`MAX_SEG` so THREE fat pipes fill the lattice
/// without clogging it into a solid block: a run past this length drops its oldest cell for every new
/// one it lays, so the tail retreats continuously rather than the whole run vanishing.
const KEEP_SEG: usize = MAX_SEG * 3 / 5;

/// The teapot. The Utah teapot is the graphics world's in-joke, and the real 3D-Pipes screensaver
/// drops one in at random - so ours appears at the growing tip on a flourish and rides along for
/// `TEAPOT_MS`, drawn in `hot`. This is OUR OWN 9x7 silhouette (spout left, handle right, domed lid),
/// one row per `u16` with bit 8 the leftmost column - not lifted from any asset.
const TEAPOT_W: i32 = 9;
const TEAPOT_H: i32 = 7;
const TEAPOT_MS: f32 = 600.0;
const TEAPOT: [u16; TEAPOT_H as usize] = [
    0b000_010_000, // ....#....  the knob
    0b001_111_100, // ..#####..  the lid
    0b101_111_111, // #.#######  spout tip, body, top of the handle
    0b011_111_101, // .######.#  spout joins the body; the handle's hole
    0b001_111_111, // ..#######  body, bottom of the handle
    0b001_111_100, // ..#####..  body
    0b000_111_000, // ...###...  the foot
];

/// A lattice cell. `x` runs across the panel, `j` is the vertical level (0 = top), `k` the depth plane.
#[derive(Clone, Copy, Default, PartialEq)]
struct Cell {
    x: i32,
    j: i32,
    k: i32,
}

#[derive(Clone, Default)]
struct Run {
    seg: Vec<Cell>,
    at: Cell,
    dir: u8,
    due: f32,
    seed: u32,
    /// Set on any frame a new segment was laid, so that segment glows `hot` for exactly that frame.
    grew: bool,
}

#[derive(Default)]
pub struct Pipes {
    /// The lattice width the runs were built for. Separate from `fit`, which is height-derived only.
    nx: i32,
    /// The projection this family is currently fitted to, so a resize can be noticed - see `Fit`.
    fit: Option<Fit>,
    runs: Vec<Run>,
    onset: crate::dsp::onset::Flux,
    flourish: crate::dsp::flourish::Trigger,
    reset: crate::dsp::flourish::Envelope,
    /// Milliseconds the teapot has left at the tip; set to `TEAPOT_MS` on a flourish.
    teapot_ms: f32,
    /// The top-left the teapot was last drawn at, for the test to find it. `None` when not showing.
    teapot_at: Option<(i32, i32)>,
    /// Every segment of every run for this frame, depth-sorted before painting. Kept on the struct and
    /// only ever cleared, so after the first frame `draw` does not allocate - see `Seg`.
    list: Vec<Seg>,
}

/// One segment queued for painting: its two cells, which run (and so which hue) it belongs to, whether
/// it is the tip laid this frame, whether a ball joint sits on its far cell, and a sort key that is
/// unique per segment, so the UNSTABLE (allocation-free) sort still paints in a fixed order.
#[derive(Clone, Copy)]
struct Seg {
    from: Cell,
    to: Cell,
    run: usize,
    hot_tip: bool,
    joint: bool,
    key: (i32, usize, usize),
}

/// The widest cross-section a tube can be drawn at: `rad.clamp(.., 6.0)` gives rr <= 6, so 13 px, and a
/// joint is one wider again.
const PROFILE_MAX: usize = 15;

/// The shading across a tube's cross-section, from the side facing the light (index 0) to the side
/// facing away. This is what makes a pipe a CYLINDER rather than a ribbon: a rim, a hard 1px specular
/// stripe one pixel in from the lit edge, the body, then a falloff into a shaded underside. Written into
/// a fixed array so the per-sample paint is a table lookup.
fn profile(out: &mut [Rgba; PROFILE_MAX], n: usize, body: Rgba, panel: Rgba) -> usize {
    let n = n.clamp(1, PROFILE_MAX);
    let white = Rgba::new(255, 255, 255, 255);
    let black = Rgba::new(0, 0, 0, 255);
    let spec = Rgba::lerp_linear(body, white, 0.55);
    let under = Rgba::lerp_linear(body, panel, 0.62);
    for (u, px) in out.iter_mut().enumerate().take(n) {
        let t = if n > 1 { u as f32 / (n - 1) as f32 } else { 0.5 };
        *px = if n >= 3 && u == n - 1 {
            under
        } else if (n >= 5 && u == 1) || (n < 5 && u == 0) {
            spec
        } else if u == 0 {
            Rgba::lerp_linear(body, white, 0.18) // the rim, above the stripe
        } else {
            // Past the middle the body turns away from the light.
            Rgba::lerp_linear(body, black, ((t - 0.45) * 0.9).clamp(0.0, 0.45))
        };
    }
    n
}

fn resp(level: f32, sensitivity: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    let x = ((level - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0);
    (x.powf(LEVEL_GAMMA) * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

fn rand01(seed: u32, n: u32) -> f32 {
    let mut v = seed ^ n.wrapping_mul(0x9e37_79b9);
    v ^= v << 13;
    v ^= v >> 17;
    v ^= v << 5;
    (v % 100_000) as f32 / 100_000.0
}

/// The projection FITTED TO THE PANEL HEIGHT, rather than hard-coded for one.
///
/// This family drew NOTHING on any panel shorter than about 49 rows, and it was arithmetic rather than a
/// bug in the gate: with `F`, `Y_TOP`, `NY` and `NZ` all fixed, the lowest row anything can be drawn on is
/// `CY + F*(Y_TOP+NY)/Z_NEAR + R_PIPE*F/Z_NEAR` = 54.88, which fits the 58 interior rows of a 60px panel
/// and cannot fit the 46 of a 48px one. So it shed - correctly, by its own rule, and invisibly, because
/// shedding looks exactly like a black screen. Measured before this change: 4594 lit pixels at 380x60 and
/// 0 at 380x48, 380x40, 380x34 and 380x30.
///
/// The panel's height is not ours to choose. It follows the Widgets button, so it moves with the
/// taskbar's size, the monitor and the DPI - 48 rows at 100% is as ordinary as 60 at 125%.
///
/// SHRINKING THE FOCAL LENGTH ALONE DOES NOT WORK, and that is the whole difficulty. The depth planes are
/// separated by `Y_TOP * f * (1/z - 1/(z+1))`, which is smallest at the far end; at eight planes and
/// f = 32 that is 1.16px, already within a whisker of the 1.15px floor the module note derives. Scale f
/// down for a shorter panel and the far planes land on the same integer row, so the lattice stops reading
/// as depth at all - which is worse than shedding, because it looks like it is working.
///
/// So the DEPTH COUNT comes down with the height: this walks `nz` from the tuned maximum downwards and
/// takes the first count whose far-end gap still clears the floor. A short panel gets a shallower lattice,
/// which is the honest trade - fewer planes, all of them legible - and the fitted f is capped at the tuned
/// `F` so a taller panel keeps exactly the look it has now rather than growing into something untested.
#[derive(Clone, Copy, PartialEq)]
struct Fit {
    f: f32,
    nz: i32,
    cy: f32,
}

/// The tuned focal length, which is now the CEILING rather than the value - see `Fit`.
///
/// 32 is what the module note's depth-distinctness inequality solves to, and it is ISOTROPIC: the same
/// number on both axes. A separate `fy` chosen to "fill the panel vertically" would be an anamorphic
/// squeeze - vertical parallax would stop matching horizontal, which is exactly the class of fake that
/// got the isometric version rejected. Fitting scales BOTH axes together for the same reason.
const F_MAX: f32 = 32.0;

/// The depth counts the fit will consider, deepest first. `NZ_MAX` gives the 8 planes the family was
/// tuned with; `NZ_MIN` is 4 planes, below which the lattice has no depth left worth projecting.
const NZ_MAX: i32 = 7;
const NZ_MIN: i32 = 3;

/// The smallest far-end gap between depth planes that still reads as two planes, in pixels.
///
/// 1.15 is the module note's own `g_min`, which is what `F = 32` was originally solved for. Keeping the
/// same number here means a fitted projection is held to exactly the standard the hand-tuned one was.
const G_MIN: f32 = 1.15;

/// The thinnest the FAR pipe may be projected, in pixels.
///
/// The second legibility criterion, and the fit enforced only the first for one run: at h=30 it happily
/// produced a lattice whose depth planes were well separated and whose far pipe was 0.806px, which is not
/// a tube, it is a dotted line. A radius is a half-width, so 0.9 is a pipe about 1.8px across - the least
/// that still reads as round rather than as a scratch.
///
/// It binds at the short end, where the row budget forces `f` down: at h=30 it is what rejects a 5-plane
/// lattice in favour of a 4-plane one with a thicker far pipe. Trading depth for legibility, again, and
/// for the same reason - a plane you cannot see is not depth.
const R_MIN_FAR: f32 = 0.9;

fn fit_to(h: i32) -> Option<Fit> {
    // The interior rows the lattice may use: 3 .. h-3.
    let avail = (h - 6) as f32;
    if avail < 12.0 {
        return None;
    }
    let mut nz = NZ_MAX;
    while nz >= NZ_MIN {
        let zf = Z_NEAR + nz as f32;
        // Rows consumed per unit of focal length: the near bottom level plus its radius, less the far
        // top level.
        let per_f = (Y_TOP + NY as f32 + R_PIPE) / Z_NEAR - Y_TOP / zf;
        if per_f > 0.0 {
            let f = (avail / per_f).min(F_MAX);
            let gap = Y_TOP * f * (1.0 / (zf - 1.0) - 1.0 / zf);
            let far_r = R_PIPE * f / zf;
            if gap >= G_MIN && far_r >= R_MIN_FAR {
                let span = f * per_f;
                let top = Y_TOP * f / zf;
                // The principal-point row, derived rather than chosen - as the fixed `CY` was before it,
                // and for the same reason: a UNIFORM shift preserves every inter-plane gap, so moving
                // the whole block cannot break the distinctness the gap check just established. It comes
                // out negative on a tall panel, i.e. above the canvas, which is what puts the camera
                // over the lattice rather than inside it.
                // Centred in the interior, so a fitted panel is not top-heavy.
                let cy = 3.0 + (avail - span) * 0.5 - top;
                if f.is_finite() && cy.is_finite() {
                    return Some(Fit { f, nz, cy });
                }
            }
        }
        nz -= 1;
    }
    None
}

/// The six lattice directions: -x, +x, up, down, nearer, further.
const DIRS: [(i32, i32, i32); 6] =
    [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];

/// A cell's eye-space position. `nx` centres the lattice on the camera's x axis.
fn eye(c: Cell, nx: i32) -> (f32, f32, f32) {
    (
        c.x as f32 - (nx - 1) as f32 * 0.5,
        -(Y_TOP + c.j as f32),
        Z_NEAR + c.k as f32,
    )
}

/// Whether an eye-space point is safely in front of the near plane.
///
/// `>=` and not `!(z < Z_NEAR)`: for a NaN `z` this returns FALSE and the point is rejected, where the
/// negated form returns TRUE and leaks a NaN into the divide.
fn in_front(z: f32) -> bool {
    z >= Z_NEAR.max(Z_MIN)
}

/// Projects an eye-space point. `None` when it is not safely in front - the caller must skip it.
///
/// Returns the projected column, row, and the projected pipe radius at that depth.
fn project(cx: f32, x: f32, y: f32, z: f32, fit: Fit) -> Option<(i32, i32, f32)> {
    if !in_front(z) || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let inv = fit.f / z;
    let col = cx + x * inv;
    let row = fit.cy - y * inv;
    if !col.is_finite() || !row.is_finite() {
        return None;
    }
    // The clamp bounds the arithmetic; the clip above is what bounds the coordinate.
    Some((
        (col.round() as i32).clamp(-COORD_LIMIT, COORD_LIMIT),
        (row.round() as i32).clamp(-COORD_LIMIT, COORD_LIMIT),
        R_PIPE * inv,
    ))
}

/// A filled disc, for the ball joints at every turn. Same scanline form the orbit family uses; the
/// +0.5 bias keeps the outline round rather than square at the small radii these joints run at.
fn disc(c: &mut Canvas, col: i32, row: i32, r: i32, colour: Rgba) {
    if r <= 0 {
        c.fill_rect(col, row, 1, 1, colour);
        return;
    }
    for dy in -r..=r {
        let dx = (((r * r - dy * dy) as f32).max(0.0).sqrt() + 0.5) as i32;
        c.fill_rect(col - dx, row + dy, dx * 2 + 1, 1, colour);
    }
}

impl Run {
    fn restart(&mut self, nx: i32, nz: i32) {
        self.seg.clear();
        self.at = Cell {
            x: (rand01(self.seed, 11) * nx as f32) as i32 % nx.max(1),
            j: (rand01(self.seed, 13) * (NY + 1) as f32) as i32 % (NY + 1),
            k: (rand01(self.seed, 17) * (nz + 1) as f32) as i32 % (nz + 1).max(1),
        };
        self.dir = 1;
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    }

    fn inside(c: Cell, nx: i32, nz: i32) -> bool {
        (0..nx).contains(&c.x) && (0..=NY).contains(&c.j) && (0..=nz).contains(&c.k)
    }

    fn step(c: Cell, dir: u8) -> Cell {
        let d = DIRS[dir as usize];
        Cell { x: c.x + d.0, j: c.j + d.1, k: c.k + d.2 }
    }

    /// Turns on an onset; otherwise carries straight on until the lattice wall.
    fn steer(&mut self, turn: bool, nx: i32, nz: i32) {
        if !turn && Self::inside(Self::step(self.at, self.dir), nx, nz) {
            return;
        }
        // Never a straight reversal - a pipe doubling back on itself reads as a mistake, not a corner.
        let back = self.dir ^ 1;
        let start = (rand01(self.seed, self.seg.len() as u32 * 7 + 5) * 6.0) as u8 % 6;
        for i in 0..6u8 {
            let cand = (start + i) % 6;
            if cand != back && Self::inside(Self::step(self.at, cand), nx, nz) {
                self.dir = cand;
                return;
            }
        }
    }

    fn grow(&mut self, drive: f32, turn: bool, dt: f32, nx: i32, nz: i32) {
        // A cell outside the lattice cannot recover on its own, so this is the backstop rather than a
        // nicety: `steer` only redirects to a neighbour that is INSIDE, so with the current cell outside
        // there is no valid candidate, `dir` is left unchanged, and the step below is rejected. The run
        // freezes for good and nothing else in the family notices.
        //
        // The resize guard in `draw` is what should normally prevent this; this holds the invariant
        // whatever the cause, which matters because the guard has already been wrong once by covering
        // only one of the two axes.
        if !Self::inside(self.at, nx, nz) {
            self.restart(nx, nz);
        }
        let period = (GROW_SLOW_MS + (GROW_FAST_MS - GROW_SLOW_MS) * drive.clamp(0.0, 1.0)).max(30.0);
        self.due += dt;
        if !self.due.is_finite() {
            self.due = 0.0;
        }
        self.due = self.due.min(period * 4.0);
        let mut pending = turn;
        self.grew = false;
        while self.due >= period {
            self.due -= period;
            self.steer(pending, nx, nz);
            pending = false;
            let next = Self::step(self.at, self.dir);
            if !Self::inside(next, nx, nz) {
                continue;
            }
            self.at = next;
            self.seg.push(next);
            self.grew = true;
            // Past 60% of the cell budget the tail dissolves segment by segment - see KEEP_SEG.
            if self.seg.len() > KEEP_SEG {
                self.seg.remove(0);
            }
        }
    }
}

impl Family for Pipes {
    fn id(&self) -> &'static str {
        "pipes"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let reset = self.reset.update(fired, dt, RESET_MS);
        // The teapot rides the tip for TEAPOT_MS from the flourish, then continues - see TEAPOT.
        if fired {
            self.teapot_ms = TEAPOT_MS;
        }
        self.teapot_ms = (self.teapot_ms - dt).max(0.0);
        self.teapot_at = None;

        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);

        // Height is the gate: the row budget is fixed by F, Y_TOP, NY and NZ. Width only decides how
        // many cells fit ACROSS, and x costs no rows at all - which is exactly what isometric could not
        // offer, since there the x axis fed the height too.
        // The LOWEST row anything can be drawn on: the bottom level at the near plane, plus its radius.
        // Compared against the interior, not against h.
        //
        // Getting this wrong drew NOTHING at 380x60 on the first run. The earlier version summed the
        // extent from zero and demanded 63 rows of a 60px panel - but CY is NEGATIVE, four rows ABOVE
        // the canvas, so the real footprint is rows 6.6..54.9 and fits with room to spare. The bug was
        // in the gate, not in the projection.
        // FITTED to the height rather than gated against it - see `Fit`. This used to compute the lowest
        // drawable row from fixed constants and shed if it did not fit, which meant a black panel on any
        // height below about 49 rows - measured 0 lit pixels at 48, 40, 34 and 30.
        let Some(fit) = fit_to(h) else {
            return; // genuinely too short for even the shallowest lattice
        };
        let near_step = fit.f / Z_NEAR;
        let nx = ((w - 6) as f32 / near_step).floor() as i32;
        if nx < 4 {
            return; // too narrow to hold a lattice
        }
        // A resize invalidates cells in BOTH axes, and this guard only covered one of them.
        //
        // `Fit` is derived entirely from the HEIGHT, so a width-only change leaves it equal and this
        // never fired - and the run count below (then `nx / 8` clamped to 4; now `nx / 4` clamped to
        // 3) is the same at both 380px (nx 46) and 330px (nx 40), so that check did not fire either. A run left at `at.x = 44`
        // with 40 columns is then stranded PERMANENTLY: `steer` finds none of its six candidates inside,
        // so `dir` is left as it was, and `grow` rejects the step it takes. One pipe froze into
        // a static streak while the others kept growing.
        if self.fit != Some(fit) || self.nx != nx {
            self.fit = Some(fit);
            self.nx = nx;
            self.runs.clear();
        }
        let cx = w as f32 * 0.5;

        let runs = ((nx as usize) / 4).clamp(1, MAX_RUNS);
        if self.runs.len() != runs {
            self.runs = (0..runs)
                .map(|i| {
                    let mut r = Run {
                        seed: 0x9e37_79b9 ^ (i as u32).wrapping_mul(0x85eb_ca6b),
                        ..Run::default()
                    };
                    r.restart(nx, fit.nz);
                    r
                })
                .collect();
            // Every segment of every run, plus a tip each - reserved once so `list` never grows in draw.
            self.list = Vec::with_capacity(MAX_RUNS * (MAX_SEG + 2));
            for r in &mut self.runs {
                r.seg.reserve(MAX_SEG + 2);
            }
        }
        // Deliberately NOT a restart - see `RESET_MS`. Clearing the runs was the reported jarring cut.

        let turn = self.onset.update(&d.levels, dt, 2.8, 200.0);
        let bands = d.levels.len();

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let key = Rgba::from_hex(&t.panel, 1.0);
        let blend = |a: Rgba, b: Rgba, k: f32| {
            let m = |p: u8, q: u8| (p as f32 * (1.0 - k) + q as f32 * k) as u8;
            Rgba::new(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b), 255)
        };
        // THREE pipes, each in its own hue: `lit`, `hot`, and the linear midpoint of the two. Keyed on
        // the run index so the trio is stable rather than shuffling, which is what lets several runs
        // read as several pipes rather than as copies of one signal.
        let mid = Rgba::lerp_linear(lit, hot, 0.5);
        let hues = [lit, hot, mid];

        // Grow, then gather every segment of every run so they can be painted FAR TO NEAR across runs
        // as well as within one. Sorting on k descending IS the occlusion - no z-buffer - and it is only
        // correct because the projection keeps distinct depth planes on distinct rows.
        //
        // Each entry carries its run (so its hue), whether it is the freshly-laid tip (glows `hot` for one
        // frame), and whether its far cell is a turn (a ball joint sits there).
        self.list.clear();
        for ri in 0..runs {
            let lo = (ri * bands) / runs;
            let hi = (((ri + 1) * bands) / runs).clamp(lo + 1, bands);
            let band = d.levels[lo..hi].iter().fold(0.0f32, |m, &v| m.max(v));
            // The surge scales the RATE, never the geometry, so a flourish cannot move anything
            // discontinuously - it only makes the next segment arrive sooner.
            let drive = (resp(band, t.sensitivity) + reset * (SURGE_RATE - 1.0)).clamp(0.0, 1.0);
            self.runs[ri].grow(drive, turn, dt, nx, fit.nz);
            let grew = self.runs[ri].grew;
            let seg = &self.runs[ri].seg;
            let n = seg.len();
            for i in 0..n {
                let from = if i == 0 { seg[i] } else { seg[i - 1] };
                // A turn: the direction into this cell differs from the direction out of it. The tip
                // also gets a joint so a run ends in a rounded cap rather than a flat face.
                let joint = if i > 0 && i + 1 < n {
                    let d1 = (seg[i].x - seg[i - 1].x, seg[i].j - seg[i - 1].j, seg[i].k - seg[i - 1].k);
                    let d2 = (seg[i + 1].x - seg[i].x, seg[i + 1].j - seg[i].j, seg[i + 1].k - seg[i].k);
                    d1 != d2
                } else {
                    i + 1 == n
                };
                let hot_tip = grew && i == n - 1;
                self.list.push(Seg { from, to: seg[i], run: ri, hot_tip, joint, key: (-seg[i].k, ri, i) });
            }
        }
        // Unstable: `sort_by_key` is a merge sort that allocates a buffer. The key is unique per segment,
        // so the order is as fixed as a stable sort's would be.
        self.list.sort_unstable_by_key(|s| s.key);

        // Painted one DEPTH PLANE at a time: every keyline in the plane, then every tube in it. The keyline
        // is what separates crossing pipes (the trick chroma established), but drawn per segment it cut a
        // notch into the end of the previous segment of the SAME pipe, so a far pipe read as a dotted line.
        // Per plane, a pipe only outlines itself against what is behind it.
        let white = Rgba::new(255, 255, 255, 255);
        let mut prof = [Rgba::new(0, 0, 0, 0); PROFILE_MAX];
        let mut g0 = 0;
        while g0 < self.list.len() {
            let k = self.list[g0].to.k;
            let mut g1 = g0;
            while g1 < self.list.len() && self.list[g1].to.k == k {
                g1 += 1;
            }
            for pass in 0..2 {
                for gi in g0..g1 {
                    let sg = self.list[gi];
                    let (fx, fy, fz) = eye(sg.from, nx);
                    let (tx, ty, tz) = eye(sg.to, nx);
                    let (Some((c0, r0, rad0)), Some((c1, r1, rad1))) =
                        (project(cx, fx, fy, fz, fit), project(cx, tx, ty, tz, fit))
                    else {
                        continue;
                    };
                    let steps = (c1 - c0).abs().max((r1 - r0).abs()).max(1);
                    // A run of overlapping squares along the segment. Sampling every pixel over-draws - a
                    // 7px square advancing 1px repaints 6 columns it already owns - so it strides by the
                    // thinnest radius on the segment, which still leaves no gap.
                    let rr_min = rad0.min(rad1).clamp(0.6, 6.0).round().max(1.0) as i32;
                    let stride = rr_min.max(1);
                    let at = |s: i32| {
                        let col = c0 + (c1 - c0) * s / steps;
                        let row = r0 + (r1 - r0) * s / steps;
                        let rad = rad0 + (rad1 - rad0) * s as f32 / steps as f32;
                        let rr = rad.clamp(0.6, 6.0).round().max(1.0) as i32;
                        (col, row, rr)
                    };
                    let joint_r = rad1.clamp(0.6, 6.0).round().max(1.0) as i32 + 1;
                    if pass == 0 {
                        let mut s = 0;
                        loop {
                            let (col, row, rr) = at(s);
                            c.fill_rect(col - rr - 1, row - rr - 1, rr * 2 + 3, rr * 2 + 3, key);
                            if s == steps {
                                break;
                            }
                            s = (s + stride).min(steps);
                        }
                        if sg.joint {
                            disc(c, c1, r1, joint_r + 1, key);
                        }
                        continue;
                    }

                    // Depth shading AS WELL AS the size taper. The far end is already 2.75x smaller; dimming
                    // it too is what stops the back of the lattice reading as clutter. The surge warms every
                    // pipe toward `hot` in proportion to the envelope - a blend, so the three hues survive
                    // it; the freshly-laid tip glows `hot` outright for its one frame.
                    let far01 = (k as f32 / fit.nz.max(1) as f32).clamp(0.0, 1.0);
                    let hue = hues[sg.run % hues.len()];
                    let base = if sg.hot_tip { hot } else { Rgba::lerp_linear(hue, hot, reset.clamp(0.0, 1.0) * 0.6) };
                    let body = blend(base, panel, far01 * 0.55);
                    // Lit from above-left. A mostly-vertical tube is shaded ACROSS ITS COLUMNS (stripe on
                    // the left); anything else across its rows (stripe on top). Shading a vertical tube by
                    // rows striped it like a ladder, one band per sample square.
                    let vertical = (r1 - r0).abs() > (c1 - c0).abs();
                    let mut s = 0;
                    loop {
                        let (col, row, rr) = at(s);
                        let n = profile(&mut prof, (rr * 2 + 1) as usize, body, panel);
                        for (u, &px) in prof.iter().enumerate().take(n) {
                            let o = u as i32 - rr;
                            if vertical {
                                c.fill_rect(col + o, row - rr, 1, rr * 2 + 1, px);
                            } else {
                                c.fill_rect(col - rr, row + o, rr * 2 + 1, 1, px);
                            }
                        }
                        if s == steps {
                            break;
                        }
                        s = (s + stride).min(steps);
                    }
                    // The ball joint - a filled disc of radius+1 at the corner (and the tip), the round
                    // joint the real screensaver has, shaded top to bottom like the tubes, with a glint.
                    if sg.joint {
                        let r = joint_r;
                        let n = profile(&mut prof, (r * 2 + 1) as usize, body, panel);
                        for dy in -r..=r {
                            let dx = (((r * r - dy * dy) as f32).max(0.0).sqrt() + 0.5) as i32;
                            let u = ((dy + r) as usize).min(n - 1);
                            c.fill_rect(c1 - dx, r1 + dy, dx * 2 + 1, 1, prof[u]);
                        }
                        if r >= 3 {
                            c.fill_rect(c1 - r / 2, r1 - r / 2, 1, 1, Rgba::lerp_linear(body, white, 0.8));
                        }
                    }
                }
            }
            g0 = g1;
        }

        // The teapot: at the first run's growing tip, just above it in the not-yet-laid space, drawn
        // in `hot` for as long as the flourish gives it - our own 9x7 silhouette (see TEAPOT).
        if self.teapot_ms > 0.0 {
            if let Some(run) = self.runs.first() {
                if let Some(&tip) = run.seg.last() {
                    let (ex, ey, ez) = eye(tip, nx);
                    if let Some((tc, tr, _)) = project(cx, ex, ey, ez, fit) {
                        // Centred on the tip horizontally, lifted clear of it so the silhouette sits in
                        // panel rather than on top of the hot tube, and clamped inside the interior.
                        let ax = (tc - TEAPOT_W / 2).clamp(2, (w - 2 - TEAPOT_W).max(2));
                        let ay = (tr - TEAPOT_H - 3).clamp(3, (h - 3 - TEAPOT_H).max(3));
                        // A panel-colour keyline round the silhouette first, as the pipes have, so the
                        // teapot reads as an object in front of the lattice rather than a hot smudge on it.
                        for pass in 0..2 {
                            for (r, bits) in TEAPOT.iter().enumerate() {
                                for col in 0..TEAPOT_W {
                                    if bits & (1 << (TEAPOT_W - 1 - col)) != 0 {
                                        let (px, py) = (ax + col, ay + r as i32);
                                        if pass == 0 {
                                            c.fill_rect(px - 1, py - 1, 3, 3, key);
                                        } else {
                                            c.fill_rect(px, py, 1, 1, hot);
                                        }
                                    }
                                }
                            }
                        }
                        self.teapot_at = Some((ax, ay));
                    }
                }
            }
        }

        // The bloom composites each pixel OVER its own halo, so on an opaque panel every interior pixel
        // hides its halo completely, and the clip below then clears the only pixels that could show one
        // (the margin outside the panel). It was measured costing 1.17 of the 1.26 ms this family took at
        // 380x60 for no visible pixel. Kept for a hand-authored colourway with a translucent panel, which
        // is the one case it can still change anything.
        if t.panel_alpha < 1.0 {
            c.bloom(t.bloom as i32, t.glow_strength);
        }
        // Clip last: a joint disc or the bloom at the lattice's edge is one pixel from the rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn frame(gain: f32, t_s: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        let hit = ((t_s / 0.5).fract() < 0.07) as i32 as f32;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / crate::dsp::bands::NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.5) * 0.60 + 0.14;
            let wob = 1.0 + 0.35 * ((t_s * 2.1 + f * 6.0).sin());
            *v = ((shape * wob + hit * 0.5) * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = 0.30 * gain;
        d.rms_r = 0.27 * gain;
        d
    }

    /// THE anti-collapse guard, and the most important test this family owes.
    ///
    /// Asserted on the GEOMETRY rather than on pixels, exactly as `vapor`'s equivalent
    /// (`the_shipped_perspective_keeps_one_pixel_row_per_grid_line`) is: the property being defended is
    /// "two depth planes never share an integer row", and reading that off rendered pixels would mean
    /// inferring it from whatever happened to be drawn there.
    ///
    /// The mutation this catches is the one that matters most - putting the camera INSIDE the lattice.
    /// Set `Y_TOP` to 0 and the top level lands on the horizon, where every depth plane projects to the
    /// same row, which is vapor's measured failure reproduced deliberately.
    ///
    /// Now checked at EVERY height the family will draw at, not just 60. That is a stronger test than the
    /// one it replaces, and it is the test this bug needed: the projection is fitted per height, so a
    /// height whose fit collapsed two planes onto one row would look like it was working while silently
    /// having no occlusion. The whole point of trading depth planes for rows is that the planes that
    /// remain are legible, and this is what holds that promise.
    #[test]
    fn every_level_keeps_its_depth_planes_on_distinct_integer_rows_at_every_height() {
        let nx = 45;
        let mut heights_tested = 0;
        for h in 18..=120 {
            let Some(fit) = fit_to(h) else {
                continue;
            };
            heights_tested += 1;
            assert!(
                (NZ_MIN..=NZ_MAX).contains(&fit.nz),
                "h={h}: fitted depth {} is outside the allowed range",
                fit.nz
            );
            for j in 0..=NY {
                let mut rows: Vec<i32> = Vec::new();
                for k in 0..=fit.nz {
                    let (x, y, z) = eye(Cell { x: nx / 2, j, k }, nx);
                    let (_, row, _) = project(190.0, x, y, z, fit).expect("lattice must project");
                    rows.push(row);
                }
                let mut sorted = rows.clone();
                sorted.sort_unstable();
                sorted.dedup();
                assert_eq!(
                    sorted.len(),
                    rows.len(),
                    "h={h}, level j={j}: two of its {} depth planes share an integer row: {rows:?} - \
                     that is vapor's measured collapse, and it silently disables occlusion",
                    rows.len()
                );
            }
        }
        assert!(heights_tested > 80, "the height sweep barely ran: {heights_tested} heights");
        // The tuned height must still get the full depth it was designed with.
        assert_eq!(fit_to(60).unwrap().nz, NZ_MAX, "the 60px panel lost depth planes");
    }

    /// THE REGRESSION TEST. This family drew a BLACK PANEL on every height below about 49 rows, and it did
    /// so by shedding - which is indistinguishable from a crash from the outside, and is how it reached a
    /// user's machine unnoticed. It worked on a 125% DPI panel at 60px and not on a 100% one at 48px.
    ///
    /// Mutation: restore the old fixed-constant gate (`lowest > (h - 2)`) and the 48, 40, 34 and 30 cases
    /// go to zero lit pixels.
    /// Slow (~19.8s in debug); gated out of the default suite. The lattice is drawn on short panels and not only on tall ones. Run: `cargo test --release slow_ -- --ignored`.
    #[test]
    #[ignore]
    fn slow_the_lattice_is_drawn_on_short_panels_and_not_only_on_tall_ones() {
        let t = builtin::pipes_win95_teal();
        for (w, h) in [(380, 60), (380, 52), (380, 48), (380, 44), (380, 40), (380, 34), (380, 30),
                       (190, 48), (190, 60), (150, 40)] {
            let mut fam = Pipes::default();
            let mut c = Canvas::new(w, h);
            for k in 0..200 {
                fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            }
            let dark = Rgba::from_hex(&t.panel, 1.0);
            let mut lit = 0;
            for y in 0..h {
                for x in 0..w {
                    let px = c.get(x, y);
                    if px.a > 0 && (px.r, px.g, px.b) != (dark.r, dark.g, dark.b) {
                        lit += 1;
                    }
                }
            }
            assert!(lit > 200, "{w}x{h} drew almost nothing: {lit} lit pixels");
        }
    }

    /// A WIDTH-only resize must not strand a run outside the lattice.
    ///
    /// This is the hole the depth test did not cover. `Fit` is derived entirely from the HEIGHT, so a
    /// width change leaves it equal and the resize guard never fires - and the run count is
    /// `nx / 4` clamped to MAX_RUNS (3; it was `nx / 8` clamped to 4), the same at both 380px (nx 46)
    /// and 330px (nx 40), so that check does not fire either. A run sitting at `at.x = 45` is then outside `0..40` for good: `steer` finds
    /// none of its six candidates inside, so `dir` is left alone, and `grow` rejects the step it then
    /// takes. Nothing can ever move it again, and one of the pipes becomes a static streak.
    ///
    /// Mutation: drop `nx` from the resize guard AND the stranded-cell restart in `grow`, and the
    /// narrowed run stops growing while the others carry on.
    /// Slow (~8.2s in debug); gated out of the default suite. A width only resize does not strand a run outside the lattice. Run: `cargo test --release slow_ -- --ignored`.
    #[test]
    #[ignore]
    fn slow_a_width_only_resize_does_not_strand_a_run_outside_the_lattice() {
        let t = builtin::pipes_win95_teal();
        let mut fam = Pipes::default();
        let mut wide = Canvas::new(380, 60);
        for k in 0..300 {
            fam.draw(&mut wide, &t, &frame(0.62, k as f32 * 0.0167));
        }
        assert!(fam.runs.len() > 1, "the fixture did not build several runs");

        // Narrow WITHOUT changing the height, so `Fit` is unchanged.
        let mut narrow = Canvas::new(330, 60);
        for k in 0..40 {
            fam.draw(&mut narrow, &t, &frame(0.62, 5.0 + k as f32 * 0.0167));
        }
        // The segment CONTENTS, not the length. A run at MAX_SEG has a constant length while it is
        // still moving perfectly well - its oldest cell drops as a new one is pushed - so a
        // length-based check flags healthy runs as frozen. That mistake was in the first version of
        // this test, and it passed only incidentally: the width guard happened to clear every run, so
        // all four lengths reset and grew again.
        let before: Vec<Vec<Cell>> = fam.runs.iter().map(|r| r.seg.clone()).collect();
        for k in 0..200 {
            fam.draw(&mut narrow, &t, &frame(0.62, 6.0 + k as f32 * 0.0167));
        }

        // Every run must still be inside the lattice, and every run must still be moving.
        let nx = ((330 - 6) as f32 / (fit_to(60).unwrap().f / Z_NEAR)).floor() as i32;
        let nz = fit_to(60).unwrap().nz;
        for (i, run) in fam.runs.iter().enumerate() {
            assert!(
                Run::inside(run.at, nx, nz),
                "run {i} is stranded at x={} with only {nx} columns",
                run.at.x
            );
            for cell in &run.seg {
                assert!(
                    (0..nx).contains(&cell.x),
                    "run {i} has a segment at x={} outside 0..{nx}",
                    cell.x
                );
            }
        }
        let after: Vec<Vec<Cell>> = fam.runs.iter().map(|r| r.seg.clone()).collect();
        let frozen: Vec<usize> = after
            .iter()
            .zip(before.iter())
            .enumerate()
            .filter(|(_, (a, b))| a == b)
            .map(|(i, _)| i)
            .collect();
        assert!(
            frozen.is_empty(),
            "runs {frozen:?} were byte-identical over 200 frames - a stranded run cannot recover, so it stays that way for the rest of the session"
        );
    }

    /// A resize that SHALLOWS the lattice must not leave cells pointing at depth planes that no longer
    /// exist. This is the specific hazard the fit introduced, and it only bites on a resize - which is a
    /// routine production event here, since the panel follows the Widgets button.
    #[test]
    fn a_resize_that_changes_the_depth_count_does_not_strand_cells() {
        let t = builtin::pipes_win95_teal();
        let mut fam = Pipes::default();
        for (w, h) in [(380, 60), (380, 34), (380, 60), (190, 40), (380, 48), (120, 30), (380, 60)] {
            let mut c = Canvas::new(w, h);
            for k in 0..40 {
                fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            }
            let fit = fam.fit.expect("no fit after drawing");
            for run in &fam.runs {
                for cell in &run.seg {
                    assert!(
                        (0..=fit.nz).contains(&cell.k),
                        "{w}x{h}: a segment sits at depth {} with only {} planes",
                        cell.k,
                        fit.nz + 1
                    );
                }
            }
        }
    }

    /// The near clip, and the hang it exists to make impossible.
    ///
    /// A vertex at or behind the eye must be REJECTED, not projected. `canvas.rs` line() has no
    /// off-canvas early-out, so a saturated coordinate sends one edge on ~2.1e9 iterations - measured at
    /// 294.6ms. Mutation: change `in_front` to `z > 0.0` and the z=0.0 case divides by zero; change it
    /// to `!(z < Z_NEAR)` and the NaN case leaks through.
    #[test]
    fn a_vertex_at_or_behind_the_eye_is_rejected_rather_than_projected() {
        let fit = fit_to(60).expect("the tuned height must fit");
        for z in [0.0f32, -1.0, -1000.0, 0.5, f32::NAN, f32::NEG_INFINITY] {
            assert!(
                project(190.0, 1.0, -4.0, z, fit).is_none(),
                "z={z} was projected instead of clipped; that is the 294.6ms hang"
            );
        }
        // And a NaN in the lateral coordinate must not survive either.
        assert!(project(190.0, f32::NAN, -4.0, 6.0, fit).is_none(), "a NaN x was projected");
        // Everything the lattice actually contains must project, or the family would draw nothing. Every
        // fitted height, not just the tuned one - a short panel that failed to project would be the same
        // black screen this family already shipped once.
        for h in [30, 40, 48, 60, 96] {
            let Some(fit) = fit_to(h) else { continue };
            for k in 0..=fit.nz {
            let (x, y, z) = eye(Cell { x: 0, j: 0, k }, 45);
            let p = project(190.0, x, y, z, fit);
            assert!(p.is_some(), "h={h}: lattice depth k={k} failed to project");
            let (col, row, rad) = p.unwrap();
            assert!(col.abs() < COORD_LIMIT && row.abs() < COORD_LIMIT, "coordinate escaped the clamp");
            assert!(rad > 0.5, "h={h}: projected radius {rad} at k={k} is too thin to draw as a tube");
            }
        }
    }

    /// Perspective must actually taper: a near pipe is drawn fatter than a far one.
    ///
    /// This is what distinguishes the family from the isometric version it replaced, where every pipe
    /// was the same width. Mutation: make the radius a constant instead of `R_PIPE * F / z`.
    #[test]
    fn a_near_pipe_is_drawn_fatter_than_a_far_one() {
        // At every fitted height: a shallower lattice has less depth to taper across, so this is where a
        // fit that traded away too many planes would show up as a family that no longer reads as 3D.
        for h in [30, 40, 48, 60, 96] {
            let Some(fit) = fit_to(h) else { continue };
            let near = project(190.0, 0.0, -4.0, Z_NEAR, fit).unwrap().2;
            let far = project(190.0, 0.0, -4.0, Z_NEAR + fit.nz as f32, fit).unwrap().2;
            assert!(
                near > far * 1.6,
                "h={h}: near radius {near:.2} against far {far:.2} - under 1.6x will not read as depth"
            );
            assert!(far > 0.9, "h={h}: the far pipe at {far:.2}px is too thin to read as a tube");
        }
    }

    /// THREE pipes grow at once, each in its own hue - `lit`, `hot`, and their linear midpoint - so
    /// the panel reads as a lattice of several pipes rather than copies of one signal.
    ///
    /// Checked by hue, not by run count: a body pixel at depth `k` is a straight blend of its run's
    /// hue toward the panel, so it lies on that hue's line to the panel. The crown and underside
    /// shading fall OFF those lines, as does the panel, so a pixel on the midpoint line can only have
    /// come from a midpoint-hued run existing - which is the property this defends.
    ///
    /// Mutation: give every run one hue - the midpoint line loses all its pixels and the count drops
    /// below three.
    #[test]
    fn three_pipes_grow_at_once() {
        let t = builtin::pipes_win95_teal();
        let mut fam = Pipes::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..60 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        assert!(fam.runs.len() >= 3, "fewer than three runs were built: {}", fam.runs.len());
        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let mid = Rgba::lerp_linear(lit, hot, 0.5);
        let panel = Rgba::from_hex(&t.panel, 1.0);
        // Does `p` lie on the straight line from `hue` to `panel`, i.e. is it that hue at some depth?
        let on_line = |p: Rgba, hue: Rgba| -> bool {
            let span = (hue.r as i32 - panel.r as i32).abs()
                + (hue.g as i32 - panel.g as i32).abs()
                + (hue.b as i32 - panel.b as i32).abs();
            if span < 30 {
                return false;
            }
            // Solve the blend fraction from whichever channel separates the hue from the panel most,
            // then confirm the other two channels agree.
            let (h0, p0, c0) = [
                (hue.r as f32, panel.r as f32, p.r as f32),
                (hue.g as f32, panel.g as f32, p.g as f32),
                (hue.b as f32, panel.b as f32, p.b as f32),
            ]
            .into_iter()
            .max_by(|a, b| (a.0 - a.1).abs().partial_cmp(&(b.0 - b.1).abs()).unwrap())
            .unwrap();
            let a = ((c0 - h0) / (p0 - h0)).clamp(0.0, 1.0);
            let fit = |hc: u8, pc: u8, pv: u8| {
                (hc as f32 + (pc as f32 - hc as f32) * a - pv as f32).abs() <= 4.0
            };
            fit(hue.r, panel.r, p.r) && fit(hue.g, panel.g, p.g) && fit(hue.b, panel.b, p.b)
        };
        let (mut saw_lit, mut saw_hot, mut saw_mid) = (false, false, false);
        for y in 4..56 {
            for x in 4..376 {
                let p = c.get(x, y);
                if p.a == 0 {
                    continue;
                }
                // A pixel very near the panel is ambiguous (every hue's line ends there); skip it.
                if (p.r as i32 - panel.r as i32).abs()
                    + (p.g as i32 - panel.g as i32).abs()
                    + (p.b as i32 - panel.b as i32).abs()
                    < 24
                {
                    continue;
                }
                if on_line(p, mid) {
                    saw_mid = true;
                } else if on_line(p, lit) {
                    saw_lit = true;
                } else if on_line(p, hot) {
                    saw_hot = true;
                }
            }
        }
        let n = saw_lit as u32 + saw_hot as u32 + saw_mid as u32;
        assert!(
            n >= 3,
            "fewer than three hues drawn: lit={saw_lit} hot={saw_hot} mid={saw_mid}"
        );
    }

    /// A near pipe is FAT and ROUND: a front-depth cross-section is at least 5px across and has a row
    /// lighter than its body - the specular stripe that gives the tube volume - on the side facing the
    /// light.
    ///
    /// Measured within ONE cross-section: a column's contiguous run of non-panel pixels 5..=9 tall is a
    /// horizontal tube's thickness (the keyline, which is the panel colour, bounds it). Its brightest
    /// pixel must beat the run's median by a clear margin and sit in the run's upper half. Required on
    /// many columns, so a single joint glint or two touching pipes cannot pass it.
    ///
    /// The first version of this test took the luminance range over a fixed 9-row window, which spans
    /// neighbouring pipes at different depth shades and passed with the tube shading deleted.
    ///
    /// Mutation: make `profile` return the flat body for every row (or thin the pipe with a smaller
    /// `R_PIPE`) - no cross-section has a lit stripe, or none is 5px, and this fails.
    #[test]
    fn pipes_are_fat_with_a_highlight() {
        let t = builtin::pipes_win95_teal();
        let mut fam = Pipes::default();
        let (w, h) = (380, 60);
        let mut c = Canvas::new(w, h);
        for k in 0..120 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let lum = |p: Rgba| 0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32;
        let non_panel = |p: Rgba| {
            p.a > 0
                && ((p.r as i32 - panel.r as i32).abs()
                    + (p.g as i32 - panel.g as i32).abs()
                    + (p.b as i32 - panel.b as i32).abs())
                    > 24
        };
        let mut good = 0;
        for x in 4..w - 4 {
            let mut y = 3;
            while y < h - 3 {
                if !non_panel(c.get(x, y)) {
                    y += 1;
                    continue;
                }
                let y0 = y;
                while y < h - 3 && non_panel(c.get(x, y)) {
                    y += 1;
                }
                let n = y - y0;
                if (5..=9).contains(&n) {
                    let mut ls = [0.0f32; 9];
                    for (i, l) in ls.iter_mut().enumerate().take(n as usize) {
                        *l = lum(c.get(x, y0 + i as i32));
                    }
                    let run = &mut ls[..n as usize];
                    let (imax, lmax) =
                        run.iter().enumerate().fold((0, f32::MIN), |b, (i, &l)| if l > b.1 { (i, l) } else { b });
                    run.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let median = run[run.len() / 2];
                    if lmax - median >= 12.0 && (imax as i32) < n / 2 {
                        good += 1;
                    }
                }
            }
        }
        assert!(
            good >= 12,
            "only {good} fat (5-9px) cross-sections have a lit stripe on the light side - the tubes read flat"
        );
    }

    /// The teapot appears at the growing tip on a flourish, drawn in `hot`, within three frames.
    ///
    /// Its 9x7 silhouette is matched at the anchor the family recorded: every set bit is `hot`, and at
    /// least one unset bit is NOT - so a solid hot region (which the surge paints on the near tube)
    /// cannot pass for the teapot, and neither can the empty panel with the teapot deleted.
    ///
    /// Mutation: never draw the teapot - the anchor is never set and no silhouette is found.
    #[test]
    fn teapot_appears_on_the_flourish() {
        let t = builtin::pipes_win95_teal();
        let mut fam = Pipes::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..80 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let is_hot = |p: Rgba| {
            p.a > 200
                && (p.r as i32 - hot.r as i32).abs() <= 6
                && (p.g as i32 - hot.g as i32).abs() <= 6
                && (p.b as i32 - hot.b as i32).abs() <= 6
        };
        fam.flourish.force_next();
        let mut ok = false;
        for k in 80..83 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            let Some((ax, ay)) = fam.teapot_at else { continue };
            let (mut all_set_hot, mut any_unset_cold) = (true, false);
            for (r, bits) in TEAPOT.iter().enumerate() {
                for col in 0..TEAPOT_W {
                    let set = bits & (1 << (TEAPOT_W - 1 - col)) != 0;
                    let p = c.get(ax + col, ay + r as i32);
                    if set && !is_hot(p) {
                        all_set_hot = false;
                    }
                    if !set && !is_hot(p) {
                        any_unset_cold = true;
                    }
                }
            }
            if all_set_hot && any_unset_cold {
                ok = true;
                break;
            }
        }
        assert!(ok, "the teapot silhouette never appeared at the tip on the flourish");
    }

    /// Run: cargo test --release probe_pipes_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_pipes_cost() {
        let t = builtin::pipes_win95_teal();
        let mut fam = Pipes::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..120 {
            fam.draw(&mut c, &t, &frame(0.8, k as f32 * 0.0167));
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            fam.draw(&mut c, &t, &frame(0.8, k as f32 * 0.0167));
        }
        println!("pipes: {:.3} ms/frame at 380x60", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }

    /// Run: cargo test --release dump_pipes -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_pipes() {
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
        for t in builtin::all().into_iter().filter(|t| t.family == "pipes") {
            let mut fam = Pipes::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..700 {
                fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            }
            write(format!("pipes-{}", t.id), &c);
        }
        let t = builtin::pipes_win95_teal();
        let mut fam = Pipes::default();
        let mut c = Canvas::new(380, 60);
        let mut shot = 0;
        for k in 0..1200 {
            fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            if k >= 150 && (k - 150) % 110 == 0 && shot < 5 {
                write(format!("pipes-grow-{shot}"), &c);
                shot += 1;
            }
        }
        println!("wrote pipes dumps");
        // The short panels too - this family shipped a black screen on all of them. Padded into a 60-row
        // buffer so the review stacker can show them beside the tall one.
        for hh in [52, 48, 40, 34, 30] {
            let t = builtin::pipes_win95_teal();
            let mut fam = Pipes::default();
            let mut src = Canvas::new(380, hh);
            for k in 0..300 {
                fam.draw(&mut src, &t, &frame(0.62, k as f32 * 0.0167));
            }
            let mut out = Vec::new();
            for y in 0..60 {
                for x in 0..380 {
                    let px = if y < hh { src.get(x, y) } else { Rgba::new(0, 0, 0, 0) };
                    out.extend_from_slice(&[px.r, px.g, px.b, px.a]);
                }
            }
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
            std::fs::write(dir.join(format!("pipes-h{hh}.rgba")), &out).unwrap();
        }

        // The fidelity-pass eye test: calm / loud / flourish (the teapot, a few frames in) for two
        // colourways at the wide size and at 128x44, so fat pipes, the highlight and underside, the
        // ball joints and three concurrent pipes can be judged against the glow-and-depth families.
        for tid in ["pipes-win95-teal", "pipes-neon-magenta"] {
            let theme = builtin::all().into_iter().find(|t| t.id == tid).unwrap();
            for (w, hh) in [(380, 60), (128, 44)] {
                for (tag, gain) in [("calm", 0.30), ("loud", 0.85)] {
                    let mut fam = Pipes::default();
                    let mut c = Canvas::new(w, hh);
                    for k in 0..500 {
                        fam.draw(&mut c, &theme, &frame(gain, k as f32 * 0.0167));
                    }
                    write(format!("{tid}-{tag}-{w}x{hh}"), &c);
                }
                let mut fam = Pipes::default();
                let mut c = Canvas::new(w, hh);
                for k in 0..400 {
                    fam.draw(&mut c, &theme, &frame(0.62, k as f32 * 0.0167));
                }
                fam.flourish.force_next();
                for k in 400..412 {
                    fam.draw(&mut c, &theme, &frame(0.62, k as f32 * 0.0167));
                }
                write(format!("{tid}-flourish-{w}x{hh}"), &c);
            }
        }

    }

}
