//! The reel-to-reel tape family: two spoked reels turning over a record-level strip.
//!
//! Every other family in this project answers "how loud is band N" with a shape that is
//! redrawn from scratch each frame. This one is the only family with a MEMORY: the reels
//! carry a rotation phase and an angular rate that integrate the music rather than sample
//! it, so loudness reads as SPEED. That is the whole point of the family, and it is the
//! reason rule 6 (position beats intensity) is satisfied twice over here - once by motion,
//! once by the tape.
//!
//! Three cues, deliberately independent so the panel is not three copies of one number:
//!
//! - **Rotation = overall loudness**, integrated through a flywheel (`omega`). Angular rate
//!   is a *velocity* cue, which is why the reels have to be SPOKED: a plain disc rotating is
//!   invisible, and measured on the first draft (a disc with a rim highlight) the frame-to-
//!   frame pixel difference inside the reel was 0 - literally nothing moved on screen.
//! - **Tape sag = the bass group's level**, and this is the family's POSITION cue: the free
//!   span between the reels dips, and at 190x60 its mid-span travels ~15 rows between quiet
//!   and loud. A row is the most legible unit this canvas has. The head stack in the middle
//!   of the deck is not decoration - it gives the sag a fixed thing to be measured against,
//!   the way a VU dial's printed scale does for the needle.
//! - **The record-level strip = per-band levels**, growing bars, as the secondary readout a
//!   deck's meters actually are.
//!
//! Bass drives the sag rather than the same broadband figure the reels use, so the two cues
//! cannot be redundant. Physically it is also the right band: it is bass that slaps a tape
//! loop around, not cymbals.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// Band level at which the transport starts to respond, and the span it responds over.
///
/// The same fixed window `tube.rs` uses, and for the same measured reason: `FrameData.levels`
/// only reaches about 0.15-0.65 for active bands, so anything mapped linearly over 0..1 spends
/// a third of its range and looks dead on real music. Fixed rather than a peak follower -
/// a tape deck's meters and its tape speed are absolute readings, and a follower would present
/// a quiet passage at the same reel speed as a loud one.
const RESP_FLOOR: f32 = 0.12;
const RESP_SPAN: f32 = 0.50;

/// Weight given to a group's LOUDEST band rather than its mean, when several of the 64 bands
/// feed one element.
///
/// `tube.rs` measured the cost of a plain mean at 1.46 dL* between a driven element and its
/// neighbour, below the ~2.3 dL* visible threshold; the strip here is finer-grained (about one
/// band per bar at 190px) but the bass group still spans ~21 bands, where a mean would flatten
/// exactly the kick that should throw the tape.
const GROUP_MAX_BIAS: f32 = 0.65;

/// Max bias for the BROADBAND figure that drives reel speed.
///
/// Much lower than `GROUP_MAX_BIAS` on purpose. "Overall loudness" genuinely wants a mean -
/// with the group bias applied across all 64 bands, a single sharp band spun the reels up as
/// hard as a full mix, which reads as the transport twitching rather than running. Not zero,
/// though: a mix with one dominant instrument should still get the reels moving.
const OVERALL_MAX_BIAS: f32 = 0.30;

/// Reference frame duration, for the same reason `vapor.rs` has one: the render loop sleeps a
/// fixed 16ms, so its real period is that plus however long the frame took. Rotation is an
/// integral, so a per-frame step would make the reels visibly speed up whenever the machine is
/// idle and slow down under load.
const NOMINAL_DT_MS: f32 = 16.7;

/// Angular rate at silence and at full drive, degrees per second.
///
/// The floor is not zero, mirroring `tube.rs`'s heater floor: a threaded deck that stops dead
/// reads as BROKEN, not as quiet, and the identity of this family is a transport that is
/// running. It is set just above the rate at which rotation stops being perceptible at all -
/// at a 190x60 panel the spoke tip sits ~16px from the hub, so one pixel of tip travel is
/// 1/16 rad = 3.6 degrees; 62 deg/s is ~1.03 deg/frame at 60fps, i.e. about a third of a pixel
/// per frame, which reads as a slow creep. Below ~40 deg/s it looks stationary.
///
/// ALIASING is the trap at the top end, and it is why the ceiling is where it is. A wheel with
/// `SPOKES` arms is indistinguishable from itself every 360/SPOKES = 120 degrees, so advancing
/// exactly one spoke pitch per frame (7200 deg/s here) looks perfectly still, and anything past
/// half a pitch per frame (3600 deg/s) appears to run BACKWARDS. The take-up reel is the fast
/// one, so the real worst case is SPIN_FULL_DPS * TAKEUP_RATIO = 805 deg/s = 13.4 deg/frame -
/// 4.5x below the half-pitch reversal bound and 9x below the standstill bound. Asserted by
/// `the_fastest_reel_stays_far_below_the_spoke_aliasing_bound`, because this constant is exactly
/// the sort of thing a later "make it more responsive" tweak would quietly break.
const SPIN_IDLE_DPS: f32 = 62.0;
const SPIN_FULL_DPS: f32 = 660.0;

/// The take-up reel turns faster than the supply reel because its tape pack is smaller, and
/// tape speed - not angular speed - is what is constant on a real deck.
///
/// Kept because two reels rotating in lockstep read as one object copied twice; at this size
/// the eye catches the drift between them and the transport looks mechanical instead. Each reel
/// keeps its OWN phase rather than scaling one phase by this ratio: the phase wraps at 1.0, and
/// multiplying a wrapped value jumps the second reel backwards every cycle.
const TAKEUP_RATIO: f32 = 1.22;

/// Three, not four. An odd count means the wheel has no mirror symmetry, so the direction of
/// rotation is unambiguous at a glance; with four arms a quarter-turn is indistinguishable from
/// a quarter-turn the other way. It also puts the aliasing bounds (see `SPIN_IDLE_DPS`) at 120
/// and 60 degrees per frame rather than 90 and 45, which is more headroom.
const SPOKES: usize = 3;

/// Flywheel and tape ballistics, per nominal frame, scaled by dt.
///
/// The reel is the slow one deliberately - it is carrying a kilo of tape, and an angular rate
/// that snapped to the music would read as a stepper motor rather than as a transport. ~0.10
/// settles in about 300ms. The tape has almost no mass by comparison, so it tracks nearly three
/// times faster; that difference is most of what makes the two cues look like different
/// physical things rather than one signal drawn twice.
const OMEGA_K: f32 = 0.10;
const SAG_K: f32 = 0.28;

/// Wow and flutter: the flourish. How long the speed instability lasts, its two rates, and its depth.
///
/// A tape machine's characteristic fault is that the tape does not move at a constant speed. Slow
/// variation is WOW - a worn capstan, a dragging pinch roller, a reel that is not running true - and
/// fast variation is FLUTTER, from the scrape of tape across the heads. Both are speed errors, so both
/// belong on the same quantity the reels already read: the angular rate.
///
/// The rates are the real ones. Wow lives around 0.5-6Hz and flutter above that, so 1.1Hz and 8.5Hz
/// are one of each, deliberately not harmonically related - an integer ratio would beat into a single
/// repeating pattern instead of the wandering one a real deck has.
///
/// 8.5Hz against a 60fps display is 7.1 frames a cycle, which is the fastest useful figure here: the
/// display samples the oscillator, so anything approaching 30Hz aliases into a slow wobble that reads
/// as more wow rather than as flutter.
///
/// The depths are theatrical, not authentic. A studio deck holds wow and flutter under 0.1%, which no
/// display could show; these are 0.30 and 0.10, so the rate swings between about 0.6x and 1.4x. That
/// stays inside the family's aliasing budget - `the_fastest_reel_stays_far_below_the_spoke_aliasing_bound`
/// includes the peak multiplier for exactly this reason.
///
/// 2200ms, the longest of any family's flourish. Wow at 1.1Hz needs two full cycles before it reads as
/// periodic rather than as one lurch, and a mechanical fault that clears instantly does not read as
/// mechanical.
const WARBLE_MS: f32 = 2200.0;
const WOW_HZ: f32 = 1.1;
const FLUTTER_HZ: f32 = 8.5;
const WOW_DEPTH: f32 = 0.30;
const FLUTTER_DEPTH: f32 = 0.10;

