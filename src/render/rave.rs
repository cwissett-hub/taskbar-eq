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
///
/// Then they came down AGAIN, from 0.30 and 0.62, when the canvas moved to linear-light blending.
/// These are alphas, and an alpha-0.62 wash in linear light carries far more light than the same
/// number did in gamma space: on the accent kick frenchcore's panel reached 2.26:1 against its own
/// beams and strobe's 2.33:1 - the exact failure the paragraph above describes, back again. The new
/// values are the old ones passed through the sRGB decode curve (0.62 -> 0.34, 0.30 -> 0.07), so the
/// LIGHT the wash adds is what it was, then checked by eye against the pre-change dumps and by
/// `the_accent_strobe_leaves_the_beams_readable` (worst colourway now 3.77:1, frenchcore).
const STROBE_KICK: f32 = 0.07;
const STROBE_ACCENT: f32 = 0.34;
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

/// The beam's soft cone in the haze: the inner and outer flank alphas (the spec's 0.25 and 0.1), and
/// their half-widths at the head and at the tip. The core line alone reads as a drawn line; the same
/// line inside a widening glow reads as light through smoke, and that glow is what the old bloom pass
/// was standing in for - at the cost of a full-panel layer allocated every frame.
const CONE_INNER_A: f32 = 0.25;
const CONE_OUTER_A: f32 = 0.10;
/// (half-width at the head, extra half-width per pixel of beam length).
const CONE_INNER: (f32, f32) = (0.9, 0.025);
const CONE_OUTER: (f32, f32) = (1.4, 0.060);
/// The core's shoulders either side of its 1px centre.
const CORE_SIDE_A: f32 = 0.55;

/// The haze: how far toward `edge` the densest fog goes, as a multiple of the colourway's `ghost`, and
/// how fast its two bands drift (px/s, opposite ways so the fog churns rather than slides).
const HAZE_K: f32 = 0.20;
const HAZE_DRIFT: [f32; 2] = [7.0, -4.5];
/// The glow pooled above the crowd, the light coming back off the floor - this is what the crowd is a
/// silhouette AGAINST. Again a multiple of `ghost`.
const FLOOR_GLOW_K: f32 = 0.30;

/// How the cones catch the fog: the share of their alpha a beam keeps in clear air, and how hard the
/// density drives the rest (density is squared noise, so most of it is small).
const FOG_FLOOR: f32 = 0.45;
const FOG_GAIN: f32 = 2.2;

/// The floor splash where a beam lands.
const SPLASH_A: f32 = 0.30;

/// The crowd. The most people a panel holds - a fixed pool, so `draw` never allocates.
const MAX_PEOPLE: usize = 160;
/// The share of the crowd that puts its arms up on an ordinary strong onset. A drop (the flourish)
/// gets everyone.
const RAISERS: f32 = 0.55;
/// The arm wave: how fast it crosses the room (px/ms), how long arms stay up, how long they take down.
const WAVE_PX_PER_MS: f32 = 1.3;
const ARM_HOLD_MS: f32 = 260.0;
const ARM_FALL_MS: f32 = 420.0;
/// A strong onset is an accent kick (the bar's downbeat) on music at least this loud.
const ARMS_DRIVE: f32 = 0.35;
/// The beat bob: how long the crowd sits 1px low after a kick.
const BOB_MS: f32 = 80.0;

/// The flourish's moving head: how wide it sweeps, either side of straight down.
const MOVER_SWEEP: f32 = 1.25;

/// The smallest panel this family will draw on.
const MIN_W: i32 = 60;
const MIN_H: i32 = 20;

/// Where everything in the room sits, worked out once per size.
#[derive(Clone, Copy)]
struct Room {
    ix0: i32,
    iy0: i32,
    iw: i32,
    ih: i32,
    /// The truss's two rails.
    rail_top: i32,
    rail_bot: i32,
    /// The heads' housing: top row and height. The beams leave from its bottom row.
    head_y: i32,
    head_h: i32,
    oy: f32,
    /// Where the beams land: the crowd's head height.
    floor: f32,
    /// The crowd: the front row's head-top row, the silhouette profile (row widths from the top of the
    /// head down, the last one repeating to the floor), which profile row is the shoulder line, and how
    /// far an arm reaches above the shoulders.
    crowd_top: i32,
    profile: &'static [i32],
    shoulder_row: i32,
    arm_len: i32,
    arm_w: i32,
    bottom: i32,
}

/// The silhouette, row by row from the top of the head. Big: a rounded 4x4 head, a neck, sloping
/// shoulders. Small (128x44): a 2x2 head straight onto the shoulders.
const PROFILE_BIG: [i32; 8] = [2, 4, 4, 2, 2, 6, 8, 9];
const PROFILE_SMALL: [i32; 5] = [2, 2, 1, 4, 5];

