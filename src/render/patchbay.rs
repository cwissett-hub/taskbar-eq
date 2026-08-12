//! The modular-synth patchbay family: a brushed-metal front panel, two rows of jack sockets,
//! and patch cables strung between them - one cable per band group.
//!
//! The cue this family is built on is the SAG of the cables, not their brightness. Position is
//! resolved far more readily than intensity at 190x60: the valve row measured only a 1.16x
//! brightness spread between a driven element and its idle neighbour, which is below the
//! threshold at which a difference can be seen at all. So a band pulls its cable from a deep
//! droop to a taut upward arch - 14px of midpoint travel at the reference size - and the
//! brightening is only the confirming detail.
//!
//! Four things are deliberate departures from the other families:
//!
//! - **The cables are objects, not light.** Their 3px sheath is drawn OPAQUE straight onto the
//!   panel, and only a 1px core goes on the bloomed light layer. This is forced by how `bloom`
//!   works: it blurs ALPHA as well as colour, so an opaque dark object put on the light layer
//!   would halo its own dark body, and a dim idle cable would sit in a grey smudge.
//! - **The sheath's brightness is carried by COLOUR, not alpha.** It is stroked as a run of
//!   overlapping 3x3 blocks, and a translucent stroke source-overs itself wherever it overlaps -
//!   1 - (1 - a)^2 rather than a - which would bead the cable at every sample point. An opaque
//!   stroke is idempotent, so overlap costs nothing and only the colour has to carry the level.
//! - **The collars go on last.** Each socket's rim is drawn over the composited light, so a
//!   cable reads as terminating *inside* its socket rather than being painted across it. Same
//!   reason the valve row draws its glass over its glow.
//! - **The LEDs fire on a RISE, not on a level.** Same detector shape as `vapor::update_bolt`.
//!   A level-triggered indicator sits on solidly through any loud passage, which reads as a
//!   stuck lamp rather than as a beat.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// Clear space left either side of the jack rows, in pixels.
///
/// Not merely cosmetic: the two indicator LEDs live in these margins, at the panel's vertical
/// centre, because that is the only clear space on the panel. The band between the jack rows is
/// where the cables sweep - the droop covers 14px of it by design - so a lamp placed in there
/// would be crossed by a cable at some level and read as a bright knot in it.
const MARGIN: i32 = 7;

/// Panel width one cable wants, in pixels.
///
/// A cable spans one jack pitch horizontally and needs a patched pair (its own top jack and
/// its own bottom jack), so 34 buys a ~17px jack pitch - the narrowest at which a 7px socket
/// still has visible metal either side of it. Five cables at the 190px reference, which is the
/// count the sag travel was tuned against: fewer and each cable owns too much of the spectrum
/// to differ from its neighbours, more and the droop of one cable overlaps the next.
const CABLE_PITCH: i32 = 34;

/// Below these the panel is drawn bare and nothing else.
///
/// Two jack rows plus a cable with legible droop between them needs about 30 rows; below that
/// the "cable" is a 2px diagonal smudge and the sockets merge into it, which reads as damage
/// rather than as a small patchbay. Same degradation `tube` uses, for the same reason.
const MIN_W: i32 = 44;
const MIN_H: i32 = 30;

/// Band level at which a cable starts to move, and the span it moves over.
///
/// Taken from the valve row's measurement rather than re-derived: `FrameData.levels` only
/// delivers about 0.15..0.65 for active bands, so a mapping linear over 0..1 spends barely a
/// third of its range on real music and the panel looks dead. See `tube::RESP_FLOOR`.
///
/// Fixed, not a peak follower, for the same reason the valve row's is fixed: this is a level
/// meter, so the same band level must always give the same sag. A follower would show a quiet
/// passage at the same droop as a loud one.
/// The response window: where 0 and 1 of a cable's travel sit, in group-level units.
///
/// PLACED ON THE GROUP LEVELS THIS FAMILY ACTUALLY PRODUCES, which is not the same thing as the band
/// levels the DSP produces, and the difference was a real defect. `level_for` reduces a cable's whole
/// band group biased 0.65 toward its PEAK, and at 190px there are only 5 cables, so each one peaks over
/// about 12.8 bands. That group value is far above a typical single band.
///
/// The span was 0.52, which put the top of the travel at level 0.62 - below the bass cable's MEDIAN
/// group level of 0.63-0.69 on real music. Measured over the three real-music captures, the lowest
/// cable therefore sat pinned at full deflection **64%, 96% and 100%** of all frames on the three
/// tracks, and at 380px the second cable pinned 30-67% too. A cable at the end of its travel carries no
/// information at all, which killed the family's only cue on the cable most likely to be watched.
///
/// 0.70 puts the top at 0.80, just above the p99 of every cable on every fixture (0.70-0.79). Measured
/// result: pinning falls to **0% on every cable, both widths, all three tracks**. It costs about a
/// quarter of the separation between cables - mean pairwise separation 0.206 -> 0.153 at 190px - which
/// is the honest trade and is why an intermediate 0.62 was measured too: that keeps separation at 0.173
/// but leaves the bass cable pinned 29% of the time on dynamic music, i.e. still dead a third of the
/// time. Losing a quarter of the separation to gain a cable that always moves is the better bargain.
const RESP_FLOOR: f32 = 0.10;
const RESP_SPAN: f32 = 0.70;

/// Weight given to a group's LOUDEST band rather than its mean.
///
/// Five cables across 64 log bands is ~12.8 bands each, and averaging that many flattens
/// exactly the single-band peaks that make one cable differ from the next. The valve row
/// measured a plain mean at 1.46 dL* between a driven element and its neighbour (below the
/// ~2.3 dL* visible threshold) against 9.47 dL* for a max-biased blend. Note that any test
/// driving every band to the SAME level cannot see this at all - mean equals max there.
const GROUP_MAX_BIAS: f32 = 0.65;

/// Midpoint displacement of a cable, as a fraction of the gap between the two jack rows.
///
/// Positive is downward. The whole family rests on this: 0.21 -> -0.12 is 0.33 of the 43px
/// gap, i.e. 14px of travel at 190x60, an unmistakable move at a size where a brightness
/// change of the same "size" is invisible. Expressed as a fraction so it scales with the
/// panel rather than sagging off the bottom of a shorter one.
///
/// The driven value is NEGATIVE on purpose - a driven cable arches slightly upward rather than
/// merely straightening. A quadratic stays inside the convex hull of its three control points,
/// and the control point is only ever `2 * sag` from the chord midpoint, so at these values the
/// curve provably cannot leave the band between the two jack rows: no clipping is needed and a
/// taut cable cannot collide with the socket row above it.
const SAG_IDLE: f32 = 0.21;
const SAG_DRIVEN: f32 = -0.12;

/// Per-frame slew of a cable's displayed drive, rising and falling.
///
/// A cable has mass, and the sag is a POSITION, so it is worth spending a slew on: the whole
/// travel is 14px, which is 0.33px per 0.01 of level, so an ungated band would move the cable a
/// pixel on frame-to-frame ripple alone and read as jitter rather than as movement. Asymmetric
/// (1.8 frames up, 6.3 down) so it snaps taut and settles back, which is both what a real cable
/// does and what makes the direction of travel readable. dt-scaled, so a slow render loop does
/// not change the feel.
const CABLE_ATTACK: f32 = 0.55;
const CABLE_RELEASE: f32 = 0.16;

/// Fraction of its full colour an idle cable's sheath keeps.
///
/// Never zero: a patch cable is a physical object and is there whether or not signal is flowing
/// through it. A cable that vanished at silence would make the panel look unpatched, which is the
/// same mistake the valve row's heater floor exists to avoid. 0.42 keeps an idle cable clearly
/// off the brushed panel behind it without eating the range the music needs. Measured on the
/// classic colourway: an idle cable's sheath reads 49.6 and its core 108.7, against bare metal at
/// 29.0; driven, the same pixels read 118.6 and 215.2. The greyest colourway is the tight case,
/// where the metal is already at 59.0 - see `probe_patchbay_contrast` for the whole table.
const SHEATH_IDLE: f32 = 0.42;

/// Alpha the 1px cable core keeps at idle. Low, because this is the part that blooms.
const CORE_IDLE: f32 = 0.20;

/// Rise in mean bass needed to fire the LEDs, and the floor below which nothing fires.
///
/// Same shape as `vapor::update_bolt`: a difference against the previous frame's bass, not a
/// threshold on its level. Bass bands sit at 0.15-0.65 on real music and a kick lifts the
/// 4-band mean by roughly 0.06-0.15 on the frame it lands, so 0.055 fires on beats and not on
/// the ripple between them. Divided by `sensitivity`, so the theme's one gain knob makes the
/// indicators easier rather than harder to trigger, matching what it does everywhere else.
const LED_RISE: f32 = 0.055;
const LED_FLOOR: f32 = 0.08;

/// Per-frame decay of the two indicators.
///
/// Two rates, because two identical lamps blinking together read as one wide lamp. The fast
/// one is ~9 frames (150ms) - a blink, deliberately shorter than the gap between beats, or it
/// would never be seen to go out. The slow one holds ~370ms, so the pair reads as a trigger
/// and a gate.
const LED_FALL: f32 = 0.115;
const LED_TAIL_FALL: f32 = 0.045;

