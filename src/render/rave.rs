//! The rave family: a sweeping laser rig, strobing on the kick.
//!
//! Asked for as "full 200bpm flashing visuals, something that could work for frenchcore", then clarified
//! as "for the frenchcore think rave visuals".
//!
//! # Why a laser fan, of all the rave imagery
//!
//! Because it is the one piece of the vocabulary that a 6:1 letterbox flatters. A wide fan of beams is
//! cramped in a square frame and natural in a strip - for once the aspect ratio is an advantage rather
//! than the constraint every other family here has had to design around. The zooming tunnel and the
//! checkerboard floor both want a vanishing point and a squarer frame, and they fail here for the same
//! reason a rosette kaleidoscope does. The acid smiley is a sprite, and the dolphin family already
//! established that a sprite this size needs several phases and real movement to avoid reading as a lump.
//!
//! # THE FAN'S OUTLINE IS THE SPECTRUM
//!
//! Each beam owns a slice of the band range and reaches as far as that slice is loud, so the tips of the
//! beams trace the spectrum in polar form: bass on one side, treble on the other, and the fan's silhouette
//! is the meter. On top of that the whole fan's APERTURE follows the overall level - beams spread wide
//! when it is loud and collapse toward a pencil when it is quiet.
//!
//! Both are POSITION, which is the house rule: `tube.rs:54-60` measured a driven element 1.46 dL*
//! brighter than its neighbour as invisible against a ~2.3 dL* threshold, so brightness cannot carry a
//! level here. What brightness does carry is EVENTS, which is a different thing and the whole point of
//! this family.
//!
//! # Flash rate: no limit, by explicit decision
//!
//! 200bpm is 3.33 flashes per second, against a general guidance threshold of 3 per second that carries a
//! size exemption a 380x60 taskbar strip comfortably meets. The user was given that arithmetic and said
//! "no concerns about photosensitivity", so this family strobes on every kick and is not built timid.
//!
//! The remaining limit is a LOUDNESS-DESIGN one and it is worth keeping for its own sake: if every beat is
//! maximum then none of them is. So the per-kick strobe has a ceiling below full and every fourth kick
//! goes above it, which is what gives the panel a bar structure instead of a flat wall of flashing.
//!
//! # The kick detector fires often, and that is correct
//!
//! Every other family in this project has had to fight a trigger that would not fire. This one is the
//! opposite case and it changes the design: frenchcore hands over a distorted kick that dominates the
//! spectrum, on the grid, every beat. So the trigger is a plain flux detector on the low three bands with
//! a short refractory - no median, no rarity rule - and the thing that has to be engineered is a visual
//! that survives firing three times a second rather than one that fires at all.
//!
//! `KICK_REFRACTORY_MS` is 130, which caps the detector at about 460bpm. The shared flourish machinery's
//! 180ms would cap it at 333bpm and swallow a kick roll, which is exactly the material this family is for.

use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;


/// The emitters: how many heads the rig has, and where they sit across the panel.
///
/// Asked for as "multiple laser emitters", and it fixes something as well as adding something. One origin
/// meant every beam left the same point, which is what forced the beam count to follow the level - two
/// beams from one point cannot be told apart until `r > 5 / (2*aperture/(n-1))`, and at a narrow aperture
/// that radius is off the bottom of the panel. Three origins 100px apart are separated before they emit a
/// single pixel, so each head only has to keep its OWN few beams apart.
///
/// A truss of heads across the front of a stage is also the more literal rave rig: a single point is a
/// laser, several is a lighting bar, and a lighting bar is what a 6:1 strip is shaped like.
///
/// The outer two are at 0.18 and 0.82 rather than at the edges, so a wide-open fan still has room to
/// spread outward before it leaves the panel instead of exiting through the side immediately.
const EMITTERS: usize = 3;
const EMITTER_X: [f32; EMITTERS] = [0.18, 0.5, 0.82];

/// How far apart the heads are in their sweep, as a fraction of the sweep period.
///
/// Not zero, which is the point. Three heads sweeping in lock-step read as one wide fan with gaps in it;
/// offset, they cross each other and the rig reads as several independent lights. A third of a period puts
/// them at evenly spaced phases, which is the most crossing available from three heads.
const EMITTER_PHASE: f32 = 0.3333;

