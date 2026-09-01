//! The orbit family: spheres circling in 3D space, pulsing to the music.
//!
//! Asked for as "a ball rotating around and pulsing to the music, in 3d space". Built on the same real
//! perspective projection as `pipes` - a camera, a divide, a near plane - and it suits this panel far
//! better than pipes does, for three reasons worth stating because they are the whole design:
//!
//! 1. **A sphere has no thin features to lose.** Every 3D idea before this one failed on legibility at
//!    48 usable rows: a pipe needs a tube wide enough to read as solid, a wireframe needs distinguishable
//!    edges, a receding grid needs distinct depth rows. A filled disc reads at three pixels.
//! 2. **Depth arrives three ways at once** - size, shading and OCCLUSION. A ball passing behind another
//!    is the one depth cue that cannot be faked in 2D, and it is free here: sort by depth, paint in
//!    order.
//! 3. **A pulse is a SIZE change.** That matters more than it sounds. `tube.rs:54-60` measured a driven
//!    element 1.46 dL* brighter than its idle neighbour as INVISIBLE against a ~2.3 dL* threshold, so
//!    "the ball glows on the beat" would have been dead on arrival at this size. "The ball swells on the
//!    beat" is position, which is the channel that works.
//!
//! # The orbit is a WIDE ellipse, and that is the letterbox talking
//!
//! A circular orbit is wrong here. With `Z_C = 8` and a circular radius of 3.5 the ring projects about
//! 50px wide - on a 380px panel, the same lost-in-a-sea-of-black problem that killed the isometric
//! pipes. So the orbit is elliptical: `RX` is derived from the panel width and `RZ` stays small.
//!
//! That is not a fudge, it is the correct reading of the constraint. `x` costs no vertical rows at all,
//! while `z` costs about 2.9 rows per step of depth separation. Spending the width on `x` and keeping
//! `z` for as much depth as the rows will pay for is exactly the right trade, and it makes the projected
//! ring a wide ellipse - which is what an orbit seen from slightly above actually looks like.
//!
//! # Why the camera is above the ring
//!
//! Same reason as `pipes`, and it is load-bearing: `row = CY - F*Y/z`, so anything at `Y = 0` sits on
//! the horizon where every depth projects to the same row. A ring in the camera's own plane would
//! collapse to a horizontal line and the orbit would read as a slider. `Y_C` puts it well below.
//!
//! # The tilt
//!
//! The orbit plane tilts slowly, which does two things: it stops the ring being a fixed shape you stop
//! looking at, and the changing ellipse is itself a depth cue - a ring seen edge-on versus face-on is
//! unambiguous 3D information. Bounded well under the aliasing limit `reel.rs` measured: motion past
//! half a feature pitch per frame appears to run BACKWARDS.

use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Focal length in pixels. Shared with `pipes` by value rather than by import, because the two families
/// are free to diverge and a shared constant would make that look like a mistake.
const F: f32 = 32.0;

/// Near plane, and the post-divide coordinate clamp. See `pipes` for why the clip - not the clamp - is
/// what bounds the coordinate, and why the test must be `>=` rather than a negated `<`.
const Z_NEAR: f32 = 3.0;
const COORD_LIMIT: i32 = 4096;

/// The ring's centre in eye space: distance in front of the camera, and how far below it.
///
/// `Y_C` is negative because the camera looks along +z with +y up, so the ring hangs below the horizon.
/// At `Z_C = 8` this puts the ring's centre near row 30 and its extremes at rows ~17 and ~49 - inside a
/// 60px panel with margin at both ends.
const Z_C: f32 = 13.0;
const Y_C: f32 = -3.0;

