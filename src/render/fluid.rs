//! The fluid-tank family: two submerged subwoofers pumping a shallow tank of liquid, seen
//! side-on.
//!
//! Unlike the instrument families this is a scene, and unlike the other scene (`vapor`) its
//! state is a PHYSICAL SIMULATION rather than a scroll phase plus a history ring. One float per
//! pixel column holds the surface height; the two cones force the columns directly above them,
//! and the discrete wave equation carries what they push outward, off the tank walls, and into
//! the middle where the two wavetrains INTERFERE. That interference is the whole point of the
//! family - it is the one thing here that cannot be faked with a per-column response curve,
//! because a column's height depends on what its neighbours did several frames ago.
//!
//! Four things carry the look, and each is a deliberate choice rather than an accident:
//!
//! - **The cones MOVE.** At 190x60 a 5px vertical translation is far easier to read than any
//!   brightness change, so the channel's level is spent on cone POSITION first and on the rim
//!   highlight second. A cone that only brightened would not read as a cone at all.
//! - **The liquid body is opaque and shaded by DEPTH, not by the surface.** The colour ramp is
//!   anchored to the tank floor, so a crest riding over a column does not repaint that column a
//!   different colour - only its top edge moves. Anchoring the ramp to the moving surface makes
//!   the whole body shimmer and destroys the sense of a fixed volume of liquid.
//! - **Transients are found with SPECTRAL FLUX**, not with a rise in the bass mean. Measured on
//!   `tests/fixtures/real-music-bands.csv` the largest single-frame rise in the bass mean is
//!   0.140, which is why the vaporwave family's bass-rise trigger could never fire; the same
//!   mistake here would leave the droplets permanently switched off.
//! - **Stereo is real.** `rms_l` drives the left cone and `rms_r` the right, so the tank is
//!   asymmetric whenever the mix is, and the interference pattern in the middle means something.
//!
//! STABILITY. The explicit scheme used here diverges the instant the Courant number `c*dt/dx`
//! passes 1: the field starts to alternate sign every column, doubles every step, reaches
//! infinity within about a second and - because the field is persistent state - every later
//! frame renders garbage forever. The measured frame interval varies with load, so a timestep
//! taken straight from `dt_ms` is exactly the thing that can push it over. The resolution here is
//! that **`dt_ms` never reaches the integrator at all**: the integrator only ever runs a
//! fixed-size sub-step at a fixed Courant number of 0.5 (see `COURANT`), and `dt_ms` only decides
//! HOW MANY of those sub-steps run, bounded to `MAX_SUBSTEPS`. A slow frame therefore advances
//! the water in slow motion; it cannot advance it unstably. On top of that the state is clamped
//! to `H_LIMIT` and de-NaN'd inside the loop, so what the NEXT step reads is bounded by
//! construction rather than by the renderer clamping the output.

use super::canvas::{Canvas, Rgba};
use super::{tint, Family, FrameData};
use crate::themes::Theme;

/// Reference frame interval. The render loop sleeps a fixed 16ms, so its real period is that plus
/// however long the frame took - every rate here is per millisecond and scaled by the measured
/// `dt_ms` against this, never per frame.
const NOMINAL_DT_MS: f32 = 16.7;

/// The Courant number the integrator runs at, and it is a CONSTANT of the scheme rather than
/// something derived from the frame time.
///
/// For the explicit leapfrog form used in `substep` the stability limit is `c*dt/dx <= 1`; above
/// it the field alternates sign column to column and doubles every step. 0.5 leaves a factor of
/// two in hand, which matters because the forcing term is applied to the same buffer immediately
/// before the Laplacian is evaluated - a piston shoving a column while the stencil is at the very
/// edge of stability is precisely how a "stable" scheme still rings up.
///
/// Because this is fixed, the wave speed in PIXELS PER SUB-STEP is fixed too (0.5 px), and the
/// physical wave speed is set by how many sub-steps a second contains - see `SUBSTEP_MS`.
const COURANT: f32 = 0.5;
/// `COURANT` squared, which is the coefficient the Laplacian actually carries.
const C2: f32 = COURANT * COURANT;

/// Simulated interval one sub-step covers. Four sub-steps at the nominal frame, so a wave crosses
/// 0.5px per sub-step x 4 x 60fps = 120 px/s: about 1.6s from one end of the 190px tank to the
/// other, or 0.8s for a wave from a cone to reach the middle. Slower than that and the
/// interference never establishes before the music changes; much faster and the crests blur into
/// a hatch at this pixel pitch.
const SUBSTEP_MS: f32 = NOMINAL_DT_MS / 4.0;

/// Hard cap on sub-steps per frame. 12 is three nominal frames' worth, so the water still keeps up
/// with a stutter up to ~50ms; beyond that it deliberately runs in slow motion rather than trying
/// to catch up, because "catch up" is the failure mode - an unbounded step count is an unbounded
/// amount of forcing applied between two renders, and a long stall would dump it all at once.
const MAX_SUBSTEPS: i32 = 12;

/// Absolute bound on the field, in units where 1.0 is `FluidParams::surface_gain` pixels.
///
/// Applied to what the NEXT sub-step reads, not to the drawn pixels: clamping only the output
/// leaves a diverging field diverging, and the renderer would show a frozen saturated surface
/// forever. 3.0 is above anything the forcing can produce on its own (the target is at most 1.0)
/// with room for two wavetrains constructively interfering and reflecting, so it never truncates a
/// real crest - it only stops a runaway.
const H_LIMIT: f32 = 3.0;

/// Response window for the per-channel RMS, MEASURED rather than assumed.
///
/// Recorded off 8 seconds of real music: `rms_l`/`rms_r` run p50 0.240, p90 0.399, max 0.576. A
/// mapping from 0..1 would spend three quarters of its range on levels this signal never reaches
/// and the cones would barely twitch - the single most common defect in this codebase. Across the
/// window below the response covers 0.36 (at p50) to 0.95 (at p90) of full travel.
///
/// The gamma is below 1 for the same reason the vaporwave terrain's is: it expands the lower half
/// of the distribution, which is where most frames actually live.
const RMS_FLOOR: f32 = 0.14;
const RMS_SPAN: f32 = 0.28;
const RMS_GAMMA: f32 = 0.75;

/// Bass shaping window, also measured, and the reason it is stated separately.
///
/// The fixture's bass MEAN over bands 0..6 spans only 0.320 (p10) to 0.445 (p90) - a mapping that
/// expects bass to swing from 0 to 1 renders dead. But a mean is also the wrong reducer: it
/// flattens exactly the single-band kick that should punch the water. Blending 65% toward the
/// group MAX gives a quantity that spans 0.477 (p10) to 0.608 (p90) on the same fixture, and the
/// window below turns that into 0.14..0.79 of full drive.
const BASS_BANDS: usize = 6;
const BASS_MAX_BIAS: f32 = 0.65;
const BASS_FLOOR: f32 = 0.45;
const BASS_SPAN: f32 = 0.20;

/// How much of the cone drive comes from the LOUDER of the two contributions rather than their
/// weighted mean.
///
/// Rule 7 applied to the two drivers of one element: averaging the RMS response with the bass
/// response pulls both toward the middle. Measured on the fixture, the plain weighted mean gives
/// p10 0.198 / p50 0.502 / p90 0.757; biasing 65% toward the max gives p10 0.253 / p50 0.608 /
/// p90 0.879, i.e. a visibly bigger swing at the top where the transients are.
const DRIVE_MAX_BIAS: f32 = 0.65;
/// Weight on the RMS response inside the mean half of the blend. The per-channel term dominates
/// because it is the only thing that distinguishes the two cones from each other.
const DRIVE_RMS_WEIGHT: f32 = 0.62;

/// Cone envelope follower, per millisecond. Fast up, slow down - a driver snaps out on a hit and
/// returns more gently, and the asymmetry is what makes a kick read as a kick rather than as a
/// sine. At the nominal frame these are 0.58 and 0.20 of the remaining distance.
const ATTACK_PER_MS: f32 = 0.035;
const RELEASE_PER_MS: f32 = 0.012;

/// Onset detection for the droplets and the cone kick, calibrated on the real-music fixture.
///
/// SPECTRAL FLUX - the sum of positive change across every band - against a slow-following average
/// of itself, so the threshold adapts to how busy the track is. A bass-mean rise cannot work: the
/// largest single-frame rise in the fixture's bass mean is 0.140, and that is what left the
/// vaporwave lightning unable to fire at all.
///
/// Swept over the fixture at a 200ms refractory: ratio 2.0 fires 3.25/s, 2.4 fires 2.75/s, 2.8
/// fires 1.75/s, 3.2 fires 1.50/s. Droplets are meant to be an event, not a texture, so 3.0 is
/// used and lands near 1.6/s.
const FLUX_RATIO: f32 = 3.0;
/// Follow rate of the flux average, per millisecond (0.02 per nominal frame).
// The flux average's follow rate lives in `dsp::onset` now, expressed per millisecond.
/// Minimum gap between transients. Shorter than any musical gap worth marking, long enough that
/// one hit's decaying flux peak does not register as a second hit.
const FLUX_REFRACTORY_MS: f32 = 200.0;

/// Upward kick applied to the field at a cone's mouth on a transient, scaled by that cone's own
/// excursion. The envelope follower alone cannot produce a step this sharp - it is deliberately
/// rate-limited - so without this a snare displaces no more water than a sustained pad.
const TRANSIENT_KICK: f32 = 0.35;

/// How far below the rest surface the cone's base sits, as a fraction of the tank depth.
///
/// Was 0.55, and reviewed by eye as "woofers too high, hard to see the interaction": with the cone
/// mouth that close to the surface there were only ~13px of clear water above it, so the waves it
/// launched had almost no room to be seen travelling before they reached the crest line. Pushing the
/// drivers down to 0.70 roughly doubles that clear water. It is bounded well above the tank floor -
/// at the reference geometry the basket still finishes several pixels clear of it.
const CONE_DEPTH: f32 = 0.70;

/// How long the underglow takes to fade from full to nothing, in milliseconds.
///
/// 850ms is deliberately far slower than the vaporwave lightning it was compared to: that is a
/// few-frame strike, and the brief for this was "more glow than flash". Long enough to still be
/// visible when the next kick lands at 120bpm (500ms), so successive hits ride on each other rather
/// than each starting from black.
const UNDERGLOW_RELEASE_MS: f32 = 850.0;

/// Gravity for the droplets, in px/s^2 at the 56px reference interior height, scaled with the
/// panel. With the default launch speed this gives an apex around 8px and a flight of ~0.27s,
/// which is long enough to read as an arc at 60fps and short enough that droplets do not
/// accumulate into a haze.
const DROP_GRAVITY: f32 = 900.0;
/// Ceiling on droplets in flight. Bounds the work per frame and stops a dense passage filling the
/// headroom with sparks.
const MAX_DROPS: usize = 28;
/// Dimple a landing droplet punches into the surface. Small, but it couples the ballistics back
/// into the simulation so a splash is visible as a ripple rather than as a vanishing dot.
const SPLASH: f32 = 0.10;

/// Interior height, in pixels, that every vertical constant here was tuned against (h = 60).
const REF_INTERIOR_H: f32 = 56.0;

/// The flourish: cavitation - the cone loses its grip on the liquid.
///
/// The fault that belongs to a driven liquid rather than to the electronics. Push a diaphragm hard enough
/// and the water cannot follow it: the pressure behind the cone drops below the vapour pressure, cavities
/// form and collapse, and the driver is briefly pushing vapour instead of water.
///
/// THE FIRST ATTEMPT AT THIS WAS REVERTED, and the reason is worth stating because it is not a tuning
/// story. Three models were tried that all INJECTED a disturbance into the wave field, and none can work:
/// the wave equation propagates whatever is injected, and interference then raises crests elsewhere even
/// when every injected sample is negative. This family clips on 0.00% of column-frames normally and
/// asserts under 1%; the froth took one colourway to 3.8%, then 3.2% at half amplitude, then 4.0%. The
/// number was never the problem.
///
/// Cavitation is a LOSS of coupling, not an addition of energy. So the effect is two things, and neither
/// touches the wave field:
///
/// - the cone's coupling collapses to `CAVITATE_GRIP`, so the waves already travelling run down and the
///   surface goes slack. Removing energy cannot clip anything.
/// - a froth is applied to the DRAWN surface line, downward only. On the drawn line it can neither
///   propagate nor raise a crest, so the family's guarantee that the liquid stays inside the tank holds by
///   construction rather than by tuning.
///
/// 1300ms, long enough to read as the pump losing its bite and recovering.
const CAVITATE_MS: f32 = 1300.0;

/// How much of the cone's grip on the liquid survives at full cavitation.
///
/// 0.12, so the cones nearly let go. Not zero: a cavitating pump still couples through the liquid it has
/// not lost, and a driver that stops dead reads as a severed cable rather than as cavitation.
const CAVITATE_GRIP: f32 = 0.12;

/// Damping multiplier at full cavitation.
///
/// Cutting the cone's DRIVE turned out to do nothing measurable - the waves already travelling dominate
/// the surface for far longer than 1300ms, so amplitude over the window was 8.98 against 8.for calm, i.e.
/// slightly UP rather than down. A knob that does nothing is the fault this project has already removed
/// twice, so the drive cut is backed by the lever that actually removes energy: the waves DECAY faster.
///
/// That is also the more honest model. A decoupled driver does not merely stop pushing; the liquid it has
/// lost contact with stops being driven at all and what is left runs down. Damping is where "runs down"
/// lives.
const CAVITATE_DAMP: f32 = 0.93;

/// Peak froth depth in surface units, its cap in real pixels, and how far it is biased downward.
///
/// Downward only, because cavities are VOIDS - the surface pulls away rather than rising. `SINK` of 0.9
/// puts the disturbance in -1.9..+0.1 of the amplitude, keeping a little upward spray at the edge of a
/// collapsing cavity so the texture is not a plain dip.
///
/// The pixel cap is what keeps it a roughening rather than a displacement. At 0.45 with no cap the froth
/// moved the drawn line five rows, which stops reading as a surface being disturbed and starts reading as
/// the surface having MOVED - and where the water is is this family's whole subject. `gain` scales with the
/// panel, so without a cap in real pixels a taller panel would get a proportionally larger froth.
const CAVITATE_AMP: f32 = 0.25;
const CAVITATE_MAX_PX: f32 = 3.0;
const CAVITATE_SINK: f32 = 0.9;

/// How much stronger the froth is at a cone mouth than at the far end of the tank.
///
/// 2.0. Cavitation happens at the driver, and a uniform froth across the whole surface reads as the
/// renderer adding noise rather than as the liquid failing where it is being pushed.
const CAVITATE_AT_MOUTH: f32 = 2.0;

/// Fraction of columns that cavitate at any one moment.
///
/// 0.62. Cavities are discrete bubbles, not a texture applied to every column - and the difference is
/// visible: at 1.0, with the sign alternating on parity, the froth rendered as a perfect sawtooth along
/// the waterline, which reads as a drawing artefact rather than as water. Leaving a bit over a third of the
/// columns alone each frame, and picking which ones by hash, makes it patchy the way a boil is.
const CAVITATE_DENSITY: f32 = 0.62;

#[derive(Clone, Copy)]
struct Drop {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

pub struct Fluid {
    /// Surface height per interior pixel column, in units of `surface_gain` pixels.
    cur: Vec<f32>,
    /// The same field one sub-step earlier - the second buffer the leapfrog needs.
    prev: Vec<f32>,
    /// Unspent frame time, so the fixed-size sub-steps still add up to real time. Without it a
    /// 10ms frame and a 20ms frame would both run one sub-step and the water would run at
    /// whatever rate the loop happened to tick at.
    debt: f32,
    /// Smoothed cone excursion, 0..1, left then right.
    exc: [f32; 2],
    /// The shared spectral-flux onset detector - see `dsp::onset`. This family and the vaporwave
    /// grid had independently written the same one; one copy is one threshold to get wrong.
    onset: crate::dsp::onset::Flux,
    seed: u32,
    /// The flourish: cavitation. See `CAVITATE_MS`.
    flourish: crate::dsp::flourish::Trigger,
    cavitate: crate::dsp::flourish::Envelope,
    /// Frame counter, so the froth churns rather than standing as a fixed ripple.
    cav_frame: u32,
    /// Underglow envelope, 0..1. Set to 1 on a transient, released slowly.
    glow: f32,
    drops: Vec<Drop>,
}

impl Default for Fluid {
    fn default() -> Self {
        Fluid {
            cur: Vec::new(),
            prev: Vec::new(),
            debt: 0.0,
            exc: [0.0; 2],

            // Starts past the refractory, so the first transient of a run is not swallowed. A
            // zero here would mean no droplet for the first 200ms after every theme switch.
            onset: crate::dsp::onset::Flux::default(),
            seed: 0x9e37_79b9,
            flourish: crate::dsp::flourish::Trigger::default(),
            cavitate: crate::dsp::flourish::Envelope::default(),
            cav_frame: 0,
            glow: 0.0,
            drops: Vec::new(),
        }
    }
}

/// Deterministic value hash, so a given seed always throws the same droplets and a dump is
/// reproducible. Same construction the vaporwave bolt paths use.
fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// `hash` mapped to 0.0..=1.0.
fn rand01(seed: u32, n: u32) -> f32 {
    hash(seed ^ n.wrapping_mul(0x9e37_79b9)) as f32 / u32::MAX as f32
}

/// Straight-colour blend. Used for the depth ramp and the film/sheen mixes; every result is drawn
/// at alpha 255 into the body, so this never touches the premultiplied invariant.
fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    let ch = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Rgba::new(ch(a.r, b.r), ch(a.g, b.g), ch(a.b, b.b), ch(a.a, b.a))
}