/// The fewest beams PER EMITTER, at silence. The count still follows the level.
///
/// A beam is `2 * BEAM_FLARE + 1` = 5px wide, and beams leaving the SAME origin are only distinguishable
/// beyond the radius where their angular separation exceeds that width:
/// `r > 5 / (2 * aperture / (n - 1))`. With one origin and nine beams that was r > 44px on a panel 60
/// rows tall - they could never separate, and the quiet frame rendered as a single solid wedge with the
/// beam count, which carries the spectrum, unreadable.
///
/// `EMITTERS` is most of the answer to that now: three beams per head at the narrow aperture separate
/// beyond r > 11px. The count still follows the level on top of it, one to three per head, so the total
/// still runs from three to nine.
///
/// My own note for this family ranked count as the WEAKEST of the position mappings on the grounds that a
/// discrete step reads as a glitch. True in general; here it also looks right - a rig bringing heads up
/// as a track builds.
/// THREE TO FIVE PER HEAD, not one to three. The first version at one-to-three was wrong on screen and
/// the reason is worth keeping: a head running two beams at a wide aperture emits them at plus and minus
/// the aperture and nothing between, so it draws a hard V - and three V's across a strip read as a zigzag
/// mountain range, not as a laser rig. A fan needs enough beams to BE a fan.
///
/// Five per head at the wide aperture separate beyond r > 8px, and three at the narrow one beyond
/// r > 11px, so both ends still resolve. Beams from DIFFERENT heads never need to be mutually separated -
/// they start 100px apart - which is why the total can now run to fifteen where a single origin could
/// only afford nine.
const PER_EMITTER_MIN: usize = 3;
const PER_EMITTER_MAX: usize = 5;

/// The fan's half-aperture in radians, quiet and loud. 0 is straight down.
///
/// The wide end is 1.22 rad (70 degrees), which is what reaches the panel's bottom corners from a top
/// centre origin at this aspect ratio. Wider than that and the outer beams exit through the sides near
/// the top, so the fan stops looking like a fan.
/// Narrowed for the three-head rig. A single head at the panel's centre could afford 1.22 rad, because
/// its fan had the whole width to spread into; three heads at 0.18, 0.5 and 0.82 cannot, and at 1.22 their
/// fans overlapped so heavily that the rig read as one tangle rather than as three lights. 0.85 rad keeps
/// each fan mostly over its own third.
///
/// The narrow end is 0.30 rather than 0.20 for the original reason: too tight and a head's beams overlap
/// into a single stub, which loses the beam count and with it the spectrum reading.
const APERTURE_CALM: f32 = 0.30;
const APERTURE_WILD: f32 = 0.85;

/// How fast the aperture follows the music, per millisecond.
///
/// Faster than the blossom family's wind (0.004) because a laser rig is meant to snap, but not
/// instantaneous: at 1.0 the fan would jitter on every frame's noise instead of breathing with the track.
const APERTURE_FOLLOW: f32 = 0.020;

/// The rig's slow sweep: amplitude in radians and period in milliseconds.
///
/// This is what stops the fan being a fixed shape between kicks. Deliberately slow against the kick rate
/// - at 200bpm there are 12 kicks per sweep - so the sweep reads as the rig moving and the kicks read as
/// events on top of it, rather than the two fighting.
const SWEEP_AMP: f32 = 0.32;
const SWEEP_MS: f32 = 3600.0;

/// How far a kick throws the whole fan, in radians, and how fast that settles.
///
/// A DAMPED SPRING, not a decay to rest. This family's sibling learned the hard way that a one-shot decay
/// never crosses zero, so it snaps out and creeps back - the motion of something dragged rather than
/// something elastic. A rig head that whips past centre and settles is what a real one does.
const SNAP_RAD: f32 = 0.30;
const SNAP_HZ: f32 = 4.2;
const SNAP_ZETA: f32 = 0.30;
/// The largest slice the spring is integrated over. Explicit Euler on an oscillator goes unstable as
/// omega*dt approaches 2, and this app has a known stutter on one machine.
const SPRING_STEP_S: f32 = 0.006;

