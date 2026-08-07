//! A radar warning receiver: a round passive scope showing bass transients as labelled threat
//! emitters, and an exceptional hit as a missile launch.
//!
//! **It is SUPPLEMENTARY to the sweep field, not a replacement for it.** Asked for as "a circular
//! RWR style display on the left to show bass transients as missile warning", and then explicitly:
//! "I want it to still use the existing radar theme to the right of it." So this owns a square patch
//! at the left of the radar panel, the sweep field is narrowed to make room, and both are drawn from
//! the same colourway. There is no new family and nothing is deprecated.
//!
//! **A real RWR does not sweep, and this one does not either.** That is the whole difference between
//! the two instruments sitting side by side: the field next door is an active scan, so its picture is
//! a second and a half of history built up one column at a time, while this is a passive receiver -
//! a contact appears when its emitter transmits, at the bearing it sits at, and fades. Two sweeps on
//! one panel would have read as one confused animation; one sweep and one static scope read as two
//! instruments.
//!
//! # What the audio drives, and what it does not
//!
//! This is the part that was got wrong first, and the correction is the most useful thing in this
//! file. The first version derived a contact's BEARING and its DESIGNATOR from the low-band spectral
//! centroid, on the principle that a display keyed to the music beats one keyed to a random number.
//! The principle is fine. The premise was false, and measuring it killed the design:
//!
//! ```text
//! centroid AT CONTACTS: min 0.527  p25 0.548  p50 0.554  p75 0.561  max 0.604  (spread 0.077)
//! leading band AT CONTACTS: [0, 0, 0, 6, 0, 0, 20, 0]  (bands 0..8)
//! ```
//!
//! Measured on `tests/fixtures/real-music-bands.csv`. The low centroid spans **8% of a circle**, so
//! every contact landed in one quadrant - reported from the live overlay as "everything is always in
//! the top right quadrant". Worse, fifteen designators across that 8% put the quantisation boundaries
//! right where the signal sits, so the label flickered between two neighbours on every beat -
//! reported as "the identifiers jump around too much". The obvious alternative, the single loudest
//! low band, is worse still: two distinct values out of eight.
//!
//! The bass content of a track sits where it sits. It is not a bearing.
//!
//! So an emitter's **bearing and designator are its identity**, fixed when it appears, taken from a
//! deterministic sequence - which is also what they are on real hardware, where bearing is geometry
//! and the designator is what kind of thing is out there. Neither is a property of the moment. What
//! the audio drives is everything a viewer can actually perceive as reacting:
//!
//! - **WHEN an emitter comes up**, and when it fades - onsets, and their absence.
//! - **HOW CLOSE it closes**, per transient. Nearer the centre is a bigger hit, which inverts the
//!   sweep field beside it, where loud is HIGH. The inversion is correct rather than sloppy: on a
//!   warning receiver the distance from the centre IS the threat.
//! - **WHETHER it launches** - an exceptional hit, relative to the track's own dynamics.
//!
//! # Everything relative, nothing absolute
//!
//! The second thing got wrong, and for a related reason. Both the range mapping and the launch
//! threshold were originally absolute figures in units of the detector's excess, calibrated against
//! the fixture. Live, on a different track, the launch never fired once - reported as "I havent seen a
//! missile warning and flash yet". An absolute threshold cannot survive material with different
//! dynamics, which is the same failure the vaporwave lightning shipped with (see `vapor.rs`).
//!
//! So both are measured against `typical` - a running average of how big the transients on THIS track
//! actually are. A typical hit sits mid-scope; a hit some multiple of typical is a launch, and that
//! multiple is never below 1.0 (see `LAUNCH_RATIO_MIN`). On a metronomic groove every kick is typical, so nothing launches; on anything with dynamics
//! the big hits stand out and do. It self-calibrates, and the per-colourway knob still means something
//! plain: how far above ordinary counts as exceptional.
//!
//! Print versus light follows the same discipline as the rest of the family: the rings, the cardinal
//! ticks and the own-ship symbol are print on the glass and go straight onto the panel, while the
//! contacts and the flash are light and go onto the transparent layer that gets bloomed.

use super::canvas::{Canvas, Rgba};
use crate::themes::Theme;
use std::f32::consts::TAU;

/// Live emitters the scope can hold at once.
///
/// Four. A legibility limit rather than a realism one: the scope is about 50px across and a labelled
/// contact is 7px, so beyond four the marks start colliding and the display stops showing individual
/// threats, which is the only thing it is for.
const MAX_THREATS: usize = 4;

/// **An emitter's ILLUMINATION and its LIFETIME are two different things, and conflating them was a
/// bug visible within a minute of running it: "sigs on the rwr dont go away".**
///
/// The first version had one `life` value that every transient reset to 1.0. Music delivers a transient
/// roughly twice a second and the decay ran over 2600ms, so `life` never fell below about 0.99: once
/// four contacts were up they were pinned at full brightness for ever, and because a slot could only be
/// recycled once its life had decayed, they could never be replaced either. The scope filled up in the
/// first eight seconds of a track and then never changed again.
///
/// The fix is two independent quantities:
///
/// - `glow` is illumination. A transient sets it to 1.0 and it decays over `GLOW_MS`. This is what makes
///   the contacts pulse with the beat, and what makes them dim within a second and a half of the music
///   stopping.
/// - `age` is the emitter's own clock. It only ever increases and nothing refreshes it. At
///   `EMITTER_LIFE_MS` the emitter is gone, and over the last `FADE_FRAC` of that it visibly fades out.
///
/// So a contact can be re-illuminated as often as the music likes and STILL goes away on schedule,
/// which is also what guarantees turnover: bearings and designators change over time because emitters
/// RETIRE, never because a live one was overwritten. Both halves of the report - that they never went
/// away, and that they needed to fade - fall out of this one separation.
///
/// 6000ms against an 1800ms spawn gap gives about three live contacts at steady state, turning over
/// every few seconds.
const EMITTER_LIFE_MS: f32 = 6000.0;
const GLOW_MS: f32 = 1500.0;
const FADE_FRAC: f32 = 0.30;
const SPAWN_MS: f32 = 1800.0;

/// How long a launch marker lives. Much shorter than an emitter - it is the event, and an event that
/// outlives its contact reads as a second, slower animation.
const LAUNCH_MS: f32 = 480.0;

/// Range an emitter sits at when nothing is driving it, and when a maximal hit is.
///
/// Not 1.0 at the rim: a mark centred on the outer ring reads as a break in the ring rather than as a
/// contact, the same failure `RANGE_MAX` records in the sweep field - and a 7px designator centred
/// there crosses the ring outright. Not 0.0 at the middle either: the own-ship symbol lives there.
const RANGE_RIM: f32 = 0.78;
const RANGE_CORE: f32 = 0.26;