impl Fluid {
    /// Per-channel RMS onto 0..1 through the measured window.
    fn rms_resp(rms: f32, sensitivity: f32) -> f32 {
        if !rms.is_finite() {
            return 0.0;
        }
        let x = ((rms - RMS_FLOOR) / RMS_SPAN).clamp(0.0, 1.0);
        (x.powf(RMS_GAMMA) * sensitivity.max(0.0)).clamp(0.0, 1.0)
    }

    /// Bass contribution, biased toward the loudest low band rather than their mean.
    fn bass_resp(d: &FrameData, sensitivity: f32) -> f32 {
        let n = BASS_BANDS.min(d.levels.len());
        let (mut acc, mut cnt, mut peak) = (0.0f32, 0.0f32, 0.0f32);
        for v in &d.levels[..n] {
            if v.is_finite() {
                acc += *v;
                cnt += 1.0;
                peak = peak.max(*v);
            }
        }
        if cnt <= 0.0 {
            return 0.0;
        }
        let blended = (acc / cnt) * (1.0 - BASS_MAX_BIAS) + peak * BASS_MAX_BIAS;
        let x = ((blended - BASS_FLOOR) / BASS_SPAN).clamp(0.0, 1.0);
        (x * sensitivity.max(0.0)).clamp(0.0, 1.0)
    }

    /// One cone's target excursion from its own channel's RMS response and the shared bass
    /// response - biased toward whichever is louder, see `DRIVE_MAX_BIAS`.
    fn cone_drive(rms_resp: f32, bass_resp: f32) -> f32 {
        let mean = DRIVE_RMS_WEIGHT * rms_resp + (1.0 - DRIVE_RMS_WEIGHT) * bass_resp;
        let peak = rms_resp.max(bass_resp);
        (mean * (1.0 - DRIVE_MAX_BIAS) + peak * DRIVE_MAX_BIAS).clamp(0.0, 1.0)
    }

    /// Asymmetric envelope follower, timed from `dt_ms` rather than per frame.
    fn follow(cur: f32, target: f32, dt_ms: f32) -> f32 {
        let rate = if target > cur { ATTACK_PER_MS } else { RELEASE_PER_MS };
        let k = (rate * dt_ms).clamp(0.0, 1.0);
        let out = cur + (target - cur) * k;
        // `clamp` does NOT sanitise NaN - every comparison against NaN is false, so it returns
        // NaN and poisons the excursion permanently. Guard before clamping, not after.
        if out.is_finite() {
            out.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Advances the onset detector and reports whether this frame is a transient.
    ///
    /// The detector itself lives in `dsp::onset`; this keeps only the tuning and the droplet seed.
    fn update_flux(&mut self, d: &FrameData, dt_ms: f32) -> bool {
        if self.onset.update(&d.levels, dt_ms, FLUX_RATIO, FLUX_REFRACTORY_MS) {
            self.seed = self.seed.wrapping_add(1);
            true
        } else {
            false
        }
    }

    /// How many fixed sub-steps this frame gets, and the unspent remainder.
    ///
    /// This is where the stability guarantee actually lives, so it is a function of its own rather
    /// than four lines inside `draw`: `debt` is clamped BEFORE the division, so the returned count
    /// cannot exceed `MAX_SUBSTEPS` for any `dt_ms` at all - including a 5-second stall, a
    /// negative value or a NaN. The integrator therefore never sees a large timestep; it sees more
    /// small ones, up to a ceiling.
    fn substeps(debt: &mut f32, dt_ms: f32, speed: f32) -> i32 {
        let mut acc = *debt + dt_ms * speed;
        // `clamp` returns NaN for a NaN input, so the finite check has to come first.
        if !acc.is_finite() {
            acc = 0.0;
        }
        acc = acc.clamp(0.0, SUBSTEP_MS * MAX_SUBSTEPS as f32);
        let steps = (acc / SUBSTEP_MS).floor().clamp(0.0, MAX_SUBSTEPS as f32) as i32;
        *debt = (acc - steps as f32 * SUBSTEP_MS).max(0.0);
        steps
    }

    /// One fixed-size sub-step of the damped 1-D wave equation.
    ///
    /// `acceleration = c2 * (h[i-1] - 2h[i] + h[i+1])`, integrated leapfrog against the previous
    /// field, with the velocity term scaled by `damp`. The walls are reflecting (Neumann - the
    /// stencil reuses the edge column as its own neighbour), which is what makes a tank a tank:
    /// energy comes back off the ends instead of leaving, and the returning wavetrains are half
    /// of the interference in the middle.
    ///
    /// `c2` is a parameter rather than a direct read of the `C2` constant for one reason: it lets a
    /// test drive this at an UNSTABLE Courant number and show that the divergence is real, so the
    /// choice of 0.5 is demonstrated rather than asserted. Production has exactly one caller and it
    /// passes `C2`.
    fn substep(&mut self, c2: f32, damp: f32, mouths: &[(usize, usize, f32); 2], coupling: f32) {
        let n = self.cur.len();
        if n < 3 || self.prev.len() != n {
            return;
        }
        // The piston. Each cone drives the columns over its own mouth toward its excursion; the
        // resulting change in `cur` against an unchanged `prev` IS the velocity the integrator then
        // propagates outward, which is why no separate velocity source is needed.
        //
        // Two honest consequences of forcing a DISPLACEMENT rather than injecting a velocity:
        //
        // - It adds net VOLUME, so the whole surface rides up with loudness instead of a mound over
        //   each cone being balanced by a dip elsewhere. That is not what an incompressible liquid
        //   in a closed tank does. It is kept because the lift is a second position cue spanning
        //   the full width, and because the alternative - subtracting the field's mean every step to
        //   conserve volume - would make a sustained loud passage settle back to the rest line and
        //   throw that cue away. The cost is headroom, which is measured rather than assumed:
        //   `the_crests_stay_inside_the_tank_on_real_music` holds the clamp below 1% of
        //   column-frames.
        // - A cone at rest is an ABSORBER, not a mirror: a wave arriving at an idle mouth is pulled
        //   toward zero rather than reflected. A real driver is a load, so this is roughly right,
        //   but it does mean a hard-panned mix damps the far side of the silent cone. `coupling`
        //   below 1 is what stops that being total.
        for &(lo, hi, target) in mouths.iter() {
            for i in lo..hi.min(n) {
                self.cur[i] += (target - self.cur[i]) * coupling;
            }
        }
        for i in 0..n {
            let l = self.cur[if i == 0 { 0 } else { i - 1 }];
            let r = self.cur[if i + 1 >= n { n - 1 } else { i + 1 }];
            let lap = l - 2.0 * self.cur[i] + r;
            let vel = (self.cur[i] - self.prev[i]) * damp;
            let mut next = self.cur[i] + vel + c2 * lap;
            if !next.is_finite() {
                next = 0.0;
            }
            // Bounded HERE, in the state the next sub-step reads - not on the drawn pixel.
            self.prev[i] = next.clamp(-H_LIMIT, H_LIMIT);
        }
        // `prev` now holds the new field and `cur` the old one, which is exactly the leapfrog's
        // next pair.
        std::mem::swap(&mut self.prev, &mut self.cur);
    }

    /// Throws droplets off the highest crest in each half of the tank.
    fn spawn_drops(
        &mut self,
        params: &crate::themes::FluidParams,
        ix: i32,
        rest: i32,
        gain: f32,
        hscale: f32,
    ) {
        let n = params.droplets.clamp(0, 12);
        if n <= 0 || self.cur.is_empty() {
            return;
        }
        let cols = self.cur.len();
        let half = (cols / 2).max(1);
        let speed = params.droplet_v.max(0.0) * hscale;
        for k in 0..n {
            // Alternating halves, so a transient throws water on both sides rather than piling
            // every droplet on whichever cone happened to be louder.
            let (lo, hi) = if k % 2 == 0 { (0, half) } else { (half, cols) };
            let mut best = lo;
            for i in lo..hi {
                if self.cur[i] > self.cur[best] {
                    best = i;
                }
            }
            let crest = self.cur[best].max(0.0).min(1.5) / 1.5;
            if crest < 0.05 {
                continue;
            }
            if self.drops.len() >= MAX_DROPS {
                return;
            }
            let r1 = rand01(self.seed, k as u32 * 3);
            let r2 = rand01(self.seed, k as u32 * 3 + 1);
            let r3 = rand01(self.seed, k as u32 * 3 + 2);
            let x = (ix + best as i32) as f32 + (r1 - 0.5) * 3.0;
            let y = rest as f32 - self.cur[best] * gain - 1.0;
            self.drops.push(Drop {
                x,
                y,
                // Sideways spread is small: a droplet that flies far horizontally reads as a
                // spark, not as thrown water.
                vx: (r2 - 0.5) * 26.0 * hscale,
                vy: -speed * (0.55 + 0.60 * r3) * (0.45 + 0.55 * crest),
            });
        }
    }
}

impl Family for Fluid {
    fn id(&self) -> &'static str {
        "fluid"
    }

    fn draw(&mut self, c: &mut Canvas, theme: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        let f = &theme.fluid;
        c.clear();
        let panel = Rgba::from_hex(&theme.panel, theme.panel_alpha);
        if w < 24 || h < 20 {
            // Too small for a tank with a surface, two cones and any headroom above it. Fill the
            // panel and stop, rather than drawing a two-pixel smear that reads as corruption.
            c.rounded_rect(1, 2, (w - 2).max(1), (h - 4).max(1), 3, panel);
            return;
        }
        c.rounded_rect(1, 2, w - 2, h - 4, 4, panel);

        // Tank interior. Everything below is in these coordinates, and the panel underneath is
        // opaque everywhere - the "air" above the liquid is simply panel, never a transparent
        // pixel, because the overlay is composited per-pixel over the Windows weather widget.
        let (ix, iw) = (1, w - 2);
        let (iy, ih) = (2, h - 4);
        let cols = iw as usize;
        if self.cur.len() != cols {
            self.cur = vec![0.0; cols];
            self.prev = vec![0.0; cols];
            self.drops.clear();
        }

        let hscale = ih as f32 / REF_INTERIOR_H;
        let rest = iy + ((ih - 1) as f32 * f.surface.clamp(0.15, 0.80)).round() as i32;
        let floor_y = iy + ih - 1;
        let depth = (floor_y - rest).max(2);
        let gain = f.surface_gain.max(0.0) * hscale;

        // ---- audio -> cone excursion -------------------------------------------------------
        let dt_ms = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 100.0) } else { NOMINAL_DT_MS };
        let bass = Self::bass_resp(d, theme.sensitivity);
        let target = [
            Self::cone_drive(Self::rms_resp(d.rms_l, theme.sensitivity), bass),
            Self::cone_drive(Self::rms_resp(d.rms_r, theme.sensitivity), bass),
        ];
        self.exc[0] = Self::follow(self.exc[0], target[0], dt_ms);
        self.exc[1] = Self::follow(self.exc[1], target[1], dt_ms);

        // Cone geometry. 0.22 of the width in from each end: far enough apart that the two
        // wavetrains have room to separate before they meet, close enough to the walls that the
        // reflections come back while the outgoing wave is still visible.
        let cone_hw = (((iw as f32) * 0.058).round() as i32).max(2);
        let cxs = [
            ix + ((iw as f32) * 0.22).round() as i32,
            ix + iw - 1 - ((iw as f32) * 0.22).round() as i32,
        ];
        let mouth = |k: usize| -> (usize, usize) {
            let lo = (cxs[k] - cone_hw - ix).clamp(0, iw - 1) as usize;
            let hi = ((cxs[k] + cone_hw + 1 - ix).clamp(0, iw) as usize).max(lo + 1);
            (lo, hi.min(cols))
        };
        let mouths = [
            (mouth(0).0, mouth(0).1, self.exc[0]),
            (mouth(1).0, mouth(1).1, self.exc[1]),
        ];

        // ---- transient -----------------------------------------------------------------------
        let transient = self.update_flux(d, dt_ms);
        // Underglow envelope. Instant attack, slow release - "more glow than flash". A linear
        // release rather than exponential so the tail is actually visible for its whole duration
        // instead of spending most of it below the eye's threshold.
        if transient {
            self.glow = 1.0;
        }
        self.glow = (self.glow - dt_ms / UNDERGLOW_RELEASE_MS).max(0.0);
        if !self.glow.is_finite() {
            self.glow = 0.0;
        }
        if transient {
            for (k, m) in mouths.iter().enumerate() {
                let kick = TRANSIENT_KICK * self.exc[k];
                for i in m.0..m.1 {
                    self.cur[i] = (self.cur[i] + kick).clamp(-H_LIMIT, H_LIMIT);
                }
            }
            self.spawn_drops(f, ix, rest, gain, hscale);
        }

        // ---- cavitation, THE FLOURISH ---------------------------------------------------------
        //
        // Nothing is injected into the wave field - see `CAVITATE_MS` for the three models that tried and
        // why they cannot work. This advances the envelope; the coupling collapse is applied to the
        // sub-steps below and the froth to the drawn surface line further down.
        let fired = self.flourish.update(&d.levels, dt_ms, theme.flourish);
        let cav = self.cavitate.update(fired, dt_ms, CAVITATE_MS);
        if cav > 0.01 {
            self.cav_frame = self.cav_frame.wrapping_add(1);
        }

        // ---- simulation ----------------------------------------------------------------------
        // `dt_ms` decides only HOW MANY fixed sub-steps run - see `substeps` and the module docs.
        let mut debt = self.debt;
        let steps = Self::substeps(&mut debt, dt_ms, f.wave_speed.clamp(0.25, 3.0));
        self.debt = debt;
        // Cavitation makes the tank run down - see `CAVITATE_DAMP`. Damping REMOVES energy, which is why
        // it cannot push a crest into the top of the tank the way an injected disturbance did.
        let damp = f.damping.clamp(0.80, 0.9999) * (1.0 - (1.0 - CAVITATE_DAMP) * cav);
        // Cavitation collapses the cone's grip - see `CAVITATE_GRIP`. Applied to the COUPLING rather
        // than to the field, so the effect is the driver letting go and the waves already in the tank
        // simply running down. This is the half that removes energy, which is why it cannot clip.
        let coupling = f.coupling.clamp(0.01, 0.90) * (1.0 - (1.0 - CAVITATE_GRIP) * cav);
        for _ in 0..steps {
            self.substep(C2, damp, &mouths, coupling);
        }

        // ---- surface line --------------------------------------------------------------------
        let mut surf: Vec<i32> = Vec::with_capacity(cols);
        for i in 0..cols {
            let hpx = if self.cur[i].is_finite() { self.cur[i] * gain } else { 0.0 };
            // The froth, on the DRAWN line and downward only, so it can neither propagate nor raise a
            // crest. See `CAVITATE_AMP`.
            let froth = if cav > 0.01 {
                let near = mouths
                    .iter()
                    .map(|m| ((m.0 + m.1) as f32 * 0.5 - i as f32).abs())
                    .fold(f32::MAX, f32::min);
                let local =
                    1.0 + (CAVITATE_AT_MOUTH - 1.0) * (1.0 - (near / cols as f32 * 2.0).min(1.0));
                // IRREGULAR, not a comb. Alternating the sign on strict parity puts every column's
                // disturbance at the grid's Nyquist, which maximises roughness but rendered as a perfect
                // sawtooth - a zip along the waterline, which reads as a drawing artefact rather than as
                // water. Cavities are discrete and scattered, so the sign comes from a hash and only some
                // columns cavitate at all.
                let jitter = rand01(self.cav_frame, i as u32);
                let pick = rand01(self.cav_frame ^ 0x5bf0_3635, i as u32);
                if pick > CAVITATE_DENSITY {
                    surf.push((rest - hpx.round() as i32).clamp(iy + 1, floor_y - 1));
                    continue;
                }
                let sign = if rand01(self.cav_frame ^ 0x9e37_79b9, i as u32) > 0.5 { 1.0 } else { -1.0 };
                let amp = CAVITATE_AMP * cav * local * (0.45 + 0.55 * jitter);
                // Capped at the full `CAVITATE_MAX_PX`. It used to carry an extra 0.35 factor, which held
                // the froth to 1px and meant the cap - not the amplitude - was setting the depth: with the
                // sign alternating on parity that still read as rough, but once the signs were randomised
                // adjacent columns often moved together and the roughness collapsed to 0.606 from 1.301.
                // The limiter was the cap all along.
                ((sign * amp - amp * CAVITATE_SINK) * gain).max(-CAVITATE_MAX_PX)
            } else {
                0.0
            };
            // One row of air is always kept above the liquid so the meniscus has somewhere to be.
            // `froth` is subtracted from the height, so a negative froth pushes the line DOWN.
            let hpx = hpx + froth;
            surf.push((rest - hpx.round() as i32).clamp(iy + 1, floor_y - 1));
        }