/// The flourish: the panel is re-patched and put back.
///
/// Every cable swaps to the OTHER jack of its own pair, so the chevron of leans mirrors itself, and
/// then swaps back. That is the one thing a patchbay does that nothing else in this project does, and
/// it uses the geometry already here: `cable_ends` gives each cable two jacks, and the unpatched jack
/// of each pair is sitting right there waiting to be plugged into.
///
/// It ANIMATES rather than switching. Each endpoint slides along the row to the other jack, so the
/// cables straighten to vertical, cross, lean the other way, and come back. A hard swap was tried
/// first and reads as a dropped frame - at 60fps a single-frame change of shape is not perceived as
/// motion at all, which is the same reason the tape flourish decays rather than snapping.
///
/// 1300ms for the whole out-and-back, so each direction gets ~650ms. The swing is
/// `sin(pi * (1 - level))`, which is zero at the firing frame, one at half the envelope, and zero
/// again as it expires - the envelope's linear decay is thereby used as a phase rather than as an
/// amplitude, which is what makes the cable arrive back in its own socket instead of fading out
/// halfway between two.
const REPATCH_MS: f32 = 1300.0;

/// The render loop's nominal period, for dt scaling. Matches `FrameData::default`.
const NOMINAL_DT_MS: f32 = 16.7;

/// Cables at a given panel width.
///
/// Scaled rather than fixed, for the reason the valve row measured: a fixed count stretches,
/// and at 380px five cables would sit 68px apart with a droop half the panel wide, which reads
/// as bunting rather than as a patchbay. Adding cables keeps every one the size it was tuned
/// at and narrows each one's share of the spectrum, so neighbours differ more.
fn cable_count(w: i32) -> usize {
    ((((w - MARGIN * 2) / CABLE_PITCH).max(2)) as usize).min(16)
}

/// Horizontal pitch of the jack rows. Two jacks per cable: every cable is patched from its own
/// top jack to its own bottom jack, and the *other* jack of each pair is left unpatched, which
/// is what a real panel looks like and what gives the cables air to be read separately.
fn jack_pitch(w: i32, jacks: usize) -> f32 {
    ((w - MARGIN * 2) as f32 / jacks.max(1) as f32).max(4.0)
}

/// Centre x of jack `k`. Shared with the tests rather than restated there - the sag
/// measurements sample a specific column, and a test that re-derives the geometry silently
/// starts measuring the bare panel the moment a constant changes.
fn jack_x(w: i32, jacks: usize, k: usize) -> i32 {
    MARGIN + (jack_pitch(w, jacks) * (k as f32 + 0.5)) as i32
}

fn jack_radius(h: i32) -> i32 {
    if h >= 44 {
        3
    } else {
        2
    }
}

/// Centre rows of the top and bottom jack rows.
fn jack_rows(h: i32) -> (i32, i32) {
    let r = jack_radius(h);
    (2 + r + 3, h - 3 - r - 3)
}

/// The (top jack, bottom jack) a cable is patched between.
///
/// The lean ALTERNATES. All the cables leaning the same way read as a comb, and running them
/// all as crossings read as a knot at 60px tall - in both cases you cannot follow one cable,
/// which makes its sag worthless as a cue. Alternating gives a chevron of cables that never
/// touch, so each droop is separable.
fn cable_ends(i: usize) -> (usize, usize) {
    if i % 2 == 0 {
        (2 * i, 2 * i + 1)
    } else {
        (2 * i + 1, 2 * i)
    }
}

/// Deterministic 0..1 from an integer. Used only for the brushed-metal grain, and only ever
/// keyed on the ROW INDEX: an early version mixed in a frame counter and the grain crawled,
/// which reads as noise across the whole widget rather than as a surface. Metal does not move.
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(2654435761).wrapping_add(0x9E37_79B9);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x >> 8) as f32 / 16_777_216.0
}

/// Scales a colour's brightness, leaving its alpha alone.
///
/// This, and not alpha, is how the sheath and the screw heads vary: see the module docs. An
/// opaque stroke can overlap itself freely, a translucent one cannot.
fn dim(c: Rgba, k: f32) -> Rgba {
    let k = if k.is_finite() { k.clamp(0.0, 1.0) } else { 0.0 };
    Rgba::new(
        (c.r as f32 * k).round() as u8,
        (c.g as f32 * k).round() as u8,
        (c.b as f32 * k).round() as u8,
        c.a,
    )
}

/// 1px circle outline.
///
/// `Canvas` has `fill_circle` but no outline, and the collar has to be a RING: filled, it
/// covers the plug tip the cable terminates in and the socket reads as a stud rather than a
/// hole. 0.55 is the widest band that still gives a single-pixel rim at r=2.
fn ring(c: &mut Canvas, cx: i32, cy: i32, r: i32, col: Rgba) {
    if r <= 0 {
        return;
    }
    let rf = r as f32;
    for dy in -r..=r {
        for dx in -r..=r {
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if (dist - rf).abs() <= 0.55 {
                c.fill_rect(cx + dx, cy + dy, 1, 1, col);
            }
        }
    }
}

/// Walks the quadratic from `p0` to `p1` whose MIDPOINT sits `sag` pixels below the straight
/// chord, appending one entry per distinct pixel.
///
/// The control point is placed at `2 * sag` because `P(0.5) = 0.25*P0 + 0.5*C + 0.25*P1` - so
/// half of the control point's displacement is what reaches the curve. Getting this wrong is
/// silent: the cable still curves, it just curves by half of what the constants claim, and
/// every measurement taken against it is then wrong by the same factor.
///
/// Pixels are DEDUPLICATED, which is what makes the 1px alpha core safe to draw: the same pixel
/// stroked twice with a translucent colour is visibly brighter, and at ~1.5 samples per pixel
/// most samples would otherwise repeat, beading the cable along its whole length. x is exactly
/// linear in t here (the control point sits at the chord's midpoint in x), so a pixel can only
/// repeat within one column - the last two are enough to catch it, and two rather than one
/// because at the curve's vertex y can round across a boundary and back inside a single column.
fn cable_path(p0: (f32, f32), p1: (f32, f32), sag: f32, out: &mut Vec<(i32, i32)>) {
    out.clear();
    if !p0.0.is_finite() || !p0.1.is_finite() || !p1.0.is_finite() || !p1.1.is_finite() {
        return;
    }
    let sag = if sag.is_finite() { sag } else { 0.0 };
    let cx = (p0.0 + p1.0) * 0.5;
    let cy = (p0.1 + p1.1) * 0.5 + sag * 2.0;
    // 1.5 samples per pixel of Manhattan length, so no step can skip a pixel and leave a gap
    // in the stroke. Capped, because a degenerate geometry must not turn into a long loop.
    let len = (p1.0 - p0.0).abs() + (p1.1 - p0.1).abs() + sag.abs() * 2.0;
    let steps = (len * 1.5).clamp(8.0, 600.0) as i32;
    let mut seen = [(i32::MIN, i32::MIN); 2];
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let u = 1.0 - t;
        let x = u * u * p0.0 + 2.0 * u * t * cx + t * t * p1.0;
        let y = u * u * p0.1 + 2.0 * u * t * cy + t * t * p1.1;
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let px = (x.round() as i32, y.round() as i32);
        if px == seen[0] || px == seen[1] {
            continue;
        }
        seen[1] = seen[0];
        seen[0] = px;
        out.push(px);
    }
}

#[derive(Default)]
pub struct Patchbay {
    /// The flourish: the panel re-patches itself and comes back. See `REPATCH_MS`.
    flourish: crate::dsp::flourish::Trigger,
    repatch: crate::dsp::flourish::Envelope,
    /// Slewed drive per cable, in response units. A `Vec` because `cable_count` scales with
    /// width and the std array `Default` impls stop at 32 anyway.
    disp: Vec<f32>,
    /// Previous frame's mean bass, for the rise detector.
    prev_bass: f32,
    /// The fast indicator, and the one with the longer tail.
    led: f32,
    led_tail: f32,
}