impl Room {
    fn at(w: i32, h: i32) -> Room {
        let (ix0, iy0, iw, ih) = (2, 3, w - 4, h - 6);
        let big = ih >= 48;
        let rail_top = iy0 + 1;
        let rail_bot = rail_top + if big { 3 } else { 2 };
        let head_y = rail_bot + 1;
        let head_h = if big { 3 } else { 2 };
        let bottom = iy0 + ih - 1;
        let (profile, shoulder_row): (&'static [i32], i32) =
            if big { (&PROFILE_BIG, 5) } else { (&PROFILE_SMALL, 3) };
        // The front row's shoulders sit this far above the floor of the panel.
        let crowd_top = bottom - if big { 5 } else { 2 } - shoulder_row;
        Room {
            ix0,
            iy0,
            iw,
            ih,
            rail_top,
            rail_bot,
            head_y,
            head_h,
            oy: (head_y + head_h - 1) as f32,
            floor: (crowd_top + 1) as f32,
            crowd_top,
            profile,
            shoulder_row,
            arm_len: if big { 9 } else { 6 },
            arm_w: if big { 2 } else { 1 },
            bottom,
        }
    }
}

/// One member of the crowd.
#[derive(Clone, Copy, Default)]
struct Person {
    x: i32,
    /// Rows below the front row's head line: 0 or 1 for the front row, -2 or -1 for the back.
    dy: i32,
    /// The back row: drawn first and a shade lighter, so the crowd has depth.
    back: bool,
    /// Whether they put their arms up on an ordinary strong onset.
    raises: bool,
    /// 0 left arm, 1 right arm, 2 both.
    arms: u8,
    /// Arm height 0..1, and how much longer it stays up.
    arm: f32,
    hold: f32,
    /// The last wave that reached them.
    wave: u32,
}

/// lowbias32. Deterministic, so the crowd and the haze are the same every launch.
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

/// Value noise that repeats every `period` px in x, so the haze can scroll forever without a seam.
fn haze_noise(x: i32, y: i32, period: i32, cell_w: f32, cell_h: f32, seed: u32) -> f32 {
    let cells = ((period as f32 / cell_w).round() as i32).max(1);
    let cw = period as f32 / cells as f32;
    let u = x.rem_euclid(period.max(1)) as f32 / cw;
    let v = y as f32 / cell_h;
    let (i0, j0) = (u.floor() as i32, v.floor() as i32);
    let (fu, fv) = (u - i0 as f32, v - j0 as f32);
    let (su, sv) = (fu * fu * (3.0 - 2.0 * fu), fv * fv * (3.0 - 2.0 * fv));
    let at = |i: i32, j: i32| {
        let i = i.rem_euclid(cells) as u32;
        hash01(i.wrapping_mul(73_856_093) ^ (j as u32).wrapping_mul(19_349_663) ^ seed)
    };
    let a = at(i0, j0) + (at(i0 + 1, j0) - at(i0, j0)) * su;
    let b = at(i0, j0 + 1) + (at(i0 + 1, j0 + 1) - at(i0, j0 + 1)) * su;
    a + (b - a) * sv
}

/// FNV-1a over what the baked room depends on. No allocation.
fn room_key(t: &Theme, w: i32, h: i32) -> u64 {
    let mut k: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        k ^= b as u64;
        k = k.wrapping_mul(0x0100_0000_01b3);
    };
    for s in [t.id.as_str(), t.panel.as_str(), t.edge.as_str()] {
        for b in s.bytes() {
            eat(b);
        }
        eat(0);
    }
    for b in t.ghost.to_bits().to_le_bytes().into_iter().chain(w.to_le_bytes()).chain(h.to_le_bytes()) {
        eat(b);
    }
    k | 1
}

/// The truss: dark steel, lit just enough by the heads hanging off it to read against the haze.
fn truss_colour(t: &Theme) -> Rgba {
    let panel = Rgba::from_hex(&t.panel, 1.0);
    Rgba::lerp_linear(panel, Rgba::from_hex(&t.hot, 1.0), 0.13)
}

/// The crowd: darker than the room, so it is a silhouette against the haze and the floor glow.
fn crowd_colour(t: &Theme) -> Rgba {
    let panel = Rgba::from_hex(&t.panel, 1.0);
    Rgba::lerp_linear(panel, Rgba::new(0, 0, 0, 255), 0.8)
}

/// The brightest the haze ever gets, so a test can tell "lit" from "fog".
#[cfg(test)]
fn haze_ceiling(t: &Theme) -> Rgba {
    let panel = Rgba::from_hex(&t.panel, 1.0);
    let edge = Rgba::from_hex(&t.edge, 1.0);
    Rgba::lerp_linear(panel, edge, (t.ghost * (HAZE_K + FLOOR_GLOW_K)).clamp(0.0, 0.9))
}

pub struct Rave {
    kick: crate::dsp::onset::Flux,
    kicks: u32,
    /// The strobe's brightness, decaying, and the value the last kick set it to.
    strobe: f32,
    strobe_peak: f32,
    /// The fan's aperture, smoothed toward the level.
    aperture: f32,
    /// The kick spring: angular offset and its velocity.
    snap: f32,
    snap_v: f32,
    flourish: crate::dsp::flourish::Trigger,
    blast: crate::dsp::flourish::Envelope,
    /// Time since the last kick, for the crowd's bob.
    since_kick_ms: f32,
    /// The room, baked per size and colours: two haze bands, each twice the interior width so they can
    /// scroll by an offset with a plain copy.
    room: Room,
    haze: Canvas,
    haze_key: u64,
    /// The fog's density, 0..255, at the same coordinates as `haze`: the cones are lit by it, so a beam
    /// shows the smoke it is passing through.
    fog: Vec<u8>,
    /// The crowd: a fixed pool, hashed per size.
    people: [Person; MAX_PEOPLE],
    n_people: usize,
    crowd_dim: (i32, i32),
    /// The arm wave crossing the room: its front (px), id, and whether it lifts everyone.
    wave_x: f32,
    wave_id: u32,
    wave_all: bool,
}

impl Default for Rave {
    fn default() -> Self {
        Rave {
            kick: Default::default(),
            kicks: 0,
            strobe: 0.0,
            strobe_peak: 0.0,
            aperture: 0.0,
            snap: 0.0,
            snap_v: 0.0,
            flourish: Default::default(),
            blast: Default::default(),
            since_kick_ms: 1.0e4,
            room: Room::at(0, 0),
            haze: Canvas::new(1, 1),
            haze_key: 0,
            fog: Vec::new(),
            people: [Person::default(); MAX_PEOPLE],
            n_people: 0,
            crowd_dim: (0, 0),
            wave_x: f32::INFINITY,
            wave_id: 0,
            wave_all: false,
        }
    }
}