/// The strobe: the per-kick ceiling, the accent ceiling, how often an accent lands, and the decay.
///
/// The ceiling below full is the loudness-design point from the module note, not a safety cap. The decay
/// is short against a 300ms beat period so the panel is dark again before the next kick - a strobe that
/// has not finished when the next one starts is not a strobe, it is a raised floor.
///
/// These came DOWN from 0.42 and 0.92 after looking at it. At 0.92 of near-white the wash swallowed the
/// beams completely: the panel went pastel and the thing that carries the level became the lowest-contrast
/// element on screen. The wash is also a saturated hue now rather than `hot` - see the draw code. That is
/// the same trick the backlog notes for this family: flash the HUE and leave the luminance roughly alone,
/// so the near-white beams punch through a violent colour instead of competing with a white sheet.
const STROBE_KICK: f32 = 0.30;
const STROBE_ACCENT: f32 = 0.62;
const ACCENT_EVERY: u32 = 4;
const STROBE_MS: f32 = 95.0;

/// The kick detector: bands, flux ratio and refractory.
///
/// Three bands is bins 2..5, roughly 47-117 Hz - the kick's fundamental. The same window the blossom
/// family's lightning uses, and for the same measured reason: widening it dilutes the kick with everything
/// above until the detector stops seeing a kick at all.
const KICK_BANDS: usize = 3;
const KICK_RATIO: f32 = 1.7;
const KICK_REFRACTORY_MS: f32 = 130.0;

/// The flourish: the rig blacks out, then every beam snaps to full spread at once.
///
/// The blackout is the part that makes it read. A rig that simply goes brighter on a panel that is
/// already strobing three times a second is invisible - this project measured a flourish changing 38.5%
/// of the panel and still being reported as never happening, because it was not a change of KIND. A
/// sudden hole in the noise is a change of kind.
const BLAST_MS: f32 = 900.0;
const BLAST_DARK: f32 = 0.34;

/// The level window: `vapor`'s MEASURED p10-p90 of real music. Not a 0..1 mapping, which renders dead.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// How far along its ray a beam reaches when its slice is silent, as a fraction of the ray's full length.
///
/// Not zero: a beam that vanishes leaves the fan with gaps and the count stops reading. A short stub is
/// still a beam.
const REACH_FLOOR: f32 = 0.34;

/// Beam thickness: the bright core and the dim flare either side of it.
const BEAM_CORE: i32 = 1;
const BEAM_FLARE: i32 = 2;
const FLARE_A: f32 = 0.26;

/// The bolt-style halo. A FIXED radius, deliberately not `t.bloom`, because `t.bloom` is a TOML-bindable
/// f32 with no upper clamp anywhere in the schema and `Canvas::bloom` iterates `-radius..=radius` per
/// pixel per pass - `1e300` in a TOML deserialises to infinity, which as an `i32` cast is 2147483647.
const GLOW_R: i32 = 3;
const GLOW_A: f32 = 0.75;

/// The smallest panel this family will draw on.
const MIN_W: i32 = 60;
const MIN_H: i32 = 20;

#[derive(Default)]
pub struct Rave {
    kick: crate::dsp::onset::Flux,
    kicks: u32,
    /// The strobe's brightness, decaying.
    strobe: f32,
    /// The fan's aperture, smoothed toward the level.
    aperture: f32,
    /// The kick spring: angular offset and its velocity.
    snap: f32,
    snap_v: f32,
    flourish: crate::dsp::flourish::Trigger,
    blast: crate::dsp::flourish::Envelope,
}

impl Rave {
    /// The far end of the ray leaving `(ox, oy)` at angle `a`, clamped inside the panel.
    ///
    /// Every coordinate handed to `Canvas::line` comes out of here, and that is deliberate: the line
    /// routine has no off-canvas early-out, and this project measured a single call at 294.6ms when a
    /// coordinate saturated `as i32`. `is_finite` alone is not enough - `1e30f32.is_finite()` is true and
    /// `1e30f32 as i32` is 2147483647 - so this clamps the parameter AND the result.
    fn ray_end(ox: f32, oy: f32, a: f32, w: f32, h: f32, reach: f32) -> (i32, i32) {
        let (sx, cy) = (a.sin(), a.cos());
        let big = w + h;
        let t_down = if cy > 0.001 { (h - 2.0 - oy) / cy } else { big };
        let t_side = if sx > 0.001 {
            (w - 2.0 - ox) / sx
        } else if sx < -0.001 {
            (1.0 - ox) / sx
        } else {
            big
        };
        let t = t_down.min(t_side).max(1.0).min(big) * reach.clamp(0.0, 1.0);
        let ex = (ox + sx * t).clamp(0.0, w - 1.0);
        let ey = (oy + cy * t).clamp(0.0, h - 1.0);
        (ex as i32, ey as i32)
    }
}