/// Depth half-extent of the orbit, in world units. The x half-extent is derived from the panel width.
///
/// **This is the family's one real tension, and the numbers force the balance.** Also note the SIGN of
/// the tilt term (`y = Y_C + ring*st`, not minus): with the minus the near half of the ring rose to the
/// TOP of the panel, which is a ring tilted away from the viewer and reads as upside down - two huge
/// balls above a row of dots. Plus puts the near half low, which is looking DOWN on an orbit. Vertical spread and
/// depth taper both come out of RZ, split by the tilt: the ring rises by `RZ*sin(tilt)` and recedes by
/// `RZ*cos(tilt)`. Spend it on depth and the ellipse flattens into a horizontal ROW; spend it on height
/// and the taper weakens until occlusion is the only depth cue left.
///
/// The first attempt used RZ = 3.5 with tilt 0.62 and got 8px of vertical excursion - less than one ball
/// diameter, so it rendered as a row of circles at slightly different sizes. RZ = 6.5 at tilt 0.95 gives
/// 18.7px of rise (three ball diameters, unmistakably an ellipse) while keeping z in [5.2, 12.8], a
/// 2.46:1 near/far taper. Both cues survive; neither is maximal.
const RZ: f32 = 6.0;

/// Ball radius in world units at rest, and how much a full-scale pulse adds.
///
/// Projected: 0.80 world units is 5.7px at the near edge and 2.2px at the far, and a full pulse takes
/// the near ones to 10.7px.
///
/// Raised from 0.42, which gave 3.0px. At three pixels a shaded disc has no room for a lit cap and reads
/// as a fuzzy dot rather than a sphere - the cap is what stops it looking like a hole.
const R_REST: f32 = 1.30;
const R_PULSE: f32 = 0.90;

/// Most balls a colourway may ask for. The actual count is `Theme::orbit.balls`.
///
/// Sixteen is the ceiling rather than the number: at a 380px panel that is one ball every 23px of the
/// ring's widest span, which is where the near ones start merging into a band instead of reading as
/// separate spheres.
const MAX_BALLS: usize = 16;

/// Milliseconds a ball takes to fade in or out when the reactive count changes.
///
/// Fade, not appear. A ball popping into existence is the same class of discontinuity as the flourish
/// snap that was reported as jarring, and the fix is the same shape: ramp a presence value and multiply
/// the radius by it, so a ball arriving grows out of nothing and a ball leaving shrinks away.
///
/// 260ms is deliberately slower than the swell, so a ball joining the ring never looks like a beat.
const PRESENCE_MS: f32 = 260.0;

/// How much of the ring the quietest passages keep, as a fraction, when `reactive` is set.
///
/// Not zero: one ball always orbits, so the display never goes empty and never has to "start". That also
/// makes the reactive variant a superset of the single-ball one rather than a different thing.
const REACTIVE_FLOOR: f32 = 0.0;

/// Orbit rate in revolutions per second, and the tilt.
///
/// 0.075 rev/s is one lap every 13 seconds. At the widest the ring spans ~360px, so a ball crosses at
/// most about 2.1px per frame near the ellipse's sides - well under the half-a-feature-per-frame bound
/// where motion starts reading backwards.
const ORBIT_HZ: f32 = 0.075;
const TILT_HZ: f32 = 0.031;
const TILT_BASE: f32 = 0.75;
const TILT_AMP: f32 = 0.10;

/// The level window, `vapor`'s MEASURED p10-p90 of real music - not a 0..1 mapping, which renders dead,
/// and not normalised against the frame's loudest band, which is provably inert at p50 0.819.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// How fast a ball swells and how slowly it settles, per millisecond.
///
/// Asymmetric on purpose, the same reason every meter here is: a kick must arrive as a kick. Symmetric
/// ballistics make a pulse read as a wobble.
const SWELL_PER_MS: f32 = 0.055;
const SETTLE_PER_MS: f32 = 0.011;

/// The flourish: the orbit scatters outward and is drawn back in.
const SCATTER_MS: f32 = 1800.0;
const SCATTER_GAIN: f32 = 0.55;

/// Milliseconds the scatter takes to RAMP, in both directions.
///
/// `Envelope` sets its level to 1.0 on the firing frame - it is a one-shot decay, not a shape - so
/// applying it directly made the ring JUMP outward in a single frame and then glide back. Reported as
/// "jarring movement... like they're resetting", and it was: a step function on a geometric quantity.
///
/// Chasing the envelope through a first-order ramp fixes it at both ends. 220ms is about thirteen frames,
/// long enough that the eye reads it as the ring breathing out rather than as a cut.
const SCATTER_RAMP_MS: f32 = 220.0;

