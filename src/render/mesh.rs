//! The 3D spectrum family: extruded bars staggered into depth, where depth is TIME.
//!
//! The Winamp / Windows Media Player "bars in depth" display. Rows of solid bars marching up and to
//! the right, the nearest row bright and crisp, each row behind it dimmer and drawn first so the near
//! bars occlude it. Every row is a spectrum snapshot from a moment ago, so a transient visibly walks
//! backwards into the display.
//!
//! # Why this shape of 3D and not the obvious one
//!
//! A feasibility investigation (nine agents, three of them adversarial) killed the design it first
//! recommended - an extruded EQ curve given depth by one constant offset - and the reasoning is worth
//! keeping, because it is what makes THIS family the survivor:
//!
//! **Perspective is refused.** Depth steps and amplitude compete for the same ~48 usable rows, and
//! amplitude is the channel the eye reads as a meter. `vapor` measured the wall: at its tuned
//! `persp = 2.07`, SEVEN of sixteen depth lines collapsed onto rows 28-29, which also silently
//! disabled occlusion - lines sharing an integer row cannot occlude each other. There is a hang in it
//! too: `canvas.rs:624-650` Bresenham breaks only on reaching its endpoint, so a vertex projecting near
//! the eye saturates `as i32` to 2147483647 and one edge iterates ~2.1e9 times, measured at 294.6ms.
//! Oblique geometry with constant integer offsets has no divide, no near clip and nothing to collapse.
//!
//! **But oblique is not automatically safe, and this is the part that killed the other design.** For a
//! SWEPT SURFACE - a curve extruded by an offset - the visible depth face at column x is
//! `dy - (disp(x) - disp(x - dx))`: the offset MINUS the curve's own rise over the run. It reaches zero
//! where the curve's local slope equals `dy/dx` and inverts beyond, which is the same collapse in new
//! coordinates. Discrete boxes have no rise-over-run term, so the failure cannot occur here.
//!
//! **And the depth is not informationally empty**, which was the other fatal objection. A constant
//! offset applied to the SAME data is an affine translation - the render would carry one shape and a
//! shifted copy of it, no parallax, no more information than a drop shadow. Here each depth row holds
//! DIFFERENT data: a snapshot from further back in time. The offset is constant; the content is not.
//!
//! # Depth is time, and the ring has three properties that matter
//!
//! Learned from `vapor`, whose history ring is the closest thing in the tree:
//!
//! 1. **Rotate on an interval, not per frame.** Per-frame rotation makes the depth axis span half a
//!    second and the marching stops reading as motion.
//! 2. **PEAK-HOLD between rotations.** The ring samples at a few hertz, so a snare landing between two
//!    rotations is simply dropped unless the accumulator holds the maximum. Averaging loses it too.
//! 3. **Brightness carries DEPTH, never level.** Level is bar height - position - because `tube.rs`
//!    measured a driven element 1.46 dL* brighter than its neighbour as invisible against a ~2.3 dL*
//!    threshold. Dimming with distance is free of that trap because the eye is not being asked to read
//!    a value off it, only an ordering.
//!
//! # The stage (fidelity pass 1)
//!
//! The first version was a tidy bar farm on black and was reported, with `pipes`, `brutal` and
//! `orbit`, as "very basic, not cohesive with blossom or vaporwave". What it lacked was a PLACE for
//! the bars to stand, so they now have one, and none of it touches the refusal above:
//!
//! - **A floor.** A perspective grid - 8 lines converging on a vanishing point at 30 % height, 4
//!   cross lines that scroll AWAY at the speed the history rows march back, so the floor moves the
//!   way depth moves here: into the past. Perspective is safe on the floor because the floor carries
//!   no data; the bars stay oblique with integer row steps, which is what the refusal was about.
//! - **Shadows.** A 2 px contact ellipse under every bar, 40 % darker than the floor it lands on. The
//!   floor is washed faintly with `edge` for exactly this reason - a shadow on a black floor is black
//!   on black.
//! - **Real boxes.** A lit top face with a 1 px `hot` specular edge along its front, a side face 30 %
//!   darker than `lit`, and a front face that darkens toward the floor.
//! - **Fog.** Each depth row fades 25 % further toward the panel, in linear light.
//! - **A camera.** The oblique angle swings +-6 degrees over 20 s. Only the x step changes - the row
//!   step stays an integer, so no two depth rows can ever collapse onto one pixel row.
//! - **Falling ghosts.** When a front bar drops more than 15 % below the peak it rose to, that lost
//!   height detaches as a `ghost`-alpha block and falls at 40 px/s, fading over 400 ms - one block per
//!   drop, not a cascade. Pool of 16.

use crate::dsp::bands::NUM_BANDS;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Depth rows drawn, nearest last.
///
/// Five, and the ceiling is the vertical budget rather than taste: each row costs `DEPTH_DY` rows of
/// height that the bars then cannot use. At 5 rows the shear spends 12px of a 56px interior.
const DEPTH: usize = 5;

/// The oblique offset per depth row at the CENTRE of the camera's yaw, in pixels. Up and to the right.
///
/// The row step `DEPTH_DY` is an integer and never changes, so there is no perspective divide and
/// nothing that can collapse two depth rows onto the same pixel row - the failure that disabled
/// `vapor`'s occlusion. The yaw only changes the x step, which costs no rows at all.
const DEPTH_DX: i32 = 4;
const DEPTH_DY: i32 = 3;

/// Gap between bars, and the narrowest a bar may be before the family sheds detail.
const BAR_GAP: i32 = 2;
const MIN_BAR: i32 = 3;