impl Rave {
    /// The far end of the ray leaving `(ox, oy)` at angle `a`, clamped inside the panel and above
    /// `floor`.
    ///
    /// Every coordinate handed to `Canvas::line` comes out of here, and that is deliberate: the line
    /// routine has no off-canvas early-out, and this project measured a single call at 294.6ms when a
    /// coordinate saturated `as i32`. `is_finite` alone is not enough - `1e30f32.is_finite()` is true and
    /// `1e30f32 as i32` is 2147483647 - so this clamps the parameter AND the result.
    fn ray_end(ox: f32, oy: f32, a: f32, w: f32, floor: f32, reach: f32) -> (i32, i32) {
        let (sx, cy) = (a.sin(), a.cos());
        let big = w + floor;
        let t_down = if cy > 0.001 { (floor - oy) / cy } else { big };
        let t_side = if sx > 0.001 {
            (w - 2.0 - ox) / sx
        } else if sx < -0.001 {
            (1.0 - ox) / sx
        } else {
            big
        };
        let t = t_down.min(t_side).max(1.0).min(big) * reach.clamp(0.0, 1.0);
        let ex = (ox + sx * t).clamp(0.0, w - 1.0);
        let ey = (oy + cy * t).clamp(0.0, floor.max(0.0));
        (ex as i32, ey as i32)
    }

    /// Rebuilds the room on a size or colour change. The only place this family allocates.
    fn resize(&mut self, t: &Theme, w: i32, h: i32) {
        let room = Room::at(w, h);
        self.room = room;
        if self.crowd_dim != (w, h) {
            self.crowd_dim = (w, h);
            self.seat_crowd(w, h);
        }
        let key = room_key(t, w, h);
        if key == self.haze_key {
            return;
        }
        self.haze_key = key;
        let (iw, ih) = (room.iw.max(1), room.ih.max(1));
        if self.haze.width() != iw * 2 || self.haze.height() != ih {
            self.haze = Canvas::new(iw * 2, ih);
        }
        self.fog.clear();
        self.fog.resize((iw * 2 * ih) as usize, 0);
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let ghost = if t.ghost.is_finite() { t.ghost.clamp(0.0, 1.0) } else { 0.3 };
        let split = ih / 2;
        let floor_row = (room.floor as i32 - room.iy0).clamp(0, ih - 1);
        for y in 0..ih {
            let fy = y as f32 / (ih - 1).max(1) as f32;
            // Two bands: the upper one thick under the truss where the heads light it, the lower one
            // pooled above the crowd. Each row belongs to one band, so each scrolls by its own offset.
            let (centre, spread, seed) =
                if y < split { (0.28, 0.26, 0x0001_2345u32) } else { (0.72, 0.26, 0x9e37_79b9) };
            let env = (1.0 - ((fy - centre) / spread).powi(2)).max(0.0);
            // The light coming back off the floor: a ramp up to the crowd's heads, x-uniform.
            let below = (floor_row - y) as f32;
            let glow = if below >= 0.0 { (-below / (ih as f32 * 0.13)).exp() } else { 1.0 };
            for x in 0..iw * 2 {
                let n = 0.62 * haze_noise(x, y, iw, 96.0, 22.0, seed)
                    + 0.38 * haze_noise(x, y, iw, 38.0, 11.0, seed ^ 0x5bd1_e995);
                // Contrast it up a little: fog has clear patches, not a flat grey.
                let n = ((n - 0.30) / 0.70).clamp(0.0, 1.0);
                // Squared: fog has clear patches and dense billows, not an even grey.
                let dens = env * n * n;
                self.fog[(y * iw * 2 + x) as usize] = (dens.clamp(0.0, 1.0) * 255.0) as u8;
                let mix = ghost * (HAZE_K * dens + FLOOR_GLOW_K * glow * (0.6 + 0.4 * n));
                self.haze.fill_rect(x, y, 1, 1, Rgba::lerp_linear(panel, edge, mix.clamp(0.0, 0.9)));
            }
        }
    }

    /// Hashes the crowd into place for a panel size: a back row and a front row, the pitch and height
    /// varied so it reads as people rather than a fence.
    fn seat_crowd(&mut self, w: i32, h: i32) {
        let room = Room::at(w, h);
        let big = room.ih >= 48;
        let (pitch, spread) = if big { (7, 4) } else { (6, 3) };
        let mut n = 0;
        let mut i = 0u32;
        for back in [true, false] {
            let mut x = room.ix0 + if back { 1 + pitch / 2 } else { 2 };
            while n < MAX_PEOPLE && x < room.ix0 + room.iw - 1 {
                let s = (w as u32).wrapping_mul(2_654_435_761) ^ (h as u32).wrapping_mul(40_503) ^ i;
                self.people[n] = Person {
                    x,
                    dy: (hash(s ^ 0xa5a5) % 2) as i32 - if back { 2 } else { 0 },
                    back,
                    raises: hash01(s ^ 0x5a5a) < RAISERS,
                    arms: (hash(s ^ 0x3c3c) % 3) as u8,
                    arm: 0.0,
                    hold: 0.0,
                    wave: 0,
                };
                n += 1;
                i += 1;
                x += pitch - 1 + (hash(s) % spread as u32) as i32;
            }
        }
        self.n_people = n;
    }

    /// Starts an arm wave from the left-hand wall. `all` is a drop: everyone, not just the raisers.
    fn raise_arms(&mut self, all: bool) {
        self.wave_id = self.wave_id.wrapping_add(1).max(1);
        self.wave_x = 0.0;
        self.wave_all = all;
    }