impl Family for Rave {
    fn id(&self) -> &'static str {
        "rave"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        if w < MIN_W || h < MIN_H {
            return; // shed rather than smudge
        }
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 250.0) } else { 16.7 };
        let secs = dt / 1000.0;

        // ---- the kick ----
        let nb = KICK_BANDS.min(d.levels.len());
        let kicked = self.kick.update(&d.levels[..nb], dt, KICK_RATIO, KICK_REFRACTORY_MS);
        if kicked {
            self.kicks = self.kicks.wrapping_add(1);
            let accent = self.kicks % ACCENT_EVERY == 0;
            self.strobe = if accent { STROBE_ACCENT } else { STROBE_KICK };
            // An IMPULSE into the spring, alternating direction so consecutive kicks throw the rig the
            // other way instead of pumping it further in one.
            let dir = if self.kicks % 2 == 0 { 1.0 } else { -1.0 };
            self.snap_v += dir * SNAP_RAD * std::f32::consts::TAU * SNAP_HZ;
        }
        self.strobe -= self.strobe * (dt / STROBE_MS).min(1.0);
        if !self.strobe.is_finite() {
            self.strobe = 0.0;
        }

        // The kick spring, sub-stepped for stability - see SPRING_STEP_S.
        let omega = std::f32::consts::TAU * SNAP_HZ;
        let (k, damp) = (omega * omega, 2.0 * SNAP_ZETA * omega);
        let mut left = secs;
        while left > 0.0 {
            let step = left.min(SPRING_STEP_S);
            let accel = -k * self.snap - damp * self.snap_v;
            self.snap_v += accel * step;
            self.snap += self.snap_v * step;
            left -= step;
        }
        if !self.snap.is_finite() || !self.snap_v.is_finite() {
            self.snap = 0.0;
            self.snap_v = 0.0;
        }
        self.snap = self.snap.clamp(-1.0, 1.0);

        // ---- the flourish ----
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let blast = self.blast.update(fired, dt, BLAST_MS);

        // ---- the aperture ----
        let mean = d.levels.iter().sum::<f32>() / d.levels.len().max(1) as f32;
        let drive = ((mean - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0).powf(LEVEL_GAMMA);
        let want = APERTURE_CALM + (APERTURE_WILD - APERTURE_CALM) * drive;
        // The blast overrides it to full spread, which is the event.
        let want = want + (APERTURE_WILD - want) * blast;
        self.aperture += (want - self.aperture) * (APERTURE_FOLLOW * dt).min(1.0);
        if !self.aperture.is_finite() {
            self.aperture = APERTURE_CALM;
        }
        let aperture = self.aperture.clamp(0.0, APERTURE_WILD);

        // ---- the strobe wash ----
        //
        // Drawn UNDER the beams, so a bright panel never washes out the thing that carries the level.
        // The blast darkens instead of brightening - see BLAST_MS.
        let wash = (self.strobe * (1.0 - blast) - BLAST_DARK * blast).clamp(-1.0, 1.0);
        if wash > 0.0 {
            let hue = (self.kicks as f32 * 0.137).rem_euclid(1.0);
            // `t.lit`, the SATURATED colour, not `t.hot`. A near-white sheet at this alpha turns the panel
            // pastel and the beams - which carry the level - end up the lowest-contrast thing on screen.
            // A saturated wash reads as violent while leaving the beams' near-white cores well clear of it.
            let flash = crate::render::tint(t, hue, d.time_s, false, &t.lit, wash.min(1.0));
            c.fill_rect(2, 3, w - 4, h - 6, flash);
        } else if wash < 0.0 {
            c.fill_rect(2, 3, w - 4, h - 6, Rgba::from_hex(&t.panel, (-wash).min(1.0)));
        }

        // ---- the beams ----
        //
        // On their own transparent layer so they can glow: `Canvas::bloom` composites its halo UNDERNEATH
        // the content that made it, and this family's panel is opaque, so beams bloomed on the main canvas
        // would put their glow behind the panel and nothing would reach the screen. The same reason the
        // blossom family's petals and its lightning both need a layer.
        let mut g = Canvas::new(w, h);
        let (wf, hf) = (w as f32, h as f32);
        let oy = 2.0;
        let bands = d.levels.len().max(1);
        // How many beams each head is running - see PER_EMITTER_MIN. The blast forces the full count, so
        // the flourish always shows the whole rig.
        let open = (drive + (1.0 - drive) * blast).clamp(0.0, 1.0);
        let per = (PER_EMITTER_MIN as f32 + (PER_EMITTER_MAX - PER_EMITTER_MIN) as f32 * open)
            .round()
            .clamp(1.0, PER_EMITTER_MAX as f32) as usize;
        let total = (EMITTERS * per).max(1);
        // Colour flips on every kick, and on a rainbow colourway the hue steps too. On a fixed colourway
        // `tint` returns the hex unchanged, so the flip between `lit` and `hot` is what carries it there.
        let flip = self.kicks % 2 == 0;
        let hue = (self.kicks as f32 * 0.137).rem_euclid(1.0);
        let core_hex = if flip { &t.hot } else { &t.lit };
        let core = crate::render::tint(t, hue, d.time_s, flip, core_hex, 1.0);
        let flare = crate::render::tint(t, hue, d.time_s, flip, core_hex, FLARE_A);
        let period_s = (SWEEP_MS / 1000.0).max(0.001);
        for e in 0..EMITTERS {
            let ox = wf * EMITTER_X[e];
            // Each head sweeps at its own phase, and the kick throws alternate heads the OTHER way - so
            // the rig crosses itself instead of moving as one slab. See EMITTER_PHASE.
            let phase = e as f32 * EMITTER_PHASE * period_s;
            let sweep = SWEEP_AMP
                * ((d.time_s + phase) * std::f32::consts::TAU / period_s).sin();
            let lean = if e % 2 == 0 { 1.0 } else { -1.0 };
            let base = sweep + self.snap * lean;
            for i in 0..per {
                let f = if per > 1 { i as f32 / (per - 1) as f32 } else { 0.5 };
                let a = base + (f - 0.5) * 2.0 * aperture;
                // The band slice is cut over the WHOLE rig, indexed left to right across the heads, so
                // bass sits at the left-hand head and treble at the right-hand one and the rig as a whole
                // still traces the spectrum. Slicing per head would make each one a small copy of the
                // same shape and throw the mapping away.
                let gi = e * per + i;
                let lo = gi * bands / total;
                let hi = (((gi + 1) * bands) / total).max(lo + 1).min(bands);
                let mut slice = 0.0f32;
                for v in &d.levels[lo..hi] {
                    if v.is_finite() {
                        slice = slice.max(*v);
                    }
                }
                let lv = ((slice - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0).powf(LEVEL_GAMMA);
                let reach = REACH_FLOOR + (1.0 - REACH_FLOOR) * lv;
                let (ex, ey) = Self::ray_end(ox, oy, a, wf, hf, reach);
                for dx in -BEAM_FLARE..=BEAM_FLARE {
                    g.line(ox as i32 + dx, oy as i32, ex + dx, ey, flare);
                }
                for dx in -BEAM_CORE..=BEAM_CORE {
                    g.line(ox as i32 + dx, oy as i32, ex + dx, ey, core);
                }
            }
            // The head itself. Small, and only worth drawing because three of them along the top edge
            // read as a truss - which is what tells the eye the beams come from equipment rather than
            // from nowhere.
            g.fill_rect(ox as i32 - 2, oy as i32 - 1, 5, 2, core);
        }
        if t.bloom > 0.0 && GLOW_A > 0.0 {
            g.bloom(GLOW_R, GLOW_A);
        }
        c.draw_over(&g);

        // The beams and their halo both spread outward from the origin, so the last thing that happens is
        // clipping back inside the panel this family drew.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    /// A kick every `period_frames`, so the detector has something on the grid to find.
    fn kick_frame(t_s: f32, period_frames: usize, k: usize, gain: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        let hit = k % period_frames == 0;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / crate::dsp::bands::NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.3) * 0.55 + 0.16;
            let punch = if hit && f < 0.12 { 0.55 } else { 0.0 };
            *v = ((shape + punch) * gain).clamp(0.0, 1.0);
        }
        d
    }

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

    fn lit_count(c: &Canvas, t: &Theme) -> i32 {
        let dark = Rgba::from_hex(&t.panel, 1.0);
        let mut n = 0;
        for y in 3..c.height() - 3 {
            for x in 2..c.width() - 2 {
                let p = c.get(x, y);
                if p.a > 0 && (p.r, p.g, p.b) != (dark.r, dark.g, dark.b) {
                    n += 1;
                }
            }
        }
        n
    }

    /// A 200bpm kick must be detected at 200bpm - the whole premise. At 60fps that is one every 18
    /// frames, and the detector's refractory has to clear a 300ms period with room for a roll.
    ///
    /// Mutation: raise KICK_REFRACTORY_MS to 320 and the 200bpm count collapses; raise KICK_RATIO to 4.0
    /// and nothing fires at all.
    #[test]
    fn a_two_hundred_bpm_kick_is_caught_on_every_beat() {
        let t = builtin::rave_frenchcore();
        // 200bpm = 300ms = 17.96 frames at 16.7ms. 18 frames is the closest whole frame.
        for (bpm, period) in [(200usize, 18usize), (160, 22), (240, 15), (300, 12)] {
            let mut fam = Rave::default();
            let mut c = Canvas::new(380, 60);
            let frames = 600;
            for k in 0..frames {
                fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, period, k, 0.75));
            }
            let expected = (frames / period) as u32;
            // Within one either way: the first kick seeds the detector rather than firing it.
            assert!(
                fam.kicks + 2 >= expected && fam.kicks <= expected + 1,
                "{bpm}bpm: caught {} kicks, expected about {expected}",
                fam.kicks
            );
        }
    }

    /// Louder music spreads the fan. The family's primary level-as-position mapping.
    ///
    /// Mutation: make `want` a constant, or drop the `drive` term.
    #[test]
    fn louder_music_spreads_the_fan() {
        let t = builtin::rave_frenchcore();
        let run = |gain: f32| {
            let mut fam = Rave::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..300 {
                fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
            }
            fam.aperture
        };
        let calm = run(0.12);
        let wild = run(0.95);
        assert!(
            wild > calm * 1.8,
            "the fan did not spread with level: calm {calm:.3} wild {wild:.3} rad"
        );
    }

    /// The fan's OUTLINE is the spectrum, so a bass-heavy frame and a treble-heavy one must light
    /// different beams. This is what makes the family a meter rather than an ornament, and it cannot be
    /// seen in one frame - only as a difference between two spectra.
    ///
    /// Mutation: replace `slice` with a constant, or with the frame mean, and the two become identical.
    #[test]
    fn the_beam_tips_trace_the_spectrum() {
        let t = builtin::rave_frenchcore();
        let nb = crate::dsp::bands::NUM_BANDS;
        let probe = |bassy: bool| {
            let mut fam = Rave::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..60 {
                let mut d = FrameData { dt_ms: 16.7, time_s: k as f32 * 0.0167, ..Default::default() };
                for (i, v) in d.levels.iter_mut().enumerate() {
                    let low = i * 3 < nb;
                    *v = if low == bassy { 0.9 } else { 0.13 };
                }
                fam.draw(&mut c, &t, &d);
            }
            // How much ink lands in the left half against the right half. The fan is symmetrical about
            // its centre, and beam 0 owns the lowest bands, so a bass-heavy frame must be left-heavy.
            let dark = Rgba::from_hex(&t.panel, 1.0);
            let (mut l, mut r) = (0, 0);
            for y in 3..57 {
                for x in 2..378 {
                    let p = c.get(x, y);
                    if p.a == 0 || (p.r, p.g, p.b) == (dark.r, dark.g, dark.b) {
                        continue;
                    }
                    if x < 190 {
                        l += 1;
                    } else {
                        r += 1;
                    }
                }
            }
            (l, r)
        };
        let (bass_l, bass_r) = probe(true);
        let (treb_l, treb_r) = probe(false);
        assert!(bass_l + bass_r > 200 && treb_l + treb_r > 200, "nothing was drawn");
        assert!(
            bass_l * treb_r > treb_l * bass_r,
            "the beams do not follow the spectrum: bass {bass_l}/{bass_r}, treble {treb_l}/{treb_r}"
        );
    }

    /// The strobe must fire on the kick, must have an accent every fourth one, and must be DARK again
    /// before the next kick lands - a strobe that has not finished is a raised floor, not a strobe.
    ///
    /// Mutation: set STROBE_ACCENT equal to STROBE_KICK and the accent assertion fails. Raise STROBE_MS
    /// to 400 and the decay assertion fails.
    #[test]
    fn the_strobe_fires_on_the_kick_accents_every_fourth_and_clears_before_the_next() {
        let t = builtin::rave_frenchcore();
        let mut fam = Rave::default();
        let mut c = Canvas::new(380, 60);
        let period = 18;
        let mut per_kick: Vec<f32> = Vec::new();
        let mut just_before: Vec<f32> = Vec::new();
        for k in 0..400 {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, period, k, 0.8));
            if fam.kicks >= 2 {
                if k % period == 0 {
                    per_kick.push(fam.strobe);
                } else if k % period == period - 1 {
                    just_before.push(fam.strobe);
                }
            }
        }
        assert!(per_kick.len() > 8, "not enough kicks observed: {}", per_kick.len());
        // The sampled value is POST-DECAY: `draw` sets the strobe and then applies one decay step in the
        // same call, so the largest value ever observable from outside is the ceiling times one step of
        // decay - 0.758, not 0.92. Writing the expectation with the factor in it rather than loosening
        // the threshold, because a loosened threshold would also accept a strobe that never reached its
        // ceiling at all.
        let one_step = 1.0 - (16.7 / STROBE_MS).min(1.0);
        let peak = per_kick.iter().cloned().fold(0.0f32, f32::max);
        let accent_expected = STROBE_ACCENT * one_step;
        let kick_expected = STROBE_KICK * one_step;
        assert!(
            (peak - accent_expected).abs() < 0.02,
            "no accent kick was seen: peak {peak:.3}, expected {accent_expected:.3}"
        );
        let ordinary = per_kick
            .iter()
            .cloned()
            .filter(|v| *v < accent_expected * 0.9)
            .fold(0.0f32, f32::max);
        assert!(
            (ordinary - kick_expected).abs() < 0.02,
            "an ordinary kick strobed at {ordinary:.3}, expected {kick_expected:.3}"
        );
        assert!(
            ordinary < peak * 0.75,
            "the accent is not meaningfully louder than an ordinary kick: {ordinary:.3} vs {peak:.3}"
        );
        let worst_before = just_before.iter().cloned().fold(0.0f32, f32::max);
        assert!(
            worst_before < 0.06,
            "the strobe had not cleared before the next kick: {worst_before:.3}"
        );
    }

    /// The flourish must read as a HOLE in the noise, not as more brightness on a panel that is already
    /// strobing three times a second.
    ///
    /// Mutation: set BLAST_DARK to 0.0 and the panel no longer darkens.
    #[test]
    fn the_flourish_blacks_the_rig_out() {
        let t = builtin::rave_frenchcore();
        let mut fam = Rave::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..120 {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
        }
        let before = lit_count(&c, &t);
        fam.flourish.force_next();
        let mut darkest = before;
        for k in 120..150 {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
            darkest = darkest.min(lit_count(&c, &t));
        }
        assert!(
            darkest < before,
            "the blast did not darken anything: {before} lit before, {darkest} at its darkest"
        );
        // And it must let go.
        for k in 150..320 {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
        }
        assert!(fam.blast.level() < 0.05, "the blast never let go: {:.3}", fam.blast.level());
    }

    /// Small panels shed, and a hostile frame cannot poison the state or hang the line routine.
    ///
    /// The hang is the specific worry: `Canvas::line` has no off-canvas early-out and this project
    /// measured a single call at 294.6ms when a coordinate saturated `as i32`. `is_finite` alone does not
    /// protect against it, because `1e30f32.is_finite()` is true while `1e30f32 as i32` is 2147483647.
    #[test]
    fn tiny_panels_shed_and_a_hostile_frame_is_survivable() {
        let t = builtin::rave_frenchcore();
        for (w, h) in [(1, 1), (8, 8), (59, 19), (60, 12), (12, 60), (0, 0)] {
            let mut fam = Rave::default();
            let mut c = Canvas::new(w, h);
            fam.draw(&mut c, &t, &frame(0.6, 0.1));
        }
        let mut fam = Rave::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..30 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        let t0 = std::time::Instant::now();
        for bad in [f32::NAN, f32::INFINITY, -1.0e30, 1.0e30] {
            let mut d = frame(0.6, 1.0);
            d.dt_ms = bad;
            d.levels[0] = bad;
            d.levels[5] = f32::NAN;
            d.time_s = bad;
            fam.draw(&mut c, &t, &d);
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        assert!(ms < 250.0, "a hostile frame took {ms:.1}ms - a coordinate probably saturated");
        fam.draw(&mut c, &t, &frame(0.6, 2.0));
        assert!(fam.aperture.is_finite(), "aperture went non-finite");
        assert!(fam.snap.is_finite() && fam.snap_v.is_finite(), "the spring went non-finite");
        assert!(fam.strobe.is_finite(), "strobe went non-finite");
    }

    /// Every colourway draws on both panel widths.
    #[test]
    fn every_colourway_draws_on_both_widths() {
        for t in builtin::all().into_iter().filter(|t| t.family == "rave") {
            for w in [380, 190] {
                let mut fam = Rave::default();
                let mut c = Canvas::new(w, 60);
                for k in 0..40 {
                    fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
                }
                assert!(lit_count(&c, &t) > w / 2, "{} drew almost nothing at {w}px", t.id);
            }
        }
    }

    #[test]
    #[ignore]
    fn probe_rave_cost() {
        let t = builtin::rave_frenchcore();
        let mut fam = Rave::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..60 {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
        }
        println!("rave: {:.3} ms/frame at 380x60", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }

    /// What the kick detector does over the repo's real-music fixtures. A measurement, not a gate: this
    /// family is MEANT to fire on every kick, so a high rate is correct rather than a fault.
    #[test]
    #[ignore]
    fn probe_rave_kick_rate() {
        let parse = |csv: &str| -> Vec<Vec<f32>> {
            csv.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.split(',').filter_map(|v| v.parse::<f32>().ok()).collect())
                .collect()
        };
        let fixtures = [
            ("steady groove", parse(include_str!("../../tests/fixtures/real-music-bands.csv"))),
            ("dnb, dynamic", parse(include_str!("../../tests/fixtures/real-music-dynamic.csv"))),
            ("flat-mastered", parse(include_str!("../../tests/fixtures/real-music-flat.csv"))),
        ];
        for (name, rows) in fixtures.iter() {
            let mut sm = crate::dsp::ballistics::Smoother::new(builtin::rave_frenchcore().ballistics);
            let mut flux = crate::dsp::onset::Flux::default();
            let mut n = 0;
            let frames = 3_600;
            for i in 0..frames {
                let row = &rows[i % rows.len()];
                let mut target = [0.0f32; crate::dsp::bands::NUM_BANDS];
                for (j, v) in target.iter_mut().enumerate() {
                    *v = row.get(j).copied().unwrap_or(0.0);
                }
                sm.update(&target);
                let lv = sm.levels();
                if flux.update(&lv[..KICK_BANDS], 16.7, KICK_RATIO, KICK_REFRACTORY_MS) {
                    n += 1;
                }
            }
            let mins = frames as f32 * 16.7 / 60_000.0;
            println!("{name:<15} {:>7.1} kicks/min", n as f32 / mins);
        }
    }

    #[test]
    #[ignore]
    fn dump_rave() {
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
        for t in builtin::all().into_iter().filter(|t| t.family == "rave") {
            let mut fam = Rave::default();
            let mut c = Canvas::new(380, 60);
            // Land ON a kick frame, so the dump shows the strobe rather than the gap between beats.
            for k in 0..361 {
                fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
            }
            write(format!("rave-{}", t.id), &c);
        }
        let t = builtin::rave_frenchcore();
        for (gain, tag) in [(0.12f32, "calm"), (0.95, "wild")] {
            let mut fam = Rave::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..300 {
                fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, gain));
            }
            write(format!("rave-aperture-{tag}"), &c);
        }
        let mut fam = Rave::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..180 {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.85));
        }
        fam.flourish.force_next();
        for k in 180..190 {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.85));
        }
        write("rave-blast".into(), &c);
    }
}