/// How fast an emitter's range follows the transients driving it, per hit.
///
/// Eased rather than set, so a contact GLIDES in and out with the music instead of teleporting. The
/// per-emitter variation in `Threat::lag` is what stops four contacts moving in lockstep, which looks
/// mechanical.
const RANGE_EASE: f32 = 0.45;

/// Multiple of a track's typical transient that counts as "typical" for the range mapping.
///
/// See the module note on relative-versus-absolute. 1.6 puts an average hit about 60% of the way in
/// from the rim, leaving room both to fall back and to close further.
const RANGE_REF: f32 = 1.6;

/// Multiple of a track's typical transient that makes a launch, at `launch_at` 0 and at 1.
///
/// **The floor is above 1.0, which makes "never on a metronomic groove" structural rather than tuned.**
/// A dead-steady kick has a ratio of exactly 1.0 against the median by definition, so a floor above 1.0
/// cannot fire on one at any setting. The first version multiplied a single ratio of 2.4 by `launch_at`,
/// which put the effective ratio BELOW 1.0 for every setting under 0.42 - so the "see it more often" end
/// of the knob fired on literally every beat. Caught by the metronome assertion in the tests.
///
/// The RANGE is measured, not chosen. The first guess put the floor at 1.3 on the reasoning that a
/// launch should need a hit "nearly two and a half times an ordinary one". Four tracks captured live
/// from the user's own Spotify session say that hits that big essentially do not occur in this material:
///
/// ```text
///   track                          contacts/s   ratio p90   ratio max
///   Sub Focus - Desire                   6.58        1.41        1.71
///   Campbell - Would You                 1.36        1.02        1.08
///   Ely Oaks - Running Around            5.44        1.45        1.91
///   Skepsis - Been Here Before           3.93        1.18        1.54
///   (the older committed fixture)        1.97        1.18        1.25
/// ```
///
/// Nothing reaches 2.0. A 1.3 floor fired zero times on four of the five, which is precisely the
/// reported "I havent seen a missile warning and flash yet". Real bass transients vary by tens of
/// percent around the median, not by multiples, so the usable window is 1.05..1.80 - and that window
/// spans a genuinely useful rate: on the most dynamic track, 2.12 launches/s at 0.0 down to none at 1.0.
const LAUNCH_RATIO_MIN: f32 = 1.05;
const LAUNCH_RATIO_MAX: f32 = 1.80;

/// Recent contact sizes the launch test measures against, and how many are needed before it means
/// anything.
///
/// **A MEDIAN of a window, not a running average, and that choice is what finally made the launch
/// fire at all.** Two entirely different outliers were poisoning a mean:
///
/// - **The detector's startup.** `bass_avg` begins at zero, so the first frames of audio report an
///   excess of the whole bass level - 0.807 on the real-music fixture against an ordinary 0.09.
/// - **A genuine big hit** - which is precisely the event that must NOT be allowed to redefine what
///   ordinary means, or a second big hit becomes unremarkable.
///
/// Every exponential average tried failed on one horn or the other, and the failures were measured by
/// driving the shipped code over the fixture at seven knob settings and counting real launches:
///
/// ```text
///   symmetric 0.10        max ratio 1.24   0 launches at every setting
///   asym 0.08 up/0.30 dn  max ratio 1.24   0 launches at every setting
///   slow + 2-contact warm-up, max ratio 1.00   0 launches at every setting
/// ```
///
/// A fast average tracks the beat so closely that nothing can stand out; a slow one never shakes off
/// the startup transient. A median ignores both by construction - an outlier does not move it at all -
/// and needs no warm-up special case.
///
/// 12 samples is about six seconds of music at two contacts a second: long enough to describe "what
/// this track is like", short enough to follow a change of track. `MIN_SAMPLES` stops a launch firing
/// off a two-sample median, where the startup transient could still be the middle value.
const WINDOW: usize = 12;
const MIN_SAMPLES: usize = 5;
const TYPICAL_FLOOR: f32 = 0.02;

/// Radius of the inner range ring, as a fraction of the scope radius.
///
/// 0.55 sits between `RANGE_CORE` and `RANGE_RIM`, so it is a threshold contacts actually cross: an
/// ordinary transient sits outside it and a big one inside. A ring nothing ever crosses is decoration.
const INNER_RING: f32 = 0.55;

/// Gap between the scope and the sweep field, in pixels.
const GAP_PX: i32 = 4;

/// Columns the sweep field must still be able to hold for the scope to be drawn at all.
///
/// Below this the scope has taken so much of the panel that the field is no longer a spectrum display,
/// and a warning receiver beside a six-column smear is worse than no warning receiver. 12 columns is
/// 69px at the tuned pitch; the scope drops out on its own below about 130px without any width being
/// special-cased anywhere.
const MIN_FIELD_COLS: i32 = 12;

/// Bearings emitters appear at, in turns, in order of appearance.
///
/// A fixed sequence, because the audio provably cannot supply one - see the module note. Ordered so
/// consecutive emitters land far apart rather than sweeping round like a clock hand, and so any four
/// consecutive entries are spread around the scope instead of bunched in an arc.
const BEARINGS: [f32; 8] = [0.07, 0.45, 0.20, 0.70, 0.32, 0.88, 0.57, 0.95];

/// Where the scope sits and how big it is. A SQUARE patch, because a circle drawn in a non-square box
/// is an ellipse, and an elliptical RWR is exactly the mistake that cost the radar family its original
/// PPI fan.
#[derive(Clone, Copy)]
pub struct Scope {
    pub cx: i32,
    pub cy: i32,
    pub r: i32,
}

/// Scope radius for a panel of height `panel_h`.
///
/// Measured against the PANEL interior, not the sweep field's. The field is inset five rows top and
/// bottom to leave room for its range lines and its datum; the scope needs none of that, and taking
/// the field's inset cost it four pixels of radius at every size. On the real taskbar - where the
/// overlay is about 48px tall rather than the 60px reference - that was a 33px circle where 41px fits,
/// and it was reported as wanting to be "a bit larger". This is the largest circle that clears the
/// panel's own bezel.
fn radius_for(panel_h: i32) -> i32 {
    ((panel_h - 8) / 2).clamp(0, 40)
}