    /// Advances the arm wave and every arm in it.
    fn update_arms(&mut self, dt: f32) {
        if self.wave_x.is_finite() {
            self.wave_x += dt * WAVE_PX_PER_MS;
        }
        let (front, id, all) = (self.wave_x, self.wave_id, self.wave_all);
        for p in self.people[..self.n_people].iter_mut() {
            if front.is_finite() && p.x as f32 <= front && p.wave != id {
                p.wave = id;
                if p.raises || all {
                    p.hold = ARM_HOLD_MS;
                }
            }
            if p.hold > 0.0 {
                p.hold -= dt;
                p.arm += (1.0 - p.arm) * (dt / 45.0).min(1.0);
            } else {
                p.arm = (p.arm - dt / ARM_FALL_MS).max(0.0);
            }
            if !p.arm.is_finite() || !p.hold.is_finite() {
                p.arm = 0.0;
                p.hold = 0.0;
            }
        }
        if self.wave_x > self.crowd_dim.0 as f32 + 4.0 {
            self.wave_x = f32::INFINITY;
        }
    }
}

/// Where the fog is, for the cones: the density bake plus this frame's scroll offsets.
struct Fog<'a> {
    dens: &'a [u8],
    stride: i32,
    ix0: i32,
    iy0: i32,
    iw: i32,
    ih: i32,
    split: i32,
    offs: [i32; 2],
}

impl Fog<'_> {
    /// How much light the smoke at (x, y) catches: never none, since a beam in clear air still shows a
    /// little, and densest where the visible haze is.
    fn catch(&self, x: i32, y: i32) -> f32 {
        let (fx, fy) = (x - self.ix0, y - self.iy0);
        if fx < 0 || fy < 0 || fx >= self.iw || fy >= self.ih {
            return FOG_FLOOR;
        }
        let off = self.offs[(fy >= self.split) as usize];
        let d = self.dens.get((fy * self.stride + fx + off) as usize).copied().unwrap_or(0);
        FOG_FLOOR + (1.0 - FOG_FLOOR) * FOG_GAIN.min(d as f32 / 255.0 * FOG_GAIN)
    }
}

/// A beam's soft cone: a filled wedge from the head to the tip, `hw` its half-width (px) at the head
/// and its growth per px of length, its alpha scaled per pixel by the smoke it passes through.
/// Allocates nothing.
#[allow(clippy::too_many_arguments)]
fn cone(c: &mut Canvas, fog: &Fog, ox: f32, oy: f32, ex: f32, ey: f32, hw: (f32, f32), col: Rgba) {
    let dy = ey - oy;
    if dy < 2.0 || col.a == 0 {
        return;
    }
    let dx = ex - ox;
    let len = dx.hypot(dy);
    // A slanted beam's horizontal cross-section is wider than its perpendicular one.
    let sec = (len / dy).min(4.0);
    let step = (dx / dy).abs() * 0.5;
    let (y0, y1) = (oy as i32, ey as i32);
    let w0 = hw.0;
    let w1 = hw.0 + hw.1 * len;
    let base = col.a as f32 / 255.0;
    for y in y0.max(0)..=y1.min(c.height() - 1) {
        let f = (y as f32 - oy) / dy;
        let xc = ox + dx * f;
        let half = (w0 + (w1 - w0) * f) * sec + step;
        let a = ((xc - half).round() as i32).max(0);
        let b = ((xc + half).round() as i32).min(c.width() - 1);
        for x in a..=b {
            c.fill_rect(x, y, 1, 1, with_alpha(col, base * fog.catch(x, y)));
        }
    }
}