/// How much of the wow reaches the tape slack, as a fraction of the rate error.
///
/// Not 1.0, and not 0. On a real deck the free span between the reels is where a speed error SHOWS -
/// the supply reel lags, the span goes slack, the take-up pulls it taut again - so a rate wobble with a
/// perfectly steady tape span reads as the reels being wrong rather than the transport being wrong.
/// Well under 1.0 because the sag is also the family's bass cue, and a flourish that swamped it would
/// be overwriting a reading rather than decorating it.
const WARBLE_SAG: f32 = 0.35;

/// Deepest sag, as a multiple of the reel radius, and the shallowest.
///
/// 1.0 x radius is ~20 rows of travel at 190x60. Deeper was tried and the tape reached the head
/// stack before the music was anywhere near loud, so the top of the range was unreadable.
const SAG_SPAN: f32 = 1.0;

/// Per-frame fall of a strip bar's peak cap, in displayed-response units.
///
/// Its own fast fall rather than `FrameData.peaks`, exactly as `tube.rs` found: the shared
/// peak-hold falls at 0.0055/frame, so under continuous music every bar would carry a cap at
/// near-constant height and the caps would read as a second, static bar row.
const MARKER_FALL: f32 = 0.035;

/// Response at which the strip's peak lamp lights.
/// Response at which the peak lamp lights.
///
/// 0.86 made the lamp DEAD CODE. The dB mapping below is correct, but the threshold was picked
/// against a nominal 0..1 scale rather than against what that mapping actually emits over the RMS
/// range the DSP produces: 0.02-0.12 maps to 0.191-0.562, so lighting the lamp needed rms >= 0.508,
/// about 4.3x the real ceiling. It could never light on music.
///
/// 0.54 sits just under the top of typical programme material, so the lamp does what a peak
/// indicator is for - dark most of the time, lit on the loud moments - rather than never or always.
const PEAK_AT: f32 = 0.54;

/// Bottom of the peak lamp's scale in dBFS - the lamp is the only thing here fed from
/// `rms_l`/`rms_r`, which are LINEAR and sit at 0.02-0.12 for real music. `vu.rs` documents why
/// that has to go through dB: mapped linearly, 0.02-0.12 is 2-12% of the scale and a lamp set
/// anywhere sensible would never light.
const REEL_DB_FLOOR: f32 = -42.0;

/// Longest free tape span, in pixels, and the deck margin.
///
/// Capped rather than stretched to the panel. At 380px wide the reels sat 300px apart and the
/// sag curve - the same 20 rows of travel - flattened into what looked like a straight line
/// with a kink: sag is read as a CURVE, and a curve's legibility depends on its aspect, not its
/// depth. So the transport keeps the aspect it was tuned at and the extra width becomes deck
/// plate either side, which is also what a wide deck actually looks like.
const SPAN_MAX: i32 = 104;
const MARGIN: i32 = 4;