impl Patchbay {
    /// Level feeding one cable: the mean of its bands blended toward their max.
    ///
    /// Non-finite bands are skipped rather than clamped, because `f32::clamp` does NOT
    /// sanitise NaN (every comparison with NaN is false, so it falls through and returns NaN),
    /// and a NaN here would reach the slewed `disp` and stick there permanently.
    fn level_for(d: &FrameData, i: usize, cables: usize) -> f32 {
        let n = d.levels.len();
        let cables = cables.max(1);
        let lo = i * n / cables;
        let hi = (((i + 1) * n / cables).max(lo + 1)).min(n);
        let mut acc = 0.0f32;
        let mut cnt = 0.0f32;
        let mut peak = 0.0f32;
        for v in &d.levels[lo..hi] {
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
        (mean * (1.0 - GROUP_MAX_BIAS) + peak * GROUP_MAX_BIAS).clamp(0.0, 1.0)
    }

    /// Maps a group level onto 0..1 of cable travel through the window the DSP actually
    /// occupies, scaled by the theme's `sensitivity`.
    fn response(level: f32, sensitivity: f32) -> f32 {
        if !level.is_finite() {
            return 0.0;
        }
        (((level - RESP_FLOOR) / RESP_SPAN) * sensitivity.max(0.0)).clamp(0.0, 1.0)
    }

    /// Advances the bass-transient detector. Returns nothing - the two indicator levels are
    /// read off `self` - because they decay at different rates and only one of them is "the"
    /// answer at any moment.
    fn update_leds(&mut self, d: &FrameData, sensitivity: f32, dt: f32) {
        // Mean of the lowest four bands, finite entries only - the same window vapor's bolt
        // detector uses, which is roughly everything below ~120Hz at 64 log bands.
        let n = d.levels.len().min(4).max(1);
        let mut acc = 0.0f32;
        let mut cnt = 0.0f32;
        for v in &d.levels[..n] {
            if v.is_finite() {
                acc += *v;
                cnt += 1.0;
            }
        }
        let bass = if cnt > 0.0 { acc / cnt } else { 0.0 };
        let need = LED_RISE / sensitivity.max(0.25);
        if bass - self.prev_bass > need && bass > LED_FLOOR {
            self.led = 1.0;
            self.led_tail = 1.0;
        }
        // `bass` is finite by construction above, so this cannot poison the comparison on the
        // next frame - which is exactly how the vaporwave scroll phase was corrupted once.
        self.prev_bass = bass;
        self.led = (self.led - LED_FALL * dt).max(0.0);
        self.led_tail = (self.led_tail - LED_TAIL_FALL * dt).max(0.0);
    }
}

impl Family for Patchbay {
    fn id(&self) -> &'static str {
        "patchbay"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();
        c.rounded_rect(
            1,
            2,
            (w - 2).max(1),
            (h - 4).max(1),
            3,
            Rgba::from_hex(&t.panel, t.panel_alpha),
        );
        if w < MIN_W || h < MIN_H {
            return;
        }

        // Brushed metal, in two parts: the vertical gradient gives the panel its top-lit form,
        // the striations give it grain. Both read the valve row's chassis colours - a rack
        // panel and a valve chassis are the same milled aluminium, and reusing `TubeParams`
        // keeps this family off the theme schema entirely.
        c.vertical_gradient(
            2,
            3,
            w - 4,
            h - 6,
            &[
                (0.0, Rgba::from_hex(&t.tube.chassis_top, 0.62)),
                (0.55, Rgba::from_hex(&t.tube.chassis_top, 0.26)),
                (1.0, Rgba::from_hex(&t.tube.chassis_bottom, 0.62)),
            ],
            true,
        );
        // Grain: EVERY row gets a small signed offset rather than a third of them getting a big
        // one. The first version lit only the rows hashing above 0.62, and at the alpha needed
        // to see them at all that measured 15 luminance between neighbouring rows - which reads
        // as a pinstripe, or worse as the scanline texture another family already uses. A small
        // offset on every row measures ~6 and reads as a surface.
        for y in 3..(h - 3) {
            let n = hash01(y as u32) - 0.5;
            if n > 0.0 {
                c.fill_rect(2, y, w - 4, 1, Rgba::from_hex(&t.tube.glass, 0.065 * n));
            } else {
                c.fill_rect(2, y, w - 4, 1, Rgba::from_hex(&t.tube.chassis_bottom, 0.18 * -n));
            }
        }
        // Bevel: one bright row at the top of the plate and one dark row at the bottom, so the
        // plate has a lit edge and a shadowed one. The gradient alone gives the panel a tone but
        // no thickness, and thickness is what distinguishes a piece of metal from a backdrop.
        c.fill_rect(2, 3, w - 4, 1, Rgba::from_hex(&t.tube.glass, 0.11));
        c.fill_rect(2, h - 4, w - 4, 1, Rgba::from_hex(&t.tube.chassis_bottom, 0.55));

        // Screw heads. The one detail that says "front panel" rather than "dark rectangle with
        // lines on it" - and they are drawn before the cables purely because nothing else ever
        // reaches the corners, so no ordering question arises.
        let screw = dim(Rgba::from_hex(&t.tube.collar, 1.0), 0.62);
        let slot = Rgba::from_hex(&t.tube.internals, 0.9);
        for (sx, sy) in [(5, 6), (w - 6, 6), (5, h - 7), (w - 6, h - 7)] {
            c.fill_circle(sx, sy, 2, screw);
            c.fill_rect(sx - 1, sy, 3, 1, slot);
            c.fill_rect(sx - 1, sy - 1, 1, 1, Rgba::from_hex(&t.tube.glass, 0.42));
        }

        let cables = cable_count(w);
        let jacks = cables * 2;
        let jr = jack_radius(h);
        let (jy_top, jy_bot) = jack_rows(h);

        // Socket wells, all of them - patched and unpatched alike, and UNDER the cables so a
        // cable can be seen to enter one.
        let well = Rgba::from_hex(&t.tube.socket, 1.0);
        for k in 0..jacks {
            let x = jack_x(w, jacks, k);
            c.fill_circle(x, jy_top, jr, well);
            c.fill_circle(x, jy_bot, jr, well);
        }

        // `clamp` does NOT sanitise NaN, so dt has to be tested before it is clamped, or a
        // single NaN frame permanently freezes every cable's slew.
        let dt = if d.dt_ms.is_finite() {
            (d.dt_ms / NOMINAL_DT_MS).clamp(0.25, 4.0)
        } else {
            1.0
        };
        self.update_leds(d, t.sensitivity, dt);

        // THE FLOURISH: a re-patch. See `REPATCH_MS`. `swing` is how far each cable has slid toward
        // the other jack of its pair, 0..1..0 across the envelope.
        let fired = self.flourish.update(&d.levels, d.dt_ms, t.flourish);
        let repatch = self.repatch.update(fired, d.dt_ms, REPATCH_MS);
        let swing = if repatch > 0.0 {
            (std::f32::consts::PI * (1.0 - repatch)).sin().clamp(0.0, 1.0)
        } else {
            0.0
        };

        if self.disp.len() != cables {
            // Seeded from the live spectrum, not from zero. Zero means "idle" in the sag
            // mapping, so a fresh panel - or one that has just been resized, or had its theme
            // switched - otherwise starts with every cable at full droop and visibly hauls
            // them all up over the following ~150ms.
            self.disp = (0..cables)
                .map(|i| Self::response(Self::level_for(d, i, cables), t.sensitivity))
                .collect();
        }

        // Everything that emits light goes on its own transparent layer, to be bloomed once and
        // composited over the opaque panel. `Canvas::bloom` puts its halo UNDERNEATH what is
        // already on the canvas, so blooming the panel in place leaves the halo behind an
        // opaque wall and invisible - the trap already documented in segmented, scope, vu and
        // tube.
        let mut lit = Canvas::new(w, h);
        let span = (jy_bot - jy_top).max(6) as f32;
        let mut path: Vec<(i32, i32)> = Vec::with_capacity(256);

        for i in 0..cables {
            // Colour by position along the row, via `lit_at`/`hot_at`. With no zones declared
            // that is just `t.lit`, so a single-colour colourway needs no special case; with
            // them it is how the rainbow and primary-cable colourways get a different colour
            // per cable without adding anything to the theme schema.
            let frac = (i as f32 + 0.5) / cables as f32;
            let resp = Self::response(Self::level_for(d, i, cables), t.sensitivity);
            let k = if resp > self.disp[i] { CABLE_ATTACK } else { CABLE_RELEASE };
            self.disp[i] += (resp - self.disp[i]) * (k * dt).clamp(0.0, 1.0);
            if !self.disp[i].is_finite() {
                self.disp[i] = 0.0;
            }
            let drive = self.disp[i].clamp(0.0, 1.0);

            // Endpoints, slid toward the other jack of this cable's pair by `swing`. At 0.5 both
            // ends sit on the pair's midpoint and the cable is vertical; at 1.0 the lean is fully
            // inverted and the plugs are in the two sockets that are normally empty.
            let (a, b) = cable_ends(i);
            let xa = jack_x(w, jacks, a) as f32;
            let xb = jack_x(w, jacks, b) as f32;
            let p0 = (xa + (xb - xa) * swing, jy_top as f32);
            let p1 = (xb + (xa - xb) * swing, jy_bot as f32);
            let sag = span * (SAG_IDLE + (SAG_DRIVEN - SAG_IDLE) * drive);
            cable_path(p0, p1, sag, &mut path);

            // Sheath: opaque, 3px, straight onto the panel. See the module docs for why it is
            // neither translucent nor on the light layer.
            let sheath = dim(
                Rgba::from_hex(t.lit_at(frac), 1.0),
                SHEATH_IDLE + (1.0 - SHEATH_IDLE) * drive,
            );
            for &(x, y) in path.iter() {
                c.fill_rect(x - 1, y - 1, 3, 3, sheath);
            }

            // Plug tips, before the core so the one pixel they share is not blended twice.
            // These brighten with the cable, so the sockets confirm what the sag is saying.
            let tip = Rgba::from_hex(t.hot_at(frac), (0.26 + 0.64 * drive).clamp(0.0, 1.0));
            for (px, py) in [p0, p1] {
                lit.fill_rect(px as i32 - 1, py as i32 - 1, 3, 3, tip);
            }

            // Core: 1px down the middle of the sheath, and the only part of the cable that
            // blooms.
            let core = Rgba::from_hex(t.hot_at(frac), (CORE_IDLE + (1.0 - CORE_IDLE) * drive).clamp(0.0, 1.0));
            for &(x, y) in path.iter() {
                lit.fill_rect(x, y, 1, 1, core);
            }
        }