/// An elliptical splash, row by row.
fn splash(c: &mut Canvas, cx: i32, cy: i32, rx: f32, ry: i32, col: Rgba) {
    for dy in -ry..=ry {
        let f = dy as f32 / (ry as f32 + 0.6);
        let half = (rx * (1.0 - f * f).max(0.0).sqrt()).round() as i32;
        c.fill_rect(cx - half, cy + dy, half * 2 + 1, 1, col);
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
        self.resize(t, w, h);
        let room = self.room;
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 250.0) } else { 16.7 };
        let secs = dt / 1000.0;
        let time_s = if d.time_s.is_finite() { d.time_s } else { 0.0 };

        // ---- the room: the haze, two bands scrolling opposite ways ----
        //
        // A plain copy, not a blend: the haze was baked already composited over the panel, so this IS
        // the panel interior. Each row belongs to one band - see `resize`.
        let split = room.ih / 2;
        let mut offs = [0i32; 2];
        for (band, speed) in HAZE_DRIFT.iter().enumerate() {
            let off = (time_s * speed).rem_euclid(room.iw.max(1) as f32) as i32;
            offs[band] = off;
            let (r0, r1) = if band == 0 { (0, split) } else { (split, room.ih) };
            c.copy_region(&self.haze, (off, r0), (room.ix0, room.iy0 + r0), room.iw, r1 - r0);
        }

        // ---- the kick ----
        let nb = KICK_BANDS.min(d.levels.len());
        let kicked = self.kick.update(&d.levels[..nb], dt, KICK_RATIO, KICK_REFRACTORY_MS);
        let mut accent = false;
        if kicked {
            self.kicks = self.kicks.wrapping_add(1);
            accent = self.kicks.is_multiple_of(ACCENT_EVERY);
            self.strobe = if accent { STROBE_ACCENT } else { STROBE_KICK };
            self.strobe_peak = self.strobe;
            self.since_kick_ms = 0.0;
            // An IMPULSE into the spring, alternating direction so consecutive kicks throw the rig the
            // other way instead of pumping it further in one.
            let dir = if self.kicks.is_multiple_of(2) { 1.0 } else { -1.0 };
            self.snap_v += dir * SNAP_RAD * std::f32::consts::TAU * SNAP_HZ;
        } else {
            self.since_kick_ms = (self.since_kick_ms + dt).min(1.0e4);
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
        let drive = if drive.is_finite() { drive } else { 0.0 };
        let want = APERTURE_CALM + (APERTURE_WILD - APERTURE_CALM) * drive;
        // The blast overrides it to full spread, which is the event.
        let want = want + (APERTURE_WILD - want) * blast;
        self.aperture += (want - self.aperture) * (APERTURE_FOLLOW * dt).min(1.0);
        if !self.aperture.is_finite() {
            self.aperture = APERTURE_CALM;
        }
        let aperture = self.aperture.clamp(0.0, APERTURE_WILD);

        // ---- the crowd's arms: a wave on a strong onset, everyone on the drop ----
        if fired {
            self.raise_arms(true);
        } else if accent && drive > ARMS_DRIVE {
            self.raise_arms(false);
        }
        self.update_arms(dt);

        // ---- the strobe wash ----
        //
        // Drawn UNDER the beams, so a bright panel never washes out the thing that carries the level.
        // The blast darkens instead of brightening - see BLAST_MS.
        //
        // The wash is the strobe SHAPED to a sharper tail: strobe^1.5 / peak^0.5, which is the full peak
        // on the kick frame and 3% of it once the strobe itself has decayed to 10%. In linear light the
        // plain exponential tail left a 3% sheet of `lit` over the room for most of every beat - a raised
        // floor, the panel reading as one flat colour, which was half of "very bland". The strobe state
        // (and every test of it) is untouched; only what it paints is.
        let peak = self.strobe_peak.max(1.0e-4);
        let shaped = if self.strobe.is_finite() && self.strobe > 0.0 {
            self.strobe * (self.strobe / peak).min(1.0).sqrt()
        } else {
            0.0
        };
        let wash = (shaped * (1.0 - blast) - BLAST_DARK * blast).clamp(-1.0, 1.0);
        if wash > 0.0 {
            let hue = (self.kicks as f32 * 0.137).rem_euclid(1.0);
            // `t.lit`, the SATURATED colour, not `t.hot`. A near-white sheet at this alpha turns the panel
            // pastel and the beams - which carry the level - end up the lowest-contrast thing on screen.
            // A saturated wash reads as violent while leaving the beams' near-white cores well clear of it.
            let flash = crate::render::tint(t, hue, time_s, false, &t.lit, wash.min(1.0));
            c.fill_rect(room.ix0, room.iy0, room.iw, room.ih, flash);
        } else if wash < 0.0 {
            c.fill_rect(room.ix0, room.iy0, room.iw, room.ih, Rgba::from_hex(&t.panel, (-wash).min(1.0)));
        }

        // ---- the beams: a core line in a soft cone, landing in a splash ----
        let fog = Fog {
            dens: &self.fog,
            stride: room.iw * 2,
            ix0: room.ix0,
            iy0: room.iy0,
            iw: room.iw,
            ih: room.ih,
            split,
            offs,
        };
        let (wf, oy, floor) = (w as f32, room.oy, room.floor);
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
        let flip = self.kicks.is_multiple_of(2);
        let hue = (self.kicks as f32 * 0.137).rem_euclid(1.0);
        let core_hex = if flip { &t.hot } else { &t.lit };
        let core = crate::render::tint(t, hue, time_s, flip, core_hex, 1.0);
        let side = with_alpha(core, CORE_SIDE_A);
        let inner = with_alpha(core, CONE_INNER_A);
        let outer = with_alpha(core, CONE_OUTER_A);
        let splash_c = with_alpha(core, SPLASH_A);
        let period_s = (SWEEP_MS / 1000.0).max(0.001);
        let tip_r = if room.ih >= 48 { (8.0, 2) } else { (5.0, 1) };
        // The cones scale with the panel, so a 128x44 fan is still a fan rather than a wedge.
        let cs = (room.ih as f32 / 54.0).clamp(0.5, 1.5);
        for e in 0..EMITTERS {
            let ox = (wf * EMITTER_X[e]).round();
            // Each head sweeps at its own phase, and the kick throws alternate heads the OTHER way - so
            // the rig crosses itself instead of moving as one slab. See EMITTER_PHASE.
            let phase = e as f32 * EMITTER_PHASE * period_s;
            let sweep = SWEEP_AMP * ((time_s + phase) * std::f32::consts::TAU / period_s).sin();
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
                let (ex, ey) = Self::ray_end(ox, oy, a, wf, floor, reach);
                cone(c, &fog, ox, oy, ex as f32, ey as f32, (CONE_OUTER.0 * cs, CONE_OUTER.1 * cs), outer);
                cone(c, &fog, ox, oy, ex as f32, ey as f32, (CONE_INNER.0 * cs, CONE_INNER.1 * cs), inner);
                c.line(ox as i32 - 1, oy as i32, ex - 1, ey, side);
                c.line(ox as i32 + 1, oy as i32, ex + 1, ey, side);
                c.line(ox as i32, oy as i32, ex, ey, core);
                // Where it meets the floor, it splashes.
                if ey as f32 >= floor - 1.0 {
                    splash(c, ex, floor as i32 - 1, tip_r.0, tip_r.1, splash_c);
                }
            }
        }

        // ---- the flourish's moving head: one white beam swept across the room ----
        if blast > 0.0 {
            let p = 1.0 - blast;
            let p = p * p * (3.0 - 2.0 * p);
            let a = -MOVER_SWEEP + 2.0 * MOVER_SWEEP * p;
            let white = Rgba::new(255, 255, 255, 255);
            let ox = (wf * 0.5).round();
            let (ex, ey) = Self::ray_end(ox, oy, a, wf, floor, 1.0);
            let fade = blast.min(0.25) / 0.25;
            cone(c, &fog, ox, oy, ex as f32, ey as f32, (2.5, 0.16), with_alpha(white, 0.10 * fade));
            cone(c, &fog, ox, oy, ex as f32, ey as f32, (1.5, 0.07), with_alpha(white, 0.28 * fade));
            for dx in -1..=1 {
                c.line(ox as i32 + dx, oy as i32, ex + dx, ey, with_alpha(white, fade));
            }
            if ey as f32 >= floor - 1.0 {
                splash(c, ex, floor as i32, tip_r.0 * 2.0, tip_r.1 + 1, with_alpha(white, 0.35 * fade));
            }
        }

        // ---- the truss and the heads hanging off it ----
        let steel = truss_colour(t);
        let (x0, x1) = (room.ix0, room.ix0 + room.iw);
        c.fill_rect(x0, room.rail_top, room.iw, 1, steel);
        c.fill_rect(x0, room.rail_bot, room.iw, 1, steel);
        let gap = room.rail_bot - room.rail_top;
        let mut x = x0;
        let mut up = false;
        while x < x1 {
            let (ya, yb) = if up { (room.rail_bot, room.rail_top) } else { (room.rail_top, room.rail_bot) };
            c.line(x, ya, (x + gap).min(x1 - 1), yb, steel);
            x += gap;
            up = !up;
        }
        let housing = Rgba::lerp_linear(steel, Rgba::from_hex(&t.hot, 1.0), 0.12);
        for e in 0..EMITTERS {
            let ox = (wf * EMITTER_X[e]).round() as i32;
            c.fill_rect(ox - 1, room.rail_bot, 3, 1, housing); // the clamp
            c.fill_rect(ox - 3, room.head_y, 7, room.head_h, housing);
            c.fill_rect(ox - 1, room.head_y + room.head_h - 1, 3, 1, core); // the aperture
        }
        if blast > 0.0 {
            let ox = (wf * 0.5).round() as i32;
            c.fill_rect(ox - 1, room.head_y + room.head_h - 1, 3, 1, Rgba::new(255, 255, 255, 255));
        }

        // ---- the crowd ----
        let ink = crowd_colour(t);
        let ink_back = Rgba::lerp_linear(ink, Rgba::from_hex(&t.edge, 1.0), 0.05);
        let bob = if self.since_kick_ms < BOB_MS { 1 } else { 0 };
        let prof = room.profile;
        let last = prof[prof.len() - 1];
        for p in self.people[..self.n_people].iter() {
            let col = if p.back { ink_back } else { ink };
            let top = room.crowd_top + p.dy + bob;
            for (r, wd) in prof.iter().enumerate() {
                c.fill_rect(p.x - wd / 2, top + r as i32, *wd, 1, col);
            }
            let body = top + prof.len() as i32;
            c.fill_rect(p.x - last / 2, body, last, room.bottom - body + 1, col);
            if p.arm > 0.02 {
                let len = (p.arm * room.arm_len as f32).round() as i32;
                let shoulder = top + room.shoulder_row;
                let half = prof[room.shoulder_row as usize + 1] / 2;
                for s in [-1i32, 1] {
                    let wanted = match p.arms {
                        0 => s < 0,
                        1 => s > 0,
                        _ => true,
                    };
                    if !wanted || len < 2 {
                        continue;
                    }
                    // A stroke up from the shoulder, the hand leaning out a pixel.
                    let ax = if s < 0 { p.x - half } else { p.x + half - room.arm_w };
                    c.fill_rect(ax, shoulder - len + 2, room.arm_w, len - 1, col);
                    c.fill_rect(ax + if s < 0 { -1 } else { room.arm_w }, shoulder - len, 1, 3, col);
                }
            }
        }

        // The beams spread outward from the heads, so the last thing that happens is clipping back
        // inside the panel this family drew.
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
        let hit = k.is_multiple_of(period_frames);
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

    /// WCAG relative luminance of a pixel, from the same decode table the blend uses.
    fn rel_lum(p: Rgba) -> f32 {
        let d = Rgba::srgb_to_linear;
        0.2126 * d(p.r) + 0.7152 * d(p.g) + 0.0722 * d(p.b)
    }

    /// (median, maximum) relative luminance over the panel interior: the median is the washed
    /// panel - the beams are thin, so they never reach it - and the maximum is a beam core.
    fn wash_and_core(c: &Canvas) -> (f32, f32) {
        let mut ys: Vec<f32> = (4..c.height() - 4)
            .flat_map(|y| (4..c.width() - 4).map(move |x| (x, y)))
            .map(|(x, y)| rel_lum(c.get(x, y)))
            .collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        (ys[ys.len() / 2], *ys.last().unwrap())
    }

    /// Paint over the room: brighter than the haze ever gets on its own. The room is no longer the flat
    /// panel colour, so "not the panel colour" would count every pixel of fog as ink.
    fn is_lit(p: Rgba, t: &Theme) -> bool {
        let fog = rel_lum(haze_ceiling(t));
        p.a > 0 && rel_lum(p) > fog * 1.15 + 0.002
    }

    fn lit_count(c: &Canvas, t: &Theme) -> i32 {
        let mut n = 0;
        for y in 3..c.height() - 3 {
            for x in 2..c.width() - 2 {
                if is_lit(c.get(x, y), t) {
                    n += 1;
                }
            }
        }
        n
    }

    /// Silence: no kick, no flourish, the beams at their floor-length stubs.
    fn silent(t_s: f32) -> FrameData {
        FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() }
    }

    fn snapshot(c: &Canvas) -> Vec<Rgba> {
        (0..c.height()).flat_map(|y| (0..c.width()).map(move |x| (x, y))).map(|(x, y)| c.get(x, y)).collect()
    }

    /// The haze is a room, not a backdrop: it drifts even when nothing is playing.
    ///
    /// Measured below the beams' silent stubs and above the crowd, so the only thing that can change
    /// there between two silent frames a second apart is the fog.
    ///
    /// Mutation: set HAZE_DRIFT to [0.0, 0.0] and the two frames are identical.
    #[test]
    fn haze_drifts() {
        let t = builtin::rave_frenchcore();
        let mut fam = Rave::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..30 {
            fam.draw(&mut c, &t, &silent(k as f32 * 0.0167));
        }
        fam.draw(&mut c, &t, &silent(1.0));
        let a = snapshot(&c);
        for k in 1..=60 {
            fam.draw(&mut c, &t, &silent(1.0 + k as f32 * 0.0167));
        }
        let room = fam.room;
        let (y0, y1) = (room.oy as i32 + 18, room.crowd_top - 4);
        assert!(y1 - y0 >= 8, "no clear band to measure in: rows {y0}..{y1}");
        let mut differ = 0;
        for y in y0..y1 {
            for x in room.ix0..room.ix0 + room.iw {
                if a[(y * 380 + x) as usize] != c.get(x, y) {
                    differ += 1;
                }
            }
        }
        assert!(differ > 300, "the haze did not drift: {differ} pixels changed in a second of silence");
    }

    /// A beam is light through smoke: its line sits inside a dimmer cone, so the pixels just outside
    /// the core are lit - less than the core, more than the room.
    ///
    /// Mutation: skip the two `cone` calls for the fan and the flanks fall to the room's own level.
    #[test]
    fn beams_are_cones_in_the_haze() {
        let t = builtin::rave_frenchcore();
        let mut fam = Rave::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..90 {
            fam.draw(&mut c, &t, &frame(0.9, k as f32 * 0.0001));
        }
        let room = fam.room;
        let y = room.oy as i32 + 20;
        let lum: Vec<f32> = (0..380).map(|x| rel_lum(c.get(x, y))).collect();
        // The room on this row: its darker fifth, which the beams - thin, and few - never reach.
        let mut sorted = lum.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let room_lum = sorted[sorted.len() / 5];
        let mut cones = 0;
        let mut cores = 0;
        for x in 4..376 {
            let v = lum[x];
            if v < 0.5 || v < lum[x - 1] || v < lum[x + 1] {
                continue;
            }
            cores += 1;
            let (l, r) = (lum[x - 2], lum[x + 2]);
            let lit = |f: f32| f > room_lum * 2.0 + 0.02 && f < v * 0.6;
            if lit(l) && lit(r) {
                cones += 1;
            }
        }
        assert!(cores >= 6, "found only {cores} beam cores on row {y}");
        assert!(
            cones * 2 >= cores,
            "only {cones} of {cores} beams have a lit, dimmer cone either side of the core"
        );
    }

    /// The crowd's arms go up on a strong onset - an accent kick on loud music - and come down again.
    ///
    /// Counted as crowd-silhouette pixels ABOVE every head, which only an arm can put there.
    ///
    /// Mutation: set ARMS_DRIVE to 2.0 and no wave starts; set ARM_FALL_MS to 1e9 and the arms never
    /// come down.
    #[test]
    fn the_crowd_raises_arms_on_strong_onsets() {
        let t = builtin::rave_frenchcore();
        let mut fam = Rave::default();
        let mut c = Canvas::new(380, 60);
        let front = crowd_colour(&t);
        let back = Rgba::lerp_linear(front, Rgba::from_hex(&t.edge, 1.0), 0.05);
        let arms = |fam: &Rave, c: &Canvas| {
            let room = fam.room;
            let mut n = 0;
            for y in room.head_y + room.head_h + 2..room.crowd_top - 2 {
                for x in room.ix0..room.ix0 + room.iw {
                    let p = c.get(x, y);
                    if p == front || p == back {
                        n += 1;
                    }
                }
            }
            n
        };
        fam.draw(&mut c, &t, &silent(0.0));
        assert_eq!(arms(&fam, &c), 0, "arms are up before anything has happened");
        // Loud kicks on the grid: the fourth is an accent, which is a strong onset.
        let mut most = 0;
        for k in 0..100 {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.9));
            most = most.max(arms(&fam, &c));
        }
        assert!(most > 40, "no arms went up on the accent kicks: at most {most} arm pixels");
        for k in 100..250 {
            fam.draw(&mut c, &t, &silent(k as f32 * 0.0167));
        }
        assert_eq!(arms(&fam, &c), 0, "the arms never came down");
    }

    /// The heads hang off a truss: a lattice across the top rows, and a housing under each head.
    ///
    /// Mutation: skip the truss block and both checks fail.
    #[test]
    fn the_truss_hangs_the_heads() {
        let t = builtin::rave_frenchcore();
        let mut fam = Rave::default();
        let mut c = Canvas::new(380, 60);
        fam.draw(&mut c, &t, &silent(0.0));
        let room = fam.room;
        let steel = truss_colour(&t);
        let mut n = 0;
        for y in room.rail_top..=room.rail_bot {
            for x in room.ix0..room.ix0 + room.iw {
                if c.get(x, y) == steel {
                    n += 1;
                }
            }
        }
        assert!(n > room.iw * 2, "no truss across the top: {n} steel pixels");
        let housing = Rgba::lerp_linear(steel, Rgba::from_hex(&t.hot, 1.0), 0.12);
        for ex in EMITTER_X {
            let x = (380.0 * ex).round() as i32;
            assert_eq!(c.get(x - 3, room.head_y), housing, "no head hanging at x={x}");
        }
    }

    /// A 200bpm kick must be detected at 200bpm - the whole premise. At 60fps that is one every 18
    /// frames, and the detector's refractory has to clear a 300ms period with room for a roll.
    ///
    /// Mutation: raise KICK_REFRACTORY_MS to 320 and the 200bpm count collapses; raise KICK_RATIO to 4.0
    /// and nothing fires at all.
    /// Slow (~40.5s in debug); gated out of the default suite. A two hundred bpm kick is caught on every beat. Run: `cargo test --release slow_ -- --ignored`.
    #[test]
    #[ignore]
    fn slow_a_two_hundred_bpm_kick_is_caught_on_every_beat() {
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
    /// Slow (~9.0s in debug); gated out of the default suite. Louder music spreads the fan. Run: `cargo test --release slow_ -- --ignored`.
    #[test]
    #[ignore]
    fn slow_louder_music_spreads_the_fan() {
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
            let (mut l, mut r) = (0, 0);
            for y in 3..57 {
                for x in 2..378 {
                    if !is_lit(c.get(x, y), &t) {
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
    /// Slow (~6.7s in debug); gated out of the default suite. The strobe fires on the kick accents every fourth and clears before the next. Run: `cargo test --release slow_ -- --ignored`.
    #[test]
    #[ignore]
    fn slow_the_strobe_fires_on_the_kick_accents_every_fourth_and_clears_before_the_next() {
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
    /// Mutation: set BLAST_DARK to 0.0 and the room no longer darkens.
    /// Slow (~5.2s in debug); gated out of the default suite. The flourish blacks the rig out. Run: `cargo test --release slow_ -- --ignored`.
    #[test]
    #[ignore]
    fn slow_the_flourish_blacks_the_rig_out() {
        let t = builtin::rave_frenchcore();
        let mut fam = Rave::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..120 {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
        }
        // The ROOM's light, not the beams': the blast opens the whole fan and sweeps the moving head,
        // so beam ink goes UP during it. The hole is the haze going dark around them.
        let room_light = |c: &Canvas| -> f32 {
            let mut sum = 0.0;
            for y in 3..c.height() - 3 {
                for x in 2..c.width() - 2 {
                    let p = c.get(x, y);
                    if !is_lit(p, &t) {
                        sum += rel_lum(p);
                    }
                }
            }
            sum
        };
        let before = room_light(&c);
        fam.flourish.force_next();
        let mut darkest = before;
        for k in 120..150 {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
            darkest = darkest.min(room_light(&c));
        }
        assert!(
            darkest < before * 0.8,
            "the blast did not darken the room: {before:.1} of light before, {darkest:.1} at its darkest"
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
        println!("rave steady: {:.3} ms/frame at 380x60", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);

        // The flourish: the blackout, the moving head's sweep and the whole crowd's arms, over the
        // blast's whole envelope.
        fam.flourish.force_next();
        let m = 54;
        let t1 = std::time::Instant::now();
        for k in n..(n + m) {
            fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
        }
        println!("rave flourish: {:.3} ms/frame at 380x60", t1.elapsed().as_secs_f64() * 1000.0 / m as f64);
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
                sm.update(&target, 16.667);
                let lv = sm.levels();
                if flux.update(&lv[..KICK_BANDS], 16.7, KICK_RATIO, KICK_REFRACTORY_MS) {
                    n += 1;
                }
            }
            let mins = frames as f32 * 16.7 / 60_000.0;
            println!("{name:<15} {:>7.1} kicks/min", n as f32 / mins);
        }
    }

    /// The strobe wash must never swallow the beams. Measured on the accent kick - the brightest
    /// wash the family produces - after 20 kicks at 200bpm, which is the frame `dump_rave` lands on
    /// and where this regressed: when blending moved to linear light, an alpha-0.62 wash became a
    /// flat sheet and the near-white beams were the lowest-contrast thing on screen (frenchcore
    /// 2.26:1, strobe 2.33:1 at the old constants). The rule is the family's own: the beam core
    /// must clear the colourway's contrast floor against the washed panel.
    ///
    /// Mutation: put STROBE_ACCENT back to 0.62 and frenchcore and strobe fail.
    /// Slow (~30.8s in debug); gated out of the default suite. The accent strobe leaves the beams readable. Run: `cargo test --release slow_ -- --ignored`.
    #[test]
    #[ignore]
    fn slow_the_accent_strobe_leaves_the_beams_readable() {
        for t in builtin::all().into_iter().filter(|t| t.family == "rave") {
            let mut fam = Rave::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..361 {
                fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.8));
            }
            assert_eq!(fam.kicks % ACCENT_EVERY, 0, "{}: frame 360 must be an accent kick", t.id);
            assert!(fam.strobe > STROBE_KICK, "{}: the accent wash must be up on its own frame", t.id);
            let (panel, core) = wash_and_core(&c);
            let contrast = (core + 0.05) / (panel + 0.05);
            assert!(
                contrast >= t.contrast_floor,
                "{}: at the accent kick the beam core ({core:.3}) only reaches {contrast:.2}:1 against the                  washed panel ({panel:.3}); the wash has swallowed the beams",
                t.id
            );
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

        // The fidelity set: calm, loud and the flourish (~300 ms in) on two colourways, and 128x44.
        for (id, w, h) in [("rave-frenchcore", 380, 60), ("rave-uv", 380, 60), ("rave-frenchcore", 128, 44)] {
            let t = builtin::all().into_iter().find(|t| t.id == id).unwrap();
            let tag = if w == 380 { id.to_string() } else { format!("{id}-{w}x{h}") };
            for (gain, state) in [(0.12f32, "calm"), (0.9, "loud")] {
                let mut fam = Rave::default();
                let mut c = Canvas::new(w, h);
                // Frame 372: 200 ms after the accent kick at 360 - the strobe gone, the arm wave mid-room.
                for k in 0..373 {
                    fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, gain));
                }
                write(format!("fid-{tag}-{state}"), &c);
            }
            let mut fam = Rave::default();
            let mut c = Canvas::new(w, h);
            for k in 0..180 {
                fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.85));
            }
            fam.flourish.force_next();
            for k in 180..199 {
                fam.draw(&mut c, &t, &kick_frame(k as f32 * 0.0167, 18, k, 0.85));
            }
            write(format!("fid-{tag}-flourish"), &c);
        }
    }
}