/// Total width the scope wants, including the gap to the field - 0 if there is no room for it.
///
/// `usable` is the field width the panel would have with no scope at all. Returns 0 rather than an
/// Option because every caller adds it to an x offset, and the no-scope case genuinely consumes no
/// width.
pub fn width_for(usable: i32, panel_h: i32, col_pitch: f32) -> i32 {
    let r = radius_for(panel_h);
    if r < 8 {
        return 0;
    }
    let want = 2 * r + 1 + GAP_PX;
    let left = usable - want;
    if ((left as f32 / col_pitch).floor() as i32) < MIN_FIELD_COLS {
        return 0;
    }
    want
}

/// The scope for a panel, with its left edge at `x`.
pub fn scope(x: i32, panel_h: i32) -> Option<Scope> {
    let r = radius_for(panel_h);
    if r < 8 {
        return None;
    }
    Some(Scope { cx: x + r, cy: panel_h / 2, r })
}

/// A received emitter.
#[derive(Clone, Copy, Default)]
struct Threat {
    /// Illumination, 1 on a transient and decaying over `GLOW_MS`. NOT the emitter's lifetime - see the
    /// note on `EMITTER_LIFE_MS` for why those have to be separate.
    glow: f32,
    /// Milliseconds since this emitter appeared. Monotonic; nothing refreshes it.
    age: f32,
    /// Radians, 0 at twelve o'clock, increasing clockwise. Fixed for the emitter's whole life.
    bearing: f32,
    /// 0 at the centre, 1 at the rim. The one thing about an emitter the music moves.
    range: f32,
    /// Launch marker, decaying faster than `life`. 0 for an ordinary contact.
    launch: f32,
    /// Which designator this emitter reports, as an index into the colourway's table. Fixed for life:
    /// a code that changed while the contact was up was the "identifiers jump around" report.
    code: usize,
    /// Per-emitter range responsiveness, so four contacts do not glide in lockstep.
    lag: f32,
    /// Set once the slot holds a real emitter, so a zeroed slot is never drawn as a live contact at
    /// twelve o'clock.
    used: bool,
}

impl Threat {
    /// Fraction of its life this emitter has left to run.
    fn remaining(&self) -> f32 {
        (1.0 - self.age / EMITTER_LIFE_MS).clamp(0.0, 1.0)
    }

    /// What the mark is actually drawn at: illumination, tapered by the end-of-life fade.
    fn alpha(&self) -> f32 {
        if !self.used {
            return 0.0;
        }
        let env = (self.remaining() / FADE_FRAC).clamp(0.0, 1.0);
        (self.glow * env).clamp(0.0, 1.0)
    }
}

#[derive(Default)]
pub struct Rwr {
    threats: [Threat; MAX_THREATS],
    /// Rim flash on a launch, shared by all contacts - the scope itself reacting, not a mark.
    flash: f32,
    /// How many emitters have ever appeared, which is what picks the next one's bearing and code.
    seq: u32,
    /// Time until another emitter may join.
    spawn_wait: f32,
    /// The last `WINDOW` contact sizes, oldest overwritten first. The median of these is what
    /// everything is measured against - see the note on `WINDOW`.
    recent: [f32; WINDOW],
    /// How many have been written, saturating at `WINDOW`, and where the next one goes.
    seen_n: usize,
    head: usize,
    /// Launches fired, for the calibration probe. It counts through the REAL code path rather than a
    /// probe reimplementing the rule, which is the difference between measuring the shipped behaviour
    /// and measuring a copy of it that has since drifted.
    #[cfg(test)]
    pub(super) launches: u32,
    /// Every (ratio, typical) pair at a contact, for the calibration probe.
    #[cfg(test)]
    pub(super) seen: Vec<(f32, f32)>,
}

/// Shortest signed angle from `a` to `b`, in -PI..PI. Used by the tests to compare bearings.
#[cfg(test)]
fn angle_delta(a: f32, b: f32) -> f32 {
    let mut d = (b - a).rem_euclid(TAU);
    if d > TAU / 2.0 {
        d -= TAU;
    }
    d
}

/// The threat mark, authored in MISSILE SPACE: `(forward, sideways, brightness)`.
///
/// Forward runs along the bearing TOWARD own ship, because the thing is incoming. Sideways runs across
/// it. Each cell is rotated onto the screen by `missile`.
///
/// Authoring the silhouette as cells and rotating them - rather than rotating an outline and filling
/// it - is what makes this survive the size. `Canvas::fill_poly` takes integer points, so a 9px outline
/// rounded to whole pixels loses its nose at every oblique bearing and comes out as a blob at about
/// half of them; the nose is the entire reason a viewer reads "incoming" rather than "dot". Placing
/// cells keeps the same silhouette at all 360 bearings.
///
/// 9 long by 5 across at the fins, and the length is the second attempt. The first was 7 long with a
/// two-cell stem, and the zoomed dump showed it reading as a bright blob: at this size what makes a
/// shape legible is NEGATIVE space, not added pixels, and a stem only two cells long left no gap
/// between the head and the fins for the eye to find. Four cells of bare axis is what separates them.
///
/// It is a dart at this size, not a detailed missile - the cue is "elongated, pointed, aimed at the
/// middle", which is what carries on a 50px scope.
const MISSILE: [(i32, i32, f32); 18] = [
    (8, 0, 1.00), // nose
    (7, -1, 0.70),
    (7, 0, 1.00),
    (7, 1, 0.70),
    (6, -1, 0.60),
    (6, 0, 1.00),
    (6, 1, 0.60),
    (5, 0, 1.00), // stem: four cells of bare axis, which is what makes the head read as a head
    (4, 0, 0.95),
    (3, 0, 0.95),
    (2, 0, 1.00),
    (1, -1, 0.45), // fin roots
    (1, 0, 1.00),
    (1, 1, 0.45),
    (0, -2, 0.75), // fins
    (0, -1, 0.40),
    (0, 0, 0.90),
    (0, 2, 0.75),
];