        // Indicator lenses. The dark lens body goes on the PANEL, because an unlit LED is a
        // dark lens and not a hole; only the light goes on the bloomed layer. Drawn dark on
        // the light layer instead, an idle LED bloomed its own black body into a grey smear.
        // Placed in the SCREWS' own columns, so each margin reads as a deliberate column of
        // hardware - screw, lamp, screw - rather than as three things that happened to land
        // near each other. Also moves the lens a pixel off the bezel, which it was touching.
        let lens_y = h / 2;
        let lens = dim(Rgba::from_hex(&t.lit, 1.0), 0.16);
        for (x, level) in [(5, self.led), (w - 6, self.led_tail)] {
            c.fill_circle(x, lens_y, 2, lens);
            c.fill_rect(x - 1, lens_y - 1, 1, 1, Rgba::from_hex(&t.tube.glass, 0.30));
            if level > 0.02 {
                lit.fill_circle(x, lens_y, 1, Rgba::from_hex(&t.hot, level.clamp(0.0, 1.0)));
            }
        }

        if t.bloom > 0.0 {
            let mut glow = lit.clone();
            glow.bloom(t.bloom.max(0.0) as i32, t.glow_strength.clamp(0.0, 1.0));
            c.draw_over(&glow);
        }
        c.draw_over(&lit);

        // Collars LAST, over the composited light - see the module docs. Every jack gets one,
        // so the unpatched ones read as sockets too rather than as dents.
        let collar = Rgba::from_hex(&t.tube.collar, 0.80);
        for k in 0..jacks {
            let x = jack_x(w, jacks, k);
            ring(c, x, jy_top, jr, collar);
            ring(c, x, jy_bot, jr, collar);
        }

        // The bloom above can spread `t.bloom` pixels in every direction, which is far more
        // than the 1-2px margin the panel is inset by, so without this the halo of the outer
        // cables and of the right-hand LED spills onto the bare taskbar as a bright edge
        // outside the panel. Same rect and radius the panel was drawn with, so the rounded
        // corners are respected exactly rather than clipped to a bounding box.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);

        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
        c.fill_rect(1, 2, 1, h - 4, e);
        c.fill_rect(w - 2, 2, 1, h - 4, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where cable 0 crosses a row near its top end, as the brightest column within its pair's span.
    ///
    /// Measured off the pixels rather than recomputed from `swing`, because recomputing it would
    /// assert that the arithmetic equals itself. The claim under test is that the cable MOVES.
    ///
    /// The row is 5 below the jack centres, and the offset matters: a jack is a filled well plus a
    /// collar ring of radius 3, so any row within 3 of `jy_top` contains bright static socket pixels at
    /// every jack column. Sampled there, the measurement snapped to a jack column on every frame and
    /// reported that the cable teleported when it was in fact sliding smoothly.
    fn top_end_x(c: &Canvas) -> i32 {
        let (w, h) = (c.width(), c.height());
        let jacks = cable_count(w) * 2;
        let (jy_top, _) = jack_rows(h);
        let (x0, x1) = (jack_x(w, jacks, 0), jack_x(w, jacks, 1));
        let y = jy_top + 5;
        ((x0 - 3)..=(x1 + 3))
            .max_by(|a, b| lum(c.get(*a, y)).partial_cmp(&lum(c.get(*b, y))).unwrap())
            .unwrap_or(x0)
    }

    /// Light in the socket of jack `k` on the top row - a plug tip is drawn there when a cable
    /// terminates in it, so this is how "it is plugged into the OTHER jack" gets asserted directly
    /// rather than inferred from a column measurement.
    fn socket_light(c: &Canvas, k: usize) -> f64 {
        let (w, h) = (c.width(), c.height());
        let jacks = cable_count(w) * 2;
        let (jy_top, _) = jack_rows(h);
        let x = jack_x(w, jacks, k);
        let mut acc = 0.0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                acc += lum(c.get(x + dx, jy_top + dy));
            }
        }
        acc
    }

    /// Draws `frames` frames of steady audio, forcing the flourish on the first if asked.
    ///
    /// Returns every frame's measured top-end position, the FINAL canvas, and the canvas of the frame
    /// where jack 1's socket was brightest - two different frames answering two different questions,
    /// and returning only one of them silently broke the recovery test when it was reading the peak.
    ///
    /// Forced on the trigger instance rather than posted through `flourish::request()`: that is one
    /// process-global atomic and every family's `draw` consumes it, so in a parallel suite an
    /// unrelated drawing test eats it. See the note in `Trigger::update`.
    fn repatch_trace(fire: bool, frames: usize) -> (Vec<i32>, Canvas, Canvas) {
        let mut t = builtin::all()
            .into_iter()
            .find(|t| t.family == "patchbay")
            .expect("no patchbay colourway");
        // Zero, so the audio path cannot fire a second time and the arms differ only in the force.
        t.flourish = 0.0;
        let mut p = Patchbay::default();
        let mut c = Canvas::new(190, 60);
        let d = flat(0.35);
        for _ in 0..20 {
            p.draw(&mut c, &t, &d);
        }
        if fire {
            p.flourish.force_next();
        }
        let mut xs = Vec::new();
        let mut peak = (f64::MIN, Canvas::new(190, 60));
        for _ in 0..frames {
            p.draw(&mut c, &t, &d);
            xs.push(top_end_x(&c));
            // Keep the frame where jack 1's socket is brightest - the moment the plug is furthest into
            // the socket that is normally empty.
            let l = socket_light(&c, 1);
            if l > peak.0 {
                peak = (l, c.clone());
            }
        }
        (xs, c, peak.1)
    }