/// Maps a raw band level onto 0..1 through the window the DSP actually occupies.
fn remap(level: f32, sensitivity: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    (((level - RESP_FLOOR) / RESP_SPAN) * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

/// Blend of a band group's mean and its max. Non-finite bands are skipped rather than clamped,
/// because `f32::clamp` does NOT sanitise NaN (every comparison with NaN is false, so clamp
/// returns NaN unchanged) and this value feeds `omega` and `sag`, both of which persist across
/// frames and would stay poisoned until the process restarted.
fn group_level(levels: &[f32], lo: usize, hi: usize, max_bias: f32) -> f32 {
    let hi = hi.min(levels.len());
    let lo = lo.min(hi.saturating_sub(1));
    let (mut acc, mut cnt, mut peak) = (0.0f32, 0.0f32, 0.0f32);
    for v in &levels[lo..hi] {
        if v.is_finite() {
            acc += *v;
            cnt += 1.0;
            peak = peak.max(*v);
        }
    }
    if cnt <= 0.0 {
        return 0.0;
    }
    let mean = acc / cnt;
    (mean * (1.0 - max_bias) + peak * max_bias).clamp(0.0, 1.0)
}

/// Linear RMS to 0..1 through dB. Silence maps to exactly 0 rather than -inf.
fn rms_resp(rms: f32, sensitivity: f32) -> f32 {
    if !rms.is_finite() || rms <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * rms.log10();
    (((db - REEL_DB_FLOOR) / -REEL_DB_FLOOR) * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

#[derive(Default)]
pub struct Reel {
    /// Rotation phase in TURNS, wrapping 0->1. Turns rather than radians so wrapping is
    /// `rem_euclid(1.0)` and cannot accumulate the drift a repeated `- TAU` would.
    phase_l: f32,
    phase_r: f32,
    /// Smoothed angular rate of the supply reel, degrees per second.
    omega: f32,
    /// Smoothed sag depth of the free tape span, in pixels.
    sag: f32,
    /// Fast-falling peak hold per strip bar. A `Vec` because the bar count follows the panel
    /// width, and `#[derive(Default)]` has no impl for arrays past 32 anyway.
    marker: Vec<f32>,
    /// The flourish: wow and flutter. See `WARBLE_MS`.
    flourish: crate::dsp::flourish::Trigger,
    warble: crate::dsp::flourish::Envelope,
    /// Seconds since the warble started, driving its two oscillators.
    ///
    /// Its own clock rather than `FrameData::time_s` so the wobble always STARTS at zero phase, which
    /// is where both oscillators cross zero going up. Seeded from a shared wall clock instead, a hit
    /// could land at the bottom of the wow cycle and the transport would appear to lurch before it
    /// wavered.
    warble_t: f32,
}

impl Reel {
    /// Overall loudness across the whole spectrum, which is what the reels turn on.
    fn broadband(d: &FrameData) -> f32 {
        group_level(&d.levels, 0, d.levels.len(), OVERALL_MAX_BIAS)
    }

    /// The bass third, which drives the sag.
    fn bass(d: &FrameData) -> f32 {
        group_level(&d.levels, 0, (d.levels.len() / 3).max(1), GROUP_MAX_BIAS)
    }

    /// One tapered spoke, as a quad from the inner pack radius out to the rim.
    ///
    /// A quad rather than `Canvas::line`: a 1px diagonal arm at this size reads as a scratch
    /// rather than as a structural part of the reel, and it disappears entirely at the angles
    /// where Bresenham puts most of its pixels in one row. The taper (wider at the rim) is what
    /// makes three arms read as a reel rather than as a Y-shaped sticker.
    fn spoke(lit: &mut Canvas, cx: i32, cy: i32, r: i32, turns: f32, col: Rgba, tip: Rgba) {
        let a = turns * std::f32::consts::TAU;
        let (dx, dy) = (a.cos(), a.sin());
        // Perpendicular, for the arm's width.
        let (nx, ny) = (-dy, dx);
        let ri = r as f32 * 0.40;
        let ro = r as f32 * 0.86;
        let hwi = (r as f32 / 12.0).max(1.0);
        let hwo = (r as f32 / 7.0).max(1.5);
        let p = |rad: f32, hw: f32, s: f32| -> (i32, i32) {
            (
                (cx as f32 + dx * rad + nx * hw * s).round() as i32,
                (cy as f32 + dy * rad + ny * hw * s).round() as i32,
            )
        };
        lit.fill_poly(
            &[p(ri, hwi, 1.0), p(ro, hwo, 1.0), p(ro, hwo, -1.0), p(ri, hwi, -1.0)],
            col,
        );
        // A rivet at the tip, in the hot colour. Small, but it is the one feature on the arm
        // that is unambiguously a POINT, so it is what the eye actually tracks around the
        // circle - the arm itself is symmetric enough that a slow turn is hard to follow.
        let (tx, ty) = p(ro, 0.0, 0.0);
        lit.fill_rect(tx, ty, 1, 1, tip);
    }
}

impl Family for Reel {
    fn id(&self) -> &'static str {
        "reel"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);

        // Below this there is no room for a reel with a visible interior plus a strip under it.
        // Fill the deck plate and stop, rather than drawing two unreadable smudges - the same
        // choice `tube.rs` makes, and the reason the degenerate sizes in the dispatch test
        // cannot reach the geometry below.
        if w < 34 || h < 22 {
            c.rounded_rect(1, 2, (w - 2).max(1), (h - 4).max(1), 3, panel);
            return;
        }

        // ---- audio ----

        // dt first, and sanitised before anything derived from it can reach `omega`/`sag`.
        let dt = if d.dt_ms.is_finite() {
            (d.dt_ms / NOMINAL_DT_MS).clamp(0.25, 4.0)
        } else {
            1.0
        };
        let drive = remap(Self::broadband(d), t.sensitivity)
            // ...or the RMS reading, whichever is higher. Both come from the same audio in
            // production, so this is normally the band figure - but the reels are the family's
            // identity and a transport that stops dead because one input path happens to be
            // silent is a failure worth a single `max` to exclude.
            .max(rms_resp(d.rms_l, t.sensitivity).max(rms_resp(d.rms_r, t.sensitivity)));
        let bass = remap(Self::bass(d), t.sensitivity);

        let target = SPIN_IDLE_DPS + (SPIN_FULL_DPS - SPIN_IDLE_DPS) * drive;
        self.omega += (target - self.omega) * (OMEGA_K * dt).min(1.0);
        if !self.omega.is_finite() {
            self.omega = SPIN_IDLE_DPS;
        }

        // THE FLOURISH: wow and flutter. See `WARBLE_MS`.
        //
        // Applied to the phase step rather than to `omega` itself, and that is not a detail: `omega`
        // is a smoothed state that feeds back into itself every frame, so injecting a wobble there
        // would be low-pass filtered by its own OMEGA_K ballistics - the flutter would vanish almost
        // entirely and the wow would arrive late and shallow. The flywheel has inertia; a slipping
        // capstan does not go through it.
        let fired = self.flourish.update(&d.levels, d.dt_ms, t.flourish);
        let warble = self.warble.update(fired, d.dt_ms, WARBLE_MS);
        let wow = if warble > 0.001 {
            self.warble_t += NOMINAL_DT_MS * dt / 1000.0;
            if !self.warble_t.is_finite() {
                self.warble_t = 0.0;
            }
            let tau = std::f32::consts::TAU;
            warble * WOW_DEPTH * (tau * WOW_HZ * self.warble_t).sin()
        } else {
            self.warble_t = 0.0;
            0.0
        };
        let flutter = if warble > 0.001 {
            warble * FLUTTER_DEPTH * (std::f32::consts::TAU * FLUTTER_HZ * self.warble_t).sin()
        } else {
            0.0
        };
        // Floored above zero: a transport that momentarily STOPS or reverses is a different fault
        // (a snapped belt), and this family's identity is a deck that is running.
        let speed = (1.0 + wow + flutter).max(0.15);

        // Integrate with the SANITISED dt, not `d.dt_ms` - see `vapor.rs`, where a NaN dt
        // permanently corrupted the scroll phase.
        let secs = NOMINAL_DT_MS * dt / 1000.0;
        let step = self.omega * speed * secs / 360.0;
        self.phase_l = (self.phase_l + step).rem_euclid(1.0);
        self.phase_r = (self.phase_r + step * TAKEUP_RATIO).rem_euclid(1.0);
        if !self.phase_l.is_finite() || !self.phase_r.is_finite() {
            self.phase_l = 0.0;
            self.phase_r = 0.0;
        }

        // ---- geometry ----

        let strip_h = (h / 8).clamp(3, 8);
        let deck_top = 3;
        let deck_bot = h - 4 - strip_h - 1; // last deck row, above the strip
        // Height caps the reel, never width - the same lesson the VU family learned when a
        // width-derived radius put the dial arc off the top of a 60px panel.
        let r = (((deck_bot - deck_top) / 2 - 1).min(20)).max(3);
        let cy = (deck_top + deck_bot) / 2;
        let transport = (4 * r + SPAN_MAX).min(w - MARGIN * 2);
        let x0 = (w - transport) / 2;
        let cx_l = x0 + r;
        let cx_r = x0 + transport - 1 - r;

        // The free span runs between the reels' 45-degree SHOULDERS, not their tops.
        //
        // Anchoring it at (cx, cy - r) is the obvious version and it is wrong: measured at full
        // sag the curve dropped ~10 rows within the first r pixels, which is still inside the
        // flange, so the tape drew a dark diagonal scar across the top-right of the supply reel
        // and the top-left of the take-up. From the shoulder the curve leaves tangentially and
        // clears the disc at every sag depth this family can produce.
        let shoulder = (r as f32 * std::f32::consts::FRAC_1_SQRT_2).round() as i32;
        let xl = cx_l + shoulder;
        let xr = cx_r - shoulder;
        let yt = cy - shoulder;
        let sag_min = (r as f32 / 8.0).max(2.0);
        let sag_max = ((r as f32 * SAG_SPAN).min((deck_bot - 3 - yt) as f32)).max(sag_min + 1.0);
        // The wow reaches the tape slack too - see `WARBLE_SAG`. Only the wow: flutter is faster
        // than a span of tape carrying any tension can follow, so putting it here would be drawing a
        // vibration the physical object would damp out.
        let sag_target = (sag_min + (sag_max - sag_min) * bass) * (1.0 - wow * WARBLE_SAG);
        self.sag += (sag_target - self.sag) * (SAG_K * dt).min(1.0);
        if !self.sag.is_finite() {
            self.sag = sag_min;
        }
        let sag = self.sag.clamp(0.0, sag_max);

        // ---- deck plate ----

        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        c.vertical_gradient(
            2,
            3,
            w - 4,
            h - 6,
            &[
                (0.0, Rgba::from_hex(&t.tube.chassis_top, 0.62)),
                (1.0, Rgba::from_hex(&t.tube.chassis_bottom, 0.62)),
            ],
            true,
        );
        // Four countersunk screws. Cheap (four pixels each), but they are the detail that keeps
        // a wide panel from reading as an empty plate once the transport stops growing with it.
        let screw = Rgba::from_hex(&t.tube.collar, 0.55);
        let screw_dark = Rgba::from_hex(&t.tube.internals, 0.7);
        for sx in [4, w - 6] {
            for sy in [deck_top + 1, deck_bot - 2] {
                c.fill_rect(sx, sy, 2, 2, screw_dark);
                c.fill_rect(sx, sy, 1, 1, screw);
            }
        }
        // Ventilation slots, but ONLY on plate the capped transport has left bare - so the tuned
        // 190px layout, where the reels reach the margins, is untouched. Without them the wide
        // panel was 100px of dead flat gradient at each end: correct, and boring.
        if x0 > MARGIN + 6 {
            let vent = Rgba::from_hex(&t.tube.internals, 0.30);
            let (vy, vh) = (cy - r / 2, r.max(2));
            let mut vx = MARGIN + 2;
            while vx < x0 - 4 {
                c.fill_rect(vx, vy, 1, vh, vent);
                c.fill_rect(w - 1 - vx, vy, 1, vh, vent);
                vx += 4;
            }
        }

        // ---- record-level strip: the recess, and the dormant bars ----

        let strip_y = h - 4 - strip_h;
        let strip_x0 = 3;
        let strip_x1 = w - 3; // exclusive
        let lamp_w = 3;
        let lamp_x = strip_x1 - lamp_w - 1;
        let pitch = 3;
        let bw = 2;
        let nb = (((lamp_x - 1 - strip_x0) / pitch).max(1)) as usize;
        let bar_top = strip_y + 1;
        let bar_max = (strip_h - 2).max(1);

        c.fill_rect(
            strip_x0 - 1,
            strip_y,
            strip_x1 - strip_x0 + 2,
            strip_h,
            Rgba::from_hex(&t.tube.internals, 0.92),
        );
        c.fill_rect(
            strip_x0 - 1,
            strip_y + strip_h - 1,
            strip_x1 - strip_x0 + 2,
            1,
            Rgba::from_hex(&t.tube.collar, 0.28),
        );

        let nbands = d.levels.len();
        let bar_resp: Vec<f32> = (0..nb)
            .map(|i| {
                let lo = i * nbands / nb;
                let hi = ((i + 1) * nbands / nb).max(lo + 1);
                remap(group_level(&d.levels, lo, hi, GROUP_MAX_BIAS), t.sensitivity)
            })
            .collect();

        // Dormant bars, on the opaque layer so they stay crisp and dim - a meter with an unlit
        // scale reads as switched off.
        if t.ghost > 0.0 {
            for i in 0..nb {
                c.fill_rect(
                    strip_x0 + i as i32 * pitch,
                    bar_top,
                    bw,
                    bar_max,
                    Rgba::from_hex(&t.lit, t.ghost),
                );
            }
        }

        // ---- reels, opaque parts ----

        let rim = Rgba::from_hex(&t.tube.collar, 1.0);
        let flange = Rgba::from_hex(&t.tube.socket, 1.0);
        let tape_col = Rgba::from_hex(&t.tube.internals, 1.0);
        let pack_out = ((r as f32 * 0.80) as i32).max(2);
        let pack_in = ((r as f32 * 0.36) as i32).max(1);
        let hub_r = ((r as f32 * 0.22) as i32).max(1);
        for &cx in &[cx_l, cx_r] {
            c.fill_circle(cx, cy, r, rim);
            c.fill_circle(cx, cy, r - 2, flange);
            // The wound tape pack. Dark, and it is what the lit spokes are read AGAINST - the
            // first draft put the spokes over the flange colour and at a 1.16x luminance ratio
            // they were invisible, the same measurement that killed brightness-only cues on the
            // valve row.
            //
            // Drawn as a ring plus a fill so the pack's outermost layer catches a little light.
            // A flat dark disc read as a HOLE punched in the flange rather than as wound tape,
            // and one pixel of edge is what tells the eye it is a surface.
            c.fill_circle(cx, cy, pack_out, Rgba::from_hex(&t.tube.glass, 0.22));
            c.fill_circle(cx, cy, pack_out - 1, tape_col);
            c.fill_circle(cx, cy, pack_in, flange);
            c.fill_circle(cx, cy, hub_r, rim);
            c.fill_circle(cx, cy, (hub_r - 1).max(0), Rgba::from_hex(&t.tube.internals, 0.9));
            // Sheen, offset up-left so the deck reads as lit from above. Elliptical rather than
            // radial only because it must not spill past the rim onto the deck plate.
            c.elliptical_gradient(
                cx - r / 3,
                cy - r / 3,
                r as f32 * 0.72,
                r as f32 * 0.72,
                &[
                    (0.0, Rgba::from_hex(&t.tube.glass, 0.16)),
                    (1.0, Rgba::from_hex(&t.tube.glass, 0.0)),
                ],
            );
        }

        // ---- the light ----

        // On its OWN transparent layer. `Canvas::bloom` composites its halo BENEATH whatever is
        // already on the canvas, so blooming the deck plate directly leaves the halo behind an
        // opaque wall and invisible - the trap already documented in segmented, scope, vu and
        // tube.
        let mut lit = Canvas::new(w, h);

        let spoke_col = Rgba::from_hex(&t.lit, (0.55 + 0.40 * drive).clamp(0.0, 1.0));
        let rivet = Rgba::from_hex(&t.hot, (0.70 + 0.30 * drive).clamp(0.0, 1.0));
        for (cx, phase) in [(cx_l, self.phase_l), (cx_r, self.phase_r)] {
            for k in 0..SPOKES {
                let turns = phase + k as f32 / SPOKES as f32;
                Self::spoke(&mut lit, cx, cy, r, turns, spoke_col, rivet);
            }
        }

        if self.marker.len() != nb {
            self.marker.resize(nb, 0.0);
        }
        let lit_col = Rgba::from_hex(&t.lit, 1.0);
        let hot_col = Rgba::from_hex(&t.hot, 1.0);
        for i in 0..nb {
            let resp = bar_resp[i];
            self.marker[i] = (self.marker[i] - MARKER_FALL).max(resp);
            let x = strip_x0 + i as i32 * pitch;
            let hgt = ((resp * bar_max as f32).round() as i32).clamp(0, bar_max);
            if hgt > 0 {
                lit.fill_rect(x, bar_top + bar_max - hgt, bw, hgt, lit_col);
                lit.fill_rect(x, bar_top + bar_max - hgt, bw, 1, hot_col);
            }
            let pk = ((self.marker[i] * bar_max as f32).round() as i32).clamp(0, bar_max);
            if pk > hgt {
                lit.fill_rect(x, bar_top + bar_max - pk, bw, 1, hot_col);
            }
        }

        // Peak lamp. The only consumer of rms here, and the reason it is a LAMP: an indicator
        // that is either on or off is the one place where pure intensity is legible, because
        // there is nothing to compare it against except itself a moment ago.
        let peak = rms_resp(d.rms_l, t.sensitivity).max(rms_resp(d.rms_r, t.sensitivity));
        let lamp = if peak >= PEAK_AT {
            Rgba::from_hex(&t.hot, 1.0)
        } else {
            Rgba::from_hex(&t.lit, 0.18)
        };
        lit.fill_rect(lamp_x, bar_top, lamp_w, bar_max.min(2).max(1), lamp);

        if t.bloom > 0.0 {
            let mut glow = lit.clone();
            glow.bloom(t.bloom as i32, t.glow_strength.clamp(0.0, 1.0));
            c.draw_over(&glow);
        }
        c.draw_over(&lit);

        // ---- tape, over the light, because it passes in FRONT of the reels ----

        let sheen = Rgba::from_hex(&t.tube.glass, 0.30);
        let span = (xr - xl).max(1) as f32;
        let mut prev: Option<i32> = None;
        for x in xl..=xr {
            let u = (x - xl) as f32 / span;
            // Parabola, 0 at both shoulders and 1 at mid-span. A catenary is the honest curve
            // but at a 100x20 aspect the two are within a pixel of each other, and the parabola
            // cannot produce the NaN a cosh of a poisoned argument could.
            let s = 4.0 * u * (1.0 - u);
            let y = yt + (sag * s).round() as i32;
            // Fill the whole run since the previous column. At small panel sizes the span is
            // short enough that the curve can move more than a pixel per column, and a
            // per-column 2px stroke would then leave the tape visibly dashed.
            let (lo, hi) = match prev {
                Some(p) if p < y => (p, y),
                Some(p) => (y, p),
                None => (y, y),
            };
            c.fill_rect(x, lo, 1, (hi - lo) + 2, tape_col);
            c.fill_rect(x, lo - 1, 1, 1, sheen);
            prev = Some(y);
        }

        // Head stack, sitting just below the tape's deepest reach so a loud passage brings the
        // tape down onto it. This is the sag's reference mark: without it the tape is a curve
        // floating in a featureless deck, and "how deep is it" has no answer at a glance.
        let head_top = yt + sag_max.round() as i32 + 2;
        let head_w = (r.max(5)).min(11) | 1;
        let hx = (xl + xr) / 2 - head_w / 2;
        if head_top <= deck_bot {
            c.fill_rect(hx, head_top, head_w, deck_bot - head_top + 1, tape_col);
            c.fill_rect(hx, head_top, head_w, 1, Rgba::from_hex(&t.tube.collar, 0.85));
            // The head gap - one bright column, which is the detail that makes the block read
            // as a head rather than as a screw boss.
            c.fill_rect(hx + head_w / 2, head_top, 1, (deck_bot - head_top + 1).min(4), sheen);
        }

        // ---- bezel ----

        // Clip with EXACTLY the rect the plate was drawn with, or the spokes' halo escapes onto
        // the bare taskbar outside the rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    /// A flat spectrum plus a matching RMS, i.e. what a test frame looks like when nothing about
    /// the SHAPE of the spectrum is under test.
    fn frame(level: f32) -> FrameData {
        let mut d = FrameData::default();
        for v in d.levels.iter_mut() {
            *v = level;
        }
        d.peaks = d.levels;
        // Roughly the RMS the DSP would report for that band level - see `vu.rs`: real music at
        // 0.15-0.65 band level sits at 0.02-0.12 RMS.
        d.rms_l = level * 0.15;
        d.rms_r = level * 0.15;
        d
    }

    /// An UNEVEN spectrum: `tilt` > 0 loads the bass, < 0 loads the treble, and the MEAN is the
    /// same either way, so a family that only ever looks at a broadband average cannot tell the
    /// two apart. Every test in `tube.rs` originally drove all 64 bands to one level, where a
    /// group's mean equals its max and the whole reducer is a no-op; this is the shape that can
    /// actually see a per-band bug.
    fn tilted(mid: f32, tilt: f32) -> FrameData {
        let mut d = FrameData::default();
        let n = d.levels.len() as f32;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let x = i as f32 / (n - 1.0);
            *v = (mid + tilt * (0.5 - x)).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = mid * 0.15;
        d.rms_r = mid * 0.15;
        d
    }

    fn lum(p: Rgba) -> f32 {
        0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32
    }

    /// Renders `frames` frames of the same input and returns the canvas, so the flywheel and the
    /// tape ballistic have time to settle - a single frame only ever shows 10% of the reel's
    /// response and 28% of the tape's.
    fn settled(t: &Theme, d: &FrameData, w: i32, h: i32, frames: usize) -> (Reel, Canvas) {
        let mut r = Reel::default();
        let mut c = Canvas::new(w, h);
        for _ in 0..frames {
            r.draw(&mut c, t, d);
        }
        (r, c)
    }

    #[test]
    fn the_response_window_spends_its_range_on_levels_the_dsp_actually_produces() {
        // Guards RESP_FLOOR/RESP_SPAN. A mapping linear over 0..1 looks right on a synthetic
        // sweep and dead on music, which only reaches ~0.15-0.65.
        let lo = remap(0.15, 1.0);
        let hi = remap(0.65, 1.0);
        assert!(hi - lo > 0.75, "the music window must cover most of the range: {lo} -> {hi}");
        assert_eq!(remap(0.0, 1.0), 0.0, "silence must map to zero, not a pedestal");
        assert!(remap(0.4, 2.0) > remap(0.4, 1.0), "sensitivity must scale it");
        // Both flavours of non-finite are treated as NO SIGNAL rather than as full scale. An
        // infinity is a broken reading, not a loud one, and mapping it to 1.0 would slam the
        // transport to its top speed on a single bad FFT frame.
        assert_eq!(remap(f32::NAN, 1.0), 0.0, "NaN must not survive the mapping");
        assert_eq!(remap(f32::INFINITY, 1.0), 0.0, "an infinity is a broken reading, not a loud one");
        assert_eq!(remap(f32::NEG_INFINITY, 1.0), 0.0);
    }

    #[test]
    fn the_group_reducer_is_biased_toward_the_peak_not_the_mean() {
        // One loud band among many quiet ones: the reducer must land nearer the max, or a kick
        // inside the bass third would be averaged away before it ever reached the tape.
        let mut levels = [0.10f32; 64];
        levels[7] = 0.90;
        let got = group_level(&levels, 0, 21, GROUP_MAX_BIAS);
        let mean = levels[..21].iter().sum::<f32>() / 21.0;
        assert!(got > mean + (0.90 - mean) * 0.5, "must sit above the midpoint: {got}");
        assert!(got <= 0.90 + 1e-6, "but never above the peak: {got}");
        // ...and the broadband figure must NOT be, or one sharp band spins the transport up as
        // hard as a full mix.
        let broad = group_level(&levels, 0, 64, OVERALL_MAX_BIAS);
        assert!(broad < got, "the broadband figure must be far less peak-biased: {broad} vs {got}");
    }

    #[test]
    fn the_reels_spin_faster_as_the_music_gets_louder_and_never_stop() {
        let t = builtin::reel_studio_grey();
        let quiet = settled(&t, &frame(0.05), 190, 60, 90).0.omega;
        let mid = settled(&t, &frame(0.35), 190, 60, 90).0.omega;
        let loud = settled(&t, &frame(0.80), 190, 60, 90).0.omega;
        assert!(mid > quiet * 1.5, "mid must clearly out-run quiet: {quiet} -> {mid}");
        assert!(loud > mid * 1.3, "loud must clearly out-run mid: {mid} -> {loud}");
        // The floor. A threaded deck that stops dead reads as broken, not as quiet - the same
        // argument as the valve row's heater floor.
        let silent = settled(&t, &FrameData::default(), 190, 60, 90).0;
        assert!(
            silent.omega >= SPIN_IDLE_DPS * 0.95,
            "the transport must keep creeping at silence, got {}",
            silent.omega
        );
    }

    #[test]
    fn rotation_actually_moves_pixels_inside_the_reel() {
        // The whole point of the family, and the thing a plain rotating disc fails: measured on
        // a disc-with-rim-highlight draft the frame-to-frame difference inside the reel was
        // exactly ZERO. Compares two frames far enough apart that even the idle creep has moved
        // a spoke, and looks only INSIDE the left reel so the strip and the tape cannot carry
        // the test.
        let t = builtin::reel_studio_grey();
        let d = frame(0.45);
        let mut r = Reel::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..40 {
            r.draw(&mut c, &t, &d);
        }
        let before: Vec<f32> = (5..46)
            .flat_map(|y| (5..44).map(move |x| (x, y)))
            .map(|(x, y)| lum(c.get(x, y)))
            .collect();
        for _ in 0..6 {
            r.draw(&mut c, &t, &d);
        }
        let after: Vec<f32> = (5..46)
            .flat_map(|y| (5..44).map(move |x| (x, y)))
            .map(|(x, y)| lum(c.get(x, y)))
            .collect();
        let moved = before
            .iter()
            .zip(&after)
            .filter(|(a, b)| (*a - *b).abs() > 12.0)
            .count();
        assert!(
            moved > 30,
            "the spokes must visibly move between frames; only {moved} pixels in the reel changed"
        );
    }

    /// Per-frame rotation steps and tape sag, over `frames` frames of CONSTANT audio starting the
    /// moment a flourish fires.
    ///
    /// Constant audio is the whole design of this fixture: `omega` and `sag` both follow the music, so
    /// against varying input any wobble measured here could just as easily be the music. Held steady,
    /// the transport settles to one rate and one sag, and everything left moving is the flourish.
    fn warble_trace(fire: bool, frames: usize) -> (Vec<f32>, Vec<f32>) {
        let t = builtin::reel_studio_grey();
        let mut r = Reel::default();
        let mut c = Canvas::new(190, 60);
        let mut d = frame(0.5);
        d.dt_ms = NOMINAL_DT_MS;
        // Settle the flywheel. 400 frames is what `rotation_is_paced_by_dt_not_by_the_frame_count`
        // uses to call `omega` settled.
        for _ in 0..400 {
            r.draw(&mut c, &t, &d);
        }
        // Fired by REQUEST rather than by the audio firing sequence, which is not a shortcut - it is
        // the only way to hold the audio constant. The firing sequence is a loud transient, so it
        // drives `omega` and `sag` hard in BOTH arms and they then decay back over ~300ms; measured,
        // that transient alone gave the no-flourish arm 0.29 of rate spread and 0.46 of sag spread,
        // swamping the effect under test. A request fires past the rarity check and past a strength of
        // zero, so both arms see byte-identical, unchanging input.
        if fire {
            crate::dsp::flourish::request();
            r.draw(&mut c, &t, &d);
        }
        let (mut steps, mut sags) = (Vec::new(), Vec::new());
        let mut prev = r.phase_l;
        for _ in 0..frames {
            r.draw(&mut c, &t, &d);
            steps.push((r.phase_l - prev).rem_euclid(1.0));
            sags.push(r.sag);
            prev = r.phase_l;
        }
        (steps, sags)
    }

    /// Peak-to-peak of a series as a fraction of its mean. Catches slow variation - the wow.
    fn spread(v: &[f32]) -> f32 {
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
        (hi - lo) / mean.abs().max(1e-9)
    }

    /// Mean absolute change BETWEEN consecutive samples, as a fraction of the mean.
    ///
    /// Deliberately a different statistic from `spread`, because wow and flutter are not
    /// distinguishable by amplitude alone. A 1.1Hz wow of depth 0.30 moves slowly - about 0.035 of the
    /// rate per frame at 60fps - while an 8.5Hz flutter of depth 0.10 moves nearly three times faster
    /// despite being a third of the depth. So `spread` sees mostly wow and this sees mostly flutter,
    /// and a test using only the first would pass with flutter deleted.
    fn jitter(v: &[f32]) -> f32 {
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        let acc: f32 = v.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
        acc / (v.len() - 1) as f32 / mean.abs().max(1e-9)
    }

    #[test]
    fn the_flourish_puts_wow_and_flutter_into_the_transport() {
        // 160 frames is 2.67s against a 2200ms envelope, so the window covers the wobble AND a clear
        // 470ms of recovery past the end of it. 132 frames - exactly the envelope - was tried first and
        // is subtly wrong: the last frames still carry ~9% of the envelope, which left 0.0315 of
        // peak-to-peak in a "recovered" tail that was never actually past the effect.
        let _g = crate::dsp::flourish::test_guard();
        let (steady, steady_sag) = warble_trace(false, 160);
        let (warbled, warbled_sag) = warble_trace(true, 160);

        // The fixture has to be genuinely steady or nothing below means anything.
        assert!(
            spread(&steady) < 0.02 && spread(&steady_sag) < 0.02,
            "the transport was not settled before the hit: rate spread {:.4}, sag spread {:.4}",
            spread(&steady),
            spread(&steady_sag)
        );

        // Wow: the rate wanders over a large fraction of itself.
        assert!(
            spread(&warbled) > 0.30,
            "the rate barely moved: peak-to-peak {:.3} of the mean against {:.3} steady",
            spread(&warbled),
            spread(&steady)
        );

        // Flutter: it also moves FAST, which is a separate property from moving far. Measured over the
        // first third of the window only - the envelope decays linearly across the whole 2.2s, so
        // averaging over all of it halves the figure and buys nothing.
        //
        // 0.028 sits between two measured values, not around a predicted one: 0.0439 with flutter and
        // 0.0161 with `FLUTTER_DEPTH` set to zero, so roughly 1.6x of margin either way. Worth
        // recording that the first-principles estimate was 0.089 and wrong by 2x, because it forgot
        // that the envelope decays across the measurement window.
        assert!(
            jitter(&warbled[..44]) > 0.028,
            "the wobble is all wow and no flutter: {:.4} mean change per frame against {:.4} steady",
            jitter(&warbled[..44]),
            jitter(&steady[..44])
        );

        // The tape slack shows the speed error too, or the reels read as wrong rather than the
        // transport reading as wrong.
        assert!(
            spread(&warbled_sag) > 0.08,
            "the tape span stayed rigid through the speed error: {:.4} against {:.4} steady",
            spread(&warbled_sag),
            spread(&steady_sag)
        );

        // And it clears. The last 12 frames sit 270-470ms past the end of the envelope, so they must be
        // steady again - measured on the rate, which is where the wobble was injected.
        let tail = &warbled[warbled.len() - 12..];
        assert!(
            spread(tail) < 0.03,
            "the transport never recovered: rate still moving {:.4} peak-to-peak at the end",
            spread(tail)
        );
    }

    /// Run: cargo test --release dump_reel_warble -- --ignored --nocapture
    ///
    /// A filmstrip, because wow and flutter are a property of MOTION and no single frame can show
    /// them. Both rows are sampled at identical 100ms intervals from identical constant audio, so the
    /// spoke rivet should sit at evenly spaced angles down the steady row and at uneven ones down the
    /// warbled row. That uneven spacing IS the effect.
    #[test]
    #[ignore]
    fn dump_reel_warble() {
        let _g = crate::dsp::flourish::test_guard();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();

        const CELLS: usize = 8;
        const EVERY: usize = 6; // frames between samples, ~100ms at 60fps
        let (cw, ch) = (190i32, 60i32);
        let mut rows: Vec<Vec<Canvas>> = Vec::new();
        for fire in [false, true] {
            let t = builtin::reel_studio_grey();
            let mut r = Reel::default();
            let mut c = Canvas::new(cw, ch);
            let mut d = frame(0.5);
            d.dt_ms = NOMINAL_DT_MS;
            for _ in 0..400 {
                r.draw(&mut c, &t, &d);
            }
            if fire {
                crate::dsp::flourish::request();
            }
            let mut shots = Vec::new();
            for k in 0..(CELLS * EVERY) {
                r.draw(&mut c, &t, &d);
                if k % EVERY == 0 {
                    shots.push(c.clone());
                }
            }
            rows.push(shots);
        }

        // One image: two rows of CELLS frames, each cropped to the transport's left half so the reel
        // fills the cell. Written un-premultiplied onto the dark taskbar, as the other dumps are.
        let (cropw, croph) = (cw / 2, ch);
        let (ow, oh) = (cropw * CELLS as i32, croph * 2 + 4);
        let mut out = vec![22u8; (ow * oh * 4) as usize];
        for (ri, shots) in rows.iter().enumerate() {
            for (ci, shot) in shots.iter().enumerate() {
                for y in 0..croph {
                    for x in 0..cropw {
                        let px = shot.get(x, y);
                        let a = px.a as f32 / 255.0;
                        let (ox, oy) = (ci as i32 * cropw + x, ri as i32 * (croph + 4) + y);
                        let o = ((oy * ow + ox) * 4) as usize;
                        for (k, ch8) in [px.r, px.g, px.b].iter().enumerate() {
                            out[o + k] = (*ch8 as f32 + 22.0 * (1.0 - a)).min(255.0) as u8;
                        }
                        out[o + 3] = 255;
                    }
                }
            }
        }
        let path = dir.join(format!("reel-warble-{ow}x{oh}.rgba"));
        std::fs::write(&path, &out).unwrap();
        println!("wrote {} ({}x{}) - top row steady, bottom row warbled", path.display(), ow, oh);

        // The filmstrip alone is weak evidence, and it is worth saying why rather than shipping it as
        // if it were strong: three spokes are symmetric every 120 degrees, so uneven angular spacing
        // between samples is genuinely hard to see. The rate series is the unambiguous artefact - a
        // steady transport plots as a flat line, a warbling one as a wave - so write that too.
        let (steady, _) = warble_trace(false, 160);
        let (warbled, sag) = warble_trace(true, 160);
        let mut csv = String::from("frame,steady_dps,warbled_dps,warbled_sag_px
");
        for i in 0..steady.len() {
            let dps = |turns: f32| turns * 360.0 / (NOMINAL_DT_MS / 1000.0);
            csv.push_str(&format!(
                "{i},{:.3},{:.3},{:.3}
",
                dps(steady[i]),
                dps(warbled[i]),
                sag[i]
            ));
        }
        let csvp = dir.join("reel-warble.csv");
        std::fs::write(&csvp, csv).unwrap();
        println!("wrote {}", csvp.display());
    }

    #[test]
    fn the_fastest_reel_stays_far_below_the_spoke_aliasing_bound() {
        // A spoked wheel advancing one spoke pitch per frame looks STATIONARY, and past half a
        // pitch it looks like it is running backwards. This is a pure arithmetic guard on the
        // constants, because the failure is invisible to any pixel test: each individual frame
        // is drawn perfectly correctly.
        let pitch_deg = 360.0 / SPOKES as f32;
        // Including the flourish's peak speed multiplier. The wow/flutter depths are the sort of
        // thing that gets nudged up for effect, and doing so silently spends this family's aliasing
        // budget - at which point the reels appear to run BACKWARDS during the flourish.
        let peak_speed = 1.0 + WOW_DEPTH + FLUTTER_DEPTH;
        let worst = SPIN_FULL_DPS * TAKEUP_RATIO * peak_speed * (NOMINAL_DT_MS / 1000.0);
        assert!(
            worst < pitch_deg * 0.5 / 2.0,
            "worst-case {worst:.1} deg/frame is too close to the {:.1} deg reversal bound",
            pitch_deg * 0.5
        );
        // And the other end: too SLOW is also a standstill. At a 20px radius the spoke tip sits
        // ~17px out, so a third of a pixel of tip travel needs ~1.1 deg/frame.
        let idle = SPIN_IDLE_DPS * (NOMINAL_DT_MS / 1000.0);
        assert!(idle > 0.9, "the idle creep of {idle:.2} deg/frame would look stationary");
    }

    #[test]
    fn rotation_is_paced_by_dt_not_by_the_frame_count() {
        // The render loop sleeps a fixed 16ms, so its real period varies with load; a per-frame
        // step would make the reels speed up whenever the machine is idle.
        //
        // Both reels are warmed to a settled `omega` first, deliberately: while the flywheel is
        // still converging the two dt schedules integrate different velocities and the
        // comparison would be second-order rather than exact.
        let t = builtin::reel_studio_grey();
        let (mut a, mut ca) = (Reel::default(), Canvas::new(190, 60));
        let (mut b, mut cb) = (Reel::default(), Canvas::new(190, 60));
        let mut warm = frame(0.5);
        warm.dt_ms = NOMINAL_DT_MS;
        for _ in 0..400 {
            a.draw(&mut ca, &t, &warm);
            b.draw(&mut cb, &t, &warm);
        }
        assert!((a.phase_l - b.phase_l).abs() < 1e-6, "identical warm-ups must agree");

        a.draw(&mut ca, &t, &warm);
        let mut half = frame(0.5);
        half.dt_ms = NOMINAL_DT_MS / 2.0;
        b.draw(&mut cb, &t, &half);
        b.draw(&mut cb, &t, &half);

        let diff = (a.phase_l - b.phase_l).abs();
        assert!(
            diff < 1e-4,
            "one full-dt frame must turn as far as two half-dt frames: {} vs {} (diff {diff})",
            a.phase_l,
            b.phase_l
        );
        // Non-vacuous: the step must be big enough that a per-frame implementation would fail
        // this by a mile.
        assert!(a.phase_l > 0.0, "the reel must actually have turned");
    }

    #[test]
    fn the_tape_sag_tracks_the_bass_and_moves_the_tape_down_the_deck() {
        // The family's POSITION cue. Measured as the row the tape occupies at QUARTER span -
        // not mid-span, where the head stack is also dark and would be found instead.
        //
        // The two spectra have the same mean and differ only in tilt, so a broadband-only
        // implementation scores identically on both. That is the per-band bug this catches.
        let t = builtin::reel_studio_grey();
        let tape_row = |d: &FrameData| -> i32 {
            let (_, c) = settled(&t, d, 190, 60, 60);
            // The tape is the darkest thing in this column: near-black over a mid-grey deck.
            let x = 66;
            let mut best = (f32::MAX, 0);
            for y in 6..44 {
                let l = lum(c.get(x, y));
                if l < best.0 {
                    best = (l, y);
                }
            }
            best.1
        };
        let treble = tape_row(&tilted(0.40, -0.30));
        let bass = tape_row(&tilted(0.40, 0.30));
        assert!(
            bass > treble + 4,
            "a bass-loaded spectrum must drop the tape clearly further: row {treble} -> {bass}"
        );

        // And the state itself, so a failure points at the mechanism rather than at the metric.
        let quiet = settled(&t, &frame(0.10), 190, 60, 60).0.sag;
        let loud = settled(&t, &frame(0.80), 190, 60, 60).0.sag;
        assert!(loud > quiet + 8.0, "sag must travel a readable distance: {quiet} -> {loud}");
    }

    #[test]
    fn the_strip_reads_per_band_and_not_one_averaged_level() {
        // A tilted spectrum must light the left of the strip differently from the right. Driving
        // every band to the same level - which is what most of this project's earlier tests did -
        // cannot see this at all.
        let t = builtin::reel_studio_grey();
        let (_, c) = settled(&t, &tilted(0.40, 0.34), 190, 60, 20);
        let strip = |x0: i32, x1: i32| -> f32 {
            let mut s = 0.0;
            for y in 49..57 {
                for x in x0..x1 {
                    s += lum(c.get(x, y));
                }
            }
            s
        };
        let low = strip(4, 50);
        let high = strip(136, 182);
        assert!(
            low > high * 1.4,
            "the bass end of the strip must clearly out-read the treble end: {low:.0} vs {high:.0}"
        );
    }

    #[test]
    fn louder_audio_changes_the_panel_overall() {
        // The blunt guard: the whole picture must differ, not just one measured feature.
        let t = builtin::reel_studio_grey();
        let (_, quiet) = settled(&t, &frame(0.08), 190, 60, 60);
        let (_, loud) = settled(&t, &frame(0.85), 190, 60, 60);
        let changed = (0..60)
            .flat_map(|y| (0..190).map(move |x| (x, y)))
            .filter(|&(x, y)| (lum(quiet.get(x, y)) - lum(loud.get(x, y))).abs() > 10.0)
            .count();
        assert!(changed > 400, "audio must visibly change the panel; only {changed} pixels moved");
    }

    #[test]
    fn the_peak_lamp_lights_only_when_the_rms_is_actually_hot() {
        let t = builtin::reel_studio_grey();
        let lamp = |rms: f32| -> f32 {
            let mut d = frame(0.4);
            d.rms_l = rms;
            d.rms_r = rms;
            let (_, c) = settled(&t, &d, 190, 60, 10);
            let mut best = 0.0f32;
            for y in 49..54 {
                for x in 182..187 {
                    best = best.max(lum(c.get(x, y)));
                }
            }
            best
        };
        let normal = lamp(0.06);
        let hot = lamp(0.95);
        assert!(hot > normal * 1.8, "the lamp must clearly light when hot: {normal} -> {hot}");
    }

    #[test]
    fn renders_at_every_plausible_size_without_panicking() {
        let t = builtin::reel_studio_grey();
        let d = tilted(0.45, 0.2);
        for (w, h) in [
            (190, 60),
            (380, 60),
            (456, 60),
            (240, 72),
            (150, 48),
            (96, 40),
            (40, 24),
            (33, 21),
            (12, 12),
            (2, 40),
            (1, 1),
        ] {
            let mut r = Reel::default();
            let mut c = Canvas::new(w, h);
            for _ in 0..4 {
                r.draw(&mut c, &t, &d);
            }
            assert_eq!(c.bits().len(), (w * h) as usize, "{w}x{h} changed the canvas size");
        }
    }

    #[test]
    fn nan_and_infinity_never_poison_the_persistent_state() {
        // `f32::clamp` does NOT sanitise NaN, and this family keeps four persistent floats that
        // a single poisoned frame would corrupt until the process restarted - the bug that hit
        // the vaporwave scroll phase and the VU needles.
        let t = builtin::reel_studio_grey();
        for spoil in 0..3 {
            let mut d = frame(0.5);
            match spoil {
                0 => {
                    d.levels[0] = f32::NAN;
                    d.levels[40] = f32::NAN;
                    d.rms_l = f32::NAN;
                    d.dt_ms = f32::NAN;
                }
                1 => {
                    d.levels[3] = f32::INFINITY;
                    d.levels[9] = f32::NEG_INFINITY;
                    d.rms_r = f32::INFINITY;
                    d.dt_ms = 0.0;
                }
                _ => {
                    for v in d.levels.iter_mut() {
                        *v = f32::NAN;
                    }
                    d.rms_l = f32::NAN;
                    d.rms_r = f32::NAN;
                    d.dt_ms = f32::NEG_INFINITY;
                }
            }
            let mut r = Reel::default();
            let mut c = Canvas::new(190, 60);
            for _ in 0..8 {
                r.draw(&mut c, &t, &d);
            }
            assert!(r.omega.is_finite(), "spoil {spoil}: omega poisoned ({})", r.omega);
            assert!(r.sag.is_finite(), "spoil {spoil}: sag poisoned ({})", r.sag);
            assert!(r.phase_l.is_finite() && r.phase_r.is_finite(), "spoil {spoil}: phase poisoned");
            // And it must RECOVER: a clean frame after a poisoned one has to render normally.
            r.draw(&mut c, &t, &frame(0.5));
            assert!(r.omega > 0.0 && r.omega.is_finite(), "did not recover: {}", r.omega);
        }
    }

    #[test]
    fn nothing_is_drawn_outside_the_rounded_plate() {
        // The spokes are bloomed, and a bloom spreads several pixels past the plate's 1-2px
        // margin - without the clip the halo sits on the bare taskbar as a bright box.
        let t = builtin::reel_studio_grey();
        let (_, c) = settled(&t, &frame(0.9), 190, 60, 40);
        for x in 0..190 {
            assert_eq!(c.get(x, 0), Rgba::TRANSPARENT, "row 0 is above the plate, x={x}");
            assert_eq!(c.get(x, 59), Rgba::TRANSPARENT, "row 59 is below the plate, x={x}");
        }
        for y in 0..60 {
            assert_eq!(c.get(0, y), Rgba::TRANSPARENT, "column 0 is left of the plate, y={y}");
            assert_eq!(c.get(189, y), Rgba::TRANSPARENT, "column 189 is right of the plate, y={y}");
        }
    }

    #[test]
    fn a_wide_panel_keeps_the_transport_the_aspect_it_was_tuned_at() {
        // At 380px the reels sat 300px apart and the sag curve flattened into a straight line
        // with a kink. The free span is what must stay put, not the panel fraction.
        let t = builtin::reel_studio_grey();
        let (_, wide) = settled(&t, &frame(0.5), 380, 60, 30);
        // The deck plate either side of the transport must be bare. Measured as luminance
        // VARIANCE rather than by looking for a specific colour: the bare plate is a smooth
        // vertical gradient and the transport is rim/pack/spoke structure, so variance separates
        // them without hard-coding any of the theme's hexes.
        let variance = |x0: i32, x1: i32| -> f32 {
            let vals: Vec<f32> = (6..44)
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .map(|(x, y)| lum(wide.get(x, y)))
                .collect();
            let mean = vals.iter().sum::<f32>() / vals.len() as f32;
            vals.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / vals.len() as f32
        };
        let outside = variance(6, 40);
        let inside = variance(100, 140);
        assert!(
            inside > outside * 4.0,
            "the transport must stay centred and capped: variance {inside:.1} inside vs \
             {outside:.1} out on the bare plate"
        );
    }

    #[test]
    fn every_reel_colourway_renders_and_differs() {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        for t in builtin::all().into_iter().filter(|t| t.family == "reel") {
            let (_, c) = settled(&t, &tilted(0.45, 0.25), 190, 60, 12);
            let bits = c.bits().to_vec();
            assert!(bits.iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for prior in &seen {
                assert_ne!(prior, &bits, "{} renders identically to another colourway", t.id);
            }
            seen.push(bits);
        }
        assert_eq!(seen.len(), 5, "expected five reel colourways, got {}", seen.len());
    }

    /// Run: cargo test --release dump_reel_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_reel_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        // The same uneven spectrum the valve row dumps with, so the two can be compared, plus
        // an RMS in the range the DSP really reports.
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            let x = i as f32 / 63.0;
            *v = (0.15 + 0.85 * (x * 9.0).sin().abs()) * (1.0 - x * 0.45);
        }
        d.peaks = d.levels;
        d.rms_l = 0.09;
        d.rms_r = 0.055;

        let mut n = 0usize;
        for t in builtin::all().into_iter().filter(|t| t.family == "reel") {
            for (tag, w, h) in [("", 190, 60), ("-wide", 380, 60)] {
                let mut reel = Reel::default();
                let mut c = Canvas::new(w, h);
                // Enough frames for the flywheel to settle, so the dump shows the steady state
                // rather than a transport still spinning up.
                for _ in 0..60 {
                    reel.draw(&mut c, &t, &d);
                }
                let mut out = Vec::with_capacity((w * h * 4) as usize);
                for y in 0..h {
                    for x in 0..w {
                        let px = c.get(x, y);
                        let a = px.a as f32 / 255.0;
                        for ch in [px.r, px.g, px.b] {
                            out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                        }
                        out.push(255);
                    }
                }
                std::fs::write(dir.join(format!("reel-{}{tag}.rgba", t.id)), &out).unwrap();
                n += 1;
            }
        }
        println!("wrote {n} reel dumps to {}", dir.display());
    }

    /// A rotation strip: the same colourway at eight successive phases, so a human can check
    /// that the spokes actually appear to turn (and turn one way) rather than strobe.
    /// Run: cargo test --release dump_reel_spin -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_reel_spin() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let t = builtin::reel_black_chrome();
        let d = tilted(0.55, 0.15);
        let mut reel = Reel::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..60 {
            reel.draw(&mut c, &t, &d);
        }
        let (w, h) = (190, 60);
        let mut out = Vec::with_capacity((w * h * 8 * 4) as usize);
        // Eight frames stacked vertically into one 190x480 image.
        for f in 0..8 {
            if f > 0 {
                for _ in 0..4 {
                    reel.draw(&mut c, &t, &d);
                }
            }
            for y in 0..h {
                for x in 0..w {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for ch in [px.r, px.g, px.b] {
                        out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                    }
                    out.push(255);
                }
            }
        }
        std::fs::write(dir.join("reel-spin.rgba"), &out).unwrap();
        println!("wrote reel-spin.rgba (190x480 - eight frames, 4 render frames apart)");
    }
}