/// Draws the threat mark at `(x, y)`, nose pointing at own ship.
fn missile(lit: &mut Canvas, x: i32, y: i32, bearing: f32, a: f32, hot: &str) {
    if !bearing.is_finite() {
        return;
    }
    // Forward is toward the centre: 0 bearing puts a contact at twelve o'clock, and its nose has to
    // point DOWN the scope at own ship.
    let (fx, fy) = (-bearing.sin(), bearing.cos());
    // Sideways is forward rotated a quarter turn.
    let (sx, sy) = (bearing.cos(), bearing.sin());
    let mut px = [(0i32, 0i32, 0.0f32); MISSILE.len()];
    let mut n = 0usize;
    for (u, v, w) in MISSILE {
        // Centred on the contact rather than nose-first, so the RANGE the mark reports is the middle
        // of the mark and a bearing change rotates it in place instead of swinging it around a tip.
        let uu = u as f32 - 4.0;
        let gx = x + (fx * uu + sx * v as f32).round() as i32;
        let gy = y + (fy * uu + sy * v as f32).round() as i32;
        // Dedup, keeping the brighter cell. At oblique bearings two cells round onto one pixel, and
        // `fill_rect` composites - drawing both would leave a hot spot wherever the silhouette folds,
        // which is the same trap the ring below documents.
        if let Some(e) = px[..n].iter_mut().find(|e| e.0 == gx && e.1 == gy) {
            e.2 = e.2.max(w);
        } else {
            px[n] = (gx, gy, w);
            n += 1;
        }
    }
    for (gx, gy, w) in &px[..n] {
        lit.fill_rect(*gx, *gy, 1, 1, Rgba::from_hex(hot, (a * w).clamp(0.0, 1.0)));
    }
}

/// A 1px ring, exact and gapless.
///
/// Every pixel in the bounding box is tested ONCE against a one-pixel-wide annulus, rather than
/// walking the circle and plotting two points per row. That matters at these alphas: a walk hits the
/// 45-degree pixels from both its row pass and its column pass, and `fill_rect` composites, so a ring
/// printed at 0.13 alpha would have come out at 0.24 on its diagonals - visible as four bright spots
/// on a 45px circle. Testing each pixel once cannot double-composite.
fn ring(c: &mut Canvas, cx: i32, cy: i32, r: i32, col: Rgba) {
    if r < 2 {
        return;
    }
    let rf = r as f32;
    for dy in -r..=r {
        for dx in -r..=r {
            let d = ((dx * dx + dy * dy) as f32).sqrt();
            if d >= rf - 0.5 && d < rf + 0.5 {
                c.fill_rect(cx + dx, cy + dy, 1, 1, col);
            }
        }
    }
}

impl Rwr {
    /// Advance the scope: illuminate the live emitters, and let a new one join when it is due.
    ///
    /// `excess` is how far the low band has risen above its own slew-limited average and `rise` the
    /// threshold that counts as an onset - both owned by `radar.rs`, which already computes them for
    /// the sweep field's close-in contact. This module deliberately owns no opinion about what an onset
    /// is; the project already has two separately-written onset detectors and a third would be a third
    /// threshold to get wrong.
    ///
    /// `launch_at` is the per-colourway knob, 0..1, scaling `LAUNCH_RATIO`: how far above an ordinary
    /// transient a hit has to be before it reads as a launch.
    pub fn update(&mut self, dt: f32, excess: f32, rise: f32, launch_at: f32) {
        let dt = if dt.is_finite() { dt.clamp(0.0, 120.0) } else { 16.7 };
        for t in self.threats.iter_mut() {
            t.glow = (t.glow - dt / GLOW_MS).max(0.0);
            t.launch = (t.launch - dt / LAUNCH_MS).max(0.0);
            t.age += dt;
            if !t.bearing.is_finite()
                || !t.range.is_finite()
                || !t.glow.is_finite()
                || !t.age.is_finite()
            {
                *t = Threat::default();
            }
            // Retired on schedule, whatever the music is doing. THIS is the line that makes a contact go
            // away rather than being pinned up for ever by a continuous groove.
            if t.age >= EMITTER_LIFE_MS {
                *t = Threat::default();
            }
        }
        self.flash = (self.flash - dt / LAUNCH_MS).max(0.0);
        self.spawn_wait = (self.spawn_wait - dt).max(0.0);
        for v in self.recent.iter_mut() {
            if !v.is_finite() {
                *v = TYPICAL_FLOOR;
            }
        }

        let rise = if rise.is_finite() && rise > 0.0 { rise } else { return };
        if !excess.is_finite() || excess <= rise {
            return;
        }
        // Recorded BEFORE the comparison, so the window always describes the material including this
        // hit. A median cannot be skewed by the one sample being tested, which is the whole reason it
        // can be updated first and read immediately.
        self.recent[self.head] = excess;
        self.head = (self.head + 1) % WINDOW;
        self.seen_n = (self.seen_n + 1).min(WINDOW);

        let typical = self.typical_now();
        // Both relative to this track's own dynamics - see the module note.
        let closeness = (excess / (typical * RANGE_REF)).clamp(0.0, 1.0);
        let ratio = excess / typical;
        let need = LAUNCH_RATIO_MIN
            + (LAUNCH_RATIO_MAX - LAUNCH_RATIO_MIN) * launch_at.clamp(0.0, 1.0);
        // No launch until the window describes something. Until then the startup transient could still
        // be the middle value.
        let launched = self.seen_n >= MIN_SAMPLES && ratio >= need;
        #[cfg(test)]
        self.seen.push((ratio, typical));

        // Every emitter still within its life is re-illuminated: they are all out there transmitting.
        // Keyed on `used`, NOT on the current glow - an emitter that dimmed to nothing across a sparse
        // passage has to be able to light up again, or one two-second gap would strand it dark for the
        // rest of its life. Their ranges each ease toward the new reading at their own rate, so the group
        // breathes rather than moving as one object.
        let target = RANGE_RIM - (RANGE_RIM - RANGE_CORE) * closeness;
        let mut any_live = false;
        for t in self.threats.iter_mut() {
            if t.used {
                t.glow = 1.0;
                t.range += (target - t.range) * (RANGE_EASE * t.lag).clamp(0.05, 0.95);
                any_live = true;
            }
        }

        // A new emitter joins when one is due, or immediately if the scope is empty - waiting out a
        // cooldown with nothing on screen would leave the display blank at the start of a track.
        if !any_live || self.spawn_wait <= 0.0 {
            // The faintest slot, so the scope keeps the most recent emitters rather than holding three
            // stale marks and recycling one slot for ever.
            // Only a FREE slot. Retirement by age is what frees them, so nothing ever needs evicting -
            // and evicting would be the one thing that changes a designator under the viewer's eye,
            // which is the failure the whole identity model exists to prevent.
            if let Some(slot) = self.threats.iter().position(|t| !t.used) {
                let n = self.seq as usize;
                self.seq = self.seq.wrapping_add(1);
                self.threats[slot] = Threat {
                    glow: 1.0,
                    age: 0.0,
                    bearing: BEARINGS[n % BEARINGS.len()] * TAU,
                    range: target,
                    launch: 0.0,
                    // Stride 7 against a 15-entry default table: coprime, so the sequence visits every
                    // designator before repeating instead of cycling through three of them.
                    code: n.wrapping_mul(7),
                    lag: 0.7 + 0.2 * (n % 4) as f32,
                    used: true,
                };
                self.spawn_wait = SPAWN_MS;
            }
        }

        // The launch lands on the CLOSEST live emitter, which is the most dangerous one and therefore
        // the one a viewer is already looking at. Not on the newest: a launch from a contact that
        // appeared in the same frame gives the eye nothing to connect it to.
        if launched {
            let mut best: Option<usize> = None;
            for (i, t) in self.threats.iter().enumerate() {
                if t.alpha() > 0.05 && best.is_none_or(|b| t.range < self.threats[b].range) {
                    best = Some(i);
                }
            }
            if let Some(i) = best {
                self.threats[i].launch = 1.0;
                self.flash = 1.0;
                #[cfg(test)]
                {
                    self.launches += 1;
                }
            }
        }
    }