    /// Run: cargo test --release dump_patchbay_window -- --ignored --nocapture
    ///
    /// The before and after of re-placing the response window, on real music, for eyeballing - this
    /// changes how a shipped family looks and the numbers alone should not decide that.
    ///
    /// The OLD mapping is reproduced exactly rather than approximated: `response` is linear in
    /// sensitivity, so `(level - FLOOR) / 0.52` is identical to `((level - FLOOR) / 0.70) * (0.70/0.52)`.
    /// Setting sensitivity to that ratio therefore renders the pre-fix behaviour through the post-fix
    /// code, so the two rows differ in the window and in nothing else.
    #[test]
    #[ignore]
    fn dump_patchbay_window() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let frames: Vec<Vec<f32>> =
            include_str!("../../tests/fixtures/real-music-dynamic.csv")
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.split(',').filter_map(|v| v.parse::<f32>().ok()).collect())
                .filter(|f: &Vec<f32>| f.len() >= crate::dsp::bands::NUM_BANDS)
                .collect();
        assert!(frames.len() > 120, "fixture too short: {}", frames.len());

        const OLD_SPAN: f32 = 0.52;
        let (w, h) = (190i32, 60i32);
        let mut shots = Vec::new();
        for sens in [RESP_SPAN / OLD_SPAN, 1.0] {
            let mut t = builtin::all()
                .into_iter()
                .find(|t| t.family == "patchbay")
                .expect("no patchbay colourway");
            t.sensitivity = sens;
            let mut pb = Patchbay::default();
            let mut c = Canvas::new(w, h);
            // 120 frames of the real capture, so the cable slew has settled on real content rather
            // than on a synthetic step.
            for f in frames.iter().take(120) {
                let mut d = FrameData::default();
                d.levels.copy_from_slice(&f[..crate::dsp::bands::NUM_BANDS]);
                d.peaks = d.levels;
                d.dt_ms = NOMINAL_DT_MS;
                pb.draw(&mut c, &t, &d);
            }
            shots.push(c);
        }

        let (ow, oh) = (w, h * 2 + 4);
        let mut out = vec![22u8; (ow * oh * 4) as usize];
        for (ri, shot) in shots.iter().enumerate() {
            for y in 0..h {
                for x in 0..w {
                    let px = shot.get(x, y);
                    let a = px.a as f32 / 255.0;
                    let o = (((ri as i32 * (h + 4) + y) * ow + x) * 4) as usize;
                    for (k, ch) in [px.r, px.g, px.b].iter().enumerate() {
                        out[o + k] = (*ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8;
                    }
                    out[o + 3] = 255;
                }
            }
        }
        let path = dir.join(format!("patchbay-window-{ow}x{oh}.rgba"));
        std::fs::write(&path, &out).unwrap();
        println!(
            "wrote {} ({ow}x{oh}) - top row: the OLD window (span {OLD_SPAN}), bottom row: the new one (span {})",
            path.display(),
            RESP_SPAN
        );
    }

    /// Run: cargo test --release probe_patchbay_spread -- --ignored --nocapture
    ///
    /// Quantifies the defect the README records: with only 5 cables at 190px each one folds 12.8 of the
    /// 64 bands, so on real music every cable ends up reporting much the same thing and the sag - the
    /// family's one cue - flattens.
    ///
    /// Measured on the three real-music captures rather than on synthetic spectra, because that is the
    /// claim: a comb fixture would separate the cables perfectly and say nothing about music.
    #[test]
    #[ignore]
    fn probe_patchbay_spread() {
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
        println!("cables at 190px: {} | at 380px: {}", cable_count(190), cable_count(380));
        for (name, frames) in &fixtures {
            for w in [190i32, 380] {
                let cables = cable_count(w);
                // Per frame: the spread across cables of the DISPLAYED response, which is what the sag
                // is a linear function of. Averaged over the capture, plus the share of frames where
                // every cable sits within a tenth of the others - a frame in which the panel reads as
                // one object rather than as five independent cues.
                let (mut spread_acc, mut flat_frames, mut n) = (0.0f64, 0usize, 0usize);
                let mut per_cable = vec![0.0f64; cables];
                // Two sharper questions than "is it flat": how often is a cable PINNED at the top of
                // its travel, where it carries no information at all; and how far apart are the
                // cables that are not the bass one, which is where the means bunch up.
                let mut pinned = vec![0usize; cables];
                let mut mid_spread = 0.0f64;
                for f in frames {
                    if f.len() < crate::dsp::bands::NUM_BANDS {
                        continue;
                    }
                    let mut d = FrameData::default();
                    d.levels.copy_from_slice(&f[..crate::dsp::bands::NUM_BANDS]);
                    d.peaks = d.levels;
                    let resp: Vec<f32> = (0..cables)
                        .map(|i| Patchbay::response(Patchbay::level_for(&d, i, cables), 1.0))
                        .collect();
                    let (lo, hi) = resp.iter().fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
                    spread_acc += (hi - lo) as f64;
                    if hi - lo < 0.10 {
                        flat_frames += 1;
                    }
                    for (c, v) in per_cable.iter_mut().zip(&resp) {
                        *c += *v as f64;
                    }
                    for (k, v) in resp.iter().enumerate() {
                        if *v >= 0.99 {
                            pinned[k] += 1;
                        }
                    }
                    // Mean pairwise separation among every cable except the lowest, whose saturation
                    // would otherwise dominate the figure.
                    if cables > 2 {
                        let rest = &resp[1..];
                        let (mut acc, mut pairs) = (0.0f64, 0usize);
                        for a in 0..rest.len() {
                            for b in (a + 1)..rest.len() {
                                acc += (rest[a] - rest[b]).abs() as f64;
                                pairs += 1;
                            }
                        }
                        mid_spread += acc / pairs.max(1) as f64;
                    }
                    n += 1;
                }
                if n == 0 {
                    continue;
                }
                // The GROUP levels the family actually feeds `response`, which is the quantity the
                // window should have been placed on. Percentiles, per cable.
                let mut lv: Vec<Vec<f32>> = vec![Vec::new(); cables];
                for f in frames {
                    if f.len() < crate::dsp::bands::NUM_BANDS {
                        continue;
                    }
                    let mut d = FrameData::default();
                    d.levels.copy_from_slice(&f[..crate::dsp::bands::NUM_BANDS]);
                    d.peaks = d.levels;
                    for (i, out) in lv.iter_mut().enumerate() {
                        out.push(Patchbay::level_for(&d, i, cables));
                    }
                }
                let pct = |v: &mut Vec<f32>, q: f64| -> f32 {
                    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    v[((v.len() - 1) as f64 * q) as usize]
                };
                let p50: Vec<String> = lv.iter_mut().map(|v| format!("{:.2}", pct(v, 0.50))).collect();
                let p99: Vec<String> = lv.iter_mut().map(|v| format!("{:.2}", pct(v, 0.99))).collect();
                println!("{:<18}group p50 [{}]
{:<18}group p99 [{}]", "", p50.join(" "), "", p99.join(" "));
                let means: Vec<String> = per_cable.iter().map(|c| format!("{:.2}", c / n as f64)).collect();
                let pins: Vec<String> = pinned.iter().map(|c| format!("{:.0}%", 100.0 * *c as f64 / n as f64)).collect();
                println!(
                    "  {name:<14} w={w} n={cables}  spread {:.3}  flat {:>4.1}%  mid-sep {:.3}
                     {:<18}means [{}]
{:<18}pinned at 1.0 [{}]",
                    spread_acc / n as f64,
                    100.0 * flat_frames as f64 / n as f64,
                    mid_spread / n as f64,
                    "",
                    means.join(" "),
                    "",
                    pins.join(" ")
                );
            }
        }
    }

    #[test]
    fn the_flourish_repatches_every_cable_to_the_other_jack_of_its_pair() {
        let jacks = cable_count(190) * 2;
        let (x0, x1) = (jack_x(190, jacks, 0), jack_x(190, jacks, 1));
        assert!(x1 - x0 >= 8, "the pair is too close together to measure: {x0} to {x1}");

        // 78 frames is 1.3s, the whole envelope.
        let (steady, _, steady_peak) = repatch_trace(false, 78);
        let (moved, _, peak_frame) = repatch_trace(true, 78);

        // The fixture has to hold still, or "it moved" means nothing. The sag follows the music, but
        // the ENDPOINTS never move without the flourish.
        assert!(
            steady.iter().all(|x| (x - steady[0]).abs() <= 1),
            "the cable wandered without the flourish: {steady:?}"
        );

        // It travels most of the way across its pair. Measured against the pair's own width rather
        // than against an absolute jack column, because the sample row is 5px down the curve and so is
        // already a little way along it - an absolute test there would be measuring the sag as well.
        let travel = (x1 - x0) as f64;
        let far = *moved.iter().max().unwrap();
        assert!(
            (far - steady[0]) as f64 >= travel * 0.7,
            "the cable barely moved: {}px of a {travel}px pair",
            far - steady[0]
        );

        // And it SLID rather than jumping. A hard swap reads as a dropped frame at 60fps, so the
        // intermediate positions are the effect and not a detail of it.
        let mid = moved
            .iter()
            .filter(|x| {
                let f = (**x - steady[0]) as f64 / travel;
                f > 0.2 && f < 0.7
            })
            .count();
        assert!(
            mid >= 8,
            "the cable teleported: only {mid} of {} frames were partway across",
            moved.len()
        );

        // The plug really does end up in the socket that is normally empty - asserted on that
        // socket's own pixels, not inferred from the column measurement above.
        let (lit_now, lit_before) = (socket_light(&peak_frame, 1), socket_light(&steady_peak, 1));
        assert!(
            lit_now > lit_before * 1.25,
            "jack 1 never got a plug in it: {lit_now:.0} against {lit_before:.0} unpatched"
        );
    }

    /// Run: cargo test --release dump_patchbay_repatch -- --ignored --nocapture
    ///
    /// Five frames through the swing - at rest, halfway, fully crossed, halfway back, at rest - because
    /// the effect is the TRAVEL and a single frame of it only shows one lean. Written as one stacked
    /// image so the chevron can be compared row to row.
    #[test]
    #[ignore]
    fn dump_patchbay_repatch() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        // 1300ms at 16.7ms is 78 frames; the swing peaks at half the envelope.
        let picks = [0usize, 19, 39, 58, 77];
        let mut t = builtin::all()
            .into_iter()
            .find(|t| t.family == "patchbay")
            .expect("no patchbay colourway");
        t.flourish = 0.0;
        let mut p = Patchbay::default();
        let mut c = Canvas::new(190, 60);
        let d = flat(0.35);
        for _ in 0..20 {
            p.draw(&mut c, &t, &d);
        }
        p.flourish.force_next();
        let mut shots = Vec::new();
        for f in 0..=*picks.last().unwrap() {
            p.draw(&mut c, &t, &d);
            if picks.contains(&f) {
                shots.push(c.clone());
            }
        }
        let (ow, oh) = (190, 60 * shots.len() as i32 + 4 * (shots.len() as i32 - 1));
        let mut out = vec![22u8; (ow * oh * 4) as usize];
        for (ri, shot) in shots.iter().enumerate() {
            for y in 0..60 {
                for x in 0..190 {
                    let px = shot.get(x, y);
                    let a = px.a as f32 / 255.0;
                    let oy = ri as i32 * 64 + y;
                    let o = ((oy * ow + x) * 4) as usize;
                    for (k, ch) in [px.r, px.g, px.b].iter().enumerate() {
                        out[o + k] = (*ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8;
                    }
                    out[o + 3] = 255;
                }
            }
        }
        let path = dir.join(format!("patchbay-repatch-{ow}x{oh}.rgba"));
        std::fs::write(&path, &out).unwrap();
        println!("wrote {} ({ow}x{oh}) - rows are frames {picks:?} of the swing", path.display());
    }

    #[test]
    fn the_cables_come_back_to_their_own_sockets() {
        // The envelope is used as a PHASE, not an amplitude - `sin(pi * (1 - level))` returns to zero
        // as the level expires. If it were used as an amplitude the cable would fade out stranded
        // halfway between two sockets, which is why this is asserted on the canvas and not just on the
        // measured x: byte-identical also proves no residue is left anywhere else.
        let (_, moved, _) = repatch_trace(true, 90);
        let (_, steady, _) = repatch_trace(false, 90);
        assert_eq!(
            moved.bits(),
            steady.bits(),
            "the panel did not return to its own patching"
        );
    }

    use crate::themes::builtin;

    fn flat(level: f32) -> FrameData {
        let mut d = FrameData::default();
        for v in d.levels.iter_mut() {
            *v = level;
        }
        d.peaks = d.levels;
        d
    }

    /// One band peaking inside an otherwise quiet group.
    ///
    /// The case a flat spectrum cannot see at all: with every band equal a group's mean IS its
    /// max, so `GROUP_MAX_BIAS` is a mathematical no-op and no flat-spectrum test can tell a
    /// working per-band reducer from a broken one. This project has shipped three tests that
    /// passed against a no-op; that is the shape of the mistake.
    fn one_loud_band(cables: usize, loud_cable: usize, loud: f32, quiet: f32) -> FrameData {
        let mut d = FrameData::default();
        let n = d.levels.len();
        for v in d.levels.iter_mut() {
            *v = quiet;
        }
        let lo = loud_cable * n / cables;
        let hi = ((loud_cable + 1) * n / cables).min(n);
        d.levels[(lo + hi) / 2] = loud;
        d.peaks = d.levels;
        d
    }

    /// Copy of `d` with its lowest four bands stepped up - a kick landing on one frame.
    ///
    /// A free function rather than a `Clone` impl on `FrameData`: nothing in production needs
    /// to copy a frame (it carries a 256-entry waveform), and adding the impl from a family
    /// file would put a shared type's API in a place nobody would look for it.
    fn with_kick(d: &FrameData, v: f32) -> FrameData {
        let mut out = FrameData { levels: d.levels, peaks: d.peaks, ..FrameData::default() };
        for x in out.levels[..4].iter_mut() {
            *x = v;
        }
        out.peaks = out.levels;
        out
    }

    fn lum(p: Rgba) -> f64 {
        0.2126 * p.r as f64 + 0.7152 * p.g as f64 + 0.0722 * p.b as f64
    }

    /// Renders `frames` frames of one spectrum, so the cable slew has settled.
    fn render(t: &Theme, d: &FrameData, w: i32, h: i32, frames: usize) -> Canvas {
        let mut fam = Patchbay::default();
        let mut c = Canvas::new(w, h);
        for _ in 0..frames {
            fam.draw(&mut c, t, d);
        }
        c
    }

    /// Column midway between cable `i`'s two jacks - i.e. where its sag is largest.
    fn cable_mid_x(w: i32, cables: usize, i: usize) -> i32 {
        let jacks = cables * 2;
        let (a, b) = cable_ends(i);
        (jack_x(w, jacks, a) + jack_x(w, jacks, b)) / 2
    }

    /// Where the cable sits in a given column: the centre of the band of rows whose luminance
    /// deviates from the bare metal, weighted by how far each deviates.
    ///
    /// Two earlier versions were each wrong in an instructive way.
    ///
    /// Brightest row: worked on four colourways and silently reported the PANEL on the cream one,
    /// where the cables are darker than the panel they hang on - measured, it returned row 15 at
    /// a luminance of 194.0 against the metal's 191.1, identically at every level. A locator that
    /// quietly measures the background is how a test comes to pass against nothing.
    ///
    /// Most-deviating row: polarity-agnostic, but it picked an arbitrary member of the band
    /// rather than its middle. A cable is locally HORIZONTAL at the apex of its own sag, which is
    /// exactly the column being sampled, so the cable covers 8-10 rows of it (measured: rows
    /// 20-29 on the cream panel) and which of them peaks depends on the colour, not on the
    /// geometry. That cost 3px of the 13px travel and failed the assertion on one colourway.
    ///
    /// The window excludes the rows the sockets and screws occupy, and the brushed grain deviates
    /// by only about 4 luminance, so the half-peak cut-off keeps the panel out of the centroid.
    fn cable_row(c: &Canvas, x: i32, jy_top: i32, jy_bot: i32) -> i32 {
        let metal = lum(c.get(x, jy_top + 3));
        let rows: Vec<(i32, f64)> = ((jy_top + 4)..(jy_bot - 3))
            .map(|y| (y, (lum(c.get(x, y)) - metal).abs()))
            .collect();
        let peak = rows.iter().map(|r| r.1).fold(0.0f64, f64::max);
        if peak <= 0.0 {
            return jy_top;
        }
        let (mut wsum, mut w) = (0.0f64, 0.0f64);
        for &(y, dev) in rows.iter().filter(|r| r.1 > peak * 0.5) {
            wsum += y as f64 * dev;
            w += dev;
        }
        (wsum / w).round() as i32
    }

    #[test]
    fn cable_count_holds_at_five_when_narrow_and_grows_with_width() {
        assert_eq!(cable_count(190), 5, "the reference panel must keep the five tuned cables");
        assert_eq!(cable_count(380), 10, "double the width doubles the cables, not their span");
        assert!(cable_count(4000) <= 16, "capped");
        assert!(cable_count(44) >= 2, "and never fewer than a patched pair");
    }

    #[test]
    fn a_wide_panel_keeps_the_jacks_the_size_they_were_tuned_at() {
        // A fixed count stretches: at 380px five cables would sit 68px apart with a droop half
        // the panel wide. Jack pitch is what must stay put.
        let reference = jack_pitch(190, cable_count(190) * 2);
        for w in [190, 240, 380, 456, 600] {
            let p = jack_pitch(w, cable_count(w) * 2);
            assert!(
                (p - reference).abs() < 3.0,
                "at width {w} the jack pitch drifted to {p:.1} from the tuned {reference:.1}"
            );
        }
    }

    #[test]
    fn the_response_window_spends_its_range_on_levels_the_dsp_actually_produces() {
        // This test USED TO ASSERT ON BAND LEVELS and that is why it missed a real defect for so long.
        // It checked that 0.15-0.65 per band covered most of the travel - true, and irrelevant, because
        // `response` is never handed a band level. It is handed `level_for`, a group reduction biased
        // toward the PEAK of ~12.8 bands, which routinely runs half again as high. The window was
        // therefore placed on a quantity this family does not produce, and the lowest cable sat pinned
        // at full deflection on most frames of real music.
        //
        // So it now asserts on the real thing: the group levels, over the real-music captures, and the
        // property that actually matters - no cable spends meaningful time stuck at either end, where
        // it carries no information. `probe_patchbay_spread` prints the whole table.
        let parse = |csv: &str| -> Vec<Vec<f32>> {
            csv.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.split(',').filter_map(|v| v.parse::<f32>().ok()).collect())
                .collect()
        };
        let fixtures = [
            ("steady groove", parse(include_str!("../../tests/fixtures/real-music-bands.csv"))),
            ("dnb, dynamic", parse(include_str!("../../tests/fixtures/real-music-flat.csv"))),
            ("flat-mastered", parse(include_str!("../../tests/fixtures/real-music-dynamic.csv"))),
        ];
        for (name, frames) in &fixtures {
            for w in [190i32, 380] {
                let cables = cable_count(w);
                let mut pinned = vec![0usize; cables];
                let mut n = 0usize;
                for f in frames {
                    if f.len() < crate::dsp::bands::NUM_BANDS {
                        continue;
                    }
                    let mut d = FrameData::default();
                    d.levels.copy_from_slice(&f[..crate::dsp::bands::NUM_BANDS]);
                    d.peaks = d.levels;
                    for (i, p) in pinned.iter_mut().enumerate() {
                        if Patchbay::response(Patchbay::level_for(&d, i, cables), 1.0) >= 0.99 {
                            *p += 1;
                        }
                    }
                    n += 1;
                }
                assert!(n > 100, "{name}: fixture too short to measure ({n} frames)");
                for (i, count) in pinned.iter().enumerate() {
                    let share = 100.0 * *count as f64 / n as f64;
                    assert!(
                        share < 8.0,
                        "{name} at w={w}: cable {i} is pinned at full deflection on {share:.0}% of \
                         frames, so its sag carries no information there. The window is placed on the \
                         wrong quantity - see RESP_FLOOR"
                    );
                }
            }
        }

        // The window still has to SPEND its range, or the fix for pinning would just be a dead panel.
        // Group levels sit around 0.22-0.69 on these captures, so that band must cover most of the
        // travel.
        let (lo, hi) = (Patchbay::response(0.22, 1.0), Patchbay::response(0.69, 1.0));
        assert!(hi - lo > 0.60, "the music window must cover most of the range: {lo} -> {hi}");
        assert_eq!(Patchbay::response(0.0, 1.0), 0.0, "silence maps to zero, not a pedestal");
        assert_eq!(Patchbay::response(1.0, 1.0), 1.0, "full scale reaches the top");
        assert!(
            Patchbay::response(0.3, 2.0) > Patchbay::response(0.3, 1.0),
            "sensitivity is the user-facing knob and must actually do something"
        );
    }

    #[test]
    fn the_group_reducer_is_biased_toward_the_peak_not_the_mean() {
        // Guards GROUP_MAX_BIAS directly, on the one input shape that can see it.
        let d = one_loud_band(5, 0, 0.9, 0.1);
        let n = d.levels.len();
        let hi = (n / 5).max(1);
        let mean = d.levels[..hi].iter().sum::<f32>() / hi as f32;
        let peak = d.levels[..hi].iter().copied().fold(0.0f32, f32::max);
        let got = Patchbay::level_for(&d, 0, 5);
        assert!(got > mean + (peak - mean) * 0.5, "must sit above the midpoint: {got} in [{mean}, {peak}]");
        assert!(got <= peak + 1e-6, "but never above the peak itself: {got} vs {peak}");
    }

    #[test]
    fn the_control_point_puts_the_curve_exactly_where_the_sag_says() {
        // A quadratic only travels HALF way to its control point, so `2 * sag` is load-bearing.
        // Get it wrong and the cable still curves - just by half of what every constant and
        // every measurement in this file claims, silently.
        let mut path = Vec::new();
        for sag in [-6.0f32, 0.0, 9.0] {
            cable_path((10.0, 8.0), (40.0, 50.0), sag, &mut path);
            let want_y = (8.0 + 50.0) / 2.0 + sag;
            let at_mid = path
                .iter()
                .copied()
                .min_by_key(|(x, _)| (x - 25).abs())
                .expect("the walk must produce pixels");
            assert!(
                (at_mid.1 as f32 - want_y).abs() <= 1.5,
                "sag {sag}: midpoint landed at y {} but should be {want_y}",
                at_mid.1
            );
        }
    }

    #[test]
    fn the_walk_never_returns_the_same_pixel_twice() {
        // The 1px core is drawn with a translucent colour, so a repeated pixel is a visibly
        // brighter bead - and at 1.5 samples per pixel most samples would repeat.
        let mut path = Vec::new();
        for sag in [-8.0f32, 0.0, 4.0, 14.0] {
            cable_path((15.0, 8.0), (33.0, 51.0), sag, &mut path);
            let mut sorted = path.clone();
            sorted.sort_unstable();
            let before = sorted.len();
            sorted.dedup();
            assert_eq!(sorted.len(), before, "sag {sag} revisited a pixel");
            assert!(before > 30, "sag {sag} produced only {before} pixels - the stroke would be dotted");
        }
    }

    #[test]
    fn a_driven_cable_pulls_taut_and_lifts_its_midpoint() {
        // THE cue. Brightness alone measured a 1.16x spread on the valve row, below the visible
        // threshold; this is the mechanism that replaces it, so it is measured in pixels of
        // travel and not in luminance.
        let (jy_top, jy_bot) = jack_rows(60);
        let cables = cable_count(190);
        let x = cable_mid_x(190, cables, 0);
        // Every colourway, because the sag is geometry and must not depend on the palette -
        // including the cream one, whose cables are darker than the panel they hang on.
        for t in builtin::all().into_iter().filter(|t| t.family == "patchbay") {
            let idle = render(&t, &flat(0.10), 190, 60, 30);
            let driven = render(&t, &flat(0.95), 190, 60, 30);
            let y_idle = cable_row(&idle, x, jy_top, jy_bot);
            let y_driven = cable_row(&driven, x, jy_top, jy_bot);
            assert!(
                y_idle - y_driven >= 10,
                "{}: driving the band must haul the cable taut: midpoint row {y_idle} at idle \
                 vs {y_driven} driven (13px measured on the reference, want at least 10)",
                t.id
            );
        }
    }

    #[test]
    fn one_peaking_band_lifts_and_lights_only_its_own_cable() {
        // The per-band guard, on an UNEVEN spectrum. A flat spectrum cannot distinguish a
        // working per-cable mapping from one that feeds every cable the same number.
        let t = builtin::patch_classic();
        let cables = cable_count(190);
        let (jy_top, jy_bot) = jack_rows(60);
        let d = one_loud_band(cables, 1, 0.75, 0.16);
        let c = render(&t, &d, 190, 60, 30);

        let driven_x = cable_mid_x(190, cables, 1);
        let quiet_x = cable_mid_x(190, cables, 3);
        let y_driven = cable_row(&c, driven_x, jy_top, jy_bot);
        let y_quiet = cable_row(&c, quiet_x, jy_top, jy_bot);
        assert!(
            y_quiet - y_driven >= 6,
            "the cable whose group holds the peak must sit clearly higher: driven row \
             {y_driven} vs quiet row {y_quiet}"
        );

        // And it must be brighter too - the confirming cue, measured over each cable's own
        // horizontal share of the panel so the two regions cannot overlap.
        let band = |cx: i32| -> f64 {
            let mut s = 0.0;
            for y in (jy_top + 2)..(jy_bot - 1) {
                for x in (cx - 8)..(cx + 9) {
                    s += lum(c.get(x, y));
                }
            }
            s
        };
        let (hot, cold) = (band(driven_x), band(quiet_x));
        assert!(
            hot > cold * 1.15,
            "the driven cable must also out-glow the quiet one: {hot:.0} vs {cold:.0}"
        );
    }

    #[test]
    fn every_cable_is_still_visible_at_silence() {
        // A patch cable is a physical object: it is there whether or not signal flows through
        // it. A panel whose cables vanished at silence looked UNPATCHED, which is the same
        // mistake the valve row's heater floor exists to avoid.
        //
        // Measured against the bare metal in the cable's OWN column, a few rows above where the
        // droop reaches, so this cannot pass on the brushed gradient or on a neighbouring
        // cable - it is a local contrast check, which is what "visible" actually means here.
        let (jy_top, jy_bot) = jack_rows(60);
        let cables = cable_count(190);
        for t in builtin::all().into_iter().filter(|t| t.family == "patchbay") {
            let c = render(&t, &flat(0.0), 190, 60, 8);
            for i in 0..cables {
                let x = cable_mid_x(190, cables, i);
                let y = cable_row(&c, x, jy_top, jy_bot);
                let cable = lum(c.get(x, y));
                let metal = lum(c.get(x, jy_top + 3));
                // Measured at silence: 79.7 of separation on the classic panel, 49.7 on the
                // greyest one, and 166 on the cream one where the sign is the other way round.
                // Hence a signed-agnostic floor of 25 rather than a brightness ratio.
                assert!(
                    (cable - metal).abs() > 25.0,
                    "{}: cable {i} is invisible at silence: {cable:.1} against bare metal \
                     {metal:.1}",
                    t.id
                );
            }
        }
    }

    #[test]
    fn the_audio_actually_changes_the_pixels() {
        let t = builtin::patch_classic();
        let quiet = render(&t, &flat(0.12), 190, 60, 30);
        let loud = render(&t, &flat(0.90), 190, 60, 30);
        assert_ne!(quiet.bits(), loud.bits(), "the level must change the render");
        let count = |c: &Canvas, min: f64| -> usize {
            (0..60)
                .flat_map(|y| (0..190).map(move |x| (x, y)))
                .filter(|&(x, y)| lum(c.get(x, y)) > min)
                .count()
        };
        assert!(
            count(&loud, 120.0) > count(&quiet, 120.0) + 40,
            "a loud frame must light substantially more of the panel: {} vs {}",
            count(&loud, 120.0),
            count(&quiet, 120.0)
        );
    }

    #[test]
    fn the_indicators_blink_on_a_bass_rise_and_not_on_a_held_level() {
        // A level-triggered indicator reads as a stuck lamp. This is the test that
        // distinguishes the two: a HELD loud bass must leave the LEDs dark, and only the frame
        // the bass steps up may light them.
        let t = builtin::patch_classic();
        let mut fam = Patchbay::default();
        let mut c = Canvas::new(190, 60);
        let loud = flat(0.60);

        // 40 frames of sustained loud bass - one rise at the very start, then nothing.
        for _ in 0..40 {
            fam.draw(&mut c, &t, &loud);
        }
        assert!(
            fam.led <= 0.001 && fam.led_tail <= 0.001,
            "a sustained level must NOT hold the indicators on: led {} tail {}",
            fam.led,
            fam.led_tail
        );
        let dark = lum(c.get(5, 30));

        // Now a genuine step up in the bass bands only.
        let kick = with_kick(&loud, 0.95);
        fam.draw(&mut c, &t, &kick);
        assert!(fam.led > 0.8, "a bass rise must fire the indicator, got {}", fam.led);
        let bright = lum(c.get(5, 30));
        assert!(
            bright > dark + 25.0,
            "and it must be visible on the panel: {bright:.1} lit vs {dark:.1} dark"
        );

        // ...and it must go out again on its own, or it is a level trigger by another name.
        for _ in 0..20 {
            fam.draw(&mut c, &t, &kick);
        }
        assert!(fam.led <= 0.001, "the blink must decay while the level stays up, got {}", fam.led);
    }

    #[test]
    fn renders_at_every_plausible_size_without_panicking() {
        let t = builtin::patch_uv();
        let mut d = flat(0.0);
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = 0.15 + 0.5 * ((i % 7) as f32 / 6.0);
        }
        d.peaks = d.levels;
        for (w, h) in [(190, 60), (380, 60), (456, 60), (150, 48), (240, 72), (96, 40), (44, 30), (40, 24), (12, 12), (1, 1)] {
            let c = render(&t, &d, w, h, 3);
            assert_eq!(c.bits().len(), (w.max(0) * h.max(0)) as usize, "{w}x{h} changed the canvas size");
        }
    }

    #[test]
    fn survives_nan_and_infinity_without_poisoning_the_slew() {
        // The strong form: a poisoned frame must not leave the persistent state unable to
        // respond afterwards. `f32::clamp` returns NaN for NaN, so a single bad frame reaching
        // `disp` would stick there for the life of the process - this has bitten the project
        // twice.
        let t = builtin::patch_noir();
        let mut fam = Patchbay::default();
        let mut c = Canvas::new(190, 60);

        let mut bad = flat(0.5);
        bad.levels[0] = f32::NAN;
        bad.levels[7] = f32::INFINITY;
        bad.levels[40] = f32::NEG_INFINITY;
        bad.peaks[3] = f32::NAN;
        bad.dt_ms = f32::NAN;
        for _ in 0..6 {
            fam.draw(&mut c, &t, &bad);
        }
        assert!(fam.disp.iter().all(|v| v.is_finite()), "NaN reached the slew: {:?}", fam.disp);
        assert!(fam.prev_bass.is_finite() && fam.led.is_finite());

        // And it must still respond to clean audio afterwards.
        let (jy_top, jy_bot) = jack_rows(60);
        let x = cable_mid_x(190, cable_count(190), 0);
        for _ in 0..30 {
            fam.draw(&mut c, &t, &flat(0.05));
        }
        let low = cable_row(&c, x, jy_top, jy_bot);
        for _ in 0..30 {
            fam.draw(&mut c, &t, &flat(0.95));
        }
        let high = cable_row(&c, x, jy_top, jy_bot);
        assert!(low - high >= 8, "after a poisoned frame the sag must still move: {low} -> {high}");

        // Degenerate canvases with a poisoned spectrum, for the same reason.
        for (w, h) in [(1, 1), (12, 12), (190, 12), (60, 60)] {
            let mut tiny = Canvas::new(w, h);
            fam.draw(&mut tiny, &t, &bad);
        }
    }

    #[test]
    fn nothing_is_drawn_outside_the_panel_rect() {
        // The bloom spreads up to `t.bloom` px in every direction, far more than the panel's
        // 1-2px inset, so without the clip the halo of the outer cables and the right-hand LED
        // leaks onto the bare taskbar as a bright edge around the widget.
        let t = builtin::patch_uv();
        let c = render(&t, &flat(0.95), 190, 60, 30);
        for x in 0..190 {
            assert_eq!(c.get(x, 0), Rgba::TRANSPARENT, "row 0 is above the panel, x={x}");
            assert_eq!(c.get(x, 1), Rgba::TRANSPARENT, "row 1 is above the panel, x={x}");
            assert_eq!(c.get(x, 58), Rgba::TRANSPARENT, "row 58 is below the panel, x={x}");
            assert_eq!(c.get(x, 59), Rgba::TRANSPARENT, "row 59 is below the panel, x={x}");
        }
        for y in 0..60 {
            assert_eq!(c.get(0, y), Rgba::TRANSPARENT, "column 0 is left of the panel, y={y}");
            assert_eq!(c.get(189, y), Rgba::TRANSPARENT, "column 189 is right of the panel, y={y}");
        }
    }

    #[test]
    fn every_patchbay_colourway_renders_and_differs() {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        let d = flat(0.55);
        for t in builtin::all().into_iter().filter(|t| t.family == "patchbay") {
            let c = render(&t, &d, 190, 60, 8);
            let bits = c.bits().to_vec();
            assert!(bits.iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for prior in &seen {
                assert_ne!(prior, &bits, "{} renders identically to another colourway", t.id);
            }
            seen.push(bits);
        }
        assert_eq!(seen.len(), 5, "expected five patchbay colourways, got {}", seen.len());
    }

    #[test]
    fn the_multi_colour_colourways_really_do_give_each_cable_its_own_colour() {
        // The rainbow and primary-cable looks ride entirely on `zones` + `lit_at`, so a
        // regression there would silently make every cable the same colour - which still
        // renders, still responds, and still passes every other test in this file.
        for t in [builtin::patch_rainbow(), builtin::patch_classic()] {
            let cables = cable_count(190);
            let hues: Vec<&str> = (0..cables)
                .map(|i| t.lit_at((i as f32 + 0.5) / cables as f32))
                .collect();
            let mut uniq = hues.clone();
            uniq.sort_unstable();
            uniq.dedup();
            assert!(
                uniq.len() >= 3,
                "{} should give the cables at least three distinct colours, got {hues:?}",
                t.id
            );
        }
    }

    /// Measurement, not an assertion. Prints the idle and driven cable pixels against the bare
    /// brushed metal beside them, which is the number that decides whether an idle cable is
    /// visible at all. Run: cargo test --release probe_patchbay_contrast -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_patchbay_contrast() {
        let (jy_top, jy_bot) = jack_rows(60);
        let cables = cable_count(190);
        for t in builtin::all().into_iter().filter(|t| t.family == "patchbay") {
            let x = cable_mid_x(190, cables, 0);
            let mut line = format!("{:<15}", t.id);
            for (label, level) in [("idle", 0.05f32), ("driven", 0.95)] {
                let c = render(&t, &flat(level), 190, 60, 30);
                let y = cable_row(&c, x, jy_top, jy_bot);
                let core = lum(c.get(x, y));
                let sheath = lum(c.get(x - 1, y));
                let metal = lum(c.get(x, jy_top + 3));
                line += &format!(
                    "  {label}: row {y:2} core {core:5.1} sheath {sheath:5.1} metal {metal:5.1}"
                );
            }
            println!("{line}");
        }
    }

    /// Run: cargo test --release dump_patchbay_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_patchbay_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();

        // An uneven spectrum, so the cables sit at visibly different sags. A flat one would
        // show five identical droops and tell the reviewer nothing.
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            let x = i as f32 / 63.0;
            *v = (0.15 + 0.85 * (x * 9.0).sin().abs()) * (1.0 - x * 0.45);
        }
        d.peaks = d.levels;
        // Bass stepped up on the last frame, so the indicators are caught mid-blink.
        let kick = with_kick(&d, 0.98);

        let write = |name: &str, c: &Canvas| {
            let (w, h) = (c.width(), c.height());
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    // Composited over background grey 22, exactly as the other dumps do, so
                    // the panel is judged against the taskbar it actually sits on.
                    for ch in [px.r, px.g, px.b] {
                        out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("patchbay-{name}.rgba")), &out).unwrap();
        };

        let mut n = 0usize;
        for t in builtin::all().into_iter().filter(|t| t.family == "patchbay") {
            let mut fam = Patchbay::default();
            let mut c = Canvas::new(190, 60);
            for _ in 0..30 {
                fam.draw(&mut c, &t, &d);
            }
            fam.draw(&mut c, &t, &kick);
            write(&t.id, &c);
            n += 1;
        }

        // The wide mode, and a staircase - one cable per level - so the sag scale can be read
        // off a single image.
        let t = builtin::patch_classic();
        let mut wide = Canvas::new(380, 60);
        let mut fam = Patchbay::default();
        for _ in 0..30 {
            fam.draw(&mut wide, &t, &d);
        }
        fam.draw(&mut wide, &t, &kick);
        write("patch-classic-wide", &wide);
        n += 1;

        let cables = cable_count(190);
        let mut ladder = FrameData::default();
        let nb = ladder.levels.len();
        for (i, v) in ladder.levels.iter_mut().enumerate() {
            let cable = i * cables / nb;
            *v = 0.12 + 0.52 * cable as f32 / (cables - 1).max(1) as f32;
        }
        ladder.peaks = ladder.levels;
        let mut c = Canvas::new(190, 60);
        let mut fam = Patchbay::default();
        for _ in 0..30 {
            fam.draw(&mut c, &t, &ladder);
        }
        write("patch-classic-ladder", &c);
        n += 1;

        // The position cue, in the units it has to be judged in. Printed rather than only
        // asserted, because the number is what tells the reviewer whether the move is big enough
        // to see before they have even opened the images.
        let (jy_top, jy_bot) = jack_rows(60);
        let cables = cable_count(190);
        let x = cable_mid_x(190, cables, 0);
        let quiet = render(&t, &flat(0.10), 190, 60, 30);
        let loud = render(&t, &flat(0.95), 190, 60, 30);
        println!(
            "cable 0 midpoint row: {} at idle -> {} driven ({} px of lift, jack rows {} and {})",
            cable_row(&quiet, x, jy_top, jy_bot),
            cable_row(&loud, x, jy_top, jy_bot),
            cable_row(&quiet, x, jy_top, jy_bot) - cable_row(&loud, x, jy_top, jy_bot),
            jy_top,
            jy_bot
        );
        println!("wrote {} patchbay dumps to {}", n, dir.display());
    }
}