/// How often the history ring rotates, in milliseconds. See the module docs, property 1.
///
/// 130ms gives the five depth rows a 650ms span - long enough that a transient is visibly *travelling*
/// backwards rather than blinking, short enough that the far row still belongs to the same phrase.
const ROTATE_MS: f32 = 130.0;

/// Fog: how much further toward the panel each depth row sits, compounding, in linear light.
///
/// Depth cueing, and the one legitimate use of brightness here. 25 % a step leaves the far row at
/// 0.75^4 = 32 % of its light - the same depth as the old flat 0.72 fade, but now every step is the
/// same ratio, which is what fog does and what reads as distance rather than as five shades.
const FOG_PER_STEP: f32 = 0.25;

/// The level window, `vapor`'s MEASURED p10-p90 of real music.
///
/// Not a 0..1 mapping, which renders dead, and not normalised against the frame's loudest band, which
/// is provably inert - that band sits at p50 0.819 so the normaliser settles near 1.1x. Four attempts
/// in that family failed exactly that way.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// Bars, at the two panel widths this app actually gets.
const BARS_WIDE: usize = 22;
const BARS_NARROW: usize = 12;

/// The flourish: the whole stack surges forward one depth step and settles back.
const SURGE_MS: f32 = 900.0;

/// The box's own depth, in rows of top face. Two, so a gap of floor is left between it and the row
/// behind (a row step is 3): boxes that touched front-to-back would hide every history row that is
/// lower than the one in front of it, and the history is the point of the family.
const TOP_ROWS: usize = 2;

/// Face shading. The side is `lit` darkened 30 % (the spec's number); the front darkens from 35 % at
/// its top to 62 % where it meets the floor, an occlusion ramp, so the boxes stand ON something.
const SIDE_DARKEN: f32 = 0.30;
const FRONT_TOP: f32 = 0.35;
const FRONT_FOOT: f32 = 0.62;
/// How much of `hot` the top face catches from the sky. The 1 px front edge is `hot` outright.
const TOP_SHEEN: f32 = 0.18;
/// How far the furthest row's specular edge is pulled from `hot` toward `lit` before it is fogged.
const SPEC_FAR: f32 = 1.0;

/// The horizon, as a fraction of the panel height, and the stage lighting.
const HORIZON: f32 = 0.30;
/// `edge` mixed into the panel at the horizon (the haze) and at the front of the floor. The floor has
/// to be lit at all for a shadow to be visible on it.
const HAZE: f32 = 0.20;
const FLOOR_FAR: f32 = 0.07;
const FLOOR_NEAR: f32 = 0.05;
/// Grid lines: 8 converging, 4 across. Their alpha is `edge_alpha` times this, fading to the horizon.
const GRID_V: i32 = 8;
const GRID_H: usize = 4;
const GRID_GAIN: f32 = 2.6;
/// Perspective spacing of the cross lines, and how long one line takes to scroll back one spacing.
/// Matched to the history's own march - 3 px per 130 ms at the front - so floor and bars recede together.
const GRID_STEP: f32 = 0.6;
const GRID_SCROLL_MS: f32 = 1000.0;

/// The contact shadow: black at 40 % over whatever floor it lands on (so the floor is darkened 40 %).
const SHADOW_DARKEN: f32 = 0.40;

/// Camera yaw: +-6 degrees on a 20 s period.
const YAW_DEG: f32 = 6.0;
const YAW_MS: f32 = 20_000.0;

/// Falling ghosts: shed on a 15 % drop from the peak, fall at 40 px/s, fade over 400 ms, 16 at most.
const GHOST_DROP: f32 = 0.15;
const GHOST_FALL_PX_S: f32 = 40.0;
const GHOST_MS: f32 = 400.0;
const GHOSTS: usize = 16;
/// `ghost` is a trail alpha (0.10 across the colourways) - right for a smear, too faint for a block
/// that has to be SEEN falling. The fill takes 3x it and the leading edge 6x, both scaled by `ghost`
/// so a colourway that sets it to zero gets no ghosts at all.
const GHOST_FILL: f32 = 3.0;
const GHOST_EDGE: f32 = 6.0;

/// A block of lost height, falling. Plain data in a fixed pool so `draw` never allocates.
#[derive(Clone, Copy, Default)]
struct Ghost {
    live: bool,
    x: i32,
    w: i32,
    top: f32,
    h: f32,
    age: f32,
}

#[derive(Default)]
pub struct Mesh {
    /// The NEAREST row, updated every frame with the current spectrum.
    ///
    /// Separate from the history ring, and that separation is the whole reason this family animates.
    /// The first version put the live spectrum INTO the ring and drew `rows[0]` as the near row - so the
    /// front row, the one the eye actually reads a level from, only changed when the ring rotated. At a
    /// 130ms interval that is 7.7 updates a second for the entire display, and it was reported
    /// immediately as "very low fps". Nothing was slow: the render was fine and the DATA was stale.
    live: Vec<f32>,
    /// The history, `DEPTH - 1` snapshots of `bars` levels. Index 0 is the row just BEHIND the live one.
    rows: Vec<Vec<f32>>,
    /// Peak-hold accumulator since the last rotation. See the module docs, property 2.
    acc: Vec<f32>,
    /// The height each front bar has risen to since it last shed a ghost; zero once it has shed.
    hold: Vec<f32>,
    /// Each front bar's height last frame, so the hold can tell rising from falling.
    last: Vec<f32>,
    /// Unspent time toward the next rotation.
    due: f32,
    /// Camera yaw and floor scroll clocks, in ms, each wrapped to its own period.
    yaw_ms: f32,
    grid_ms: f32,
    /// The x step per depth row this frame, after the yaw. Kept for the tests.
    dxs: f32,
    ghosts: [Ghost; GHOSTS],
    flourish: crate::dsp::flourish::Trigger,
    surge: crate::dsp::flourish::Envelope,
}