    /// Median of the recent contact sizes - what "an ordinary transient on this track" means.
    ///
    /// Insertion-sorted over at most twelve values on the frames where a transient arrives, which is a
    /// couple of times a second. Sorting a fixed twelve is not worth a cleverer structure.
    fn typical_now(&self) -> f32 {
        let n = self.seen_n.min(WINDOW);
        if n == 0 {
            return TYPICAL_FLOOR;
        }
        let mut buf = [0.0f32; WINDOW];
        buf[..n].copy_from_slice(&self.recent[..n]);
        let slice = &mut buf[..n];
        slice.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = if n % 2 == 1 {
            slice[n / 2]
        } else {
            (slice[n / 2 - 1] + slice[n / 2]) * 0.5
        };
        if mid.is_finite() {
            mid.max(TYPICAL_FLOOR)
        } else {
            TYPICAL_FLOOR
        }
    }

    /// The graticule: print on the glass, straight onto the panel and NOT through the bloom.
    ///
    /// Takes the two ink colours the sweep field already measured against its own wake rather than
    /// choosing its own, so the two halves of the panel carry identically weighted print. Picking
    /// separate values here is how one half of a display ends up looking like a different instrument.
    pub fn print(&self, c: &mut Canvas, s: Scope, ink: Rgba, ink_faint: Rgba) {
        ring(c, s.cx, s.cy, s.r, ink_faint);
        let inner = (s.r as f32 * INNER_RING).round() as i32;
        if inner >= 4 {
            ring(c, s.cx, s.cy, inner, ink_faint);
        }
        // Cardinal ticks, as stubs standing on the outer ring rather than a full cross. A cross through
        // the middle of a 50px scope is 100 lit pixels against a contact's twenty, so the graticule
        // would out-read every threat on the display - the same arithmetic that made the sweep field's
        // bearing lines dashed.
        let stub = (s.r / 4).max(2);
        c.fill_rect(s.cx, s.cy - s.r, 1, stub, ink);
        c.fill_rect(s.cx, s.cy + s.r - stub + 1, 1, stub, ink);
        c.fill_rect(s.cx - s.r, s.cy, stub, 1, ink);
        c.fill_rect(s.cx + s.r - stub + 1, s.cy, stub, 1, ink);
        // Own ship at the centre: a 3px cross, so the scope has a datum to read range against.
        c.fill_rect(s.cx - 1, s.cy, 3, 1, ink);
        c.fill_rect(s.cx, s.cy - 1, 1, 3, ink);
    }

    /// The contacts and the flash: light, onto the layer that gets bloomed.
    ///
    /// **A contact is drawn one of two ways, and which one is the display's whole grammar.** An
    /// ordinary emitter shows its DESIGNATOR - the alphanumeric a real receiver annotates a tracking
    /// emitter with. One that has just launched shows a MISSILE, nose toward own ship, with a ring
    /// expanding off it.
    ///
    /// Never both at once, and that is a size decision rather than a stylistic one: a 9px missile and a
    /// 7px label on the same 50px scope overlap into mush at most bearings. It also happens to be the
    /// correct reading - a receiver tells you WHAT is tracking you until something is launched, at which
    /// point what it is stops mattering.
    ///
    /// There is deliberately NO halo drawn under either mark. There was one, and the zoomed dump showed
    /// exactly what it cost: at this size the halo is as big as the mark, so it filled every gap in a
    /// 1px-featured silhouette and the missile came out as a bright blob. This layer gets `Canvas::bloom`
    /// applied to it afterwards; adding a second halo here was doubling up, and the duplicate was the
    /// one destroying the shape.
    pub fn light(&self, lit: &mut Canvas, t: &Theme, s: Scope, codes: &[String]) {
        let hot = &t.hot;
        for th in self.threats.iter() {
            let a = th.alpha();
            if a <= 0.02 {
                continue;
            }
            let rr = th.range.clamp(0.0, 1.0) * s.r as f32;
            let x = s.cx + (rr * th.bearing.sin()).round() as i32;
            let y = s.cy - (rr * th.bearing.cos()).round() as i32;
            if th.launch > 0.02 {
                missile(lit, x, y, th.bearing, a, hot);
                // A ring expanding away from the contact. Expanding OUTWARD rather than closing in,
                // because it has to be legible in the few frames it is brightest, and a mark that grows
                // is the one animation the eye picks up without looking for it.
                //
                // Starts OUTSIDE the missile, at 6px against its 4.5px half-length. It started at 2px
                // and expanded through the silhouette, which put a bright arc across the fins at the
                // exact moment the shape most needed to be readable.
                let grow = 6.0 + (1.0 - th.launch) * 7.0;
                ring(
                    lit,
                    x,
                    y,
                    grow.round() as i32,
                    Rgba::from_hex(hot, (th.launch * 0.8).clamp(0.0, 1.0)),
                );
            } else if !codes.is_empty() {
                let label = codes[th.code % codes.len()].as_str();
                // Centred on the contact's own position, so the RANGE the label reports is its middle.
                // `text_3x5_width` rather than a character count times four, because the last cell
                // carries no trailing gap and a label centred on the wrong width sits a pixel off.
                let w = Canvas::text_3x5_width(label);
                lit.text_3x5(x - w / 2, y - 2, label, Rgba::from_hex(hot, a));
            }
        }
        // Rim flash: the scope itself reacting.
        //
        // 0.30, down from 0.65 on the first eyeball dump, where the flashing rim was measurably the
        // brightest thing on the panel - a whole circle of near-white `hot` against a sweep line of 0.95
        // on ONE pixel column, and then bloomed on top of that. It read as the display's subject rather
        // than as an event on a supplementary instrument, and it drowned the spectrum beside it.
        if self.flash > 0.02 {
            ring(lit, s.cx, s.cy, s.r, Rgba::from_hex(hot, (self.flash * 0.30).clamp(0.0, 1.0)));
        }
    }