        // ---- droplets ------------------------------------------------------------------------
        let g = DROP_GRAVITY * hscale;
        let dt_s = dt_ms / 1000.0;
        let mut splashes: Vec<usize> = Vec::new();
        self.drops.retain_mut(|p| {
            p.vy += g * dt_s;
            p.x += p.vx * dt_s;
            p.y += p.vy * dt_s;
            if !(p.x.is_finite() && p.y.is_finite() && p.vy.is_finite()) {
                return false;
            }
            let col = (p.x.round() as i32 - ix).clamp(0, cols as i32 - 1) as usize;
            if p.x < ix as f32 || p.x > (ix + iw - 1) as f32 {
                return false;
            }
            // Landing: falling and back at the surface. Punches a dimple, which is how the
            // ballistics feed back into the wave field.
            if p.vy > 0.0 && p.y >= surf[col] as f32 {
                splashes.push(col);
                return false;
            }
            p.y > (iy - 2) as f32
        });
        for col in splashes {
            self.cur[col] = (self.cur[col] - SPLASH).clamp(-H_LIMIT, H_LIMIT);
        }

        // ---- the liquid body -----------------------------------------------------------------
        // Depth ramp anchored to the TANK, not to the moving surface: a crest passing over a
        // column must move its top edge, not repaint the column.
        // Routed through `tint` rather than read straight from the hex, so the body obeys a
        // rainbow/ink colourway. `tint` returns `from_hex(fallback)` whenever `rainbow` is 0, which
        // is every colourway that does not opt in - so the five physical ones are byte-identical.
        //
        // The two ends are tinted at DIFFERENT positions, which makes an ink colourway a DUOTONE:
        // two process inks with the ramp running between them, instead of one ink fading to itself.
        // Without this the body ignores the ink machinery completely and a Pantone colourway differs
        // from plain water only in its meniscus - the "all too similar except bars" failure the
        // Pantone colourways already had once.
        let body_top = tint(theme, 0.0, d.time_s, false, &f.body_top, 1.0);
        let deep_ink = tint(theme, 0.62, d.time_s, false, &f.body_deep, 1.0);
        // Darkened only on the tinted path: `tint` returns a full-value colour, and a full-value
        // deep end would flatten the depth read the ramp exists for.
        let body_deep = if theme.rainbow > 0.0 {
            mix(deep_ink, Rgba::from_hex("#000000", 1.0), 0.55)
        } else {
            deep_ink
        };
        // The specular is the shared `hot`, not a family-private colour: it IS this family's
        // hot accent, and routing it through `tint` means a rainbow colourway works here too.
        let glint = tint(theme, 0.5, d.time_s, true, &theme.hot, 1.0);
        let ramp: Vec<Rgba> = (0..ih)
            .map(|r| {
                let t = (((iy + r) - rest) as f32 / depth as f32).clamp(0.0, 1.0);
                mix(body_top, body_deep, t)
            })
            .collect();
        // Specular band just under the surface. A liquid METAL has a hard bright horizon there
        // where a body of water does not, which is one of the structural differences between the
        // colourways rather than a hue change.
        let sheen = f.sheen.clamp(0.0, 1.0);
        let sheen_rows = ((ih as f32 * 0.06).round() as i32).max(2) as f32;
        for i in 0..cols {
            let x = ix + i as i32;
            let s = surf[i];
            for y in s..=floor_y {
                let mut col = ramp[(y - iy).clamp(0, ih - 1) as usize];
                if sheen > 0.0 {
                    let below = (y - s) as f32;
                    if below < sheen_rows {
                        col = mix(col, glint, sheen * (1.0 - below / sheen_rows));
                    }
                }
                c.fill_rect(x, y, 1, 1, col);
            }
        }
        // Tank floor, so the body has a bottom rather than running into the bezel.
        c.fill_rect(ix, floor_y, iw, 1, mix(body_deep, Rgba::from_hex(&theme.edge, 1.0), 0.35));

        // ---- the cones -----------------------------------------------------------------------
        // Drawn AFTER the body and slightly transparent over it, so they read as submerged - a
        // fully opaque cone reads as a cut-out sitting in front of the glass.
        let cone_col = Rgba::from_hex(&f.cone, 0.88);
        let cone_dark = Rgba::from_hex(&f.cone_dark, 0.92);
        let travel = ((depth as f32) * f.cone_travel.clamp(0.0, 0.45)).max(1.0);
        let cone_h = ((depth as f32 * 0.16).round() as i32).max(3);
        let cone_base = rest + ((depth as f32) * CONE_DEPTH).round() as i32;
        let mut rims: [(i32, i32); 2] = [(0, 0); 2];
        for k in 0..2 {
            let cx = cxs[k];
            // Motor and basket are BOLTED TO THE TANK and do not move; only the diaphragm does,
            // which is what makes the movement legible - there is a static reference beside it.
            c.fill_rect(
                cx - (cone_hw / 3).max(1),
                cone_base + 1,
                (cone_hw / 3).max(1) * 2 + 1,
                (depth / 5).max(2),
                cone_dark,
            );
            c.fill_poly(
                &[
                    (cx - cone_hw, cone_base - 1),
                    (cx + cone_hw + 1, cone_base - 1),
                    (cx + (cone_hw / 2).max(1), cone_base + 3),
                    (cx - (cone_hw / 2).max(1), cone_base + 3),
                ],
                cone_dark,
            );
            // The diaphragm: a shallow cone pointing down at the voice coil. Its APEX IS WELDED TO
            // THE MOTOR and only the mouth travels, so the whole cone deepens and shallows.
            //
            // It used to translate as a rigid body - apex and rim moving together by the excursion -
            // and that was reported as "the inner part of the sub coming disconnected from the
            // bottom looks bad". It was: the apex is drawn sitting exactly on the surround at rest,
            // so any upward travel opened a visible gap between the cone's point and the motor it is
            // supposed to be driven by, and the part read as having come loose.
            //
            // Anchoring the apex and travelling the rim keeps it attached in every frame and still
            // reads as a cone moving in and out - which is also the more honest mechanism, since a
            // real driver's surround flexes while its coil stays in the gap. POSITION remains the
            // primary cue at this size (rule 6): the mouth is the wide, high-contrast edge, so it is
            // the part whose movement is actually legible at 60px.
            let apex = cone_base - 1;
            let rim = apex - cone_h - (self.exc[k] * travel).round() as i32;
            rims[k] = (cx, rim);
            c.fill_poly(
                &[(cx - cone_hw, rim), (cx + cone_hw + 1, rim), (cx, apex)],
                cone_col,
            );
            // Dust cap, a touch brighter than the cone so the centre reads.
            c.fill_rect(
                cx - (cone_hw / 3).max(1),
                rim + (cone_h / 3).max(1),
                (cone_hw / 3).max(1) * 2 + 1,
                2,
                mix(Rgba::from_hex(&f.cone, 1.0), glint, 0.30),
            );
        }

        // ---- everything that emits light, on its own transparent layer ------------------------
        // `bloom` composites its halo UNDER existing content, so blooming a canvas that already
        // carries the opaque tank leaves the halo invisible. Light is built here, bloomed here,
        // and only then drawn over the tank.
        let mut lit = Canvas::new(w, h);
        let film = Rgba::from_hex(&f.film, 1.0);
        let irid = f.iridescence.clamp(0.0, 1.0);
        let emissive = f.emissive.clamp(0.0, 1.0);
        let emissive_rows = ((ih as f32 * 0.09).round() as i32).max(2);

        // ---- underglow: the tank lit from beneath on a bass hit ------------------------------
        // On the light layer and therefore bloomed, which is what makes it a glow rather than a
        // flat brighter band - and it is drawn FIRST so the meniscus, glints and droplets all sit
        // over it. Only inside the liquid: a column is skipped above its own surface, so the glow
        // stops at the waterline and the air above stays dark. Alpha only ever ADDS, so this cannot
        // punch a hole for the weather widget to show through.
        let underglow = f.underglow.clamp(0.0, 1.0);
        if underglow > 0.0 && self.glow > 0.0 {
            let g = (self.glow * underglow).clamp(0.0, 1.0);
            let band = (((depth as f32) * 0.60).round() as i32).max(2);
            let col = tint(theme, 0.5, d.time_s, false, &theme.lit, 1.0);
            for y in (floor_y - band).max(iy)..=floor_y {
                // Brightest at the floor, falling off quadratically upward.
                let up = (floor_y - y) as f32 / band as f32;
                let a = g * (1.0 - up) * (1.0 - up) * 0.80;
                if a <= 0.004 {
                    continue;
                }
                let alpha = (a * 255.0).clamp(0.0, 255.0) as u8;
                for i in 0..cols {
                    if y >= surf[i] {
                        lit.fill_rect(ix + i as i32, y, 1, 1, Rgba::new(col.r, col.g, col.b, alpha));
                    }
                }
            }
        }

        for i in 0..cols {
            let x = ix + i as i32;
            let s = surf[i];
            let x01 = i as f32 / (cols - 1).max(1) as f32;
            // Local slope, from the drawn surface rather than the field, so the highlight lands
            // exactly on the pixels the eye sees as the crest.
            let left = surf[i.saturating_sub(1)];
            let right = surf[(i + 1).min(cols - 1)];
            let slope = (right - left) as f32 * 0.5;
            let crest = (if self.cur[i].is_finite() { self.cur[i] } else { 0.0 }).clamp(0.0, 1.5) / 1.5;

            // Thin-film iridescence: the interference colour of an oil film depends on the angle
            // you see it at, so the meniscus is mixed toward `film` by the local SLOPE. Zero
            // iridescence leaves the meniscus exactly as declared.
            // The meniscus is the shared `lit` - the one element of this family that is "the lit
            // colour" - so the project-wide 3:1 contrast test measures the colour actually drawn.
            let base = tint(theme, x01, d.time_s, false, &theme.lit, 1.0);
            let men = mix(base, film, irid * (0.5 + 0.5 * (slope / 3.0).clamp(-1.0, 1.0)));
            lit.fill_rect(x, s, 1, 1, Rgba::new(men.r, men.g, men.b, 235));

            // Specular glint: only where the surface is BOTH high and locally flat, which is
            // where a real crest catches the light. Keyed on slope as well as height so it picks
            // out crest apexes instead of washing the whole raised region.
            if crest > 0.12 && slope.abs() <= 1.0 {
                let a = (0.25 + 0.60 * crest).clamp(0.0, 1.0);
                lit.fill_rect(x, s, 1, 1, Rgba::new(glint.r, glint.g, glint.b, (a * 255.0) as u8));
                if f.caustics {
                    // Light focused by the crest, a couple of rows down in the body.
                    let a2 = (0.10 + 0.35 * crest).clamp(0.0, 1.0);
                    for dy in 2..4 {
                        if s + dy < floor_y {
                            lit.fill_rect(
                                x,
                                s + dy,
                                1,
                                1,
                                Rgba::new(glint.r, glint.g, glint.b, (a2 * 255.0) as u8),
                            );
                        }
                    }
                }
            }

            // Emissive liquid: the top rows of the body put light INTO the layer that gets
            // bloomed, so a coolant glows outward instead of merely being a bright colour.
            if emissive > 0.0 {
                for dy in 1..=emissive_rows {
                    if s + dy <= floor_y {
                        let fade = 1.0 - (dy - 1) as f32 / emissive_rows as f32;
                        lit.fill_rect(
                            x,
                            s + dy,
                            1,
                            1,
                            Rgba::new(
                                body_top.r,
                                body_top.g,
                                body_top.b,
                                (emissive * fade * 200.0) as u8,
                            ),
                        );
                    }
                }
            }
        }

        // Cone rim highlight: the secondary, INTENSITY cue for a channel, on top of the position
        // one. Deliberately second - ten brightnesses have to be compared pairwise where a
        // translation is read at a glance.
        for k in 0..2 {
            let (cx, rim) = rims[k];
            let a = (0.30 + 0.55 * self.exc[k]).clamp(0.0, 1.0);
            lit.fill_rect(
                cx - cone_hw,
                rim,
                cone_hw * 2 + 1,
                1,
                Rgba::new(glint.r, glint.g, glint.b, (a * 255.0) as u8),
            );
        }

        // Droplets.
        for p in &self.drops {
            lit.fill_rect(p.x.round() as i32, p.y.round() as i32, 1, 1, Rgba::new(glint.r, glint.g, glint.b, 250));
        }

        if theme.bloom > 0.0 {
            let mut glow = lit.clone();
            glow.bloom(theme.bloom as i32, theme.glow_strength.clamp(0.0, 1.0));
            c.draw_over(&glow);
        }
        c.draw_over(&lit);

        // Clip AFTER blooming, with the panel's own rect, or the halo leaks past the rounded
        // corners and shows as a bright fringe on the taskbar.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);
        let e = Rgba::from_hex(&theme.edge, theme.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    /// 8 seconds of real music, captured with `--levels` and committed as a fixture: 792 frames of
    /// 64 RAW bands at about 99 frames per second.
    ///
    /// Every audio mapping in this project that was calibrated against a synthetic spectrum turned
    /// out to be wrong, the vaporwave terrain four separate times, so the fixture is the only
    /// acceptable basis for the numbers in this file.
    fn real_music() -> Vec<Vec<f32>> {
        include_str!("../../tests/fixtures/real-music-bands.csv")
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split(',').filter_map(|v| v.parse::<f32>().ok()).collect())
            .collect()
    }

    /// The capture ran at about 99 frames per second, so this is the fixture's frame interval.
    /// Milliseconds per fixture row. EMPIRICAL, and correct - do not "derive" it.
    ///
    /// `main::measure_levels` captures for exactly `Duration::from_secs(8)` and the three fixtures hold 790
    /// to 793 rows, so the interval is 10.09 to 10.13ms and 1000/99 is within 0.3% of all three.
    ///
    /// I replaced this with `HOP / 48kHz` in `dsp::onset` on the reasoning that a fixture row is one DSP
    /// frame, and it was wrong: that gives 10.67ms, which would make an 8-second capture 750 rows rather
    /// than 792. The capture loop does not emit exactly one frame per hop - it processes whatever the device
    /// hands it per iteration - so this rate is measured, not calculated.
    const FIXTURE_DT_MS: f32 = 1000.0 / 99.0;

    /// Reconstruction of `rms_l`/`rms_r` from a fixture frame, and it needs stating plainly:
    /// **the fixture recorded the 64 bands only, not the per-channel RMS.**
    ///
    /// The same capture DID record the RMS distribution - p50 0.240, p90 0.399, max 0.576 - so the
    /// reconstruction is a power law on the frame's band mean whose own percentiles reproduce those
    /// three numbers: 0.240 / 0.399 / 0.572 against the recorded 0.240 / 0.399 / 0.576. That is
    /// asserted by `the_reconstructed_rms_matches_the_recorded_rms_distribution`, so this cannot
    /// quietly drift into a convenient signal.
    ///
    /// What it proves and what it does not: it drives the cones with real musical DYNAMICS at the
    /// real level distribution, which is what every defect in this codebase has come from getting
    /// wrong. It does NOT prove anything about real stereo decorrelation - the two channels are
    /// split here by the frame's own spectral tilt, which makes them differ by about 0.03 (p50)
    /// where a real mix differs by more.
    const FIT_SCALE: f32 = 2.192;
    const FIT_GAMMA: f32 = 1.583;
    const FIT_TILT: f32 = 0.35;

    fn fixture_rms(row: &[f32]) -> (f32, f32) {
        let n = row.len().max(1);
        let mean = row.iter().sum::<f32>() / n as f32;
        let proxy = FIT_SCALE * mean.powf(FIT_GAMMA);
        let half = n / 2;
        let lo = row[..half].iter().sum::<f32>() / half.max(1) as f32;
        let hi = row[half..].iter().sum::<f32>() / (n - half).max(1) as f32;
        let tilt = (hi - lo) / (hi + lo).max(1e-6);
        (proxy * (1.0 + FIT_TILT * tilt), proxy * (1.0 - FIT_TILT * tilt))
    }