fn resp(level: f32, sensitivity: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    let x = ((level - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0);
    (x.powf(LEVEL_GAMMA) * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

/// The panel layout, shared by `draw` and the tests so neither guesses where a bar is.
#[derive(Clone, Copy, Debug)]
struct Layout {
    bars: usize,
    pitch: i32,
    bar_w: i32,
    /// The keyline row the front row stands on; its faces end one row above.
    base_y: i32,
    max_bar: i32,
    horizon: i32,
}

/// The x step at the widest yaw, which is what the layout has to leave room for.
fn dxs_max() -> f32 {
    let theta0 = (DEPTH_DY as f32).atan2(DEPTH_DX as f32);
    DEPTH_DY as f32 / (theta0 - YAW_DEG.to_radians()).tan()
}

/// The x offset of the top face `k` rows above the front face, for an x step of `dxs` per row step.
fn shear(dxs: f32) -> [i32; TOP_ROWS + 1] {
    let mut s = [0i32; TOP_ROWS + 1];
    for (k, v) in s.iter_mut().enumerate() {
        *v = (k as f32 * dxs / DEPTH_DY as f32).round() as i32;
    }
    s
}

fn layout(w: i32, h: i32) -> Option<Layout> {
    let shear_x = (dxs_max() * (DEPTH - 1) as f32).ceil() as i32;
    let side_x = shear(dxs_max())[TOP_ROWS];
    let shear_y = DEPTH_DY * (DEPTH as i32 - 1);
    let bars = if w >= 300 { BARS_WIDE } else { BARS_NARROW };
    let avail_w = (w - 4) - shear_x - side_x;
    let pitch = avail_w / bars as i32;
    let bar_w = pitch - BAR_GAP;
    // Two rows under the front row for its shadow, inside the panel's last interior row (h - 3).
    let base_y = h - 5;
    let max_bar = base_y - shear_y - TOP_ROWS as i32 - 4;
    if bar_w < MIN_BAR || max_bar < 6 || pitch < MIN_BAR + BAR_GAP {
        return None; // shed rather than smudge
    }
    Some(Layout { bars, pitch, bar_w, base_y, max_bar, horizon: (h as f32 * HORIZON).round() as i32 })
}

/// `c` with its alpha replaced - the floor and the panel share the panel's own opacity.
fn with_alpha(c: Rgba, a: f32) -> Rgba {
    Rgba::new(c.r, c.g, c.b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// The colours one depth row is drawn in, fog already applied.
struct Faces {
    spec: Rgba,
    top: Rgba,
    side: Rgba,
    /// The front face, indexed by rows above the floor.
    front: [Rgba; RAMP],
}
const RAMP: usize = 128;

impl Mesh {
    fn fit(&mut self, bars: usize) {
        if self.acc.len() != bars || self.live.len() != bars || self.hold.len() != bars || self.last.len() != bars {
            self.acc = vec![0.0; bars];
            self.live = vec![0.0; bars];
            self.hold = vec![0.0; bars];
            self.last = vec![0.0; bars];
            self.rows = vec![vec![0.0; bars]; DEPTH - 1];
        }
        if self.rows.len() != DEPTH - 1 {
            self.rows = vec![vec![0.0; bars]; DEPTH - 1];
        }
    }

    /// Advances the LIVE row every frame, peak-holds into the accumulator, and rotates the history
    /// when its interval elapses.
    ///
    /// The live row is what makes this animate at the frame rate. The history behind it deliberately
    /// does not - it is a record, and a record that changed continuously would not read as depth.
    fn advance(&mut self, d: &FrameData, t: &Theme, dt: f32, bars: usize) {
        let b = &t.ballistics;
        for i in 0..bars {
            let lo = (i * NUM_BANDS) / bars;
            let hi = (((i + 1) * NUM_BANDS) / bars).clamp(lo + 1, NUM_BANDS);
            let band = d.levels[lo..hi].iter().copied().fold(0.0f32, f32::max);
            let target = resp(band, t.sensitivity);

            // Live, with the colourway's own ballistics - fast up, slower down, per frame.
            let cur = self.live[i];
            let k = if target > cur { b.attack } else { b.decay };
            let next = cur + (target - cur) * k.clamp(0.0, 1.0);
            self.live[i] = if next.is_finite() { next.clamp(0.0, 1.0) } else { 0.0 };

            // And the peak since the last rotation, which is what the history will keep.
            if target > self.acc[i] {
                self.acc[i] = target;
            }
        }
        self.due += dt;
        if !self.due.is_finite() {
            self.due = 0.0;
        }
        // Bounded, so a long stall cannot rotate the whole history in one frame and flush it.
        self.due = self.due.min(ROTATE_MS * DEPTH as f32);
        while self.due >= ROTATE_MS {
            self.due -= ROTATE_MS;
            // Rotate in place: the oldest row's storage becomes the newest, so nothing allocates.
            self.rows.rotate_right(1);
            self.rows[0].copy_from_slice(&self.acc);
            for i in 0..bars {
                self.acc[i] = 0.0;
            }
        }
    }

    /// Sheds a ghost for every front bar that has dropped `GHOST_DROP` below the peak it rose to, and
    /// ages the pool. A free slot is taken first, else the oldest ghost is recycled.
    fn shed(&mut self, l: &Layout, dt: f32) {
        for g in self.ghosts.iter_mut().filter(|g| g.live) {
            g.age += dt;
            g.top += GHOST_FALL_PX_S * dt / 1000.0;
            if g.age >= GHOST_MS || !g.top.is_finite() {
                g.live = false;
            }
        }
        for i in 0..l.bars {
            let live = self.live[i];
            let rising = live > self.last[i];
            self.last[i] = live;
            // The hold follows the bar UP only. Once it has shed it is disarmed (zero) until the bar
            // rises again, so one drop sheds one block rather than a cascade as the bar decays.
            if rising {
                self.hold[i] = self.hold[i].max(live);
            }
            let hold = self.hold[i];
            if hold <= 0.0 || hold - live <= GHOST_DROP {
                continue;
            }
            let slot = match self.ghosts.iter().position(|g| !g.live) {
                Some(s) => s,
                None => {
                    let mut oldest = 0;
                    for (k, g) in self.ghosts.iter().enumerate() {
                        if g.age > self.ghosts[oldest].age {
                            oldest = k;
                        }
                    }
                    oldest
                }
            };
            let mb = l.max_bar as f32;
            self.ghosts[slot] = Ghost {
                live: true,
                x: 2 + i as i32 * l.pitch,
                w: l.bar_w,
                top: l.base_y as f32 - hold * mb,
                h: (hold - live) * mb,
                age: 0.0,
            };
            self.hold[i] = 0.0;
        }
    }
}

/// One box: keyline, front face with its occlusion ramp, side face, top face with its specular edge.
///
/// Drawn as per-row spans of a parallelogram-extruded rectangle. `base` is the keyline row the box
/// stands on; the faces occupy `base - bar_h ..= base - 1` plus `TOP_ROWS` of top face above that.
#[allow(clippy::too_many_arguments)]
fn draw_box(c: &mut Canvas, x: i32, base: i32, bar_h: i32, bw: i32, s: &[i32; TOP_ROWS + 1], f: &Faces, key: Rgba) {
    let y = base - bar_h;
    let top = y - TOP_ROWS as i32;
    // The silhouette of row r: [left, right). The left edge shears right on the top rows; the right
    // edge is the side face's, which is lifted near the floor by the same shear (the box's back-bottom
    // edge sits TOP_ROWS above its front-bottom one).
    let sil = |r: i32| -> Option<(i32, i32)> {
        if r < top || r >= base {
            return None;
        }
        let above = (base - 1 - r) as usize;
        let right = x + bw + s[above.min(TOP_ROWS)];
        let left = if r < y { x + s[((y - r) as usize).min(TOP_ROWS)] } else { x };
        Some((left, right))
    };
    // Keyline first, one pixel all round, the union of this row and its neighbours. Without it
    // adjacent depth rows in the same hue have nothing between them and the stack reads as one lumpy
    // surface instead of as five rows - the same trick the dolphin family needed.
    for r in (top - 1)..=base {
        let mut span: Option<(i32, i32)> = None;
        for q in [r - 1, r, r + 1] {
            if let Some((l, rr)) = sil(q) {
                span = Some(match span {
                    Some((a, b)) => (a.min(l), b.max(rr)),
                    None => (l, rr),
                });
            }
        }
        if let Some((l, rr)) = span {
            c.fill_rect(l - 1, r, rr - l + 2, 1, key);
        }
    }
    for r in top..base {
        let Some((left, right)) = sil(r) else { continue };
        if r < y {
            // Top face. Its front edge - the row touching the front face - is the specular line.
            let k = ((y - r) as usize).min(TOP_ROWS);
            let face_r = x + bw + s[k];
            c.fill_rect(left, r, face_r - left, 1, if k == 1 { f.spec } else { f.top });
            c.fill_rect(face_r, r, right - face_r, 1, f.side);
        } else {
            let above = ((base - 1 - r) as usize).min(RAMP - 1);
            c.fill_rect(x, r, bw, 1, f.front[above]);
            c.fill_rect(x + bw, r, right - x - bw, 1, f.side);
        }
    }
}

impl Family for Mesh {
    fn id(&self) -> &'static str {
        "mesh"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let surge = self.surge.update(fired, dt, SURGE_MS);

        // Opaque panel first.
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);

        let Some(l) = layout(w, h) else { return };
        self.fit(l.bars);
        self.advance(d, t, dt, l.bars);
        self.shed(&l, dt);

        // ---- the camera ----
        self.yaw_ms = if (self.yaw_ms + dt).is_finite() { (self.yaw_ms + dt) % YAW_MS } else { 0.0 };
        self.grid_ms = if (self.grid_ms + dt).is_finite() { (self.grid_ms + dt) % GRID_SCROLL_MS } else { 0.0 };
        let tau = std::f32::consts::TAU;
        let theta0 = (DEPTH_DY as f32).atan2(DEPTH_DX as f32);
        let yaw = YAW_DEG.to_radians() * (tau * self.yaw_ms / YAW_MS).sin();
        let dxs = DEPTH_DY as f32 / (theta0 + yaw).tan();
        self.dxs = dxs;
        let s = shear(dxs);

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let key = Rgba::from_hex(&t.panel, 1.0);
        let black = Rgba::new(0, 0, 0, 255);
        let pa = t.panel_alpha;

        // ---- the stage: haze above the horizon, a lit floor below it ----
        let hy = l.horizon;
        let yb = h - 3; // the last interior row
        let haze = with_alpha(Rgba::lerp_linear(key, edge, HAZE), pa);
        let far = with_alpha(Rgba::lerp_linear(key, edge, FLOOR_FAR), pa);
        let near = with_alpha(Rgba::lerp_linear(key, edge, FLOOR_NEAR), pa);
        let sky = with_alpha(key, pa);
        c.vertical_gradient(1, 2, w - 2, hy - 2, &[(0.0, sky), (0.45, sky), (1.0, haze)], false);
        c.vertical_gradient(1, hy, w - 2, yb - hy + 1, &[(0.0, far), (1.0, near)], false);

        // The grid, in `edge`, fading into the haze. Verticals converge on a vanishing point placed
        // along the bars' own depth direction from the panel centre, so the floor and the boxes agree
        // about which way is "back" - and it swings with the yaw.
        let ga = (t.edge_alpha * GRID_GAIN).clamp(0.0, 1.0);
        let span_y = (yb - hy).max(1) as f32;
        let vpx = w as f32 * 0.5 + (l.base_y - hy) as f32 * dxs / DEPTH_DY as f32;
        let spread = w as f32 / 6.0;
        for k in 0..GRID_V {
            let xb = vpx + (k as f32 - (GRID_V - 1) as f32 * 0.5) * spread;
            for y in (hy + 2)..=yb {
                let f0 = (y - hy) as f32 / span_y;
                let f1 = ((y + 1 - hy) as f32 / span_y).min(1.0);
                let x0 = (vpx + (xb - vpx) * f0).round() as i32;
                let x1 = (vpx + (xb - vpx) * f1).round() as i32;
                let (lo, hi) = (x0.min(x1), x0.max(x1));
                // One span per row, so a shallow line is continuous and no pixel is blended twice.
                let a = ga * f0.powf(0.7);
                c.fill_rect(lo, y, (hi - lo).max(1), 1, with_alpha(edge, a));
            }
        }
        let p = self.grid_ms / GRID_SCROLL_MS;
        for k in 0..GRID_H {
            let dk = 1.0 + (k as f32 + p) * GRID_STEP;
            let y = hy + (span_y / dk).round() as i32;
            if y <= hy + 1 || y > yb {
                continue;
            }
            let f0 = (y - hy) as f32 / span_y;
            // Fades in as it leaves the bottom edge, and out into the haze.
            let arrive = ((k as f32 + p) / 0.35).min(1.0);
            c.fill_rect(1, y, w - 2, 1, with_alpha(edge, ga * f0.powf(0.7) * arrive));
        }
        c.fill_rect(1, hy, w - 2, 1, with_alpha(edge, (t.edge_alpha * 2.2).min(1.0)));

        // BACK TO FRONT. The painter's algorithm is the whole occlusion story: a near bar is drawn
        // after the row behind it, over the top of it, with its own keyline. No z-buffer, no sorting
        // beyond this loop order - and within a row, left to right, because the side face is on the
        // right and the neighbour's front face is nearer than it.
        let shadow = Rgba::new(0, 0, 0, (SHADOW_DARKEN * 255.0).round() as u8);
        for depth in (0..DEPTH).rev() {
            let fog = 1.0 - (1.0 - FOG_PER_STEP).powi(depth as i32);
            // The flourish pulls the whole stack one depth step nearer, then lets it settle.
            let pull = surge * DEPTH_DY as f32;
            let dx = (dxs * depth as f32).round() as i32;
            let dy = (DEPTH_DY * depth as i32) as f32 - pull;
            let base = l.base_y - dy.round() as i32;

            let body = if depth == 0 { Rgba::lerp_linear(lit, hot, surge * 0.8) } else { lit };
            let fogged = |c: Rgba| Rgba::lerp_linear(c, key, fog);
            let mut faces = Faces {
                // The far rows' edges lean toward `lit` before the fog: a near-white edge fogged
                // toward a black panel is GREY, and grey tops read as dead boxes, not distant ones.
                spec: fogged(Rgba::lerp_linear(hot, body, SPEC_FAR * depth as f32 / (DEPTH - 1) as f32)),
                top: fogged(Rgba::lerp_linear(body, hot, TOP_SHEEN)),
                side: fogged(Rgba::lerp_linear(body, black, SIDE_DARKEN)),
                front: [key; RAMP],
            };
            let rows = (l.max_bar as usize + 1).min(RAMP);
            for (k, v) in faces.front.iter_mut().enumerate().take(rows) {
                let up = k as f32 / l.max_bar.max(1) as f32;
                *v = fogged(Rgba::lerp_linear(body, key, FRONT_FOOT + (FRONT_TOP - FRONT_FOOT) * up));
            }

            for i in 0..l.bars {
                // Depth 0 is the LIVE row; everything behind it is history.
                let level = if depth == 0 { self.live[i] } else { self.rows[depth - 1][i] };
                let bar_h = (level * l.max_bar as f32).round() as i32;
                if bar_h <= 0 {
                    continue;
                }
                let x = 2 + dx + i as i32 * l.pitch;
                // The contact shadow first, on the floor under the box: 2 px, the bar's width,
                // tapered on the lower row so it reads as an ellipse rather than a strip.
                c.fill_rect(x, base + 1, l.bar_w + s[1], 1, shadow);
                c.fill_rect(x + 1, base + 2, l.bar_w + s[1] - 2, 1, shadow);
                draw_box(c, x, base, bar_h, l.bar_w, &s, &faces, key);
            }
        }

        // The ghosts, over everything: they have detached from the front row and are nearest of all.
        let gha = t.ghost.clamp(0.0, 1.0);
        for g in self.ghosts.iter().filter(|g| g.live) {
            let fade = (1.0 - g.age / GHOST_MS).clamp(0.0, 1.0);
            let (top, gh) = (g.top.round() as i32, g.h.round().max(1.0) as i32);
            c.fill_rect(g.x, top, g.w, gh, with_alpha(lit, gha * GHOST_FILL * fade));
            c.fill_rect(g.x, top, g.w, 1, with_alpha(hot, gha * GHOST_EDGE * fade));
        }

        // The bloom composites each pixel OVER its own halo, so on an opaque panel it changes no
        // interior pixel, and the clip below clears the margin it could otherwise reach - `pipes`
        // measured exactly that, and here it was ~1 ms of a 1.07 ms frame. Kept for a translucent panel.
        if t.panel_alpha < 1.0 {
            c.bloom(t.bloom as i32, t.glow_strength);
        }
        // Clip last: a surging front row and its shadows sit a pixel from the rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    /// A moving, bass-weighted spectrum with an occasional transient, so the depth rows differ.
    fn frame(gain: f32, t_s: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        // A snare every 0.6s, one band wide, to prove a transient walks backwards into the display.
        let hit = ((t_s / 0.6).fract() < 0.06) as i32 as f32;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.5) * 0.62 + 0.14;
            let wobble = 1.0 + 0.35 * ((t_s * 2.3 + f * 7.0).sin());
            let snare = if (0.42..0.52).contains(&f) { hit * 0.55 } else { 0.0 };
            *v = ((shape * wobble + snare) * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = 0.30 * gain;
        d.rms_r = 0.27 * gain;
        d
    }

    fn bits(c: &Canvas) -> Vec<u8> {
        let mut v = Vec::new();
        for y in 0..c.height() {
            for x in 0..c.width() {
                let px = c.get(x, y);
                v.extend_from_slice(&[px.r, px.g, px.b, px.a]);
            }
        }
        v
    }

    #[test]
    fn the_near_row_is_live_and_changes_every_frame() {
        // THE BUG THIS EXISTS FOR, and it shipped: the first version put the live spectrum into the
        // history ring and drew rows[0] as the near row, so the whole display only changed when the ring
        // rotated - 7.7 updates a second at a 130ms interval. It was reported as "very low fps" and
        // nothing was slow; the data was stale.
        //
        // Between two rotations the display must STILL change, because the near row is live. Putting the
        // live data back into the ring makes consecutive frames identical and fails this.
        let t = builtin::mesh_wmp_cyan();
        let mut fam = Mesh::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..40 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        // ROTATE_MS is 130ms and a frame is 16.7ms, so these four frames sit inside one interval.
        let mut seen: Vec<Vec<u8>> = Vec::new();
        for k in 40..44 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            seen.push(bits(&c));
        }
        let changes = seen.windows(2).filter(|w| w[0] != w[1]).count();
        assert!(
            changes >= 2,
            "the display changed on only {changes} of 3 consecutive frames inside one rotation \
             interval - the near row is not live, which is what 'very low fps' looks like"
        );
    }

    #[test]
    fn the_history_rows_hold_still_between_rotations() {
        // The other half of the same design, and it must not be fixed by making everything live: the
        // rows BEHIND the live one are a record. If they changed every frame the depth axis would stop
        // reading as time and the stack would just be five copies of now.
        let t = builtin::mesh_wmp_cyan();
        let mut fam = Mesh::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..60 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        // Wait for a rotation to LAND rather than assuming where in the interval we are. Three frames
        // is 50ms against a 130ms interval, but `due` could already be at 125ms - which is exactly how
        // the first version of this test failed, on a rotation it had not accounted for rather than on
        // the behaviour it was checking.
        let mut prev = fam.rows.clone();
        let mut rotated = false;
        for k in 60..80 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            if fam.rows != prev {
                rotated = true;
                break;
            }
            prev = fam.rows.clone();
        }
        assert!(rotated, "no rotation happened in 20 frames, so this test proved nothing");

        // Now we are just past one, with a full interval ahead.
        let before = fam.rows.clone();
        for k in 80..84 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        assert_eq!(
            before, fam.rows,
            "the history changed inside a rotation interval, so depth is no longer time"
        );
    }

    #[test]
    fn a_transient_enters_at_the_front_and_is_kept_by_the_peak_hold() {
        // Property 2 from the module docs. The ring samples at 7.7Hz, so a snare landing between two
        // rotations is dropped unless the accumulator holds the maximum. Replacing the peak-hold with
        // "whatever the last frame happened to be" fails this.
        let t = builtin::mesh_wmp_cyan();
        let mut fam = Mesh::default();
        let mut c = Canvas::new(380, 60);
        let bars = BARS_WIDE;
        // Quiet for a while, so the history is low everywhere.
        for k in 0..80 {
            fam.draw(&mut c, &t, &frame(0.18, k as f32 * 0.0167));
        }
        // ONE loud frame - a single-frame transient, the hardest case.
        let mut d = frame(1.0, 1.4);
        for v in d.levels.iter_mut() {
            *v = 0.95;
        }
        fam.draw(&mut c, &t, &d);
        // Then quiet again, long enough for exactly one rotation to carry it into the history.
        for k in 0..9 {
            fam.draw(&mut c, &t, &frame(0.18, 1.5 + k as f32 * 0.0167));
        }
        let peak = (0..bars).map(|i| fam.rows[0][i]).fold(0.0f32, f32::max);
        assert!(
            peak > 0.5,
            "a single-frame transient reached only {peak:.2} in the row behind the front - the \
             peak-hold is dropping it, which is what averaging or last-value sampling does"
        );
    }

    fn lum(px: Rgba) -> f32 {
        0.2126 * px.r as f32 + 0.7152 * px.g as f32 + 0.0722 * px.b as f32
    }

    fn flat(level: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
        for v in d.levels.iter_mut() {
            *v = level;
        }
        d.peaks = d.levels;
        d
    }

    #[test]
    fn bars_stand_on_a_floor_with_shadows() {
        // The floor: at rest (silence, no bars at all) the lower half must carry a GRID - lines, not
        // a wash - so this looks for local maxima along a row (the converging lines) and for a row
        // that is brighter than the rows either side of it (a cross line).
        let t = builtin::mesh_wmp_cyan();
        let (w, h) = (380, 60);
        let l = layout(w, h).unwrap();
        let mut fam = Mesh::default();
        let mut c = Canvas::new(w, h);
        for _ in 0..30 {
            fam.draw(&mut c, &t, &flat(0.0));
        }
        let rest = c.clone();
        let y = (l.horizon + h - 3) / 2 + 4;
        let row: Vec<f32> = (0..w).map(|x| lum(rest.get(x, y))).collect();
        let peaks = (3..(w - 3) as usize).filter(|&x| row[x] > row[x - 2] + 2.0 && row[x] > row[x + 2] + 2.0).count();
        assert!(peaks >= 4, "only {peaks} grid-line crossings on row {y} at rest - there is no floor grid");
        let med = |y: i32| {
            let mut v: Vec<f32> = (4..w - 4).map(|x| lum(rest.get(x, y))).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        };
        let cross = (h / 2..h - 4).filter(|&y| med(y) > med(y - 2) + 1.5 && med(y) > med(y + 2) + 1.5).count();
        assert!(cross >= 1, "no cross line in the lower half at rest - the floor has no perspective rows");

        // The shadow: one loud bar, and the floor directly under it (the row below its keyline) is
        // darker than the same floor pixel in a run with no bar. Both runs draw the same number of
        // frames, so their clocks - and the grid - agree.
        let bar = 10;
        let mut d = flat(0.0);
        let lo = (bar * NUM_BANDS) / l.bars;
        let hi = ((bar + 1) * NUM_BANDS) / l.bars;
        for v in d.levels[lo..hi].iter_mut() {
            *v = 0.95;
        }
        let (mut fam, mut probe) = (Mesh::default(), Mesh::default());
        let (mut c, mut quiet) = (Canvas::new(w, h), Canvas::new(w, h));
        for _ in 0..40 {
            fam.draw(&mut c, &t, &d);
            probe.draw(&mut quiet, &t, &flat(0.0));
        }
        let x = 2 + bar as i32 * l.pitch + l.bar_w / 2;
        let under = l.base_y + 1;
        let (shadowed, floor) = (lum(c.get(x, under)), lum(quiet.get(x, under)));
        assert!(
            shadowed < floor * 0.85 && shadowed + 3.0 < floor,
            "the floor under a loud bar is {shadowed:.1} against {floor:.1} with no bar - no shadow"
        );
    }

    #[test]
    fn a_dropping_peak_sheds_a_falling_ghost() {
        // Two identical runs, one with `ghost` zeroed: the ghosts are the ONLY thing that alpha
        // touches, so the difference between the canvases is exactly the ghost pixels.
        let t = builtin::mesh_wmp_cyan();
        let mut t0 = t.clone();
        t0.ghost = 0.0;
        let (w, h) = (380, 60);
        let l = layout(w, h).unwrap();
        let (mut a, mut b) = (Mesh::default(), Mesh::default());
        let (mut ca, mut cb) = (Canvas::new(w, h), Canvas::new(w, h));
        for _ in 0..60 {
            a.draw(&mut ca, &t, &flat(0.9));
            b.draw(&mut cb, &t0, &flat(0.9));
        }
        assert!(bits(&ca) == bits(&cb), "the two runs differ before any drop - the test is not isolating ghosts");
        let old_top = l.base_y - (a.live[0] * l.max_bar as f32).round() as i32;
        let diff = |ca: &Canvas, cb: &Canvas| {
            let (mut n, mut above) = (0, 0);
            for y in 0..h {
                for x in 0..w {
                    if ca.get(x, y) != cb.get(x, y) {
                        n += 1;
                        // Every ghost is born AT the old top and can only fall from there.
                        if y < old_top {
                            above += 1;
                        }
                    }
                }
            }
            (n, above)
        };
        let mut seen = 0;
        for k in 0..12 {
            a.draw(&mut ca, &t, &flat(0.2));
            b.draw(&mut cb, &t0, &flat(0.2));
            let (n, above) = diff(&ca, &cb);
            assert_eq!(above, 0, "frame {k}: ghost pixels above the old bar top - they must fall, not rise");
            seen = seen.max(n);
        }
        assert!(seen > 40, "a drop from 0.9 to 0.2 left only {seen} ghost pixels - nothing was shed");
        // 500 ms after the drop, every ghost has faded and gone.
        for _ in 0..18 {
            a.draw(&mut ca, &t, &flat(0.2));
            b.draw(&mut cb, &t0, &flat(0.2));
        }
        let (n, _) = diff(&ca, &cb);
        assert_eq!(n, 0, "{n} ghost pixels still showing 500 ms after the drop - they must fade in 400 ms");
    }

    #[test]
    fn back_rows_are_fogged() {
        // The same bar at the same level, once in the front row and once in the back row, each alone:
        // the back one must sit closer to the panel. dt = 0, so the ring cannot rotate under the test.
        let t = builtin::mesh_wmp_cyan();
        let (w, h) = (380, 60);
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let still = FrameData { dt_ms: 0.0, ..FrameData::default() };
        let brightest = |depth: usize| {
            let mut fam = Mesh::default();
            let mut c = Canvas::new(w, h);
            fam.draw(&mut c, &t, &still);
            let rest = c.clone();
            if depth == 0 {
                // The live row decays by the colourway's own ballistics on the next frame.
                fam.live[6] = 0.5 / (1.0 - t.ballistics.decay);
            } else {
                fam.rows[depth - 1][6] = 0.5;
            }
            fam.draw(&mut c, &t, &still);
            // The brightest pixel the bar added - its specular edge.
            let mut best = 0.0f32;
            for y in 0..h {
                for x in 0..w {
                    let px = c.get(x, y);
                    if px != rest.get(x, y) {
                        best = best.max(lum(px));
                    }
                }
            }
            best
        };
        let (front, back) = (brightest(0), brightest(DEPTH - 1));
        let lp = lum(panel);
        assert!(front > lp + 40.0, "the front bar drew nothing bright ({front:.1})");
        assert!(
            back - lp < (front - lp) * 0.6,
            "the back row's brightest pixel is {back:.1} against the front row's {front:.1} (panel {lp:.1}) - no fog"
        );
    }

    /// Run: cargo test --release probe_mesh_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_mesh_cost() {
        let t = builtin::mesh_wmp_cyan();
        let mut fam = Mesh::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..60 {
            fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
        }
        let n = 400;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
        }
        let steady = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("mesh steady: {steady:.3} ms/frame at 380x60");

        // The surge window, with ghosts shed by drops landing inside it.
        fam.flourish.force_next();
        let m = 54;
        let t1 = std::time::Instant::now();
        for k in n..(n + m) {
            let gain = if k % 12 < 6 { 0.95 } else { 0.2 };
            fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
        }
        let flourish = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
        println!("mesh flourish: {flourish:.3} ms/frame at 380x60");
    }

    /// Per-frame cost of the 3D families against the ones already measured.
    ///
    /// Run: cargo test --release probe_3d_cost -- --ignored --nocapture
    ///
    /// Exists because "very low fps" was reported and the cause turned out to be STALE DATA, not slow
    /// rendering. That is a distinction worth being able to settle with a number rather than an opinion,
    /// and the recorded figures for comparison are flame 2.02ms, segmented 1.75ms, tube 1.09ms and
    /// waterfall 0.74ms against a 16.7ms frame - all at 190x60, so double them for the 380x60 the app
    /// normally runs at.
    #[test]
    #[ignore]
    fn probe_3d_cost() {
        for (id, w, h) in [
            ("mesh-wmp-cyan", 190, 60),
            ("mesh-wmp-cyan", 380, 60),
            ("pipes-win95-teal", 190, 60),
            ("pipes-win95-teal", 380, 60),
            ("vfd-ice", 380, 60),
        ] {
            let t = builtin::all().into_iter().find(|t| t.id == id).unwrap();
            let mut fam = crate::render::family_for(&t.family);
            let mut c = Canvas::new(w, h);
            // Warm up, so allocation and first-touch are not in the measurement.
            for k in 0..60 {
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            }
            let n = 400;
            let start = std::time::Instant::now();
            for k in 0..n {
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            }
            let per = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
            println!(
                "  {id:20} {w}x{h}  {per:6.3} ms/frame  {:5.1}% of a 16.7ms budget",
                per / 16.7 * 100.0
            );
        }
    }

    /// Run: cargo test --release dump_mesh -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_mesh() {
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

        for t in builtin::all().into_iter().filter(|t| t.family == "mesh") {
            for (w, h, tag) in [(380, 60, "380"), (190, 60, "190")] {
                let mut fam = Mesh::default();
                let mut c = Canvas::new(w, h);
                for k in 0..140 {
                    fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
                }
                write(format!("mesh-{}-{tag}", t.id), &c);
            }
        }

        // A filmstrip, so the backwards march of a transient is visible as a sequence.
        let t = builtin::mesh_wmp_cyan();
        let mut fam = Mesh::default();
        let mut c = Canvas::new(380, 60);
        let mut shot = 0;
        for k in 0..200 {
            fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            if k >= 60 && (k - 60) % 8 == 0 && shot < 6 {
                write(format!("mesh-march-{shot}"), &c);
                shot += 1;
            }
        }
        println!("wrote mesh dumps to {}", dir.display());
    }

    /// The fidelity-pass eye test: calm / loud / flourish / drop (falling ghosts) for two colourways,
    /// at the wide size and at 128x44.
    ///
    /// Run: cargo test --release dump_mesh_fidelity -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_mesh_fidelity() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: String, c: &Canvas| {
            std::fs::write(dir.join(format!("{name}.rgba")), bits(c)).unwrap();
        };
        for tid in ["mesh-wmp-cyan", "mesh-magenta-deck"] {
            let theme = builtin::all().into_iter().find(|t| t.id == tid).unwrap();
            for (w, hh) in [(380, 60), (128, 44)] {
                for (tag, gain) in [("calm", 0.30), ("loud", 0.85)] {
                    let mut fam = Mesh::default();
                    let mut c = Canvas::new(w, hh);
                    for k in 0..300 {
                        fam.draw(&mut c, &theme, &frame(gain, k as f32 * 0.0167));
                    }
                    write(format!("{tid}-{tag}-{w}x{hh}"), &c);
                }
                let mut fam = Mesh::default();
                let mut c = Canvas::new(w, hh);
                for k in 0..300 {
                    fam.draw(&mut c, &theme, &frame(0.62, k as f32 * 0.0167));
                }
                fam.flourish.force_next();
                for k in 300..306 {
                    fam.draw(&mut c, &theme, &frame(0.62, k as f32 * 0.0167));
                }
                write(format!("{tid}-flourish-{w}x{hh}"), &c);
                // A drop: loud, then the music falls away, so the front row sheds its ghosts.
                let mut fam = Mesh::default();
                let mut c = Canvas::new(w, hh);
                for k in 0..300 {
                    fam.draw(&mut c, &theme, &frame(0.95, k as f32 * 0.0167));
                }
                for k in 300..306 {
                    fam.draw(&mut c, &theme, &frame(0.25, k as f32 * 0.0167));
                }
                write(format!("{tid}-drop-{w}x{hh}"), &c);
            }
        }
        println!("wrote mesh fidelity dumps to {}", dir.display());
    }
}