    /// Contacts actually VISIBLE, for the tests - the same test `light` applies, so a test can never
    /// pass on a contact that is being tracked but not drawn.
    #[cfg(test)]
    fn live(&self) -> usize {
        self.threats.iter().filter(|t| t.alpha() > 0.02).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The detector's own threshold, matching what `radar.rs` passes.
    const RISE: f32 = 0.055;
    /// A frame of quiet, i.e. the detector not firing.
    const QUIET: f32 = 0.0;

    /// Drive a groove: `beats` transients of size `excess`, spaced `gap` frames apart.
    fn groove(r: &mut Rwr, beats: usize, excess: f32, gap: usize) {
        for _ in 0..beats {
            r.update(16.7, excess, RISE, 1.0);
            for _ in 0..gap {
                r.update(16.7, QUIET, RISE, 1.0);
            }
        }
    }

    #[test]
    fn a_transient_brings_up_an_emitter_and_a_steady_level_does_not() {
        let mut r = Rwr::default();
        for _ in 0..40 {
            r.update(16.7, QUIET, RISE, 1.0);
        }
        assert_eq!(r.live(), 0, "a level that is not rising must not be received as a threat");
        r.update(16.7, RISE * 2.0, RISE, 1.0);
        assert_eq!(r.live(), 1, "a rise past the threshold must put an emitter up");
    }

    #[test]
    fn an_emitters_bearing_and_designator_never_change_while_it_is_up() {
        // THE fix for "the identifiers jump around too much". The first version recomputed both from
        // the low-band centroid on every transient, and since real music's centroid jitters across a
        // quantisation boundary, the label flickered between two codes on every beat.
        let mut r = Rwr::default();
        r.update(16.7, RISE * 2.0, RISE, 1.0);
        let (b0, c0) = (r.threats[0].bearing, r.threats[0].code);
        // Beats of wildly varying strength - exactly what changed the label before. Held inside one
        // EMITTER_LIFE_MS on purpose: an emitter retiring and a new one taking its slot is turnover,
        // which is wanted, and it would make this assertion measure the wrong thing.
        let beats = (EMITTER_LIFE_MS * 0.8 / (11.0 * 16.7)) as usize;
        for i in 0..beats {
            r.update(16.7, RISE * (1.2 + (i % 7) as f32 * 0.4), RISE, 1.0);
            for _ in 0..10 {
                r.update(16.7, QUIET, RISE, 1.0);
            }
            assert!(r.threats[0].used, "the first emitter retired early, at beat {i}");
            assert_eq!(r.threats[0].bearing, b0, "the bearing moved on beat {i}");
            assert_eq!(r.threats[0].code, c0, "the designator changed on beat {i}");
        }
    }

    #[test]
    fn a_contact_fades_and_goes_away_even_under_continuous_music() {
        // Reported straight off the live overlay: "sigs on the rwr dont go away", then "they need to
        // fade". The cause was one `life` value serving as both illumination and lifetime, which every
        // transient reset - so at two transients a second nothing ever decayed and the scope froze with
        // four permanent contacts. See the note on EMITTER_LIFE_MS.
        let mut r = Rwr::default();
        r.update(16.7, RISE * 2.0, RISE, 1.0);
        let first = (r.threats[0].bearing, r.threats[0].code);
        assert!(r.threats[0].alpha() > 0.9, "a fresh contact should be at full brightness");

        // A relentless groove: a transient every 8 frames, for twice an emitter's life. The old model
        // held every contact at full alpha for the whole run.
        let mut min_alpha_late = 1.0f32;
        let frames = (EMITTER_LIFE_MS * 2.0 / 16.7) as usize;
        let mut gone = false;
        for i in 0..frames {
            r.update(16.7, if i % 8 == 0 { RISE * 2.0 } else { QUIET }, RISE, 1.0);
            // Once the first emitter is past its fade point it must be dimming, and then absent.
            if !r.threats.iter().any(|t| t.used && (t.bearing, t.code) == first) {
                gone = true;
            }
            if gone {
                continue;
            }
            let a = r.threats.iter().find(|t| (t.bearing, t.code) == first).map(|t| t.alpha());
            if let Some(a) = a {
                if r.threats[0].remaining() < FADE_FRAC {
                    min_alpha_late = min_alpha_late.min(a);
                }
            }
        }
        assert!(gone, "the first contact was still up after two full lifetimes of music");
        assert!(
            min_alpha_late < 0.5,
            "it vanished without fading - dimmest late alpha was {min_alpha_late:.2}"
        );
        // And the scope keeps working afterwards: turnover, not exhaustion.
        assert!(r.live() >= 1, "the scope went empty and stayed empty under continuous music");
    }

    #[test]
    fn the_contacts_pulse_with_the_beat_rather_than_sitting_at_a_flat_level() {
        // The visible half of separating illumination from lifetime. A contact that never dims between
        // beats is the frozen display that was reported.
        let mut r = Rwr::default();
        r.update(16.7, RISE * 2.0, RISE, 1.0);
        let peak = r.threats[0].alpha();
        // Most of a second with no onset, which is well inside the emitter's life.
        for _ in 0..48 {
            r.update(16.7, QUIET, RISE, 1.0);
        }
        let dipped = r.threats[0].alpha();
        assert!(dipped < peak * 0.75, "no pulse: {peak:.2} -> {dipped:.2} across 0.8s of no onsets");
        assert!(dipped > 0.05, "it dimmed all the way out inside one bar: {dipped:.2}");
        // And the next transient brings it back, which is what `used` rather than `glow` gates.
        r.update(16.7, RISE * 2.0, RISE, 1.0);
        assert!(
            r.threats[0].alpha() > dipped * 1.3,
            "a dimmed contact could not be re-illuminated - it is stranded dark"
        );
    }

    #[test]
    fn emitters_build_up_and_spread_around_the_scope_rather_than_stacking_in_one_quadrant() {
        // The other half of the live report: "everything is always in the top right quadrant".
        let mut r = Rwr::default();
        // Past three spawn cooldowns but inside one emitter lifetime, so all of them are still up.
        let beats = (SPAWN_MS * 3.2 / (25.0 * 16.7)) as usize;
        groove(&mut r, beats, RISE * 2.0, 24);
        assert!(r.live() >= 3, "three spawn gaps should find several emitters, got {}", r.live());
        let bearings: Vec<f32> = r
            .threats
            .iter()
            .filter(|t| t.alpha() > 0.02)
            .map(|t| t.bearing.rem_euclid(TAU))
            .collect();
        // Every pair clearly apart, so no two marks collide.
        for (i, a) in bearings.iter().enumerate() {
            for b in &bearings[i + 1..] {
                assert!(
                    angle_delta(*a, *b).abs() > 0.5,
                    "two emitters only {:.2}rad apart",
                    angle_delta(*a, *b).abs()
                );
            }
        }
        // And spanning more than one quadrant, which is the property that actually failed.
        let quadrants: std::collections::HashSet<usize> =
            bearings.iter().map(|b| (*b / (TAU / 4.0)) as usize % 4).collect();
        assert!(
            quadrants.len() >= 2,
            "all {} emitters landed in quadrant(s) {:?}",
            bearings.len(),
            quadrants
        );
    }

    #[test]
    fn a_hit_bigger_than_its_neighbours_pulls_the_contacts_closer_to_the_centre() {
        // The family's core mapping, and the one that is easy to get backwards: on a warning receiver
        // closer in is MORE dangerous, the opposite of the sweep field beside it where loud is high.
        //
        // Note WHAT is compared, because the first version of this test compared the wrong thing and
        // failed once the reference became adaptive: it drove one run at a quiet level and another at a
        // level eight times higher, and expected the loud one to sit closer in. Under a median-relative
        // reference that is not just untestable, it is the wrong requirement - a track mastered louder
        // adapts and looks normal, which is the entire point. What must move the mark is a hit that is
        // big RELATIVE TO ITS NEIGHBOURS, so both runs here share one baseline groove and differ only
        // in the spikes at the end.
        let after = |spike: f32| -> f32 {
            let mut r = Rwr::default();
            groove(&mut r, 14, RISE * 2.0, 20);
            // Three spikes, not one: the range EASES toward its target, so a single hit moves it only
            // about a third of the way and the two cases came out 0.09 apart - measurably in the right
            // direction, but too close to assert on meaningfully.
            for _ in 0..3 {
                r.update(16.7, RISE * 2.0 * spike, RISE, 1.0);
                for _ in 0..14 {
                    r.update(16.7, QUIET, RISE, 1.0);
                }
            }
            let live: Vec<f32> = r.threats.iter().filter(|t| t.used).map(|t| t.range).collect();
            assert!(!live.is_empty(), "nothing was up to measure");
            live.iter().sum::<f32>() / live.len() as f32
        };
        let (soft, hard) = (after(0.6), after(1.8));
        assert!(
            hard < soft - 0.15,
            "a hit above the local average must close on the centre: soft {soft:.2} hard {hard:.2}"
        );
        assert!(soft <= RANGE_RIM + 1e-4, "a soft hit must stay off the outer ring, got {soft:.2}");
        assert!(hard >= RANGE_CORE - 1e-4, "and a big one off the own-ship symbol, got {hard:.2}");
    }

    #[test]
    fn a_launch_fires_on_any_material_because_it_is_measured_against_that_material() {
        // THE fix for "I havent seen a missile warning and flash yet". The launch threshold used to be
        // an absolute figure in detector units, calibrated on one fixture; on a track whose transients
        // were all smaller it could not fire at all. Two grooves three octaves apart in absolute size
        // must BOTH launch on their own big hits, and neither on their ordinary ones.
        for base in [RISE * 1.2, RISE * 4.0, RISE * 20.0] {
            let mut r = Rwr::default();
            groove(&mut r, 12, base, 20);
            assert!(r.flash < 0.05, "a steady groove at {base:.3} launched, which must be rare");
            // One hit well above this material's own ordinary size.
            r.update(16.7, base * 4.0, RISE, 1.0);
            assert!(
                r.flash > 0.5,
                "an exceptional hit on material of size {base:.3} did not launch"
            );
            assert!(
                r.threats.iter().any(|t| t.launch > 0.5),
                "the flash fired but no contact carries the launch marker"
            );
        }
    }

    #[test]
    fn the_launch_knob_moves_the_rate_and_a_metronomic_groove_never_fires() {
        // "keep it fairly rare, just for big hits, but allow it to be tunable per theme".
        let fires = |launch_at: f32, spike: f32| -> bool {
            let mut r = Rwr::default();
            for _ in 0..12 {
                r.update(16.7, RISE * 2.0, RISE, launch_at);
                for _ in 0..20 {
                    r.update(16.7, QUIET, RISE, launch_at);
                }
            }
            r.update(16.7, RISE * 2.0 * spike, RISE, launch_at);
            r.flash > 0.5
        };
        // A dead-steady groove must never launch at any setting - that is what "for big hits" means.
        // Including 0.0, the loosest the knob goes: the floor has to hold at the very bottom of its
        // range, which is precisely where the first mapping broke.
        for at in [0.0f32, 0.2, 0.4, 0.7, 1.0] {
            assert!(!fires(at, 1.0), "a metronomic groove launched at threshold {at}");
        }
        // Spikes sized against the window real music actually occupies - see LAUNCH_RATIO_MIN. A 2x
        // hit is off the top of the scale for this material and would pass at every setting, so it
        // cannot tell a working knob from a stuck one.
        assert!(fires(0.2, 1.3), "a 1.3x hit must launch on a loose setting");
        assert!(!fires(1.0, 1.3), "a 1.3x hit must NOT launch on the strictest setting");
        // And the biggest hits that do occur get through even when strict.
        assert!(fires(1.0, 2.0), "a 2x hit must launch even on the strictest setting");
    }

    #[test]
    fn silence_clears_the_scope() {
        let mut r = Rwr::default();
        groove(&mut r, 8, RISE * 2.0, 20);
        assert!(r.live() >= 1, "the groove should have found an emitter");
        // Longer than one emitter's whole life, so both the glow decay and the retirement have run.
        for _ in 0..((EMITTER_LIFE_MS * 1.2 / 16.7) as usize) {
            r.update(16.7, QUIET, RISE, 1.0);
        }
        assert_eq!(r.live(), 0, "silence must clear the scope");
        assert!(
            r.threats.iter().all(|t| !t.used),
            "the slots were never released, so nothing new can ever appear"
        );
    }

    #[test]
    fn a_live_emitter_is_never_overwritten_while_it_is_up() {
        // Turnover has to happen by RETIREMENT, never by eviction: an evicted slot would change a
        // designator under the viewer's eye, which is the failure the identity model exists to prevent.
        // Sampled every frame, because a swap could otherwise happen and be undone between checks.
        let mut r = Rwr::default();
        let mut seen: [(bool, f32, usize); MAX_THREATS] = [(false, 0.0, 0); MAX_THREATS];
        for i in 0..((EMITTER_LIFE_MS * 3.0 / 16.7) as usize) {
            r.update(16.7, if i % 12 == 0 { RISE * 2.0 } else { QUIET }, RISE, 1.0);
            for (slot, t) in r.threats.iter().enumerate() {
                let prev = seen[slot];
                if t.used {
                    if prev.0 {
                        assert!(
                            prev.1 == t.bearing && prev.2 == t.code,
                            "slot {slot} was overwritten in place at frame {i}"
                        );
                    }
                    seen[slot] = (true, t.bearing, t.code);
                } else {
                    // Retired: the slot is free, and whatever appears next is a NEW emitter.
                    seen[slot] = (false, 0.0, 0);
                }
            }
        }
        // And over three lifetimes there must have been real turnover, or nothing is being tested.
        assert!(r.seq >= 4, "only {} emitters ever appeared in three lifetimes", r.seq);
    }

    #[test]
    fn nan_cannot_wedge_the_scope() {
        let mut r = Rwr::default();
        groove(&mut r, 4, RISE * 3.0, 20);
        r.update(f32::NAN, f32::NAN, f32::NAN, f32::NAN);
        r.update(16.7, f32::INFINITY, RISE, 1.0);
        r.update(16.7, RISE * 2.0, f32::NAN, 1.0);
        assert!(
            r.threats.iter().all(|t| t.glow.is_finite()
                && t.age.is_finite()
                && t.bearing.is_finite()
                && t.range.is_finite()),
            "a poisoned frame left the scope holding NaN, which never clears"
        );
        assert!(r.typical_now().is_finite(), "the adaptive reference was poisoned");
        assert!(r.recent.iter().all(|v| v.is_finite()), "the sample window holds NaN");
        // And it must still receive afterwards.
        groove(&mut r, 4, RISE * 3.0, 20);
        assert!(r.live() >= 1, "the scope never recovered after a NaN");
    }

    #[test]
    fn the_ring_is_gapless_and_never_double_composites() {
        // The reason `ring` tests every pixel once instead of walking the circle: a walk hits the
        // 45-degree pixels twice and `fill_rect` composites, so a 0.13-alpha ring shows four bright
        // spots.
        let mut c = Canvas::new(60, 60);
        let col = Rgba::from_hex("#ffffff", 0.20);
        ring(&mut c, 30, 30, 20, col);
        let solo = c.get(30, 10).a;
        assert!(solo > 0, "the ring did not draw at twelve o'clock");
        let diag = (20.0f32 * 0.7071).round() as i32;
        let d = c.get(30 + diag, 30 - diag).a;
        assert!(d > 0, "the ring has a gap on its diagonal");
        assert_eq!(d, solo, "the diagonal composited twice: {d} vs {solo}");
        // Gapless: every 10-degree bearing must find ink within a pixel of the ring.
        for step in 0..36 {
            let th = step as f32 * TAU / 36.0;
            let (x, y) =
                (30 + (20.0 * th.sin()).round() as i32, 30 - (20.0 * th.cos()).round() as i32);
            let hit = (-1..=1)
                .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
                .any(|(dx, dy)| c.get(x + dx, y + dy).a > 0);
            assert!(hit, "no ink near bearing {}deg", step * 10);
        }
    }

    #[test]
    fn the_missile_is_elongated_along_its_bearing_and_not_a_blob() {
        // A dart drawn sideways, or collapsed to a blob, would pass every state test here while the
        // display lost the one cue that says "incoming".
        for (bearing, name) in [(0.0f32, "twelve o'clock"), (TAU / 4.0, "three o'clock")] {
            let mut c = Canvas::new(60, 60);
            missile(&mut c, 30, 30, bearing, 1.0, "#ffffff");
            let (mut x0, mut x1, mut y0, mut y1) = (99, -1, 99, -1);
            for y in 0..60 {
                for x in 0..60 {
                    if c.get(x, y).a > 0 {
                        x0 = x0.min(x);
                        x1 = x1.max(x);
                        y0 = y0.min(y);
                        y1 = y1.max(y);
                    }
                }
            }
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            // At twelve o'clock the long axis is vertical; at three o'clock it is horizontal.
            let (long, short) = if bearing == 0.0 { (h, w) } else { (w, h) };
            assert!(long >= 9, "{name}: the mark is only {long}px along its axis");
            assert!(
                long > short + 2,
                "{name}: {long}x{short} is not elongated - the shape collapsed"
            );
        }
    }

    #[test]
    fn the_scope_gives_up_its_width_before_it_starves_the_spectrum() {
        assert!(width_for(184, 60, 5.75) > 0, "the reference panel has room for the scope");
        assert!(width_for(184, 48, 5.75) > 0, "the real taskbar height has room for it too");
        assert_eq!(width_for(60, 60, 5.75), 0, "a narrow panel must lose the scope, not the spectrum");
        assert_eq!(width_for(184, 14, 5.75), 0, "a 14px-tall panel cannot hold a scope");
        for w in [90, 130, 150, 184, 300, 374] {
            let took = width_for(w, 60, 5.75);
            if took > 0 {
                let left = ((w - took) as f32 / 5.75).floor() as i32;
                assert!(left >= MIN_FIELD_COLS, "at {w}px the field was left {left} columns");
            }
        }
    }

    #[test]
    fn the_scope_is_as_large_as_the_panel_allows_and_stays_inside_it() {
        // It was reported as wanting to be "a bit larger", and the reason was that it was sized off the
        // sweep field's inset rather than the panel's. These are the sizes that fix records.
        for (h, min_r) in [(60, 25), (48, 19), (40, 15)] {
            let s = scope(3, h).unwrap_or_else(|| panic!("no scope at panel height {h}"));
            assert!(s.r >= min_r, "at panel height {h} the radius is only {}", s.r);
            // The panel occupies rows 2..h-3; the ring must clear both.
            assert!(s.cy - s.r >= 2, "at h={h} the ring's top ({}) is off the panel", s.cy - s.r);
            assert!(s.cy + s.r <= h - 3, "at h={h} the ring's bottom ({}) is off the panel", s.cy + s.r);
            assert!(s.cx - s.r >= 3, "at h={h} the scope overhangs the left edge");
        }
    }

    #[test]
    fn every_designator_gets_used_rather_than_three_of_them_on_a_loop() {
        // Stride 7 against 15 codes. A stride sharing a factor with the table length would cycle
        // through a handful for ever, which looks like a bug in the table.
        let codes = 15usize;
        let seen: std::collections::HashSet<usize> =
            (0..codes).map(|n| n.wrapping_mul(7) % codes).collect();
        assert_eq!(seen.len(), codes, "only {} of {codes} designators are reachable", seen.len());
    }
}