    fn fixture_frame(row: &[f32], time_s: f32) -> FrameData {
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = row.get(i).copied().unwrap_or(0.0);
        }
        d.peaks = d.levels;
        let (l, r) = fixture_rms(row);
        d.rms_l = l;
        d.rms_r = r;
        d.dt_ms = FIXTURE_DT_MS;
        d.time_s = time_s;
        d
    }

    fn pct(v: &mut Vec<f32>, p: f32) -> f32 {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[(((v.len() - 1) as f32) * p) as usize]
    }

    /// Everything a fixture run produces, so one pass can answer several questions.
    struct Trace {
        exc_l: Vec<f32>,
        exc_r: Vec<f32>,
        /// Field at the exact middle column, per frame - the interference site.
        mid: Vec<f32>,
        /// Peak-to-trough surface travel across the tank, in pixels, per frame.
        range: Vec<f32>,
        /// Mean absolute column-to-column step in the field, per frame - a scale-free measure of
        /// how FINE the ripples are, which is what `wave_speed` changes and `surface_gain` does not.
        rough: Vec<f32>,
        /// Transients detected (each fires the cone kick and, where enabled, the droplets).
        transients: u32,
        /// Droplets in flight at the end of the run, and the most ever in flight at once.
        drops_alive: usize,
        drops_peak: usize,
        /// Frames on which the droplet list GREW - i.e. volleys actually thrown.
        ///
        /// Counted separately from `transients` because they are not the same claim and a test that
        /// conflates them is vacuous: mutation testing showed that asserting the droplet RATE via
        /// `transients` still passed with `spawn_drops` replaced by an immediate `return`, because
        /// the detector fires whether or not anything is thrown. This counts the throw.
        drop_volleys: u32,
        worst_field: f32,
        /// Fraction of (frame, column) pairs whose crest hit the top of the tank and had to be
        /// clamped. A flat-topped surface is a visible defect, so this is asserted, not just
        /// printed - see `the_crests_stay_inside_the_tank_on_real_music`.
        clamped: f32,
    }

    /// Height of the DRAWN liquid body, in pixels, per frame and per column.
    ///
    /// Every other measurement here reads `fam.cur` - the simulated field - which is what let two
    /// mutants through review: pinning the opaque body at a fixed row while the 1px light layer kept
    /// following the field changed 8.5-12.5% of channels and broke no test, because the one test that
    /// looked at pixels found the BRIGHTEST row, which is the meniscus in the bloomed `lit` layer and
    /// not the body at all. This measures the body itself.
    ///
    /// The body is isolated by painting it a colour nothing else here uses and switching off every
    /// element that would recolour it - sheen mixes the specular into the top rows, caustics and
    /// iridescence tint it, emission blooms it, and a droplet lands in front of it. What remains is
    /// the plain depth ramp, so a pixel is liquid iff it is that magenta.
    ///
    /// Counted from the FLOOR UPWARD, stopping at the first non-liquid row. Bloom and spray above
    /// the surface therefore cannot be mistaken for liquid: they are above the stopping point, never
    /// below it. Columns over the two cone mouths are excluded by the caller - the cone is drawn
    /// over the body at 0.88 alpha, so those columns are legitimately not magenta.
    fn drawn_body(theme: &Theme, frames: &[Vec<f32>], w: i32, h: i32) -> Vec<Vec<i32>> {
        drawn_body_firing(theme, frames, w, h, None)
    }

    /// `drawn_body`, with the option to force the flourish before frame `fire_at`.
    ///
    /// The classifier is the same one, deliberately: it is the only measurement in this family that reads
    /// the DRAWN body rather than the simulated field, and the cavitation froth is applied to the drawn
    /// surface. My first attempt at measuring the froth built a fresh luminance scan instead and reported
    /// 0.00 roughness for both arms while a probe confirmed the froth reaching the surface - the meniscus
    /// is a single bright row over a dark gradient with dimmer water beneath, so a threshold scan finds the
    /// wrong row. This one classifies by an exclusive marker colour and counts from the floor up.
    fn drawn_body_firing(
        theme: &Theme,
        frames: &[Vec<f32>],
        w: i32,
        h: i32,
        fire_at: Option<usize>,
    ) -> Vec<Vec<i32>> {
        let mut t = theme.clone();
        t.fluid.body_top = "#ff00ff".into();
        t.fluid.body_deep = "#ff00ff".into();
        t.fluid.sheen = 0.0;
        t.fluid.caustics = false;
        t.fluid.iridescence = 0.0;
        t.fluid.emissive = 0.0;
        t.fluid.droplets = 0;
        // The underglow is composited over the body from the light layer, so it tints the marker
        // cyan and the classifier stops recognising the body at all - every column measured 0. It is
        // the SECOND element to break this harness the same way, after the ink path below, which is
        // why the rule is stated as a rule: anything that can repaint a body pixel has to be
        // neutralised here, and a new one has to be added to this list.
        t.fluid.underglow = 0.0;
        // The ink path repaints the body from the colourway's hue cycle, which overwrites the marker
        // and made every column measure zero on `fluid-pantone` - a harness artifact, not a render
        // bug. Anything that can recolour the marker has to be neutralised here, aberration
        // included: it splits the channels and would fail the classifier on the body's edges.
        t.rainbow = 0.0;
        t.inks = 0;
        t.aberration = 0.0;
        let mut fam = Fluid::default();
        let mut c = Canvas::new(w, h);
        let (ix, iy, iw, ih) = (1, 2, w - 2, h - 4);
        let mut out = Vec::with_capacity(frames.len());
        for (k, row) in frames.iter().enumerate() {
            if fire_at == Some(k) {
                fam.flourish.force_next();
            }
            fam.draw(&mut c, &t, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
            let mut heights = Vec::with_capacity(iw as usize);
            for x in ix..(ix + iw) {
                let mut liquid = 0;
                // From the floor up. `floor_y` itself is the tank floor, a blend of body_deep and
                // the bezel, so it is not magenta and is skipped.
                for y in (iy..(iy + ih - 1)).rev() {
                    let p = c.get(x, y);
                    if p.r > 200 && p.b > 200 && p.g < 90 {
                        liquid += 1;
                    } else {
                        break;
                    }
                }
                heights.push(liquid);
            }
            out.push(heights);
        }
        out
    }

    /// Columns a body-height measurement must ignore, in `drawn_body` index space.
    ///
    /// Two reasons, both measured rather than assumed - the height profile of one frame at 190x60
    /// reads `[0, 30, 30, ..., 34, 34, 14, 13, 12, 11, 7, 7, 7, 7, 11, 12, 13, 14, 34, ..., 31, 0]`:
    ///
    /// - THE CONE MOUTHS, columns 30-52 and 135-157 there, where the body height collapses from 34
    ///   to 7. The cone is drawn over the body at 0.88 alpha so those pixels are legitimately not
    ///   the body colour; `the_drawn_surface_line_follows_the_simulated_field` derives the same
    ///   geometry and records the same exclusion for the same reason. Widened by 3 columns because
    ///   the rim highlight sits just outside the mouth.
    /// - THE BEZEL, the single column at each end, which reads 0 in every frame because the panel
    ///   edge is stroked over it. Left in, those two columns alone put the peak-to-trough relief at
    ///   39px on a 56px interior - which is what caught this - and they say nothing about the water.
    fn masked_columns(iw: i32) -> Vec<bool> {
        let hw = (((iw as f32) * 0.058).round() as i32).max(2);
        let cxs = [
            ((iw as f32) * 0.22).round() as i32,
            iw - 1 - ((iw as f32) * 0.22).round() as i32,
        ];
        (0..iw)
            .map(|i| {
                i < 2 || i >= iw - 2 || cxs.iter().any(|&cx| (i - cx).abs() <= hw + 3)
            })
            .collect()
    }

    fn drive(theme: &Theme, frames: &[Vec<f32>], w: i32, h: i32) -> Trace {
        let mut fam = Fluid::default();
        let mut c = Canvas::new(w, h);
        let mut t = Trace {
            exc_l: Vec::new(),
            exc_r: Vec::new(),
            mid: Vec::new(),
            range: Vec::new(),
            rough: Vec::new(),
            transients: 0,
            drops_alive: 0,
            drops_peak: 0,
            drop_volleys: 0,
            worst_field: 0.0,
            clamped: 0.0,
        };
        let mut seed = fam.seed;
        let mut drops_seen = 0usize;
        let gain = theme.fluid.surface_gain * ((h - 4) as f32 / REF_INTERIOR_H);
        // Headroom between the rest surface and the one row of air the renderer always keeps.
        let rest = 2 + ((h - 4 - 1) as f32 * theme.fluid.surface.clamp(0.15, 0.80)).round() as i32;
        let headroom = (rest - 3) as f32;
        let mut clamped = 0u64;
        let mut samples = 0u64;
        for (k, row) in frames.iter().enumerate() {
            fam.draw(&mut c, theme, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
            if fam.seed != seed {
                t.transients += 1;
                seed = fam.seed;
            }
            // Growth, not absolute count: drops expire in the same frame others are born, so a
            // volley is a net INCREASE. This undercounts a volley that coincides with expiries,
            // which is acceptable - the assertion is a rate band, and the mutant it must catch
            // produces exactly zero.
            let before = drops_seen;
            drops_seen = fam.drops.len();
            if drops_seen > before {
                t.drop_volleys += 1;
            }
            t.drops_peak = t.drops_peak.max(fam.drops.len());
            t.exc_l.push(fam.exc[0]);
            t.exc_r.push(fam.exc[1]);
            let mid = fam.cur.len() / 2;
            t.mid.push(fam.cur[mid]);
            let hi = fam.cur.iter().copied().fold(f32::MIN, f32::max);
            let lo = fam.cur.iter().copied().fold(f32::MAX, f32::min);
            t.range.push((hi - lo) * gain);
            let steps: f32 = fam.cur.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
            // Normalised by the frame's own amplitude, so this measures ripple FINENESS rather
            // than loudness: a big slow swell and a small fast chop must not score the same.
            t.rough.push(steps / (fam.cur.len() - 1) as f32 / (hi - lo).max(1e-4));
            for v in &fam.cur {
                t.worst_field = t.worst_field.max(v.abs());
                samples += 1;
                if v * gain >= headroom {
                    clamped += 1;
                }
            }
        }
        t.drops_alive = fam.drops.len();
        t.clamped = clamped as f32 / samples.max(1) as f32;
        t
    }

    // ---------------------------------------------------------------------------------------
    // The response mapping
    // ---------------------------------------------------------------------------------------

    #[test]
    fn the_reconstructed_rms_matches_the_recorded_rms_distribution() {
        // Guards the fixture harness itself, not the family. If this drifts, every number the
        // fixture tests below report is measured against a signal the capture never saw - which is
        // exactly the class of mistake the fixture exists to end.
        let frames = real_music();
        assert!(frames.len() > 500, "fixture looks truncated: {} frames", frames.len());
        let mut all: Vec<f32> = frames
            .iter()
            .flat_map(|r| {
                let (l, r2) = fixture_rms(r);
                [l, r2]
            })
            .collect();
        let (p50, p90, max) = (pct(&mut all, 0.5), pct(&mut all, 0.9), pct(&mut all, 1.0));
        // Recorded: p50 0.240, p90 0.399, max 0.576.
        assert!((p50 - 0.240).abs() < 0.02, "p50 {p50:.3} vs the recorded 0.240");
        assert!((p90 - 0.399).abs() < 0.03, "p90 {p90:.3} vs the recorded 0.399");
        assert!((max - 0.576).abs() < 0.05, "max {max:.3} vs the recorded 0.576");
    }

    #[test]
    fn the_cone_response_spends_its_range_on_the_measured_rms_window() {
        // The defect this exists to prevent has shipped four times in one family alone: a mapping
        // that expects its input to swing from 0 to 1 when the real signal never leaves a narrow
        // band, so the element barely moves. Measured window: p50 0.240, p90 0.399, max 0.576.
        let lo = Fluid::rms_resp(0.240, 1.0);
        let hi = Fluid::rms_resp(0.399, 1.0);
        assert!(lo > 0.25 && lo < 0.65, "a median moment must be mid-travel, got {lo:.3}");
        assert!(hi > 0.85, "the loud tenth must be near full travel, got {hi:.3}");
        assert!(hi - lo > 0.35, "p50..p90 must cover a large part of the range: {lo:.3} -> {hi:.3}");
        assert_eq!(Fluid::rms_resp(0.0, 1.0), 0.0, "silence must be still, not a pedestal");
        assert!(Fluid::rms_resp(0.576, 1.0) >= 0.999, "the loudest frame must reach the top");
        // sensitivity is the user-facing knob and must actually do something here.
        assert!(
            Fluid::rms_resp(0.24, 1.6) > Fluid::rms_resp(0.24, 1.0),
            "sensitivity must scale the cone drive"
        );
    }

    #[test]
    fn the_bass_reducer_is_biased_toward_the_loudest_low_band_not_their_mean() {
        // Rule 7. A kick lives in one or two bands; averaging six of them throws it away. The
        // fixture's bass MEAN spans only 0.320..0.445 across p10..p90, so a mean-fed mapping is
        // working inside a 0.125-wide window and cannot help but look dead.
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = if i == 2 { 0.90 } else { 0.10 };
        }
        let got = Fluid::bass_resp(&d, 1.0);
        // mean over the first 6 = 0.233; max = 0.90; the blend must sit well above the mean.
        let mean_only = ((0.2333f32 - BASS_FLOOR) / BASS_SPAN).clamp(0.0, 1.0);
        assert!(
            got > mean_only + 0.4,
            "a single loud low band must drive the cones: {got:.3} vs {mean_only:.3} from the mean"
        );
        // ...and never above what the loudest band alone would give.
        let max_only = ((0.90f32 - BASS_FLOOR) / BASS_SPAN).clamp(0.0, 1.0);
        assert!(got <= max_only + 1e-6, "but never above the peak: {got:.3} vs {max_only:.3}");
    }

    #[test]
    fn the_cone_drive_is_biased_toward_whichever_driver_is_louder() {
        // Same rule applied to the two contributions feeding one cone: a plain mean of the RMS and
        // bass responses drags both toward the middle. Measured on the fixture, the mean gives p90
        // 0.757 where the max-biased blend gives 0.879.
        let mean = DRIVE_RMS_WEIGHT * 0.9 + (1.0 - DRIVE_RMS_WEIGHT) * 0.1;
        let got = Fluid::cone_drive(0.9, 0.1);
        assert!(got > mean + 0.1, "must sit above the plain mean: {got:.3} vs {mean:.3}");
        assert!(got <= 0.9 + 1e-6, "but never above the louder driver: {got:.3}");
        assert_eq!(Fluid::cone_drive(0.0, 0.0), 0.0, "silence stays silent");
    }

    // ---------------------------------------------------------------------------------------
    // Real music
    // ---------------------------------------------------------------------------------------

    #[test]
    fn real_music_moves_both_cones_across_most_of_their_travel() {
        // The headline fixture assertion: on real material the cones must actually pump. Measured
        // over the 792 frames at the shipped settings, the excursion runs p10 0.30 / p50 0.60 /
        // p90 0.85 - i.e. it spends its time across most of the travel rather than pinned.
        let frames = real_music();
        let t = builtin::fluid_deep();
        let tr = drive(&t, &frames, 190, 60);
        let (p10, p50, p90) = (
            pct(&mut tr.exc_l.clone(), 0.10),
            pct(&mut tr.exc_l.clone(), 0.50),
            pct(&mut tr.exc_l.clone(), 0.90),
        );
        assert!(p50 > 0.35 && p50 < 0.80, "median excursion {p50:.2} should be mid-travel");
        assert!(p90 - p10 > 0.30, "the cone barely moves: p10 {p10:.2} -> p90 {p90:.2}");
        // And in PIXELS, because a swing that rounds to one pixel is not a position cue. The
        // reference travel is 0.16 of a 32px depth, i.e. 5px.
        let travel_px = 32.0 * t.fluid.cone_travel;
        assert!(
            (p90 - p10) * travel_px >= 2.0,
            "the cone's p10..p90 swing is only {:.1}px, which will not read as movement",
            (p90 - p10) * travel_px
        );
        // The right cone is fed rms_r and must not be a copy of the left.
        let diff: f32 = tr
            .exc_l
            .iter()
            .zip(tr.exc_r.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(diff > 0.005, "the two cones never differ; is rms_r actually read? max diff {diff}");
    }

    #[test]
    fn real_music_makes_the_surface_use_a_useful_amount_of_the_tank() {
        // "The waves seem very low" is the complaint this guards, and it is the same one the
        // vaporwave terrain drew four times.
        //
        // It asserts on the DRAWN body (see `drawn_body`) rather than on the field, and it asserts
        // MOTION as well as amplitude. Both of those are corrections: measured against the field
        // only, this test passed with `cone_drive` pinned to a constant - audio fully disconnected -
        // and with `substep` replaced by an immediate `return`. Neither mutant removes the
        // amplitude: a constant drive settles into a static mound over each cone, and a dead
        // integrator leaves the piston's own displacement standing at the mouths. Both are
        // peak-to-trough relief across the tank, and both are completely motionless.
        //
        // So the load-bearing measurement is the SECOND one: how far each column's own top edge
        // travels over the run, away from the cones. That is zero for a static mound, zero for a
        // field that never propagates, and it is also zero for the third mutant this family shipped
        // undetected - pinning the opaque body at a fixed row while the light layer follows.
        //
        // Measured on the fixture at 190x60: spatial relief runs p10 ~5px / p50 ~8px / p90 ~11px
        // against a 33px body and 23 rows of headroom; per-column travel over the 8 seconds has a
        // median of ~14px across the 148 non-cone columns.
        let frames = real_music();
        let t = builtin::fluid_deep();
        let body = drawn_body(&t, &frames, 190, 60);
        assert!(!body.is_empty(), "no frames measured");
        let iw = 190 - 2;
        let is_cone = masked_columns(iw);

        // ---- amplitude: the relief across the tank, per frame
        let spatial: Vec<f32> = body
            .iter()
            .map(|row| {
                let (mut lo, mut hi) = (i32::MAX, i32::MIN);
                for (i, &v) in row.iter().enumerate() {
                    if !is_cone[i] {
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
                (hi - lo) as f32
            })
            .collect();
        let p10 = pct(&mut spatial.clone(), 0.10);
        let p50 = pct(&mut spatial.clone(), 0.50);
        assert!(p50 >= 4.0, "median surface relief is only {p50:.1}px - the tank looks flat");
        assert!(p10 >= 1.5, "even the calmest tenth must show relief, got {p10:.1}px");
        assert!(
            p50 <= 30.0,
            "median surface relief {p50:.1}px is larger than the tank can show without clipping"
        );

        // ---- motion: how far each column's own top edge actually travels
        let mut travel: Vec<f32> = Vec::new();
        for i in 0..iw as usize {
            if is_cone[i] {
                continue;
            }
            let (mut lo, mut hi) = (i32::MAX, i32::MIN);
            for row in &body {
                lo = lo.min(row[i]);
                hi = hi.max(row[i]);
            }
            travel.push((hi - lo) as f32);
        }
        assert!(travel.len() > 100, "only {} columns measured", travel.len());
        let t50 = pct(&mut travel.clone(), 0.50);
        let moving = travel.iter().filter(|&&v| v >= 2.0).count();
        assert!(
            t50 >= 4.0,
            "the median column's top edge moves only {t50:.1}px over the whole 8 seconds - the \
             liquid is drawn with relief but it is not RESPONDING"
        );
        assert!(
            moving * 10 >= travel.len() * 9,
            "only {moving} of {} non-cone columns move at all; the water away from the drivers is \
             static, so either the waves are not propagating or the body is not following the field",
            travel.len()
        );
    }

    #[test]
    fn the_depth_ramp_is_anchored_to_the_tank_and_not_to_the_moving_surface() {
        // The second mutant that shipped undetected: re-anchoring the depth ramp to the surface -
        // so `t` is measured down from wherever the water happens to be rather than from the rest
        // line - changes 34.9-63.0% of channels and broke no test.
        //
        // It is also the wrong physics, which is why the ramp is worth pinning rather than merely
        // covering. Depth shading stands for how much liquid the light travelled through to reach
        // that point, and that is a property of the TANK: a point near the floor is deep whatever
        // the surface above it is doing. Anchored to the surface instead, a crest passing overhead
        // repaints the entire column beneath it, and the whole body pulses in brightness with the
        // music - the exact "merely brightening" failure `the_drawn_cone_actually_moves` guards
        // against, one layer down.
        //
        // Read at a row deep enough that it is liquid in EVERY frame, so the only thing that could
        // change its colour is the anchoring.
        let mut t = builtin::fluid_deep();
        t.fluid.body_top = "#ffffff".into();
        t.fluid.body_deep = "#000000".into();
        // Everything that legitimately varies a body pixel frame to frame, off: the specular band
        // tracks the surface by design, caustics move, and a droplet passes in front.
        t.fluid.sheen = 0.0;
        t.fluid.caustics = false;
        t.fluid.iridescence = 0.0;
        t.fluid.emissive = 0.0;
        t.fluid.droplets = 0;
        // Fires from the floor on every bass hit, so it repaints exactly the deep pixel this test
        // samples. A legitimate cause of variation, and therefore one this test has to exclude for
        // its premise ("only the anchoring could have changed this pixel") to hold.
        t.fluid.underglow = 0.0;
        let mut fam = Fluid::default();
        let mut c = Canvas::new(190, 60);
        let frames = real_music();
        let (iy, ih) = (2, 60 - 4);
        let floor_y = iy + ih - 1;
        let is_cone = masked_columns(190 - 2);
        // Three rows above the floor: below the deepest trough and above the tank floor itself.
        let probe_y = floor_y - 3;
        let probe_xs: Vec<i32> =
            (0..190 - 2).filter(|&i| !is_cone[i as usize]).map(|i| 1 + i).step_by(37).collect();
        let mut seen: Vec<Option<(u8, u8, u8)>> = vec![None; probe_xs.len()];
        let mut loud_span = 0.0f32;
        for (k, row) in frames.iter().take(300).enumerate() {
            fam.draw(&mut c, &t, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
            let hi = fam.cur.iter().copied().fold(f32::MIN, f32::max);
            let lo = fam.cur.iter().copied().fold(f32::MAX, f32::min);
            loud_span = loud_span.max(hi - lo);
            for (j, &x) in probe_xs.iter().enumerate() {
                let p = c.get(x, probe_y);
                let now = (p.r, p.g, p.b);
                match seen[j] {
                    None => seen[j] = Some(now),
                    Some(first) => assert_eq!(
                        first,
                        now,
                        "the deep pixel at column {x} changed from {first:?} to {now:?} on frame \
                         {k}; the depth ramp is following the surface instead of the tank"
                    ),
                }
            }
        }
        // The premise: the surface really did move while those pixels held still. Without this the
        // test would pass on a dead simulation.
        assert!(
            loud_span > 0.30,
            "the field only spanned {loud_span:.3} over 300 frames, so holding a deep pixel \
             constant proves nothing"
        );
    }

    #[test]
    fn a_non_finite_damping_or_wave_speed_from_toml_cannot_poison_the_integrator() {
        // Review called the `is_finite` guard inside `substep` dead code - "deleting it changes 0
        // pixels and fails 0 of 400 tests". The second half was true and the conclusion was not:
        // no test REACHED it, which is a missing test rather than an unreachable branch.
        //
        // The route in is TOML. `wave_speed` and `damping` arrive from a colourway file unclamped,
        // `draw` clamps them - and `f32::clamp` returns NaN for a NaN input, a trap this codebase
        // has already been caught by twice and which `substeps` documents directly above. TOML has
        // literal `nan` and `inf`, so `damping = nan` in a colourway file reaches that line. It is
        // the last guard before the value is stored as simulation state, where it would spread to
        // every column within a few frames and stay there for the life of the process.
        let frames = real_music();
        for (name, mutate) in [
            ("damping = nan", (|p: &mut crate::themes::FluidParams| p.damping = f32::NAN)
                as fn(&mut crate::themes::FluidParams)),
            ("damping = inf", |p| p.damping = f32::INFINITY),
            ("wave_speed = nan", |p| p.wave_speed = f32::NAN),
            ("wave_speed = -inf", |p| p.wave_speed = f32::NEG_INFINITY),
            ("coupling = nan", |p| p.coupling = f32::NAN),
            ("surface_gain = nan", |p| p.surface_gain = f32::NAN),
        ] {
            let mut t = builtin::fluid_deep();
            mutate(&mut t.fluid);
            let mut fam = Fluid::default();
            let mut c = Canvas::new(190, 60);
            for (k, row) in frames.iter().take(200).enumerate() {
                fam.draw(&mut c, &t, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
                for (i, v) in fam.cur.iter().enumerate() {
                    assert!(
                        v.is_finite(),
                        "{name}: field column {i} became {v} on frame {k}"
                    );
                    assert!(
                        v.abs() <= H_LIMIT + 1e-3,
                        "{name}: field column {i} reached {v} on frame {k}, past H_LIMIT"
                    );
                }
            }
            // And the frame still draws: a poisoned parameter must degrade to a flat tank, not to
            // an empty panel.
            let painted = (0..60 * 190).filter(|k| c.get(k % 190, k / 190).a > 0).count();
            assert!(painted > 190 * 40, "{name}: only {painted} pixels painted");
        }
    }

    /// Prints the drawn body height per column, which is how the two exclusions in
    /// `masked_columns` were identified rather than guessed. Run with
    /// `cargo test --release probe_body_profile -- --ignored --nocapture`.
    /// Per-colourway look numbers: how much relief the surface shows, how far it travels, how fine
    /// the chop is, and how strongly the body reads against the panel behind it.
    /// Run: cargo test --release probe_look -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_look() {
        fn lum(p: crate::render::canvas::Rgba) -> f32 {
            0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32
        }
        let frames = real_music();
        println!("{:<16} {:>7} {:>8} {:>7} {:>9} {:>9}", "colourway", "relief", "travel", "chop", "body_lum", "vs_panel");
        for t in builtin::all().into_iter().filter(|t| t.family == "fluid") {
            let body = drawn_body(&t, &frames, 190, 60);
            let is_m = masked_columns(188);
            let mut sp: Vec<f32> = body.iter().map(|r| {
                let (mut lo, mut hi) = (i32::MAX, i32::MIN);
                for (i, &v) in r.iter().enumerate() { if !is_m[i] { lo = lo.min(v); hi = hi.max(v); } }
                (hi - lo) as f32
            }).collect();
            let mut tv: Vec<f32> = (0..188usize).filter(|&i| !is_m[i]).map(|i| {
                let (mut lo, mut hi) = (i32::MAX, i32::MIN);
                for r in &body { lo = lo.min(r[i]); hi = hi.max(r[i]); }
                (hi - lo) as f32
            }).collect();
            // Chop: mean absolute column-to-column step in the drawn top edge, late in the run.
            let late = &body[body.len() - 1];
            let cols: Vec<i32> = (0..188usize).filter(|&i| !is_m[i]).map(|i| late[i]).collect();
            let chop: f32 = cols.windows(2).map(|w| (w[1] - w[0]).abs() as f32).sum::<f32>()
                / (cols.len() - 1) as f32;
            // Body luminance against the panel above it, both as drawn.
            let mut fam = Fluid::default();
            let mut c = Canvas::new(190, 60);
            for (k, row) in frames.iter().take(420).enumerate() {
                fam.draw(&mut c, &t, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
            }
            let bl = lum(c.get(95, 50));
            let pl = lum(c.get(95, 6));
            // The SAME metric `every_fluid_colourway_renders_and_they_differ_structurally` asserts
            // on, so the ladder can be tuned in one pass instead of one failing pair at a time.
            let tr = drive(&t, &frames, 190, 60);
            println!("  test-relief {:<16} {:>6.2}  gain={:.1}", t.id, pct(&mut tr.range.clone(), 0.50), t.fluid.surface_gain);
            println!("{:<16} {:>7.1} {:>8.1} {:>7.2} {:>9.1} {:>9.2}", t.id, pct(&mut sp, 0.50),
                     pct(&mut tv, 0.50), chop, bl, (bl + 5.0) / (pl + 5.0));
        }
    }

    #[test]
    #[ignore]
    fn probe_body_profile() {
        let frames = real_music();
        let t = builtin::fluid_deep();
        let body = drawn_body(&t, &frames, 190, 60);
        let is_cone = masked_columns(188);
        let row = &body[150];
        eprintln!("cone cols: {:?}", (0..188).filter(|&i| is_cone[i as usize]).collect::<Vec<_>>());
        for chunk in 0..(188 / 20 + 1) {
            let lo = chunk * 20;
            let hi = (lo + 20).min(188);
            if lo >= hi { break; }
            eprintln!("{lo:3}..{hi:3}: {:?}", &row[lo..hi]);
        }
    }

    #[test]
    fn the_underglow_actually_brightens_the_tank_on_a_bass_hit_and_then_fades() {
        // Guards a feature that is easy to ship inert: an envelope that never rises, or a strength
        // that is never read. Verified against both - pinning `self.glow` so it never rises fails this
        // test, and so does switching the colourway's `underglow` off. It also
        // pins the SHAPE the brief asked for - "more glow than flash" - by requiring the decay to
        // still be visible several frames later, which a few-frame strike would fail.
        let frames = real_music();
        let mut t = builtin::fluid_deep();
        // Everything else that varies a deep pixel, off, so the only thing moving is the underglow.
        t.fluid.caustics = false;
        t.fluid.droplets = 0;
        t.fluid.sheen = 0.0;
        t.fluid.emissive = 0.0;
        assert!(t.fluid.underglow > 0.0, "fluid_deep must ship with the underglow on");

        let lum = |p: crate::render::canvas::Rgba| {
            0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32
        };
        let mut fam = Fluid::default();
        let mut c = Canvas::new(190, 60);
        // A column away from both cone mouths, three rows off the floor - inside the glow band.
        let (px, py) = (20, 53);
        let mut series = Vec::new();
        for (k, row) in frames.iter().take(400).enumerate() {
            fam.draw(&mut c, &t, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
            series.push((fam.glow, lum(c.get(px, py))));
        }

        let dark = series.iter().filter(|(g, _)| *g < 0.05).map(|(_, l)| *l).fold(f32::MAX, f32::min);
        let bright = series.iter().filter(|(g, _)| *g > 0.9).map(|(_, l)| *l).fold(f32::MIN, f32::max);
        assert!(bright > f32::MIN, "the underglow envelope never reached full on real music");
        assert!(dark < f32::MAX, "the underglow envelope never returned to rest on real music");
        assert!(
            bright > dark + 12.0,
            "the underglow only moved the tank from {dark:.1} to {bright:.1} luminance, which is not a visible glow"
        );
        // HONEST LIMIT OF THIS TEST: it does NOT detect the glow being drawn on the opaque canvas
        // instead of the bloomed light layer. That mutation was tried and this test still passed.
        // The reason is legitimate rather than a hole worth papering over: this family's bloom
        // radius is deliberately small (see `fluid_base`, which keeps it tight so the halo does not
        // swallow the 1px meniscus) - far too small to carry the floor band's light up across ~25
        // rows of liquid into the air. So for THIS element the light layer buys softness, not
        // reach, and the two routings produce nearly identical pixels. Light drawn nearer the
        // surface would need its own coverage.

        // Glow rather than flash: from a peak, the envelope must still be meaningfully lit well
        // after the few frames a vaporwave-style strike would last.
        let peak = series.iter().position(|(g, _)| *g > 0.98).expect("no peak");
        let later = (peak + 12).min(series.len() - 1);
        assert!(
            series[later].0 > 0.55,
            "12 frames after a hit the envelope is already down to {:.2}; that is a flash, and the brief asked for a glow",
            series[later].0
        );
    }

    #[test]
    fn a_pantone_plate_change_never_jumps_in_a_single_frame() {
        // The reported defect, verbatim: "I dont like the random switching ... not the hard jolting
        // it currently is". Ink quantisation snapped the hue, so a plate change was a one-frame jump
        // between two fully saturated process colours.
        //
        // This is the objective form of that complaint: sample the body and require that no single
        // frame moves its colour far. It is killed by setting `ink_morph` back to 0 - the snap
        // produces a single frame delta of well over 100 - so it cannot pass against the old
        // behaviour.
        let frames = real_music();
        let mut t = builtin::fluid_pantone();
        assert!(t.ink_morph > 0.0, "fluid_pantone must ship with the morph on");
        // The underglow has a deliberately INSTANT attack, so it steps the body brightness on every
        // hit. That is the requested behaviour and a different axis from hue morphing; off here so
        // this test measures only the plate change.
        t.fluid.underglow = 0.0;
        t.fluid.caustics = false;
        t.fluid.droplets = 0;

        let mut fam = Fluid::default();
        let mut c = Canvas::new(190, 60);
        let (px, py) = (20, 50);
        let mut worst = 0.0f32;
        let mut prev: Option<(i32, i32, i32)> = None;
        // Long enough to cross at least one plate change: at rainbow 0.05 with three inks a change
        // comes every ~6.7s, and the fixture is 8s, so it is walked twice over two passes.
        for pass in 0..2 {
            for (k, row) in frames.iter().enumerate() {
                let time = (pass * frames.len() + k) as f32 * FIXTURE_DT_MS / 1000.0;
                fam.draw(&mut c, &t, &fixture_frame(row, time));
                let p = c.get(px, py);
                let now = (p.r as i32, p.g as i32, p.b as i32);
                if let Some(q) = prev {
                    let d = (((now.0 - q.0).pow(2) + (now.1 - q.1).pow(2) + (now.2 - q.2).pow(2))
                        as f32)
                        .sqrt();
                    worst = worst.max(d);
                }
                prev = Some(now);
            }
        }
        assert!(
            worst < 40.0,
            "the body colour jumps {worst:.1} in a single frame, which is the hard plate change the morph is supposed to have replaced"
        );
    }

    #[test]
    fn the_crests_stay_inside_the_tank_on_real_music() {
        // The cost of a piston that injects net volume: the whole surface rides UP with loudness
        // (see the note on the forcing term), so an over-generous `surface_gain` would push the
        // crests into the renderer's clamp and the surface would go visibly flat-topped. Measured
        // over the fixture at 190x60 the clamp is reached on 0.00% of column-frames for four
        // colourways and 0.17% for the deliberately violent coolant.
        let frames = real_music();
        for t in builtin::all().into_iter().filter(|t| t.family == "fluid") {
            let tr = drive(&t, &frames, 190, 60);
            assert!(
                tr.clamped < 0.01,
                "{}: {:.2}% of column-frames hit the top of the tank, so the surface renders \
                 flat-topped - reduce surface_gain or raise the surface line",
                t.id,
                tr.clamped * 100.0
            );
        }
    }

    #[test]
    fn droplets_fly_on_real_music_and_the_viscous_colourway_throws_none() {
        // `droplets` and `droplet_v` are the two fields most likely to end up inert, because
        // nothing else in the scene depends on them. Measured over the fixture at 190x60: deep
        // water peaks at 8 droplets in flight, mercury 6, oil 13, coolant 9 - and ink, which is too
        // viscous to throw any, exactly 0.
        let frames = real_music();
        for t in builtin::all().into_iter().filter(|t| t.family == "fluid") {
            let tr = drive(&t, &frames, 190, 60);
            if t.fluid.droplets > 0 {
                assert!(
                    tr.drops_peak >= 3,
                    "{} sets droplets = {} but only {} were ever in flight",
                    t.id,
                    t.fluid.droplets,
                    tr.drops_peak
                );
                assert!(tr.drops_peak <= MAX_DROPS, "{}: droplets unbounded", t.id);
            } else {
                assert_eq!(tr.drops_peak, 0, "{} disables droplets but threw some anyway", t.id);
            }
        }
    }

    #[test]
    fn the_droplets_fire_at_a_musical_rate_on_real_music_where_a_bass_rise_could_not() {
        // Two claims, and the second is why the detector is spectral flux.
        //
        // First: the rate is musical. Swept on this fixture at a 200ms refractory the flux
        // detector gives 3.25/s at ratio 2.0, 1.75/s at 2.8 and 1.50/s at 3.2; the shipped 3.0
        // lands in between. The band asserted is wide on purpose - the point is that it fires like
        // a listener would expect, not that it hits an exact number.
        //
        // Second: the obvious alternative CANNOT fire. The largest single-frame rise in the bass
        // mean anywhere in these 8 seconds is 0.140, which is below the 0.157 threshold the
        // vaporwave lightning compared against - so that trigger fired zero times on real music.
        let frames = real_music();
        let tr = drive(&builtin::fluid_deep(), &frames, 190, 60);
        let seconds = frames.len() as f32 * FIXTURE_DT_MS / 1000.0;
        // The rate claimed in the name is the rate DROPLETS fire at, so that is what is measured.
        // Asserting it via `tr.transients` was vacuous: the detector fires whether or not anything
        // is thrown, so the whole test passed with `spawn_drops` stubbed out to an immediate
        // `return`. `drop_volleys` counts frames on which the droplet list actually grew.
        let per_sec = tr.drop_volleys as f32 / seconds;
        assert!(
            tr.drops_peak > 0,
            "no droplet was ever in flight, so there is no droplet rate to assert"
        );
        assert!(
            tr.transients > 0 && tr.drop_volleys > 0,
            "detector fired {} times and threw {} volleys - both must be non-zero for the rate \
             below to mean anything",
            tr.transients,
            tr.drop_volleys
        );
        assert!(
            (0.7..=3.2).contains(&per_sec),
            "droplets fired {} volleys over {seconds:.1}s = {per_sec:.2}/s, outside the rate a \
             listener would call musical",
            tr.drop_volleys
        );

        let bass_mean = |r: &Vec<f32>| r[..BASS_BANDS].iter().sum::<f32>() / BASS_BANDS as f32;
        let mut worst_rise = 0.0f32;
        let mut would_fire = 0u32;
        for pair in frames.windows(2) {
            let rise = bass_mean(&pair[1]) - bass_mean(&pair[0]);
            worst_rise = worst_rise.max(rise);
            if rise > 0.157 {
                would_fire += 1;
            }
        }
        // Measured: the largest 6-band bass-mean rise anywhere in these 8 seconds is 0.159, and
        // exactly ONE frame clears 0.157. Over the 4 bands the vaporwave family averaged, the
        // largest rise is 0.140 and NOTHING clears it - which is why that lightning never fired.
        assert!(worst_rise < 0.25, "bass-mean rises now reach {worst_rise:.3}, so the note above is stale");
        assert!(
            would_fire <= 2,
            "a bass-rise trigger fires {would_fire} times in 8s here; if it has become usable, the \
             reason this family uses spectral flux needs restating"
        );
        assert!(
            tr.transients >= would_fire * 4 + 4,
            "flux fired {} times where a bass-rise trigger fired {would_fire} - flux must be the \
             clearly better detector on this material, or the choice is unjustified",
            tr.transients
        );
    }

    #[test]
    fn a_wave_launched_by_one_cone_takes_time_to_reach_the_middle() {
        // Propagation, measured as a DELAY - which is the thing that separates a simulation from a
        // per-column response curve. The middle of the tank must stay still for a while after the
        // left cone starts, and only then begin to move. A family that merely drew a mound over
        // each cone would move the middle either instantly or never.
        let mut t = builtin::fluid_deep();
        // Droplets off: a splash would seed the middle independently of the wave under test.
        t.fluid.droplets = 0;
        let mut fam = Fluid::default();
        let mut c = Canvas::new(190, 60);
        let mut d = FrameData::default();
        // Left channel only, and every band at zero so the SHARED bass term cannot drive the right
        // cone as well. This is the pathological case for the family and worth having on record.
        d.rms_l = 0.55;
        d.rms_r = 0.0;
        d.dt_ms = NOMINAL_DT_MS;
        let mid = (190 - 2) / 2;
        let mut first_move = None;
        for k in 0..120 {
            d.time_s = k as f32 * NOMINAL_DT_MS / 1000.0;
            fam.draw(&mut c, &t, &d);
            if first_move.is_none() && fam.cur[mid as usize].abs() > 0.02 {
                first_move = Some(k);
            }
        }
        let k = first_move.expect("the wave must reach the middle of the tank at all");
        // Geometry: the left cone's mouth ends ~50px short of the middle, and the wave travels at
        // 120 px/s, so about 0.4s = 25 frames. The assertion is a floor, not the exact latency:
        // what must be true is that it arrives LATE, because instant arrival means the field is
        // not carrying anything.
        assert!(
            (6..90).contains(&k),
            "the middle first moved on frame {k}; under 6 is a global response rather than a \
             travelling wave, over 90 means the wave never really gets there"
        );
        assert!(fam.cur[mid as usize].abs() > 0.02, "and it must still be moving at the end");
    }

    #[test]
    fn the_two_wavetrains_interfere_in_the_middle() {
        // The family's signature, and the claim is the strong one: ADDING the second cone can make
        // the middle of the tank move LESS than the left cone alone made it move. That is only
        // possible if two wavetrains with opposite sign are meeting there, which is interference -
        // no amount of per-column response shaping can produce it.
        //
        // Measured on the AC part of the middle column's trace (each trace has its own mean
        // removed), because both cones also raise the whole surface, and that common lift is not
        // what is being asked about.
        // The REFERENCE colourway, deliberately. Mercury is near-lossless (damping 0.9992), so its
        // tank rings at its own eigenfrequencies for the whole run and a single-frequency probe
        // lands on whatever standing pattern those reflections have already built - measured, its
        // antiphase correlation at this drive frequency comes out at -0.33 rather than -1.00, which
        // says more about the resonance than about the mechanism. Water loses enough energy per
        // trip to make the measurement clean.
        let mut t = builtin::fluid_deep();
        t.fluid.droplets = 0; // keep the cones the only sources
        // The drive stays inside the LINEAR part of the response window (0.17..0.37 against a
        // window of 0.14..0.42). That matters: driven from 0 to full scale the response clamps at
        // both ends, which half-wave-rectifies each channel, and two rectified antiphase sines do
        // not sum to a constant - the nonlinearity would masquerade as a failure to cancel.
        let trace_at_middle = |l: f32, r: f32, phase: f32| -> Vec<f32> {
            let mut fam = Fluid::default();
            let mut c = Canvas::new(190, 60);
            let mut trace = Vec::new();
            for k in 0..600 {
                let mut d = FrameData::default();
                let w = k as f32 * 0.08;
                d.rms_l = if l > 0.0 { 0.27 + l * w.sin() } else { 0.0 };
                d.rms_r = if r > 0.0 { 0.27 + r * (w + phase).sin() } else { 0.0 };
                d.dt_ms = NOMINAL_DT_MS;
                d.time_s = k as f32 * NOMINAL_DT_MS / 1000.0;
                fam.draw(&mut c, &t, &d);
                // The first 200 frames are the tank filling from flat; only the settled part is
                // measured, or the start-up transient dominates every statistic below.
                if k >= 200 {
                    trace.push(fam.cur[fam.cur.len() / 2]);
                }
            }
            trace
        };
        // Each trace also carries the common lift both cones give the whole surface, and in a
        // near-lossless liquid that lift keeps CREEPING upward for the whole run. Removing the
        // global mean is not enough - the residual ramp is far larger than the oscillation and it
        // has the same sign in both runs, which reads as a correlation of +1 and hides the very
        // thing being measured (it measured 0.09 before this was a moving average). So the trace is
        // high-passed against a moving average one drive period wide, which removes anything slower
        // than the drive and nothing at the drive frequency itself.
        let period = (std::f32::consts::TAU / 0.08).round() as usize; // 79 frames
        let ac = |v: &[f32]| -> Vec<f32> {
            let half = period / 2;
            (half..v.len() - half)
                .map(|i| v[i] - v[i - half..=i + half].iter().sum::<f32>() / (half * 2 + 1) as f32)
                .collect()
        };
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        let corr = |a: &[f32], b: &[f32]| -> f32 {
            let n = a.len().min(b.len());
            let num: f32 = (0..n).map(|i| a[i] * b[i]).sum();
            num / (rms(&a[..n]) * rms(&b[..n]) * n as f32).max(1e-9)
        };

        let amp = 0.10;
        let left = ac(&trace_at_middle(amp, 0.0, 0.0));
        let right_in = ac(&trace_at_middle(0.0, amp, 0.0));
        let right_anti = ac(&trace_at_middle(0.0, amp, std::f32::consts::PI));
        let both_in = ac(&trace_at_middle(amp, amp, 0.0));
        let both_anti = ac(&trace_at_middle(amp, amp, std::f32::consts::PI));

        let one = rms(&left);
        assert!(one > 0.005, "a single cone must move the middle at all, got {one:.4}");

        // The two wavetrains ARRIVE with a definite phase relationship - that is the mechanism.
        let c_in = corr(&left, &right_in);
        let c_anti = corr(&left, &right_anti);
        assert!(
            c_in > 0.9,
            "driven in phase, the two cones' contributions at the middle must arrive in step \
             (correlation {c_in:.2})"
        );
        assert!(
            c_anti < -0.8,
            "driven in ANTIPHASE they must arrive in opposition, or there is nothing to cancel \
             (correlation {c_anti:.2})"
        );

        // ...and the consequence, which is the visible part.
        let i_in = rms(&both_in);
        let i_anti = rms(&both_anti);
        assert!(
            i_in > one * 1.7,
            "two cones in phase must REINFORCE at the middle: {i_in:.4} against {one:.4} from one \
             cone alone"
        );
        assert!(
            i_anti < one * 0.3,
            "two cones in antiphase must CANCEL at the middle - adding the second source has to \
             make the water move LESS there, which no per-column response curve can do: \
             {i_anti:.4} against {one:.4} from one cone alone"
        );
        assert!(
            i_in > i_anti * 5.0,
            "the same total drive must produce very different motion at the middle depending only \
             on relative phase: in-phase {i_in:.4}, antiphase {i_anti:.4}"
        );
    }

    // ---------------------------------------------------------------------------------------
    // Stability
    // ---------------------------------------------------------------------------------------

    #[test]
    fn the_courant_number_is_inside_the_stability_limit_and_above_it_the_scheme_really_diverges() {
        // Demonstrates the constant rather than asserting it. Same impulse, same damping, two
        // Courant numbers: at the 0.5 this family runs the field never exceeds the impulse that
        // started it, and at 1.2 it runs away until the H_LIMIT clamp catches it.
        assert!(COURANT < 1.0, "the scheme is only stable below a Courant number of 1");
        let quiet = [(0usize, 0usize, 0.0f32), (0, 0, 0.0)];
        let peak_after = |c2: f32| -> f32 {
            let mut fam = Fluid::default();
            fam.cur = vec![0.0; 190];
            fam.prev = vec![0.0; 190];
            // A displacement AT REST: cur == prev, so the initial velocity is zero. Setting only
            // `cur` would give the column a velocity of 0.5 as well, and it would then legitimately
            // overshoot 0.5 on momentum alone - which looks exactly like instability and is not.
            fam.cur[95] = 0.5;
            fam.prev[95] = 0.5;
            let mut worst = 0.0f32;
            for _ in 0..2000 {
                fam.substep(c2, 0.999, &quiet, 0.2);
                for v in &fam.cur {
                    worst = worst.max(v.abs());
                }
            }
            worst
        };
        let stable = peak_after(C2);
        assert!(
            stable <= 0.5 + 1e-3,
            "at Courant {COURANT} a 0.5 impulse grew to {stable} - the scheme is not stable"
        );
        let unstable = peak_after(1.2 * 1.2);
        assert!(
            unstable >= H_LIMIT * 0.99,
            "at Courant 1.2 the field should have run away to the clamp, but only reached \
             {unstable} - if the scheme no longer diverges there, the stability story above is wrong"
        );
    }

    #[test]
    fn a_pathological_frame_interval_cannot_hand_the_integrator_a_big_step() {
        // The specific hazard: `dt_ms` is the MEASURED frame interval and varies with load, so a
        // constant tuned at 60fps can go unstable on a slow frame. The resolution is that dt only
        // ever decides how MANY fixed sub-steps run, capped - which is what this asserts directly.
        for dt in [0.0f32, -5.0, 1.0, 16.7, 33.0, 250.0, 5000.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for speed in [0.25f32, 1.0, 3.0] {
                let mut debt = 0.0f32;
                let steps = Fluid::substeps(&mut debt, dt, speed);
                assert!(
                    (0..=MAX_SUBSTEPS).contains(&steps),
                    "dt {dt} at speed {speed} asked for {steps} sub-steps"
                );
                assert!(debt.is_finite() && debt >= 0.0, "dt {dt} poisoned the debt: {debt}");
            }
        }
        // And the debt must make the sub-steps add up to real time rather than to frame count:
        // two 8ms frames must advance the water as far as one 16ms frame.
        let mut one = 0.0f32;
        let a = Fluid::substeps(&mut one, 16.7, 1.0);
        let mut two = 0.0f32;
        let b = Fluid::substeps(&mut two, 8.35, 1.0) + Fluid::substeps(&mut two, 8.35, 1.0);
        assert_eq!(a, b, "the sub-step count must be paced by time, not by the frame");
    }

    #[test]
    fn an_extreme_impulse_and_a_pathological_dt_leave_the_field_finite_and_bounded() {
        // Hundreds of frames of the worst input the loop can produce: full-scale audio slamming to
        // silence, frame intervals from zero to five seconds, NaN and both infinities. The field is
        // persistent state, so a single divergence would corrupt every later frame forever.
        let dts = [16.7f32, 0.0, 5000.0, f32::NAN, 1.0, 250.0, f32::INFINITY, -9.0, 33.0];
        for theme in builtin::all().into_iter().filter(|t| t.family == "fluid") {
            let mut fam = Fluid::default();
            let mut c = Canvas::new(190, 60);
            for k in 0..500 {
                let mut d = FrameData::default();
                let loud = k % 7 < 2;
                for (i, v) in d.levels.iter_mut().enumerate() {
                    *v = if loud { 1.0 } else { 0.0 };
                    if i == 13 && k % 11 == 0 {
                        *v = f32::NAN;
                    }
                }
                d.peaks = d.levels;
                d.rms_l = if loud { 1.0 } else { 0.0 };
                d.rms_r = if k % 5 == 0 { f32::INFINITY } else { 0.9 };
                d.dt_ms = dts[k % dts.len()];
                d.time_s = k as f32 * 0.0167;
                fam.draw(&mut c, &theme, &d);
                for (i, v) in fam.cur.iter().enumerate() {
                    assert!(
                        v.is_finite() && v.abs() <= H_LIMIT + 1e-3,
                        "{}: column {i} reached {v} on frame {k} (dt {})",
                        theme.id,
                        d.dt_ms
                    );
                }
                assert!(fam.exc[0].is_finite() && fam.exc[1].is_finite(), "excursion poisoned");
                assert!(fam.debt.is_finite() && fam.debt >= 0.0, "debt poisoned: {}", fam.debt);
                assert!(fam.drops.len() <= MAX_DROPS, "droplets unbounded: {}", fam.drops.len());
                for p in &fam.drops {
                    assert!(p.x.is_finite() && p.y.is_finite(), "a droplet went non-finite");
                }
            }
        }
    }

    #[test]
    fn every_frame_data_field_this_family_reads_survives_nan_and_infinity() {
        // levels, peaks, rms_l, rms_r, dt_ms and time_s are the fields read here; each is poisoned
        // on its own so a guard that only happens to cover one cannot hide a missing one.
        let t = builtin::fluid_oil();
        for spoil in 0..7 {
            let mut fam = Fluid::default();
            let mut c = Canvas::new(190, 60);
            for _ in 0..8 {
                let mut d = FrameData::default();
                for (i, v) in d.levels.iter_mut().enumerate() {
                    *v = 0.2 + 0.3 * ((i % 5) as f32 / 4.0);
                }
                d.peaks = d.levels;
                d.rms_l = 0.30;
                d.rms_r = 0.24;
                match spoil {
                    0 => d.levels[0] = f32::NAN,
                    1 => d.levels[3] = f32::INFINITY,
                    2 => d.rms_l = f32::NAN,
                    3 => d.rms_r = f32::NEG_INFINITY,
                    4 => d.dt_ms = f32::NAN,
                    5 => d.time_s = f32::INFINITY,
                    _ => {
                        d.levels = [f32::NAN; crate::dsp::bands::NUM_BANDS];
                        d.rms_l = f32::NAN;
                        d.rms_r = f32::NAN;
                        d.dt_ms = f32::NAN;
                        d.time_s = f32::NAN;
                    }
                }
                fam.draw(&mut c, &t, &d);
            }
            assert!(
                fam.cur.iter().all(|v| v.is_finite()),
                "poison case {spoil} corrupted the height field"
            );
            assert!(fam.exc.iter().all(|v| v.is_finite()), "poison case {spoil} corrupted a cone");
        }
    }

    // ---------------------------------------------------------------------------------------
    // Geometry and colourways
    // ---------------------------------------------------------------------------------------

    #[test]
    fn renders_at_every_plausible_size_including_degenerate_ones() {
        let t = builtin::fluid_coolant();
        for (w, h) in [(190, 60), (380, 60), (456, 60), (240, 72), (96, 40), (40, 24), (12, 12), (1, 1), (0, 0)] {
            let mut fam = Fluid::default();
            let mut c = Canvas::new(w, h);
            for k in 0..12 {
                let mut d = FrameData::default();
                for (i, v) in d.levels.iter_mut().enumerate() {
                    *v = 0.15 + 0.6 * (((i + k) % 9) as f32 / 8.0);
                }
                d.peaks = d.levels;
                d.rms_l = 0.38;
                d.rms_r = 0.22;
                fam.draw(&mut c, &t, &d);
            }
            assert_eq!(
                c.bits().len(),
                (w.max(0) * h.max(0)) as usize,
                "{w}x{h} changed the canvas size"
            );
        }
    }

    #[test]
    fn an_uneven_spectrum_and_a_hard_pan_are_both_visible_in_the_cones() {
        // A jagged spectrum must not average away, and a hard pan must be legible as POSITION:
        // the driven cone's diaphragm has to sit visibly higher than the idle one.
        let t = builtin::fluid_deep();
        let mut fam = Fluid::default();
        let mut c = Canvas::new(190, 60);
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = if i % 6 < 2 { 0.75 } else { 0.06 };
        }
        d.peaks = d.levels;
        d.rms_l = 0.55;
        d.rms_r = 0.02;
        d.dt_ms = NOMINAL_DT_MS;
        for _ in 0..60 {
            fam.draw(&mut c, &t, &d);
        }
        assert!(
            fam.exc[0] > fam.exc[1] + 0.15,
            "a hard pan must separate the cones: left {:.2} right {:.2}",
            fam.exc[0],
            fam.exc[1]
        );
        // In pixels on the drawn diaphragm, not just in the state.
        let depth = 60 - 4 - ((60 - 4 - 1) as f32 * t.fluid.surface).round() as i32 - 2 + 1;
        let travel = (depth as f32 * t.fluid.cone_travel).max(1.0);
        let px = ((fam.exc[0] - fam.exc[1]) * travel).round();
        assert!(px >= 2.0, "the pan is worth only {px}px of cone travel, which will not read");
    }

    #[test]
    fn the_drawn_cone_actually_moves_rather_than_merely_brightening() {
        // Rule 6, asserted on PIXELS. This gap was found by mutation testing: pinning the
        // diaphragm at a fixed height while leaving its rim highlight audio-driven broke nothing,
        // because every other test here reads `exc` out of the family's own state. A cone that only
        // brightens is not a cone, so the assertion has to be made on where the ink lands.
        //
        // The rim highlight is the brightest thing inside the cone's x-span below the rest line, so
        // its row is found by scanning for the brightest row there. Measured: row 36 at an
        // excursion of 0.12 and row 33 at 0.867 - it rises 3px.
        let t = builtin::fluid_deep();
        let iw = 190 - 2;
        let hw = (((iw as f32) * 0.058).round() as i32).max(2);
        let cx = 1 + ((iw as f32) * 0.22).round() as i32;
        let rest = 2 + (55.0f32 * t.fluid.surface).round() as i32;
        let cone_base = rest + ((60 - 4 - 1 - (rest - 2)) as f32 * 0.55).round() as i32;
        let rim_row = |rms: f32| -> (i32, f32) {
            let mut fam = Fluid::default();
            let mut c = Canvas::new(190, 60);
            let mut d = FrameData::default();
            // Every band at zero, so only the per-channel RMS drives the cone.
            d.rms_l = rms;
            d.rms_r = rms;
            d.dt_ms = NOMINAL_DT_MS;
            for _ in 0..80 {
                fam.draw(&mut c, &t, &d);
            }
            let mut best = (0.0f32, cone_base);
            for y in (rest + 5)..(cone_base + 2) {
                let mut sum = 0.0f32;
                for x in (cx - hw)..=(cx + hw) {
                    let p = c.get(x, y);
                    sum += 0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32;
                }
                if sum > best.0 {
                    best = (sum, y);
                }
            }
            (best.1, fam.exc[0])
        };
        let (quiet_row, quiet_exc) = rim_row(0.16);
        let (loud_row, loud_exc) = rim_row(0.55);
        assert!(
            loud_exc > quiet_exc + 0.4,
            "test premise: the two drives must differ ({quiet_exc:.2} vs {loud_exc:.2})"
        );
        assert!(
            quiet_row - loud_row >= 2,
            "the drawn cone must rise with its channel: row {quiet_row} at excursion \
             {quiet_exc:.2}, row {loud_row} at {loud_exc:.2} - under 2px this is not a position cue"
        );
    }

    /// Mean absolute step between neighbouring OPEN columns' drawn liquid height, over a frame range.
    ///
    /// The measure cavitation's froth lives in. Smooth waves have a small step between neighbours whatever
    /// their amplitude; a boil is rough at the grid's own scale, which no amount of swell reproduces.
    ///
    /// Cone-mouth columns are excluded, and that is not optional: `drawn_body` classifies a masked column
    /// as zero liquid, so a pair straddling a mouth measures the cone's cutout. Ignoring it made an
    /// amplitude probe report 37px of peak-to-peak on a 51px interior and show no change at all when the
    /// cones let go - the metric was reading the cutouts, not the water.
    fn body_roughness(b: &[Vec<i32>], lo: usize, hi: usize) -> f32 {
        let is_cone = masked_columns(188);
        let open = |i: usize| i < is_cone.len() && !is_cone[i];
        let (mut acc, mut n) = (0.0f32, 0.0f32);
        for f in lo..hi.min(b.len()) {
            for i in 1..b[f].len() {
                if open(i) && open(i - 1) {
                    acc += (b[f][i] - b[f][i - 1]).abs() as f32;
                    n += 1.0;
                }
            }
        }
        acc / n.max(1.0)
    }

    /// Mean per-frame peak-to-peak of the drawn liquid height, over open columns only.
    fn body_amplitude(b: &[Vec<i32>], lo: usize, hi: usize) -> f32 {
        let is_cone = masked_columns(188);
        let (mut acc, mut n) = (0.0f32, 0.0f32);
        for f in lo..hi.min(b.len()) {
            let (mut mn, mut mx) = (i32::MAX, i32::MIN);
            for (i, v) in b[f].iter().enumerate() {
                if i < is_cone.len() && !is_cone[i] {
                    mn = mn.min(*v);
                    mx = mx.max(*v);
                }
            }
            if mn <= mx {
                acc += (mx - mn) as f32;
                n += 1.0;
            }
        }
        acc / n.max(1.0)
    }

    #[test]
    fn the_flourish_roughens_the_surface_and_makes_the_tank_run_slack() {
        // TWO INDEPENDENT PROPERTIES, and each one had to be earned separately.
        //
        // The froth roughens the DRAWN surface. The damping makes the tank run down. Neither injects energy
        // into the wave field, which is the whole reason this attempt works where three earlier ones could
        // not: the wave equation propagates whatever is injected, and interference then pushed crests into
        // the top of the tank at every amplitude tried - 3.8%, 3.2%, 4.0% of column-frames against a family
        // that clips on 0.00% and asserts under 1%.
        //
        // Measured on the trusted `drawn_body` classifier rather than a fresh luminance scan. My first
        // attempt built one of those and it reported 0.00 roughness for both arms while a probe confirmed
        // the froth reaching the surface - the meniscus is a single bright row over a dark gradient with
        // dimmer water beneath it, so a threshold scan finds the wrong row.
        let mut t = builtin::fluid_deep();
        t.flourish = 0.0;
        let frames = real_music();
        let calm = drawn_body(&t, &frames, 190, 60);
        let fired = drawn_body_firing(&t, &frames, 190, 60, Some(200));

        // The froth. Measured 0.360 calm against 1.301 fired over the 40 frames after the strike.
        let (r_calm, r_fired) = (body_roughness(&calm, 200, 240), body_roughness(&fired, 200, 240));
        assert!(
            r_fired > r_calm * 2.0,
            "the surface did not break up: {r_fired:.3}px of mean step between neighbours against \
             {r_calm:.3} calm"
        );

        // The slackening. Measured 9.62 calm against 3.85 fired a little later, once the damping has had
        // time to take the energy out - the tank does not go slack in one frame.
        let (a_calm, a_fired) = (body_amplitude(&calm, 240, 280), body_amplitude(&fired, 240, 280));
        assert!(
            a_fired < a_calm * 0.7,
            "the tank did not run slack: {a_fired:.2}px of peak-to-peak against {a_calm:.2} calm"
        );

        // AND THE TWO ARE INDEPENDENT. Roughness rises while amplitude falls, which no single mechanism
        // produces - a bigger swell raises both, and a calmer tank lowers both. That is what makes deleting
        // either half fail this test rather than only weakening it.
        assert!(
            r_fired > r_calm && a_fired < a_calm,
            "cavitation must roughen AND slacken: roughness {r_calm:.3}->{r_fired:.3}, \
             amplitude {a_calm:.2}->{a_fired:.2}"
        );
    }

    #[test]
    fn the_tank_fills_back_up_after_cavitation() {
        // The recovery is SLOWER than the envelope, and that is physics rather than a leak: damping removed
        // energy, so the cones have to put it back. Measured - at frames 400-460, still 34% down; by
        // 500-560 the amplitude is 5.77 against 5.63 calm, i.e. level. The envelope is 1300ms (~129 frames
        // at the fixture rate) and the refill takes about another 1.7s.
        //
        // Asserted on amplitude rather than byte equality, because a damped wave field with a different
        // history never returns bit-for-bit and the droplet seed advances independently.
        let mut t = builtin::fluid_deep();
        t.flourish = 0.0;
        let frames = real_music();
        let calm = drawn_body(&t, &frames, 190, 60);
        let fired = drawn_body_firing(&t, &frames, 190, 60, Some(200));
        let (a_calm, a_fired) = (body_amplitude(&calm, 500, 560), body_amplitude(&fired, 500, 560));
        assert!(
            a_fired > a_calm * 0.8,
            "the tank never filled back up: {a_fired:.2}px against {a_calm:.2} calm, 300 frames after a \
             129-frame flourish"
        );
        // And the surface is smooth again, not merely full.
        let (r_calm, r_fired) = (body_roughness(&calm, 500, 560), body_roughness(&fired, 500, 560));
        assert!(
            r_fired < r_calm * 1.5,
            "the surface is still frothing long after the flourish: {r_fired:.3} against {r_calm:.3} calm"
        );
    }

    /// Run: cargo test --release probe_cavitation -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_cavitation() {
        let mut t = builtin::fluid_deep();
        t.flourish = 0.0;
        let frames = real_music();
        let calm = drawn_body(&t, &frames, 190, 60);
        let fired = drawn_body_firing(&t, &frames, 190, 60, Some(200));
        for (lo, hi) in [(200usize, 240usize), (280, 330), (400, 460), (500, 560), (600, 660), (700, 780)] {
            println!(
                "  {lo}-{hi}: roughness calm {:.3} fired {:.3} | amplitude calm {:.2} fired {:.2}",
                body_roughness(&calm, lo, hi),
                body_roughness(&fired, lo, hi),
                body_amplitude(&calm, lo, hi),
                body_amplitude(&fired, lo, hi)
            );
        }
    }

    #[test]
    fn the_drawn_surface_line_follows_the_simulated_field() {
        // The other gap mutation testing found: drawing the meniscus, the glint and the caustics at
        // a FIXED row while the body still followed the field broke no test at all, because nothing
        // checked where the light landed. The surface line is the family's main read, so this walks
        // every column and compares the brightest row against the field.
        //
        // The columns OVER THE CONES are excluded, and the reason is worth recording because it was
        // first misdiagnosed as bloom spread: the cone rim highlight is the same specular colour as
        // the crest glint and sits ~13 rows below the surface, so over the 46 columns of the two
        // mouths it legitimately wins the argmax. Measured, the deviation histogram across the tank
        // is exactly two spikes - 142 columns at 0 and 46 at +13 - and those 46 are the mouths.
        let mut t = builtin::fluid_deep();
        // Droplets off: a droplet in the air above a column is as bright as the meniscus and would
        // win the argmax on whichever column it happens to be over.
        t.fluid.droplets = 0;
        // The flourish off, and this is the SECOND exemption cavitation needs in this family. Both are the
        // same kind, and it is worth naming the kind rather than exempting case by case: these two tests
        // assert properties of NORMAL operation, and a flourish is a deliberate, rare, temporary override
        // of normal operation.
        //
        // Here the property is that the drawn surface line follows the simulated field. The cavitation
        // froth is applied to the DRAWN line precisely so that it cannot propagate through the wave
        // equation or raise a crest - which means it deviates the drawn line from the field on purpose, to
        // a measured 3 rows. With the flourish enabled this test measures that exception instead of the
        // rule it exists to defend, and the deviation is asserted deliberately by
        // `the_flourish_roughens_the_surface_and_makes_the_tank_run_slack`.
        t.flourish = 0.0;
        let mut fam = Fluid::default();
        let mut c = Canvas::new(190, 60);
        let frames = real_music();
        for (k, row) in frames.iter().take(300).enumerate() {
            fam.draw(&mut c, &t, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
        }
        let (iy, ih) = (2, 56);
        let iw = 190 - 2;
        let rest = iy + ((ih - 1) as f32 * t.fluid.surface).round() as i32;
        let floor_y = iy + ih - 1;
        let gain = t.fluid.surface_gain * (ih as f32 / REF_INTERIOR_H);
        let hw = (((iw as f32) * 0.058).round() as i32).max(2);
        let cxs = [
            1 + ((iw as f32) * 0.22).round() as i32,
            1 + iw - 1 - ((iw as f32) * 0.22).round() as i32,
        ];
        let mut agreed = 0;
        let mut checked = 0;
        let mut worst = (0i32, 0i32);
        let mut expected_rows: Vec<i32> = Vec::new();
        for i in 0..fam.cur.len() {
            let x = 1 + i as i32;
            let expected = (rest - (fam.cur[i] * gain).round() as i32).clamp(iy + 1, floor_y - 1);
            expected_rows.push(expected);
            if cxs.iter().any(|cx| (x - cx).abs() <= hw + 1) {
                continue; // a cone mouth - see the note above
            }
            let mut best = (0.0f32, floor_y);
            for y in (iy + 1)..floor_y {
                let p = c.get(x, y);
                let l = 0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32;
                if l > best.0 {
                    best = (l, y);
                }
            }
            checked += 1;
            let dev = best.1 - expected;
            if dev.abs() <= 1 {
                agreed += 1;
            } else if dev.abs() > worst.0.abs() {
                worst = (dev, x);
            }
        }
        // The test must not be satisfiable by a flat surface: the field being compared against has
        // to have real relief in it.
        let spread = expected_rows.iter().max().unwrap() - expected_rows.iter().min().unwrap();
        assert!(spread >= 3, "test premise: the field must be uneven, spread is only {spread}px");
        assert!(checked > 100, "test premise: most columns must still be measured, got {checked}");
        assert!(
            agreed * 100 / checked >= 95,
            "the brightest row tracks the field on only {}% of the {checked} open-water columns - \
             the surface line is not being drawn where the water is (worst deviation {} rows at \
             x={})",
            agreed * 100 / checked,
            worst.0,
            worst.1
        );
    }

    #[test]
    fn a_transient_punches_the_water_harder_than_the_envelope_alone_could() {
        // The third mutation-testing gap: zeroing TRANSIENT_KICK broke nothing. It exists because
        // the cone envelope is deliberately rate-limited, so a snare would otherwise displace no
        // more water than a sustained pad - which is the whole reason for detecting the onset.
        //
        // Measured over the fixture at 190x60: on the 13 frames where a transient fires, the mean
        // field over the left cone's mouth jumps by a median of +0.346, against a median absolute
        // change of 0.019 (p90 0.061) on the other 779 frames.
        let frames = real_music();
        let t = builtin::fluid_deep();
        let mut fam = Fluid::default();
        let mut c = Canvas::new(190, 60);
        let mut seed = fam.seed;
        let iw = 190 - 2;
        let hw = (((iw as f32) * 0.058).round() as i32).max(2);
        let cx = 1 + ((iw as f32) * 0.22).round() as i32;
        let (lo, hi) = ((cx - hw - 1).max(0) as usize, (cx + hw) as usize);
        let mut on: Vec<f32> = Vec::new();
        let mut off: Vec<f32> = Vec::new();
        for (k, row) in frames.iter().enumerate() {
            let before = if fam.cur.is_empty() {
                0.0
            } else {
                fam.cur[lo..hi.min(fam.cur.len())].iter().sum::<f32>() / (hi - lo) as f32
            };
            fam.draw(&mut c, &t, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
            let after = fam.cur[lo..hi.min(fam.cur.len())].iter().sum::<f32>() / (hi - lo) as f32;
            if fam.seed != seed {
                seed = fam.seed;
                on.push(after - before);
            } else {
                off.push((after - before).abs());
            }
        }
        assert!(on.len() >= 5, "no transients to measure ({} of them)", on.len());
        let median_on = pct(&mut on.clone(), 0.5);
        let p90_off = pct(&mut off.clone(), 0.9);
        assert!(
            median_on > 0.15,
            "a transient lifts the cone mouth by a median of only {median_on:.3}, which is not a punch"
        );
        assert!(
            median_on > p90_off * 3.0,
            "a transient must displace far more water than an ordinary frame: {median_on:.3} \
             against {p90_off:.3} at the 90th percentile of ordinary frames"
        );
    }

    #[test]
    fn the_cone_follower_cannot_be_poisoned_by_a_non_finite_target() {
        // Defence in depth, and mutation testing showed it needs its own test: removing the
        // is_finite guard inside `follow` broke nothing, because every caller already sanitises its
        // input. That is exactly the situation in which someone deletes the guard and a later
        // refactor of `cone_drive` then poisons the excursion permanently. `clamp` does NOT filter
        // NaN - every comparison against NaN is false, so it returns NaN.
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(Fluid::follow(0.5, bad, 16.7).is_finite(), "target {bad} poisoned the follower");
            assert!(Fluid::follow(bad, 0.5, 16.7).is_finite(), "state {bad} poisoned the follower");
            assert!(Fluid::follow(0.5, 0.5, bad).is_finite(), "dt {bad} poisoned the follower");
        }
        // ...and it must still be a follower, not a passthrough.
        let one = Fluid::follow(0.0, 1.0, NOMINAL_DT_MS);
        assert!(one > 0.0 && one < 1.0, "one frame must move part of the way, got {one}");
        assert!(
            Fluid::follow(1.0, 0.0, NOMINAL_DT_MS) > 1.0 - one,
            "release must be slower than attack"
        );
    }

    #[test]
    fn every_fluid_colourway_renders_and_they_differ_structurally_not_just_in_hue() {
        // The rejection this guards is a sibling family that shipped five colourways of which
        // three rendered near-identically. Pixel inequality is too weak a test for that - two
        // colourways can differ by one hex and pass it - so the assertion is on the PHYSICS the
        // colourways are actually separated by.
        let frames = real_music();
        let mut seen: Vec<Vec<u32>> = Vec::new();
        let mut relief = std::collections::BTreeMap::new();
        let mut elements = std::collections::BTreeMap::new();
        for mut t in builtin::all().into_iter().filter(|t| t.family == "fluid") {
            // THE FLOURISH OFF, and this is the SECOND exemption cavitation has needed in this family, so
            // it deserves justifying rather than asserting.
            //
            // This test's subject is that a colourway's own physics shows through - `mercury` at damping
            // 0.9992 must ring where `ink` at 0.945 is viscous. Cavitation deliberately overrides that: it
            // damps the tank hard for 1300ms to make it run slack, which is most of what makes the effect
            // legible. With it enabled the two converged to 2.53px against a required 1.99px gap, because
            // damping compounds per sub-step and heavy damping compresses the very difference being
            // measured.
            //
            // So the exemption is not "the test is inconvenient" - it is that the test measures NORMAL
            // operation and a flourish is a deliberate, rare, temporary override of it. The same reasoning
            // exempts `the_drawn_surface_line_follows_the_simulated_field`, and the flourish's own effect on
            // both properties is asserted deliberately in
            // `the_flourish_roughens_the_surface_and_makes_the_tank_run_slack`.
            t.flourish = 0.0;
            let mut fam = Fluid::default();
            let mut c = Canvas::new(190, 60);
            for (k, row) in frames.iter().take(300).enumerate() {
                fam.draw(&mut c, &t, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
            }
            let bits = c.bits().to_vec();
            assert!(bits.iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for prior in &seen {
                assert_ne!(prior, &bits, "{} renders identically to another colourway", t.id);
            }
            seen.push(bits);
            // SURFACE RELIEF in pixels - peak to trough across the tank, median over the run. This
            // is what `damping`, `wave_speed` and `surface_gain` actually buy, and it is the number
            // that decides whether two colourways look like the same liquid.
            // The FULL fixture, not the 300 frames used for the pixel comparison above: mercury's
            // damping is 0.9992, so its standing pattern takes several seconds to ring up. Measured
            // over 300 frames it scores 9.7px against water's 9.7px, and over the whole 8 seconds
            // 12.3px against 7.3px - a short run would have reported them as the same liquid.
            let tr = drive(&t, &frames, 190, 60);
            relief.insert(t.id.clone(), pct(&mut tr.range.clone(), 0.5));
            // ...and which whole ELEMENTS each one has, which is the other axis of the split.
            let f = &t.fluid;
            elements.insert(
                t.id.clone(),
                // The last two are Theme-level rather than FluidParams, and they belong here now
                // that the BODY is tinted: ink quantisation collapses the duotone onto the process
                // set and aberration splits the channels, both of which change what is drawn and
                // not merely its hue. Left out, this tuple could not tell `fluid-pantone` from
                // `fluid-mercury` - they enable the same five elements - and it failed on exactly
                // that, which is the test doing its job on an axis it did not yet know about.
                (
                    f.droplets > 0,
                    f.caustics,
                    f.emissive > 0.0,
                    f.iridescence > 0.0,
                    f.sheen > 0.0,
                    t.inks > 0,
                    t.aberration > 0.0,
                ),
            );
        }
        assert_eq!(seen.len(), 6, "expected six fluid colourways, got {}", seen.len());

        // Measured at 190x60 over the fixture: ink 1.4px, oil 3.7px, deep 7.3px, mercury 12.3px,
        // coolant ~17px of median peak-to-trough relief. EVERY PAIR must be at least 25% apart,
        // which is the assertion that would have caught the sibling family shipping three
        // near-identical colourways - pixel inequality would not have.
        let (merc, ink) = (relief["fluid-mercury"], relief["fluid-ink"]);
        assert!(
            merc > ink * 3.0,
            "mercury (damping 0.9992, ringing) and ink (0.945, viscous) must not produce a similar \
             surface: {merc:.2}px against {ink:.2}px"
        );
        for (a, ra) in &relief {
            for (b, rb) in &relief {
                if a >= b {
                    continue;
                }
                let ratio = ra.max(*rb) / rb.min(*ra).max(1e-3);
                assert!(
                    ratio > 1.25,
                    "{a} ({ra:.2}px) and {b} ({rb:.2}px) of surface relief are within 25% of each \
                     other, so they will read as the same liquid in a different colour"
                );
            }
        }
        // Every colourway must also switch a different SET of elements on, so the difference is
        // not only amplitude. (droplets, caustics, emissive, iridescence, sheen)
        let mut sigs: Vec<_> = elements.values().collect();
        let before = sigs.len();
        sigs.sort();
        sigs.dedup();
        assert_eq!(
            sigs.len(),
            before,
            "two fluid colourways enable exactly the same set of elements: {elements:?}"
        );
    }

    /// Every `FluidParams` field, and what a meaningfully different value for it is.
    ///
    /// This is the guard against the defect that has shipped twice: a theme field documented at
    /// length that no drawing code ever reads (the vaporwave auto-ranger, the Pantone ink
    /// quantisation). A grep proves a field is MENTIONED; this proves it changes pixels.
    fn param_probes() -> Vec<(&'static str, fn(&mut crate::themes::FluidParams))> {
        vec![
            ("surface", |p| p.surface = 0.70),
            ("body_top", |p| p.body_top = "#ff0000".into()),
            ("body_deep", |p| p.body_deep = "#00ff00".into()),
            ("film", |p| p.film = "#ffff00".into()),
            ("cone", |p| p.cone = "#ff00ff".into()),
            ("cone_dark", |p| p.cone_dark = "#0000ff".into()),
            ("wave_speed", |p| p.wave_speed = 2.4),
            ("damping", |p| p.damping = 0.90),
            ("surface_gain", |p| p.surface_gain = 14.0),
            ("cone_travel", |p| p.cone_travel = 0.40),
            ("coupling", |p| p.coupling = 0.80),
            ("droplets", |p| p.droplets = 0),
            ("droplet_v", |p| p.droplet_v = 240.0),
            ("caustics", |p| p.caustics = false),
            ("iridescence", |p| p.iridescence = 0.0),
            ("sheen", |p| p.sheen = 0.0),
            ("emissive", |p| p.emissive = 0.0),
            // Switched OFF rather than up, because the default is already 0.85 - a probe that raised
            // it could pass on a clamp alone. Off vs on is the difference that matters, and it is
            // what proves the field is read at all: this repo has twice shipped a fully documented
            // parameter that no code consumed.
            ("underglow", |p| p.underglow = 0.0),
        ]
    }

    #[test]
    fn every_fluid_theme_field_changes_what_is_drawn() {
        let frames = real_music();
        // A base with every optional element switched ON, so a field whose effect is gated by
        // another one is still reachable - `film` does nothing at zero iridescence, and testing it
        // against the reference colourway would report it as inert when it is merely unused there.
        let base = {
            let mut t = builtin::fluid_deep();
            t.fluid.iridescence = 0.6;
            t.fluid.sheen = 0.4;
            t.fluid.emissive = 0.4;
            t.fluid.caustics = true;
            t.fluid.droplets = 6;
            t
        };
        // Accumulated over every frame, not just the last one: a droplet or a transient may be
        // absent from any single frame by luck.
        let checksum = |t: &Theme| -> u64 {
            let mut fam = Fluid::default();
            let mut c = Canvas::new(190, 60);
            let mut sum = 0u64;
            for (k, row) in frames.iter().take(240).enumerate() {
                fam.draw(&mut c, t, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
                for (i, p) in c.bits().iter().enumerate() {
                    sum = sum.wrapping_add((*p as u64).wrapping_mul(i as u64 + 1));
                }
            }
            sum
        };
        let reference = checksum(&base);
        for (name, mutate) in param_probes() {
            let mut t = base.clone();
            mutate(&mut t.fluid);
            assert_ne!(
                checksum(&t),
                reference,
                "changing `{name}` changed nothing on screen, so no drawing code reads it"
            );
        }
    }

    #[test]
    fn the_liquid_body_and_the_air_above_it_are_both_fully_opaque() {
        // Rule: the overlay is composited with per-pixel alpha over the Windows weather widget, so
        // any pixel below alpha 255 inside the panel is a hole the forecast shows through. This
        // family is a per-column fill against a per-column surface line, which is exactly the shape
        // of code that leaves a one-pixel seam. Swept over levels because that seam would only open
        // at some surface heights. (The project-wide sweep in `render::opacity` covers this too;
        // this one localises the failure and reports the row.)
        let frames = real_music();
        for theme in builtin::all().into_iter().filter(|t| t.family == "fluid") {
            let mut fam = Fluid::default();
            let mut c = Canvas::new(190, 60);
            for (k, row) in frames.iter().take(200).enumerate() {
                fam.draw(&mut c, &theme, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
                for y in 6..52 {
                    for x in 6..184 {
                        assert_eq!(
                            c.get(x, y).a,
                            255,
                            "{}: hole at ({x},{y}) on frame {k}",
                            theme.id
                        );
                    }
                }
            }
        }
    }

    /// Run: cargo test --release dump_fluid_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_fluid_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let frames = real_music();
        let mut n = 0usize;
        for theme in builtin::all().into_iter().filter(|t| t.family == "fluid") {
            for (w, h) in [(190, 60), (380, 60)] {
                let mut fam = Fluid::default();
                let mut c = Canvas::new(w, h);
                // Driven by the REAL fixture, so what is judged by eye is what real music does -
                // 420 frames in, by which point the reflections have crossed the tank twice.
                for (k, row) in frames.iter().take(420).enumerate() {
                    fam.draw(&mut c, &theme, &fixture_frame(row, k as f32 * FIXTURE_DT_MS / 1000.0));
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
                let tag = if w == 190 { "" } else { "-wide" };
                std::fs::write(dir.join(format!("fluid-{}{tag}.rgba", theme.id)), &out).unwrap();
                n += 1;
            }
        }
        println!("wrote {n} fluid dumps to {}", dir.display());
    }

    /// Prints what the fixture actually does, for tuning. Not an assertion.
    /// Run: cargo test --release probe_fluid -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_fluid() {
        let frames = real_music();
        let seconds = frames.len() as f32 * FIXTURE_DT_MS / 1000.0;
        println!("{:>16} {:>7} {:>7} {:>7} {:>9} {:>9} {:>8} {:>7} {:>9}", "colourway", "exc10", "exc50", "exc90", "range50px", "mid|h|50", "tr/s", "worst", "rough50");
        for t in builtin::all().into_iter().filter(|t| t.family == "fluid") {
            let tr = drive(&t, &frames, 190, 60);
            let mut mid: Vec<f32> = tr.mid.iter().map(|v| v.abs()).collect();
            println!(
                "{:>16} {:>7.3} {:>7.3} {:>7.3} {:>9.2} {:>9.4} {:>8.2} {:>7.3} {:>9.4}",
                t.id,
                pct(&mut tr.exc_l.clone(), 0.10),
                pct(&mut tr.exc_l.clone(), 0.50),
                pct(&mut tr.exc_l.clone(), 0.90),
                pct(&mut tr.range.clone(), 0.50),
                pct(&mut mid, 0.50),
                tr.transients as f32 / seconds,
                tr.worst_field,
                pct(&mut tr.rough.clone(), 0.50),
            );
            println!(
                "                 clamped {:.4}%  drops peak {}  alive at end {}",
                tr.clamped * 100.0, tr.drops_peak, tr.drops_alive
            );
        }
    }
}