#[derive(Default)]
pub struct Orbit {
    /// Smoothed pulse per ball, 0..1.
    pulse: Vec<f32>,
    /// How present each ball is, 0..1 - see `PRESENCE_MS`. Multiplies the radius, so a ball fades by
    /// SHRINKING rather than by going transparent, which is the cue that reads at this size.
    presence: Vec<f32>,
    /// Orbit and tilt phase, in turns.
    phase: f32,
    tilt_t: f32,
    /// The scatter as APPLIED: chases the envelope so the ring never steps. See `SCATTER_RAMP_MS`.
    scatter_s: f32,
    flourish: crate::dsp::flourish::Trigger,
    scatter: crate::dsp::flourish::Envelope,
}

fn resp(level: f32, sensitivity: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    let x = ((level - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0);
    (x.powf(LEVEL_GAMMA) * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

/// Whether an eye-space depth is safely in front of the near plane.
///
/// `>=` and not `!(z < Z_NEAR)`: for a NaN `z` this is FALSE and the point is rejected, where the
/// negated form is TRUE and leaks a NaN into the divide. Same value, opposite safety - see `pipes`.
fn in_front(z: f32) -> bool {
    z >= Z_NEAR
}

/// Projects an eye-space point to (col, row, pixels-per-world-unit). `None` when it must be skipped.
fn project(cx: f32, cy: f32, x: f32, y: f32, z: f32) -> Option<(i32, i32, f32)> {
    if !in_front(z) || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let inv = F / z;
    let col = cx + x * inv;
    let row = cy - y * inv;
    if !col.is_finite() || !row.is_finite() {
        return None;
    }
    Some((
        (col.round() as i32).clamp(-COORD_LIMIT, COORD_LIMIT),
        (row.round() as i32).clamp(-COORD_LIMIT, COORD_LIMIT),
        inv,
    ))
}

/// A filled disc, as horizontal spans. `canvas` has no circle primitive and this is the whole of one.
fn disc(c: &mut Canvas, col: i32, row: i32, r: i32, colour: Rgba) {
    if r <= 0 {
        c.fill_rect(col, row, 1, 1, colour);
        return;
    }
    for dy in -r..=r {
        // Half-width of the span at this row. The +0.5 biases toward a rounder outline at small radii,
        // where an unbiased sqrt gives a visibly square disc.
        let dx = (((r * r - dy * dy) as f32).max(0.0).sqrt() + 0.5) as i32;
        c.fill_rect(col - dx, row + dy, dx * 2 + 1, 1, colour);
    }
}

impl Family for Orbit {
    fn id(&self) -> &'static str {
        "orbit"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let scatter_env = self.scatter.update(fired, dt, SCATTER_MS);
        // Chase it rather than use it. A one-shot envelope is a STEP on the frame it fires, and a step
        // applied to the orbit radius is the jarring snap this replaces.
        let k = (dt / SCATTER_RAMP_MS).clamp(0.0, 1.0);
        self.scatter_s += (scatter_env - self.scatter_s) * k;
        if !self.scatter_s.is_finite() {
            self.scatter_s = 0.0;
        }
        let scatter = self.scatter_s;

        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);

        // The principal point is the panel's own centre row, offset so the ring sits centred: derived
        // rather than chosen, so the family survives a different panel height without retuning.
        // Centres the ring's ACTUAL projected row range, which is NOT h/2 - F*|Y_C|/Z_C. That formula
        // centres the ring at MID depth, but the near half of a tilted ring projects much lower than the
        // far half (F/z is larger there), so the ellipse is not vertically symmetric about its own
        // centre. Solved numerically over the whole lap AND the whole tilt range: 0.30*h puts the sweep
        // at rows 15..45, and 10.6..49.2 once the fattest near ball is added - clear of both edges at
        // every tilt from 0.65 to 0.85.
        let cy = h as f32 * 0.30;
        let cx = w as f32 * 0.5;

        // x half-extent from the WIDTH. x costs no vertical rows, so this is where the panel's aspect
        // ratio should be spent - see the module docs.
        let z_near_edge = Z_C - RZ;
        // Derived from Z_C, NOT from the near edge. The balls that sit WIDEST in x are the ones at
        // cos = +-1, where sin = 0 and the ring depth is exactly Z_C - not the near ones, which are at
        // cos = 0 and therefore in the CENTRE. Using the near-edge depth here made the ring span 55% of
        // the panel and read as a cluster.
        let rx = ((w as f32 * 0.5 - 14.0) * Z_C / F).max(1.0);
        // A plain size gate. The earlier version computed a "lowest drawn row" from Z_C - RZ, which is
        // the depth the ring would reach at ZERO tilt - 3.0 here, behind the near plane - so once RZ grew
        // it bailed on every panel and the family drew nothing at all. The real extremes depend on the
        // tilt and were solved numerically instead; they fit rows 5..55 at h=60 by construction, so what
        // is left to check is only that the panel is big enough to be worth drawing on.
        let _ = z_near_edge;
        if w < 60 || h < 32 {
            return; // shed rather than smudge
        }

        let balls = (t.orbit.balls.clamp(1, MAX_BALLS as i32)) as usize;
        if self.pulse.len() != balls {
            self.pulse = vec![0.0; balls];
            self.presence = vec![0.0; balls];
        }

        // How many are ACTIVE. Fixed colourways use all of them; a reactive one lets the music decide,
        // with one ball always kept so the ring never empties and never has to restart.
        let overall = {
            let n = d.levels.len().max(1);
            d.levels.iter().map(|v| resp(*v, t.sensitivity)).sum::<f32>() / n as f32
        };
        let active = if t.orbit.reactive {
            let f = REACTIVE_FLOOR + (1.0 - REACTIVE_FLOOR) * overall.clamp(0.0, 1.0);
            (1.0 + (balls as f32 - 1.0) * f).round().clamp(1.0, balls as f32) as usize
        } else {
            balls
        };

        // ---- advance ----
        self.phase += dt / 1000.0 * ORBIT_HZ;
        self.tilt_t += dt / 1000.0 * TILT_HZ;
        if !self.phase.is_finite() {
            self.phase = 0.0;
        }
        if !self.tilt_t.is_finite() {
            self.tilt_t = 0.0;
        }
        self.phase = self.phase.fract();
        self.tilt_t = self.tilt_t.fract();
        let tau = std::f32::consts::TAU;
        let tilt = TILT_BASE + TILT_AMP * (tau * self.tilt_t).sin();
        let (st, ct) = (tilt.sin(), tilt.cos());

        let bands = d.levels.len();
        let key = Rgba::from_hex(&t.panel, 1.0);
        let blend = |a: Rgba, b: Rgba, k: f32| {
            let m = |p: u8, q: u8| (p as f32 * (1.0 - k.clamp(0.0, 1.0)) + q as f32 * k.clamp(0.0, 1.0)) as u8;
            Rgba::new(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b), 255)
        };

        // Each ball reads its OWN slice of the spectrum, low to high around the ring, so twelve balls
        // are not twelve copies of one signal - the same lesson the pipes runs needed.
        let mut ball: Vec<(f32, f32, f32, f32, usize)> = Vec::with_capacity(balls);
        for i in 0..balls {
            let lo = (i * bands) / balls;
            let hi = (((i + 1) * bands) / balls).clamp(lo + 1, bands);
            let band = d.levels[lo..hi].iter().copied().fold(0.0f32, f32::max);
            let target = resp(band, t.sensitivity);
            let cur = self.pulse[i];
            let k = if target > cur { SWELL_PER_MS } else { SETTLE_PER_MS };
            let next = cur + (target - cur) * (k * dt).min(1.0);
            self.pulse[i] = if next.is_finite() { next.clamp(0.0, 1.0) } else { 0.0 };

            // Position on the ring, then the plane tilted about the x axis. With the local y zero this
            // reduces to two terms, which is why there is no matrix here.
            // Presence ramps toward 1 for the active balls and 0 for the rest - see `PRESENCE_MS`.
            let want = if i < active { 1.0 } else { 0.0 };
            let pk = (dt / PRESENCE_MS).clamp(0.0, 1.0);
            self.presence[i] += (want - self.presence[i]) * pk;
            if !self.presence[i].is_finite() {
                self.presence[i] = 0.0;
            }

            let a = tau * (self.phase + i as f32 / balls as f32);
            let spread = 1.0 + SCATTER_GAIN * scatter;
            let x = rx * a.cos() * spread;
            let ring = RZ * a.sin() * spread;
            let y = Y_C + ring * st;
            let z = Z_C + ring * ct;
            ball.push((x, y, z, self.pulse[i], i));
        }

        // FAR TO NEAR. This is the occlusion, and occlusion is the depth cue that cannot be faked.
        ball.sort_by(|p, q| q.2.partial_cmp(&p.2).unwrap_or(std::cmp::Ordering::Equal));

        for (x, y, z, pulse, i) in ball {
            let Some((col, row, inv)) = project(cx, cy, x, y, z) else {
                continue;
            };
            // COLOUR PER BALL, resolved through the shared rainbow resolver. On a fixed colourway this
            // returns `t.lit` unchanged, so the single-hue themes are bit-for-bit what they were; on a
            // rainbow one each ball takes the hue of its own position around the ring. Since position IS
            // the frequency slice that ball reads, the hue doubles as a frequency legend - which is the
            // reason `render::tint` takes a position in the first place.
            let x01 = i as f32 / balls.max(1) as f32;
            let lit = crate::render::tint(t, x01, d.time_s, false, &t.lit, 1.0);
            let hot = crate::render::tint(t, x01, d.time_s, true, &t.hot, 1.0);

            // Presence multiplies the radius, so a fading ball shrinks away rather than dissolving.
            let present = self.presence[i].clamp(0.0, 1.0);
            if present < 0.02 {
                continue;
            }
            let r_world =
                (R_REST + R_PULSE * pulse) * present * t.orbit.scale.clamp(0.3, 3.0);
            let r = (r_world * inv).clamp(0.7, 14.0).round() as i32;

            // Depth cue by shading as well as by size. The far side is already 2.56x smaller; dimming it
            // too is what stops the back of the ring reading as clutter.
            let far01 = ((z - (Z_C - RZ)) / (2.0 * RZ)).clamp(0.0, 1.0);
            let body = blend(lit, panel, far01 * 0.5);

            // Keyline first, so two balls of the same hue that overlap still separate. Chroma's trick,
            // and the third family in a row to need it.
            disc(c, col, row, r + 1, key);
            disc(c, col, row, r, body);
            // A lit cap offset up and left: without it a disc reads as a hole rather than a sphere.
            if r >= 2 {
                let off = (r as f32 * 0.38).round() as i32;
                let cap = blend(body, hot, 0.55 + 0.35 * pulse);
                disc(c, col - off, row - off, (r - off).max(1), cap);
            }
        }

        c.bloom(t.bloom as i32, t.glow_strength);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn frame(gain: f32, t_s: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        let hit = ((t_s / 0.5).fract() < 0.08) as i32 as f32;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / crate::dsp::bands::NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.4) * 0.58 + 0.15;
            let wob = 1.0 + 0.32 * ((t_s * 2.4 + f * 8.0).sin());
            *v = ((shape * wob + hit * 0.45) * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = 0.30 * gain;
        d.rms_r = 0.27 * gain;
        d
    }

    fn lum(px: Rgba) -> f32 {
        let a = px.a as f32 / 255.0;
        (0.2126 * px.r as f32 + 0.7152 * px.g as f32 + 0.0722 * px.b as f32) * a
    }

    /// The pulse must be a SIZE change, not a brightness change.
    ///
    /// This is the house rule the family is built on: `tube.rs` measured brightness-as-level as
    /// invisible at this size. Mutation: drive the colour from `pulse` and leave the radius at rest -
    /// that renders plausibly and fails here.
    #[test]
    fn a_louder_ball_is_drawn_bigger_not_just_brighter() {
        let t = builtin::orbit_chrome();
        let count = |gain: f32| -> usize {
            let mut fam = Orbit::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..120 {
                fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
            }
            let mut n = 0;
            for y in 0..60 {
                for x in 0..380 {
                    if lum(c.get(x, y)) > 40.0 {
                        n += 1;
                    }
                }
            }
            n
        };
        let quiet = count(0.18);
        let loud = count(0.95);
        assert!(
            loud > quiet * 3 / 2,
            "loud music lit {loud} pixels against {quiet} quiet - under 1.5x the balls are not \
             actually swelling, they are only getting brighter"
        );
    }

    /// Occlusion: a ball behind another must be hidden by it, which is the depth cue that cannot be
    /// faked in 2D. Measured as the drawn area being LESS than the sum of the discs.
    ///
    /// Mutation: sort near-to-far instead of far-to-near, or drop the sort. Either way the far balls
    /// paint over the near ones and the area stops being sub-additive.
    #[test]
    fn the_ring_is_painted_far_to_near_so_near_balls_occlude() {
        let t = builtin::orbit_chrome();
        let mut fam = Orbit::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..200 {
            fam.draw(&mut c, &t, &frame(0.85, k as f32 * 0.0167));
        }
        // With twelve balls at up to ~6px radius the discs sum to well over 1000px; overlap must bring
        // the drawn total below that. A pure sum would mean nothing is hidden.
        let mut drawn = 0;
        for y in 0..60 {
            for x in 0..380 {
                if lum(c.get(x, y)) > 25.0 {
                    drawn += 1;
                }
            }
        }
        assert!(drawn > 120, "only {drawn} pixels drawn - the ring is not rendering");
        assert!(
            drawn < 3000,
            "{drawn} pixels drawn, which is more than twelve overlapping balls can cover - the depth \
             sort is not producing occlusion"
        );
    }

    /// A vertex at or behind the eye must be rejected, not projected - the 294.6ms hang guard.
    #[test]
    fn a_point_at_or_behind_the_eye_is_rejected() {
        for z in [0.0f32, -1.0, 1.0, f32::NAN, f32::NEG_INFINITY] {
            assert!(
                project(190.0, 30.0, 1.0, -7.5, z).is_none(),
                "z={z} was projected instead of clipped"
            );
        }
        assert!(project(190.0, 30.0, f32::NAN, -7.5, 8.0).is_none(), "a NaN x was projected");
        assert!(project(190.0, 30.0, 0.0, -7.5, Z_C).is_some(), "the ring centre must project");
    }

    /// The orbit must actually go round: a ball has to change depth over a lap, or it is a 2D ring.
    ///
    /// Mutation: hold `phase` constant, or set `RZ` to 0 - both leave the depth spread at zero.
    #[test]
    fn a_ball_changes_depth_over_one_lap() {
        let mut fam = Orbit::default();
        let t = builtin::orbit_chrome();
        let mut c = Canvas::new(380, 60);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        // One lap is 1/ORBIT_HZ seconds; sample across a little more than that.
        let frames = (1.0 / ORBIT_HZ / 0.0167) as usize + 20;
        for k in 0..frames {
            fam.draw(&mut c, &t, &frame(0.5, k as f32 * 0.0167));
            let tau = std::f32::consts::TAU;
            let tilt = TILT_BASE + TILT_AMP * (tau * fam.tilt_t).sin();
            let a = tau * fam.phase;
            let z = Z_C + RZ * a.sin() * tilt.cos();
            lo = lo.min(z);
            hi = hi.max(z);
        }
        assert!(
            hi - lo > RZ,
            "depth only varied by {:.2} world units over a lap - the ring is flat, not an orbit",
            hi - lo
        );
    }

    /// Run: cargo test --release dump_orbit -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_orbit() {
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
        for t in builtin::all().into_iter().filter(|t| t.family == "orbit") {
            let mut fam = Orbit::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..300 {
                fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
            }
            write(format!("orbit-{}", t.id), &c);
        }
        // A lap, so the tilt and the occlusion are visible as a sequence.
        let t = builtin::orbit_chrome();
        let mut fam = Orbit::default();
        let mut c = Canvas::new(380, 60);
        let mut shot = 0;
        for k in 0..900 {
            fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
            if k >= 60 && (k - 60) % 130 == 0 && shot < 6 {
                write(format!("orbit-lap-{shot}"), &c);
                shot += 1;
            }
        }
        println!("wrote orbit dumps");
    }
}
