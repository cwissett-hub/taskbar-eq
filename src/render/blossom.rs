//! The cherry blossom family: petals coming off a branch in the wind.
//!
//! Asked for as "cherry blossom in wind falling from a tree". The first family here whose subject is a
//! FIELD of many small things rather than one instrument, which changes what the design has to worry
//! about - and the thing it has to worry about most is not looking like noise.
//!
//! # What makes it a meter and not an ornament
//!
//! A drifting particle field is decorative by default. Three mappings stop it being that, and none of
//! them is brightness - `tube.rs:54-60` measured a driven element 1.46 dL* brighter than its idle
//! neighbour as INVISIBLE against a ~2.3 dL* threshold, so "the petals glow with the music" is already
//! known not to work at this size.
//!
//! - **Wind IS the level.** Petals stream faster and further across the panel as the music gets louder,
//!   and slow to a drift when it quietens. That is position, and it is the load-bearing mapping: the
//!   whole field's slope tells you the level at a glance.
//! - **A beat shakes the branch and releases a burst.** Onsets - the same shared flux detector the fluid
//!   tank and the vaporwave grid use - snap the branch and let go of a handful of petals, so the RELEASE
//!   PATTERN is the rhythm. Watching where the clumps are is watching the beat.
//! - **The bass bends the branch.** Low frequencies load it, which gives the scene a slow motion under
//!   the fast one, the way a real branch answers a gust before its petals do.
//!
//! # Why the branch exists
//!
//! Not for decoration: it is the ANCHOR. An earlier analysis in this project refused a starfield family
//! on the grounds that 150 one-pixel stars change about 1.3% of the panel and go unnoticed - and while
//! the reasoning behind that number was later corrected (magnitude does not predict noticeability; the
//! KIND of change does), the underlying worry is real for a field of small marks with nothing else in
//! frame. The branch is a solid, static, recognisable shape that tells the eye what it is looking at
//! before it has resolved a single petal, and it gives the petals somewhere to come FROM.
//!
//! # Petals tumble, and that is three masks
//!
//! A petal that slides without turning reads as a speck. Real blossom tumbles, showing its face then its
//! edge, and that flutter is most of what makes falling blossom recognisable. Each petal carries a spin
//! phase that selects one of three tiny masks - face, angled, edge - which is the same trick the dolphin
//! family uses for its tail, and for the same reason: at this size, animation frames are cheaper and
//! more legible than rotation.
//!
//! Petals also do not fall straight. Each one's horizontal drift is modulated by its own sine, at its own
//! rate, so the field never organises itself into rain.

use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Petals in the pool. Recycled rather than allocated - a petal leaving the panel is reseeded at the
/// branch, so the count is constant and there is no per-frame allocation.
const PETALS: usize = 118;

/// The level window, `vapor`'s MEASURED p10-p90 of real music - not a 0..1 mapping, which renders dead,
/// and not normalised against the frame's loudest band, which is provably inert at p50 0.819.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// Wind speed in pixels per second at silence and at full drive.
///
/// The floor is not zero: still air still has blossom drifting down, and a field that stops dead reads as
/// a freeze rather than as quiet. The ceiling is bounded by aliasing - `reel.rs` measured that motion
/// past half a feature pitch per frame appears to run BACKWARDS, and a petal is about 3px, so 90px/s
/// (1.5px per frame at 60fps) keeps the fastest petal under that.
const WIND_CALM: f32 = 14.0;
const WIND_GALE: f32 = 90.0;

/// How fast the wind follows the music, per millisecond. Slow on purpose: air has inertia, and a wind
/// that tracked every transient would read as jitter rather than as weather.
const WIND_FOLLOW: f32 = 0.004;

/// Fall speed in pixels per second, and how much of it the flutter takes away.
const FALL: f32 = 21.0;
const FLUTTER_PX: f32 = 13.0;

/// Petals released on an onset, and the trickle released per second regardless.
const BURST: usize = 7;
const TRICKLE_PER_S: f32 = 3.0;

/// The branch is a DAMPED SPRING, and an onset is an impulse into it. Peak tip travel in pixels, its
/// natural frequency, and its damping ratio.
///
/// The first version set the deflection directly - `shake = SHAKE_PX` on the firing frame, then an
/// exponential decay back to rest - and it was reported as juddering. Two faults, and the second is the
/// one that matters:
///
/// - It STEPPED to full deflection in a single frame. A step on a geometric quantity reads as a snap;
///   this project already learned that from the flourish envelope.
/// - A decay toward rest NEVER CROSSES ZERO, so the branch could not spring back. It snapped out and
///   crept home, which is the motion of something being dragged, not something elastic. Asked for as
///   "it needs to spring back naturally then get hit again" - and springing back means overshooting.
///
/// So the onset now adds VELOCITY and the spring does the rest: the branch accelerates away, decelerates,
/// comes back through rest, overshoots the other side, and settles. Nothing is ever assigned a position.
///
/// 3.4 Hz with a damping ratio of 0.22 puts the envelope at e^(-0.22*omega*t): about 7% of peak after
/// 500ms, so it has visibly settled between beats at 120bpm while still showing roughly one and a half
/// swings. Underdamped on purpose - at zeta >= 1 there is no overshoot and no spring-back to see.
const SHAKE_PX: f32 = 4.2;
const SHAKE_HZ: f32 = 3.4;
const SHAKE_ZETA: f32 = 0.22;

/// The largest slice the spring is integrated over, in seconds.
///
/// Explicit Euler on an oscillator goes unstable once omega*dt approaches 2, and omega here is 21.4, so
/// dt must stay under ~93ms or the branch flies off the panel instead of settling. A 16.7ms frame is
/// nowhere near that - but a stutter is, and this app has a KNOWN stutter on one machine. Sub-stepping
/// costs a handful of multiplies and removes the failure mode entirely, so it is not worth reasoning
/// about whether a long frame can happen.
const SPRING_STEP_S: f32 = 0.006;

/// How the shake and the bass bend taper from the trunk to the tip.
///
/// Both used to move the WHOLE spine by the same amount, which is a rigid translation - the trunk end
/// teleported along with the tip, and that is a large part of what read as judder rather than as a
/// branch. A real branch is anchored where it leaves frame and whips at its far end. Scaled by the
/// normalised distance along the branch, so the base is fixed and the tip gets the full amplitude.
const WHIP_GAMMA: f32 = 0.8;

/// The flourish: a gust. The wind spikes and the branch lets go of a great deal at once.
const GUST_MS: f32 = 2000.0;
const GUST_WIND: f32 = 1.9;
const GUST_RELEASE: usize = 26;

/// LIGHTNING. Bands fed to the shared flourish trigger, so what it ranks is a BASS hit.
///
/// Three - bins 2..5, roughly 47-117 Hz, the kick's fundamental and nothing else.
///
/// MEASURED, and the measurement is the whole point. Strikes per minute over 300 seconds of each of the
/// repo's three real-music fixtures, driven through blossom's own ballistics at strength 0.10, from
/// `probe_strike_rate` in this file:
///
///   bands 3   2.40 / 4.40 / 2.20   <- shipped: all three fixtures fire
///   bands 4   4.60 / 6.60 / 4.40
///   bands 5   2.20 / 6.60 / 0.00   <- the flat-mastered fixture stops firing entirely
///   bands 6   0.00 / 6.60 / 0.00   <- and so does the steady groove
///   bands 8   0.00 / 2.20 / 0.00   <- two of three never fire at all
///
/// Note where that puts the cliff: at SIX bands two of the three fixtures go silent, and at eight only
/// the drum-and-bass one survives. A design pass recommended eight on measured grounds and reported
/// 1.75/6.50/1.50 for it; that did not reproduce against the real crate, and the numbers above are what
/// this code actually does. Widening the window does not make a bass trigger more reliable - it dilutes
/// the kick with everything above it until the median stops seeing a kick at all.
///
/// This is the failure this project has now hit three times, and the vaporwave family shipped a version
/// of it. Hence the test that drives each fixture SEPARATELY: an aggregate passes while two of three
/// give nothing.
const STRIKE_BASS_BANDS: usize = 3;

/// The strike envelope's total length, and its rise.
///
/// `flourish::Envelope` STEPS to 1.0 on the frame it fires, and a step on a geometric quantity has now
/// been reported twice in this family alone - the gust envelope and the branch shake. So the envelope is
/// used as a CLOCK and `strike_shape` is the curve: `strike_shape(0.0)` is exactly 0.0, so the firing
/// frame draws nothing at all. Measured at dt 16.7ms the first eight frames are
/// 0.000 0.189 0.585 0.927 0.988 0.959 0.931 0.903.
const STRIKE_MS: f32 = 1200.0;
const STRIKE_RISE_MS: f32 = 60.0;

/// A gap in DRAWN time longer than this means frames were skipped, so the trigger's history is stale.
///
/// `Smoother::update` runs whether or not the family draws, while the trigger only advances when it does,
/// and the reveal gate needs 400ms above threshold before the family draws at all - so the first drawn
/// frame after the panel comes back reads a whole returning track as one jump. MEASURED: it fires on 4 of
/// 4 hide/reveal cycles on every fixture, at a gate opacity of 0.037. The one guaranteed strike would be
/// invisible AND would spend the 2500ms minimum gap.
///
/// 0.25s sits above the 100ms cap main.rs puts on `dt_ms` and below the 400ms reveal delay.
const STALE_GAP_S: f32 = 0.25;

/// The sky flash: its peak, and the mix coefficient.
///
/// ONLY THE TOP STOP IS LIFTED. `vertical_gradient` interpolates linearly, so the lift falls off to
/// nothing by the horizon for free - which keeps it off the castle standing in the lower rows, off the
/// petals that pile up near the bottom, and byte-identical to the shipped gradient whenever nothing has
/// struck. Measured: 0 pixels differ on all seven colourways at both widths when nothing is striking.
///
/// 0.15 raises mean sky luminance 1.33x (jade) to 1.65x (night), which is below the excursion the
/// vaporwave family already accepts. The cost to the reading is essentially nil: the worst dim-petal
/// contrast anywhere moves by 0.11 of a point (dusk 3.79 -> 3.68), because the lift is largest exactly
/// where the sky is darkest and the petal has the most headroom. Lifting BOTH stops instead takes dusk to
/// 3.41 and plum to 2.73, and a full-sky sheet at vaporwave's own strength takes dusk to 2.17.
const FLASH_PEAK: f32 = 0.15;
const FLASH_MIX: f32 = 0.55;

/// The bolt. Anchor column as an offset from the moon's centre, and the path's shape.
///
/// THE CORRIDOR IS TEN COLUMNS WIDE and that is the hard constraint. The columns that are both clear of
/// the moon disc and land on castle stone are exactly 291..300 at w=380: the disc starts at 301, and
/// cols 269..290 hold no stone at all, so a bolt left of 291 would end in mid-air. Hence an anchor 15
/// columns left of the moon's centre - col 296, which has 39 clear sky rows above its roofline.
///
/// SWING IS IN ABSOLUTE PIXELS, deliberately not a fraction of the width. The vaporwave family's bolt
/// jitters by `w * 0.06`, which is +/-22.8px at w=380 - ported as-is it puts a vertex over the disc on
/// essentially every seed. The upper clamp is DERIVED from `MOON_R` so it cannot drift onto the disc if
/// someone later raises the swing.
/// The sky gradient's first row, which is where a bolt starts.
const SKY_TOP_ROW: i32 = 3;
const BOLT_ANCHOR_OFF: i32 = -15;
const BOLT_SWING_PX: i32 = 4;
const BOLT_SEGS: i32 = 7;
const BOLT_FORK_AT: f32 = 0.55;
const BOLT_FORK_SEGS: u32 = 2;
const BOLT_FORK_SWING_PX: i32 = 5;

/// How far LEFT of the anchor the fork may reach, in pixels.
///
/// Wider than the main bolt's corridor on purpose, and the asymmetry is the point. The main path is
/// boxed in on both sides - it must clear the moon on the right and must land on stone on the left - so
/// clamping the fork to the same 10 columns drew it directly on top of the trunk, where it was
/// invisible. A fork does not have to land on anything, and real lightning forks end in mid-air, so it
/// is only bounded on the moon side. Left is free.
const BOLT_FORK_REACH: i32 = 13;
const BOLT_WIDE_A: f32 = 0.30;

/// The bolt's halo. A FIXED radius, deliberately not `t.bloom`.
///
/// `t.bloom` is a TOML-bindable f32 with no validation and no upper clamp anywhere in the schema, and
/// `Canvas::bloom` iterates `-radius..=radius` per pixel per pass. Measured at 380x60: r=4 is 1.03ms,
/// r=12 is 2.09ms, r=64 is 8.17ms - and `1e300` in a TOML deserialises to infinity, which as an `i32`
/// cast is 2147483647. A constant removes that exposure from this path entirely.
///
/// Counter-intuitively a LARGER radius gives a DIMMER halo for a thin source, because the box blur
/// divides by the full kernel count. The brightness lever is the wide pass, not the radius.
const BOLT_BLOOM_R: i32 = 4;
const BOLT_GLOW: f32 = 0.85;


/// The three tumble masks: face, angled, edge. `#` is the petal body, `+` its lit edge.
const TUMBLE: [[&str; 3]; 3] = [
    ["+##", "###", ".#."],  // face on
    [".#+", "##.", "#.."],  // angled
    ["..+", ".#.", ".#."],  // edge on
];
const MASK_W: i32 = 3;
const MASK_H: i32 = 3;

/// How much of the theme's `glow_strength` the PETALS get, and the floor on the halo radius.
///
/// Petals are bloomed on their OWN layer rather than with the rest of the frame, which is the only way
/// to get a halo that shows: `Canvas::bloom` composites its halo UNDERNEATH the content that made it, so
/// a petal bloomed on the main canvas puts its glow behind an opaque sky and nothing reaches the screen.
/// The same reason chroma builds a separate "film" layer.
///
/// Scaled to 1.6x rather than passed straight through because a petal is 2-3 pixels: at the frame's own
/// strength the halo is too faint to see on something that small, where the moon - 21 pixels across -
/// glows plenty at it. Per-colourway control comes for free, since `glow_strength` is already a theme
/// field with a TOML binding: a colourway that wants no petal glow sets it to 0 and gets crisp petals.
const PETAL_GLOW: f32 = 1.6;
const PETAL_BLOOM_MIN: i32 = 3;


/// The castle, as pixel masks. `#` is stone and roof, `.` is sky.
///
/// A SILHOUETTE with no interior detail, which is the only thing that works at this size: a tenshu's
/// identity is entirely in its outline - the stacked, strictly narrowing roofs with upturned eaves over a
/// battered stone base - and any interior marking at 30 rows just muddies that outline.
///
/// Chosen from four candidates by rendering all four at true scale in this scene. What decided it was
/// where each one SPENT its pixels: this one buys four strictly narrowing roofs and their upturned eave
/// corners, and that stacked roofline is the single cue that reads as a castle at 28 rows. The candidate
/// that instead spent 19 of its 30 rows on a flared stone base - defensible reasoning, since the base is
/// what separates a tenshu from a pagoda - rendered as a tent or a volcano, which is exactly the risk its
/// own designer had flagged. A taller, narrower tower crossed the moon best but read as a fir tree.
///
/// REDRAWN once the keyline went on, and the reason is worth keeping. The first version modelled upturned
/// eaves as 2-column blocks stepped up off the eave line and protruding 5-7px past each storey. That is
/// correct architecture and it looked plausible while the castle was a low-contrast smudge - but the
/// moment the outline became crisp, four pairs of thin symmetrical limbs sticking out of narrow storeys
/// read unmistakably as a SPIDER. The detail was right and the proportion was wrong, and only a legible
/// render could show it.
///
/// So the eaves are now plain: each roof simply overhangs its storey by 3-6px per side and stops. No
/// upturn, no corner notch. At 28 rows the tiering alone carries the read - four roofs, strictly
/// narrowing, over a flaring base - and an eave flick costs two columns of silhouette to say something
/// the stack already says. Widths increase monotonically down the whole shape, which is what makes it a
/// stack of trapezoids rather than a collection of parts.
///
/// Storeys are 2-3 rows, never 1: a single-row waist is erased by a 1px closing because the wide roof
/// rows above and below dilate into it, and holding the tiers apart is the entire job.

/// The tenshu, 41x28 - chosen from four candidates for having the clearest castle read.
const CASTLE_TIERS: [&str; 28] = [
    "...................###...................",
    "................#########................",
    ".............###############.............",
    "............#################............",
    "................#########................",
    "................#########................",
    ".............###############.............",
    "..........#####################..........",
    ".........#######################.........",
    "..............#############..............",
    "..............#############..............",
    "..............#############..............",
    "..........#####################..........",
    ".......###########################.......",
    "......#############################......",
    "............#################............",
    "............#################............",
    "............#################............",
    ".......###########################.......",
    "....#################################....",
    "...###################################...",
    ".........#######################.........",
    ".........#######################.........",
    ".........#######################.........",
    ".......###########################.......",
    ".....###############################.....",
    "...###################################...",
    ".#######################################.",
];

const CASTLE: &[&str] = &CASTLE_TIERS;

/// Where the castle stands: the fraction of the panel its LEFT edge sits at, and the last sky row its
/// foot rests on.
///
/// It overlaps the moon on purpose - asked for as "the moon can be peeking out behind the castle" - and
/// that is the legibility choice as much as the composition one. The castle is a flat dark silhouette, so
/// its outline is all it has; the moon is the brightest thing on the panel. A roofline crossing a bright
/// disc is the highest-contrast edge available anywhere in the frame, and against bare dusk sky the same
/// outline is dark-on-dark. The overlap is where the castle is most readable, not just prettiest.
///
/// Standing on the floor rather than floating, because the panel's own bottom row then reads as ground
/// for free - the panel colour is within about 1 dL* of the castle body, so the foot merges into it.
/// Anchored to the MOON, not to the panel - the castle's right edge lands this many pixels past the
/// disc's centre.
///
/// It was a fraction of the panel width for one render, and that was wrong twice over: it put the castle
/// ten columns short of the disc so there was no overlap at all, and because the four candidates are 25
/// to 45 px wide, a fixed LEFT edge landed each of them in a different place - so the comparison was
/// measuring position as much as design. Anchoring the right edge to the moon makes every candidate
/// overlap by construction, whatever its width, and makes the overlap the thing being compared.
/// 20 puts the castle's own centre on the moon's centre, so the disc clears the roofline on BOTH sides
/// and the moon peeks out over the top rather than off one shoulder.
const CASTLE_PAST_MOON: i32 = 20;
const CASTLE_FOOT_INSET: i32 = 4;

/// The moon: radius in pixels, and where it sits as a fraction of the panel.
///
/// Upper RIGHT, deliberately opposite the branch, which enters from the left. Two focal points on the
/// same side would fight; on opposite sides they frame the petals drifting between them.
///
/// Six was the smallest disc that still reads as a moon rather than a dot - below that it is
/// indistinguishable from a bright petal, which is the one thing it must not look like. Ten is what it
/// wants to BE: reported as "quite small" at six, and a moon is supposed to be the largest single thing
/// in the sky. At ten it is a third of the panel height and still clears the branch.
///
/// MOON_Y moved twice, and the second move is the interesting one. It was 0.30, where the disc spanned
/// rows 8-28 and a floor-standing castle topped out at row 28 - they shared two rows, which is a touch,
/// not a "peeking out from behind". At 0.42 the occlusion test measured only 12 of 317 disc pixels
/// covered, because the part of the castle reaching that high is the narrow top spire: the WIDE tiers
/// that could actually cross a disc live in the lower half of the mask, and a 28-row castle standing on
/// the floor puts them at rows 40-51.
///
/// The moon came down to meet them - and then went most of the way back up, because 0.55 was too far. The
/// castle is 41px wide and the disc only 21px across, so once the tiers cross MOST of the disc they cut it
/// into fragments and it stops reading as a moon at all: it came out as a spiky crown. A big overlap and a
/// legible moon are not both available at these sizes.
///
/// 0.42 is where the roofline crosses the disc's BOTTOM and nothing else. The moon keeps its top two
/// thirds as an unbroken curve - which is all it needs to still be a moon - and the castle's upper tiers
/// interrupt the lower edge. That reads as a moon standing behind a castle, which is what was asked for;
/// burying it does not.
const MOON_R: i32 = 10;
const MOON_X: f32 = 0.82;
const MOON_Y: f32 = 0.42;

/// The twigs: where along the branch, which way, and how long.
///
/// A TABLE rather than a hash, and hand-placed rather than random. The shape has to be identical frame
/// to frame - a tree that reshuffles itself every frame is not a tree - and eight twigs is few enough
/// that choosing them by eye beats any generator: the spacing is uneven on purpose, two fork DOWNWARD,
/// and the longest sit where the branch is thickest, which is what a real branch does.
///
/// `(along, dx, dy, len)`: `along` is the fraction of the branch, `dx`/`dy` the direction per step
/// (negative `dy` is up the screen), `len` the number of steps.
///
/// **Lengths were roughly doubled** after the first version was reported as too small: at 4-10 steps and
/// 1-2px thick they read as scratches on the branch rather than as limbs coming off it. Now 9-17 steps,
/// 3px at the fork, and each one FORKS near its end - see `FORK_AT`. A twig that does not divide is a
/// spike; dividing is most of what makes a branch look like a branch.
const TWIGS: [(f32, f32, f32, f32); 8] = [
    (0.10, 0.50, -1.00, 14.0),
    (0.21, -0.55, -1.00, 10.0),
    (0.31, 0.75, -1.00, 17.0),
    (0.42, 0.45, 1.00, 9.0),   // downward - a branch forks both ways
    (0.53, 0.65, -1.00, 15.0),
    (0.64, -0.40, -1.00, 11.0),
    (0.76, 0.80, -1.00, 13.0),
    (0.88, 0.35, 1.00, 9.0),   // downward, near the tip
];

/// Where along a twig its fork leaves, and how the fork's direction differs from its parent.
///
/// 0.55 rather than nearer the tip: a fork that splits in the last quarter reads as a frayed end, where
/// one at just over half reads as two limbs. The fork swings the horizontal component the other way,
/// which is what stops the pair looking like one thick line.
const FORK_AT: f32 = 0.55;
const FORK_SWING: f32 = -1.25;
const FORK_LEN: f32 = 0.45;

/// Blossom clusters at every twig tip.
///
/// A real cherry branch is COVERED in blossom, and this is the change that makes the twigs distinctive
/// rather than merely bigger. It also fixes something that was quietly wrong: petals were being released
/// from twig tips that had nothing on them, so blossom appeared out of bare wood. Now the wood is
/// blossoming and the petals come off the clusters.


#[derive(Clone, Copy, Default)]
struct Petal {
    x: f32,
    y: f32,
    /// Flutter phase and its rate, so no two petals drift alike.
    ph: f32,
    rate: f32,
    /// Tumble phase, and how fast this petal turns.
    spin: f32,
    spin_rate: f32,
    /// 0..1 depth-ish shade: petals nearer the front are lighter.
    shade: f32,
    /// This petal's own stable hue offset, 0..1. Fixed for its whole life - see the draw loop.
    hue: f32,
    live: bool,
}

#[derive(Default)]
pub struct Blossom {
    petals: Vec<Petal>,
    /// Smoothed wind, in px/s.
    wind: f32,
    /// Branch shake and bass load, in pixels.
    shake: f32,
    /// The spring's velocity in px/s. An onset kicks THIS, never `shake` - see `SHAKE_PX`.
    shake_v: f32,
    bend: f32,
    /// Unspent time toward the next trickle release.
    trickle: f32,
    seed: u32,
    onset: crate::dsp::onset::Flux,
    flourish: crate::dsp::flourish::Trigger,
    gust: crate::dsp::flourish::Envelope,
    /// The strike's clock. Read as an AGE, never as a level - see `strike_shape`.
    strike: crate::dsp::flourish::Envelope,
    /// The bolt's path seed, bumped on every fire so consecutive strikes differ. Deterministic, so a
    /// given seed always draws the same bolt.
    bolt_seed: u32,
    /// `d.time_s` on the previous DRAWN frame. `None` until the family has drawn once - see STALE_GAP_S.
    last_seen_s: Option<f32>,
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

impl Blossom {
    /// The branch: a shallow arc from the top-left with a few twigs, and the y of its spine at a given x.
    ///
    /// An arc rather than a straight line because a straight branch reads as a wire, and one that leaves
    /// the frame at both ends reads as a beam. This one starts at the left edge and ends inside the
    /// panel, so it is unmistakably a branch with a tip.
    fn spine_y(&self, x: f32, w: f32, h: f32) -> f32 {
        let t = (x / (w * 0.72)).clamp(0.0, 1.0);
        // Droops away from the trunk, and the bend/shake ride on top of it.
        // t^1.7 rather than t^2, and a deeper drop: the gentler exponent puts most of the curvature
        // near the TIP, which is where a real branch bends. The first version was t^2 over a shallow
        // drop and read as a cable strung across the panel rather than as a branch.
        let base = h * 0.09 + t.powf(1.7) * h * 0.44;
        // Tapered, not rigid - see WHIP_GAMMA. The trunk end is anchored off-panel and does not move.
        base + (self.bend + self.shake) * t.powf(WHIP_GAMMA)
    }


    /// The castle, behind the branch and the petals.
    ///
    /// Drawn straight after the moon so it OCCLUDES the disc - that occlusion is the whole point - and
    /// before the branch, so bark and blossom pass in front of it and put it at a distance.
    /// Deterministic value hash, the same one the vaporwave family uses, so a given seed always draws
    /// the same bolt and a golden stays reproducible.
    fn bolt_hash(mut x: u32) -> u32 {
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb_352d);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846c_a68b);
        x ^= x >> 16;
        x
    }

    fn bolt_signed(seed: u32, n: u32) -> f32 {
        (Self::bolt_hash(seed ^ n.wrapping_mul(0x9e37_79b9)) as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// The panel row of the first stone pixel in panel column `col`, or `None` if that column holds no
    /// stone. Same mask, same origin and same shed rule as `castle`, so the bolt cannot land where the
    /// castle is not drawn.
    fn roof_row(w: i32, h: i32, col: i32) -> Option<i32> {
        let rows = CASTLE.len() as i32;
        let cols = CASTLE.iter().map(|r| r.len()).max().unwrap_or(0) as i32;
        if rows + 6 > h || cols + 8 > w {
            return None; // no castle drawn, so there is nothing to strike
        }
        let x0 = (w as f32 * MOON_X) as i32 + CASTLE_PAST_MOON - cols;
        let y0 = h - CASTLE_FOOT_INSET - rows;
        let rx = col - x0;
        if rx < 0 || rx >= cols {
            return None;
        }
        for ry in 0..rows {
            let line = CASTLE[ry as usize].as_bytes();
            if (rx as usize) < line.len() && line[rx as usize] == b'#' {
                return Some(y0 + ry);
            }
        }
        None
    }

    /// The strike's brightness from its AGE in milliseconds.
    ///
    /// A smoothstep rise then a quadratic fall. `strike_shape(0.0)` is exactly 0.0, which is the whole
    /// point: the frame that fires draws nothing, so nothing steps.
    fn strike_shape(age_ms: f32) -> f32 {
        if !age_ms.is_finite() || age_ms < 0.0 {
            return 0.0;
        }
        if age_ms < STRIKE_RISE_MS {
            let u = (age_ms / STRIKE_RISE_MS).clamp(0.0, 1.0);
            u * u * (3.0 - 2.0 * u)
        } else {
            let u = ((age_ms - STRIKE_RISE_MS) / (STRIKE_MS - STRIKE_RISE_MS)).clamp(0.0, 1.0);
            (1.0 - u) * (1.0 - u)
        }
    }

    /// The strike on its own transparent layer, or `None` if there is nothing to strike.
    ///
    /// Its own layer because `Canvas::bloom` composites its halo UNDERNEATH the content that made it, and
    /// this family's sky is fully opaque - a bolt drawn straight onto the panel would get no halo at all.
    fn bolt_layer(&self, w: i32, h: i32, t: &Theme, bright: f32) -> Option<Canvas> {
        if !(bright > 0.0) {
            return None; // false for NaN, which is the point
        }
        let mx = (w as f32 * MOON_X) as i32;
        let x0 = mx + BOLT_ANCHOR_OFF;
        let end = Self::roof_row(w, h, x0)? - 1; // the last sky row under the anchor
        if end <= SKY_TOP_ROW + 4 {
            return None;
        }
        let seed = self.bolt_seed;
        let wide = Rgba::from_hex(&t.hot, (bright * BOLT_WIDE_A).clamp(0.0, 1.0));
        let core = Rgba::from_hex(&t.hot, bright.clamp(0.0, 1.0));
        // The corridor. `hi` is derived from MOON_R so the wide pass's +1 offset can never reach the
        // disc's left edge, whatever BOLT_SWING_PX is later set to.
        let lo = x0 - BOLT_SWING_PX;
        let hi = (mx - MOON_R - 2).min(x0 + BOLT_SWING_PX);
        if hi < lo {
            return None;
        }
        let mut pts: Vec<(i32, i32)> = Vec::with_capacity(BOLT_SEGS as usize + 1);
        for seg in 0..=BOLT_SEGS {
            let f = seg as f32 / BOLT_SEGS as f32;
            let y = SKY_TOP_ROW + (f * (end - SKY_TOP_ROW) as f32).round() as i32;
            let j = (Self::bolt_signed(seed, seg as u32 + 1) * BOLT_SWING_PX as f32).round() as i32;
            pts.push(((x0 + j).clamp(lo, hi), y.clamp(SKY_TOP_ROW, end)));
        }
        pts[0].1 = SKY_TOP_ROW; // starts at the top of the sky
        let n = pts.len();
        pts[n - 1] = (x0.clamp(lo, hi), end); // and ends ON the roofline
        let mut b = Canvas::new(w, h);
        for pair in pts.windows(2) {
            // The wide pass first, the core on top of it.
            for dx in -1..=1 {
                b.line(pair[0].0 + dx, pair[0].1, pair[1].0 + dx, pair[1].1, wide);
            }
        }
        for pair in pts.windows(2) {
            b.line(pair[0].0, pair[0].1, pair[1].0, pair[1].1, core);
        }
        let start = ((pts.len() as f32) * BOLT_FORK_AT) as usize;
        if start + 1 < pts.len() {
            let (mut fx, mut fy) = pts[start];
            // The fork leans LEFT, away from the moon, into the room the main path does not have. Its
            // own lower bound, not the corridor's - see BOLT_FORK_REACH.
            let fork_lo = x0 - BOLT_FORK_REACH;
            for k in 0..BOLT_FORK_SEGS {
                let swing = Self::bolt_signed(seed, 100 + k).abs() * BOLT_FORK_SWING_PX as f32;
                let nx = (fx - swing.round() as i32).clamp(fork_lo, hi);
                let ny = (fy + (((end - fy) as f32) * 0.40).round().max(1.0) as i32).min(end);
                b.line(fx, fy, nx, ny, core);
                fx = nx;
                fy = ny;
            }
        }
        // A SAFETY NET, not a nicety: whatever the path did, no bolt pixel may land on castle stone. The
        // castle's outline is its entire identity and this layer composites over it at full alpha.
        // Punching BEFORE the bloom leaves the halo free to spill down onto the roof, which is the
        // strike lighting it.
        let rows = CASTLE.len() as i32;
        let cols = CASTLE.iter().map(|r| r.len()).max().unwrap_or(0) as i32;
        let cx0 = mx + CASTLE_PAST_MOON - cols;
        let cy0 = h - CASTLE_FOOT_INSET - rows;
        for ry in 0..rows {
            let line = CASTLE[ry as usize].as_bytes();
            for rx in 0..cols {
                if (rx as usize) < line.len() && line[rx as usize] == b'#' {
                    b.punch_rect(cx0 + rx, cy0 + ry, 1, 1);
                }
            }
        }
        Some(b)
    }

    fn castle(&self, c: &mut Canvas, t: &Theme, w: i32, h: i32) {
        let mask = CASTLE;
        let rows = mask.len() as i32;
        let cols = mask.iter().map(|r| r.len()).max().unwrap_or(0) as i32;
        // Shed rather than crop: a castle with its roofline cut off is not a castle, and this family
        // already runs at widths where the branch itself is a thicket.
        if rows + 6 > h || cols + 8 > w {
            return;
        }
        // LIGHTER than the sky, not darker - which is the opposite of what "silhouette" suggests and is
        // the physically right answer for something far away. Haze lifts a distant mass toward the sky's
        // own brightness and drains its contrast; a distant ridge at dusk is a pale grey shape, not a
        // black one. Drawn darker than the sky for one render, the castle was invisible except where it
        // crossed the moon - the outline, which is the whole identity, only existed against the disc.
        let body = Rgba::from_hex(&t.tube.internals, 1.0);
        let x0 = (w as f32 * MOON_X) as i32 + CASTLE_PAST_MOON - cols;
        let y0 = h - CASTLE_FOOT_INSET - rows;
        // A HARD KEYLINE around the silhouette, which is the technique this project already needed for
        // chroma, dolphin, mesh and pipes: an opaque dark rim makes a shape legible INDEPENDENT of its own
        // colour. It matters more here than anywhere, because the castle has to survive two backgrounds
        // at once - a dark sky where only the lighter haze body shows, and a near-white moon where only a
        // darker edge shows. Body-plus-rim gives it one contrast against each, so the roofline is crisp
        // wherever it happens to fall. Without it the castle read as a soft mass with no outline except
        // the few pixels crossing the disc.
        let key = Rgba::from_hex(&t.panel, 1.0);
        let rows_v: Vec<&[u8]> = mask.iter().map(|r| r.as_bytes()).collect();
        let solid = |ry: i32, rx: i32| -> bool {
            if ry < 0 || rx < 0 || ry >= rows as i32 {
                return false;
            }
            let line = rows_v[ry as usize];
            (rx as usize) < line.len() && line[rx as usize] == b'#'
        };
        for ry in 0..rows {
            for rx in 0..cols {
                if !solid(ry, rx) {
                    continue;
                }
                // On the boundary if any 4-neighbour is sky. The mask's own edge counts as sky, so the
                // base's bottom row is rimmed too and the castle does not bleed into the panel floor.
                let edge = !solid(ry - 1, rx)
                    || !solid(ry + 1, rx)
                    || !solid(ry, rx - 1)
                    || !solid(ry, rx + 1);
                c.fill_rect(x0 + rx, y0 + ry, 1, 1, if edge { key } else { body });
            }
        }
    }

    fn branch_len(w: f32) -> f32 {
        w * 0.72
    }

    /// Where a twig's tip is. Shared by the drawing and the petal release, so blossom always comes off
    /// the end of a twig and never out of thin air beside one.
    fn twig_tip(&self, k: usize, w: f32, h: f32) -> (f32, f32) {
        let (along, dx, dy, len) = TWIGS[k % TWIGS.len()];
        let at = along * Self::branch_len(w);
        (at + dx * len, self.spine_y(at, w, h) + dy * len)
    }

    /// Reseeds a petal at a random point along the branch.
    fn seed_petal(&mut self, i: usize, w: f32, h: f32) {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let s = self.seed;
        // Mostly from a twig TIP, sometimes from along the spine. Blossom grows on the twigs, so a
        // petal appearing mid-branch reads as coming off the bark itself.
        let pick = rand01(s, i as u32 * 3);
        let (x, y) = if pick < 0.78 {
            let k = (rand01(s, i as u32 * 3 + 4) * TWIGS.len() as f32) as usize;
            let (tx, ty) = self.twig_tip(k, w, h);
            (tx + (rand01(s, i as u32 * 3 + 1) - 0.5) * 2.0, ty + rand01(s, i as u32 * 3 + 2) * 1.5)
        } else {
            let along = 0.10 + 0.90 * rand01(s, i as u32 * 3 + 5);
            let ax = along * Self::branch_len(w);
            (ax, self.spine_y(ax, w, h) + rand01(s, i as u32 * 3 + 1) * 3.0)
        };
        self.petals[i] = Petal {
            x,
            y,
            ph: rand01(s, i as u32 * 3 + 2) * std::f32::consts::TAU,
            rate: 1.4 + 2.6 * rand01(s, i as u32 * 5),
            spin: rand01(s, i as u32 * 7) * 3.0,
            spin_rate: 1.6 + 3.4 * rand01(s, i as u32 * 11),
            shade: rand01(s, i as u32 * 13),
            hue: rand01(s, i as u32 * 17),
            live: true,
        };
    }

    fn release(&mut self, n: usize, w: f32, h: f32) {
        let mut done = 0;
        for i in 0..self.petals.len() {
            if done >= n {
                break;
            }
            if !self.petals[i].live {
                self.seed_petal(i, w, h);
                done += 1;
            }
        }
    }
}

impl Family for Blossom {
    fn id(&self) -> &'static str {
        "blossom"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        if self.seed == 0 {
            self.seed = 0x9e37_79b9;
        }
        // The trigger is fed the LOW EIGHT BANDS only, so what it ranks is a bass hit rather than a
        // snare or a chord stab. Everything else is the shared machinery unchanged - the
        // median-of-recent-candidates rule, the minimum gap, the global on/off and the "flourish now"
        // button all still apply, because this is still `dsp::flourish::Trigger`.
        //
        // The strike and the GUST are therefore the same event, and that is deliberate: a gust of wind
        // and a lightning strike together is a storm, which is a better scene than either alone. The
        // honest cost is that a colourway cannot have one without the other.
        let nb = STRIKE_BASS_BANDS.min(d.levels.len());
        let mut fired = self.flourish.update(&d.levels[..nb], dt, t.flourish);
        // A gap in DRAWN time means the panel was hidden - see STALE_GAP_S.
        let stale = match self.last_seen_s {
            Some(prev) => (d.time_s - prev).rem_euclid(3600.0) > STALE_GAP_S,
            None => false, // the first flux update seeds its own history and returns false
        };
        self.last_seen_s = Some(d.time_s);
        if stale {
            fired = false;
        }
        if fired {
            self.bolt_seed = self.bolt_seed.wrapping_add(1);
        }
        let strike_lvl = self.strike.update(fired, dt, STRIKE_MS);
        let bright = {
            let b = Self::strike_shape((1.0 - strike_lvl) * STRIKE_MS);
            // Sanitised ONCE, here. `f32::clamp` returns NaN unchanged and `NaN <= 0.0` is false, so a
            // non-finite value reaching the flash below would turn the sky solid white - because
            // `f32::NAN.min(255.0)` is 255. Measured: without this, mix(0x12, NaN) is 0xff.
            if b.is_finite() { b.clamp(0.0, 1.0) } else { 0.0 }
        };
        let gust = self.gust.update(fired, dt, GUST_MS);

        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        if w < 60 || h < 28 {
            return; // shed rather than smudge
        }

        // ---- the sky ----
        //
        // A dusk gradient rather than flat black, because flat black is a void and dusk is a TIME - and
        // the whole family is a dusk by necessity anyway (a pale petal needs a dark sky to clear the 3:1
        // rule). Deep at the top, warming toward the horizon, which is the direction real dusk goes.
        //
        // Dithered: `vertical_gradient`'s 4x4 Bayer pass exists because a smooth ramp over ~56 rows in 8
        // bits per channel bands visibly, and a banded sky reads as a rendering fault rather than as sky.
        // Drawn INSIDE the rounded panel so it cannot square off the corners the panel just rounded.
        // THE FLASH. Only the TOP stop is lifted - see FLASH_PEAK.
        let sky_top = Rgba::from_hex(&t.tube.socket, 1.0);
        let sky_low = Rgba::from_hex(&t.tube.collar, 1.0);
        // `bright > 0.0` is FALSE for NaN, which is why it is written this way and not `!= 0.0`.
        let sky_top = if bright > 0.0 {
            let flash = bright * FLASH_PEAK;
            let mix = |v: u8| (v as f32 + (255.0 - v as f32) * flash * FLASH_MIX).min(255.0) as u8;
            Rgba::new(mix(sky_top.r), mix(sky_top.g), mix(sky_top.b), 255)
        } else {
            sky_top
        };
        c.vertical_gradient(2, 3, w - 4, h - 6, &[(0.0, sky_top), (1.0, sky_low)], true);

        // ---- the moon ----
        //
        // Behind everything, so the branch crosses it and petals drift in front. That occlusion is what
        // puts it at a distance; a moon drawn on top would sit in the same plane as the blossom.
        let moon = Rgba::from_hex(&t.tube.glass, 1.0);
        let (mx, my) = ((w as f32 * MOON_X) as i32, (h as f32 * MOON_Y) as i32);
        for dy in -MOON_R..=MOON_R {
            let dx = (((MOON_R * MOON_R - dy * dy) as f32).max(0.0).sqrt() + 0.5) as i32;
            c.fill_rect(mx - dx, my + dy, dx * 2 + 1, 1, moon);
        }

        // ---- the castle ----
        self.castle(c, t, w, h);

        // ---- the strike ----
        //
        // HERE, after the castle and before the branch, so the branch, the twigs, the clusters and the
        // petals all pass in FRONT of it. That is the right depth for a sky event, and at the narrow
        // width it is what stops the bolt being drawn over the branch tip.
        //
        // Gated on `bright`: `Canvas::bloom` costs the same on an empty layer as on a full one -
        // measured 1.1724ms against 1.1742ms - so an ungated layer would blur nothing on the ~87% of
        // frames with no strike, and allocate 91KB four times over while doing it.
        if bright > 0.0 {
            if let Some(mut b) = self.bolt_layer(w, h, t, bright) {
                b.bloom(BOLT_BLOOM_R, BOLT_GLOW);
                c.draw_over(&b);
            }
        }
        if self.petals.len() != PETALS {
            self.petals = vec![Petal::default(); PETALS];
        }

        let (wf, hf) = (w as f32, h as f32);
        let bands = d.levels.len().max(1);

        // ---- weather ----
        let overall = d.levels.iter().map(|v| resp(*v, t.sensitivity)).sum::<f32>() / bands as f32;
        let bass = {
            let n = (bands / 4).max(1);
            d.levels[..n].iter().map(|v| resp(*v, t.sensitivity)).fold(0.0f32, f32::max)
        };
        // Wind IS the level - the load-bearing mapping. Followed slowly, because air has inertia and a
        // wind that tracked every transient would read as jitter rather than as weather.
        let target = WIND_CALM + (WIND_GALE - WIND_CALM) * overall.clamp(0.0, 1.0);
        let target = target * (1.0 + (GUST_WIND - 1.0) * gust);
        self.wind += (target - self.wind) * (WIND_FOLLOW * dt).min(1.0);
        if !self.wind.is_finite() {
            self.wind = WIND_CALM;
        }

        // The bass loads the branch; an onset snaps it. Both decay, so neither can stick.
        self.bend += (bass * 2.4 - self.bend) * (0.004 * dt).min(1.0);

        // The spring, sub-stepped for stability - see SPRING_STEP_S.
        let omega = std::f32::consts::TAU * SHAKE_HZ;
        let (k, damp) = (omega * omega, 2.0 * SHAKE_ZETA * omega);
        let mut left = (dt / 1000.0).clamp(0.0, 0.25);
        while left > 0.0 {
            let step = left.min(SPRING_STEP_S);
            let accel = -k * self.shake - damp * self.shake_v;
            self.shake_v += accel * step;
            self.shake += self.shake_v * step;
            left -= step;
        }
        if !self.bend.is_finite() {
            self.bend = 0.0;
        }
        if !self.shake.is_finite() || !self.shake_v.is_finite() {
            self.shake = 0.0;
            self.shake_v = 0.0;
        }

        let onset = self.onset.update(&d.levels, dt, 2.8, 200.0);
        if onset {
            // An IMPULSE, and upward. For an underdamped spring the peak displacement from a velocity
            // kick is about v0/omega, so this is the kick that reaches SHAKE_PX at the tip.
            //
            // Upward because the first half-swing is the one the eye catches, and a branch flicking up
            // throws its blossom off, where a branch pressed down would carry the petals with it. After
            // that half-swing the direction stops mattering - it is oscillating either way.
            //
            // ADDED, not assigned: a beat landing while the branch is still moving should compound with
            // it, which is what makes a run of hits build instead of restarting. Assigning velocity would
            // reintroduce exactly the reset that the position-assignment version was reported for.
            self.shake_v -= SHAKE_PX * std::f32::consts::TAU * SHAKE_HZ;
            self.release(BURST, wf, hf);
        }
        if fired {
            self.release(GUST_RELEASE, wf, hf);
        }
        // A trickle regardless, so the tree is never completely still even in silence.
        self.trickle += dt / 1000.0 * TRICKLE_PER_S;
        while self.trickle >= 1.0 {
            self.trickle -= 1.0;
            self.release(1, wf, hf);
        }

        // ---- the branch, drawn first so petals pass in front of it ----
        let bark = Rgba::from_hex(&t.tube.chassis_bottom, 1.0);
        let bark_lit = Rgba::from_hex(&t.tube.chassis_top, 1.0);
        let len = Self::branch_len(wf);
        let mut x = 0.0f32;
        while x < len {
            let y = self.spine_y(x, wf, hf);
            // Tapers to a tip, which is what stops it reading as a wire.
            // Tapers from five pixels at the trunk to one at the tip. Two was not enough to read as
            // wood - a uniform 2px line is a wire whatever colour it is - and the taper is most of what
            // says branch.
            let thick = (5.0 * (1.0 - x / len).powf(0.75) + 1.0).round() as i32;
            c.fill_rect(x as i32, y as i32, 2, thick, bark);
            c.fill_rect(x as i32, y as i32, 2, 1, bark_lit);
            x += 1.5;
        }
        // The twigs - see `TWIGS`. Tapered 3px at the fork to 1px at the tip, and each one FORKS: a twig
        // that does not divide is a spike, and dividing is most of what makes a branch read as a branch.
        for (twig_i, (along, dx, dy, tlen)) in TWIGS.iter().enumerate() {
            // Clusters vary by TWIG, so a rainbow tree blossoms in more than one colour along its length
            // - and on a fixed colourway this is `t.lit` for every twig, exactly as before.
            let twig01 = twig_i as f32 / TWIGS.len() as f32;
            let cluster_col = crate::render::tint(t, twig01, d.time_s, false, &t.lit, 0.85);
            let cluster_hot = crate::render::tint(t, twig01, d.time_s, true, &t.hot, 1.0);
            let at = len * along;
            let base_y = self.spine_y(at, wf, hf);
            // CLAMPED to the room that actually exists. The lengths in TWIGS are what the shape wants;
            // near the trunk the spine sits at 9% of the height, so an upward twig there has only a few
            // rows before it leaves the panel - and the first version of these longer twigs ran straight
            // off the top edge. A branch near the top of a frame grows sideways, which is what this
            // produces: the clamp shortens the vertical run rather than the horizontal one.
            let room = if *dy < 0.0 { base_y - 4.0 } else { hf - 5.0 - base_y };
            let tlen = &tlen.min((room / dy.abs()).max(3.0));

            // The twig itself.
            let mut step = 0.0f32;
            while step < *tlen {
                let tx = at + dx * step;
                let ty = base_y + dy * step;
                let f = step / tlen;
                let thick = if f < 0.35 {
                    3
                } else if f < 0.72 {
                    2
                } else {
                    1
                };
                c.fill_rect(tx as i32, ty as i32, 2, thick, bark);
                step += 1.0;
            }

            // Its fork, leaving at FORK_AT and swinging the other way.
            let fx0 = at + dx * tlen * FORK_AT;
            let fy0 = base_y + dy * tlen * FORK_AT;
            let flen = tlen * FORK_LEN;
            let mut fs = 0.0f32;
            while fs < flen {
                let tx = fx0 + (dx + FORK_SWING) * 0.55 * fs;
                let ty = fy0 + dy * fs;
                let thick = if fs < flen * 0.5 { 2 } else { 1 };
                c.fill_rect(tx as i32, ty as i32, 1, thick, bark);
                fs += 1.0;
            }

            // A lit pixel where the twig meets the branch, which is what makes it read as JOINED rather
            // than as a mark lying across it.
            c.fill_rect(at as i32, base_y as i32, 2, 1, bark_lit);

            // Blossom clusters at both tips. Drawn with the wood, BEFORE the falling petals, so a petal
            // in flight passes in front of the cluster it came from.
            for (cx0, cy0) in [
                (at + dx * tlen, base_y + dy * tlen),
                (fx0 + (dx + FORK_SWING) * 0.55 * flen, fy0 + dy * flen),
            ] {
                // A tight PLUS, not a ring. The first version placed CLUSTER points on a circle of
                // radius 2.2 and drew each as a 2x2 block, which at this scale is a scatter of four
                // separate marks - it read as a symbol, not as blossom. Five single pixels in a plus,
                // with a lit centre, is the smallest thing that reads as one clump.
                let (bx, by) = (cx0 as i32, cy0 as i32);
                for (dx2, dy2) in [(0i32, 0i32), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let col = if dx2 == 0 && dy2 == 0 { cluster_hot } else { cluster_col };
                    c.fill_rect(bx + dx2, by + dy2, 1, 1, col);
                }
                // Two more pixels off the diagonal so it is not a perfect cross, which reads as a mark.
                c.fill_rect(bx - 1, by - 1, 1, 1, cluster_col);
                c.fill_rect(bx + 1, by + 1, 1, 1, cluster_col);
            }
        }

        // Bloom the SKY, MOON AND BRANCH here, before the petals exist - so the moon keeps the soft
        // halo it has always had, and the petals are not bloomed twice (once with the frame and again
        // on their own layer, which reads as a smear rather than a glow).
        // NO FRAME BLOOM HERE, and the reason is measured. There used to be a
        // `c.bloom(t.bloom as i32, FRAME_GLOW)` on this line, documented as existing "to give the moon
        // its halo". It was doing nothing at all.
        //
        // `Canvas::bloom` composites its halo UNDERNEATH its own source, and `blend_over` returns the
        // source unchanged at full alpha. This family's panel is opaque and its sky gradient is drawn at
        // alpha 1.0, so the entire interior is opaque and the only pixels the bloom could write were in
        // the transparent margin - which `clip_to_rounded_rect` at the end of this function then zeroes.
        // CENSUS at 380x60 on blossom-dusk: 21152 opaque, 0 semi-transparent, 1648 transparent; the call
        // changed 1648 pixels, ZERO of them inside the sky rect, and after the clip exactly 0 pixels
        // differed with it and without it. It cost ~1.03ms per frame for a provably empty result.
        //
        // So the moon has no halo today, and never did. Giving it one needs the same treatment the petals
        // and the bolt get - its own transparent layer, bloomed, composited over - which is a change
        // worth making deliberately rather than by leaving a dead call in place. Logged in the backlog.

        // ---- petals ----
        //
        // On their own transparent layer so they can glow: see PETAL_GLOW.
        let mut g = Canvas::new(w, h);
        // Resolved PER PETAL through the shared rainbow resolver. On a fixed colourway `tint` returns
        // `t.lit` unchanged, so those themes are bit-for-bit what they were.
        //
        // The position argument is the petal's OWN STABLE HUE OFFSET, not its screen position. That is a
        // deliberate departure from how the other families use `tint`, and the reason is that a petal
        // whose colour changed as it drifted across the panel would read as a rendering fault rather
        // than as variety - real blossom on one tree varies from flower to flower, not from place to
        // place. Each petal keeps its hue for its whole life and takes a new one when it is reseeded.
        let petal_dim_a = t.ghost.clamp(0.15, 1.0).max(0.45);
        let secs = dt / 1000.0;
        for i in 0..self.petals.len() {
            if !self.petals[i].live {
                continue;
            }
            let p = &mut self.petals[i];
            p.ph += p.rate * secs;
            p.spin += p.spin_rate * secs;
            // Flutter: each petal's own sine on its own rate, so the field never organises into rain.
            let flutter = (p.ph).sin() * FLUTTER_PX;
            p.x += (self.wind + flutter) * secs;
            p.y += FALL * (0.72 + 0.55 * p.shade) * secs;
            if !p.x.is_finite() || !p.y.is_finite() {
                p.live = false;
                continue;
            }
            if p.x > wf + 4.0 || p.y > hf - 2.0 {
                p.live = false;
                continue;
            }
            let mask = &TUMBLE[((p.spin as i32).rem_euclid(3)) as usize];
            // Nearer petals lighter, and the very lightest catch the light - a shade split rather than a
            // level mapping, so it carries no information and cannot mislead.
            let (hue, shade) = (p.hue, p.shade);
            let body = if shade > 0.86 {
                crate::render::tint(t, hue, d.time_s, true, &t.hot, 1.0)
            } else if shade < 0.32 {
                crate::render::tint(t, hue, d.time_s, false, &t.lit, petal_dim_a)
            } else {
                crate::render::tint(t, hue, d.time_s, false, &t.lit, 1.0)
            };
            let petal_hot = crate::render::tint(t, hue, d.time_s, true, &t.hot, 1.0);
            let (px, py) = (p.x as i32, p.y as i32);
            for (ry, line) in mask.iter().enumerate() {
                for (rx, ch) in line.chars().enumerate() {
                    if ch == '.' {
                        continue;
                    }
                    let col = if ch == '+' { petal_hot } else { body };
                    g.fill_rect(px + rx as i32, py + ry as i32, 1, 1, col);
                }
            }
            let _ = (MASK_W, MASK_H);
        }

        // The petal glow. Bloom the layer, then composite it over the frame - the halo lands behind the
        // petals within the layer and over the sky outside them, which is what makes it visible at all.
        let strength = (t.glow_strength * PETAL_GLOW).clamp(0.0, 1.0);
        if t.bloom > 0.0 && strength > 0.0 {
            g.bloom((t.bloom as i32).max(PETAL_BLOOM_MIN), strength);
        }
        c.draw_over(&g);

        // The bloom spreads a halo outward from every petal, including the ones drifting past the edge,
        // so the last thing that happens is clipping back inside the panel the family drew. Without this
        // a glowing petal at the boundary squares off the rounded corner it just passed.
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
            let shape = (1.0 - f).powf(1.5) * 0.58 + 0.15;
            let wob = 1.0 + 0.32 * ((t_s * 2.2 + f * 7.0).sin());
            *v = ((shape * wob + hit * 0.45) * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = 0.30 * gain;
        d.rms_r = 0.27 * gain;
        d
    }

    fn settled(gain: f32, frames: usize) -> (Blossom, Canvas) {
        let t = builtin::blossom_dusk();
        let mut fam = Blossom::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..frames {
            fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
        }
        (fam, c)
    }

    /// WIND IS THE LEVEL, and this is the load-bearing mapping.
    ///
    /// Mutation: make the wind a constant. The petals still fall and the scene still looks fine, which is
    /// exactly why this needs asserting rather than eyeballing.
    #[test]
    fn louder_music_blows_the_petals_faster() {
        let (quiet, _) = settled(0.15, 400);
        let (loud, _) = settled(0.95, 400);
        assert!(
            loud.wind > quiet.wind * 2.0,
            "wind was {:.1}px/s on quiet music and {:.1} on loud - under 2x the field's slope will not \
             read as a level",
            quiet.wind,
            loud.wind
        );
        assert!(quiet.wind > 1.0, "the wind died completely at low level, which reads as a freeze");
    }

    /// An onset must release petals AND snap the branch - the rhythm is the release pattern.
    ///
    /// Mutation: drop the `release(BURST, ..)` call, or the shake.
    #[test]
    fn a_beat_shakes_the_branch_and_lets_go_of_petals() {
        let t = builtin::blossom_dusk();
        let mut fam = Blossom::default();
        let mut c = Canvas::new(380, 60);
        // Quiet, so few petals are in flight and the burst is measurable against it.
        for k in 0..120 {
            fam.draw(&mut c, &t, &frame(0.12, k as f32 * 0.0167));
        }
        let before = fam.petals.iter().filter(|p| p.live).count();
        // A hard transient: a big jump across every band is what the flux detector fires on.
        let mut d = frame(1.0, 2.1);
        for v in d.levels.iter_mut() {
            *v = 0.95;
        }
        fam.draw(&mut c, &t, &d);
        let after = fam.petals.iter().filter(|p| p.live).count();
        assert!(
            after > before,
            "an onset released nothing: {before} petals in flight before, {after} after"
        );
        // The spring property, which is what was actually asked for: the branch must not merely move,
        // it must come back THROUGH rest and out the other side. The old one-shot decay would pass a
        // "did it move" assertion and fail this one, which is why the assertion is shaped this way.
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        for k in 0..90 {
            fam.draw(&mut c, &t, &frame(0.0, 4.0 + k as f32 * 0.0167));
            lo = lo.min(fam.shake);
            hi = hi.max(fam.shake);
        }
        assert!(lo < -0.4, "the branch never sprang up: lowest {lo:.2}px");
        assert!(hi > 0.4, "the branch never overshot back down: highest {hi:.2}px");
    }

    /// Petals must TUMBLE, not slide. A petal that never changes mask reads as a speck.
    ///
    /// Mutation: hold `spin` constant, or index a single mask.
    #[test]
    fn petals_tumble_through_all_three_masks() {
        let (fam, _) = settled(0.6, 300);
        let live: Vec<&Petal> = fam.petals.iter().filter(|p| p.live).collect();
        assert!(live.len() > 4, "not enough petals in flight to test tumbling: {}", live.len());
        let mut seen = [false; 3];
        for p in &live {
            seen[((p.spin as i32).rem_euclid(3)) as usize] = true;
        }
        assert!(
            seen.iter().filter(|s| **s).count() >= 2,
            "every petal in flight is showing the same face, so nothing is tumbling"
        );
        // And they must not all drift identically - the flutter rates have to differ.
        let rates: Vec<f32> = live.iter().map(|p| p.rate).collect();
        let spread = rates.iter().cloned().fold(f32::MIN, f32::max)
            - rates.iter().cloned().fold(f32::MAX, f32::min);
        assert!(spread > 0.5, "every petal has the same flutter rate ({spread:.2}) - it will read as rain");
    }

    /// The branch is the anchor and must always be there, whatever the audio does.
    ///

    /// The castle must OCCLUDE the moon - asked for as "the moon can be peeking out behind the castle" -
    /// and that deserves a test rather than trust in the arithmetic, because the placement was already
    /// wrong twice by exactly this failure: anchored to a panel fraction it landed ten columns short of
    /// the disc and overlapped nothing at all, while every other assertion about it still passed.
    ///
    /// Peeking means PARTIAL, so this pins both sides: enough of the disc is covered for it to be an
    /// overlap, and enough is left uncovered for it to still be a moon.
    ///
    /// Mutation: send CASTLE_PAST_MOON far enough left to clear the disc, or drop the castle call.
    #[test]
    fn the_castle_stands_in_front_of_the_moon_without_hiding_it() {
        let t = builtin::blossom_dusk();
        let mut fam = Blossom::default();
        let (w, h) = (380, 60);
        let mut c = Canvas::new(w, h);
        for k in 0..30 {
            fam.draw(&mut c, &t, &frame(0.5, k as f32 * 0.0167));
        }

        let moon = Rgba::from_hex(&t.tube.glass, 1.0);
        let body = Rgba::from_hex(&t.tube.internals, 1.0);
        let (mx, my) = ((w as f32 * MOON_X) as i32, (h as f32 * MOON_Y) as i32);
        let (mut disc, mut lit, mut covered) = (0, 0, 0);
        for dy in -MOON_R..=MOON_R {
            for dx in -MOON_R..=MOON_R {
                if dx * dx + dy * dy > MOON_R * MOON_R {
                    continue;
                }
                disc += 1;
                let px = c.get(mx + dx, my + dy);
                if (px.r, px.g, px.b) == (moon.r, moon.g, moon.b) {
                    lit += 1;
                } else if (px.r, px.g, px.b) == (body.r, body.g, body.b) {
                    covered += 1;
                }
            }
        }
        assert!(disc > 200, "the disc probe found no moon to test: {disc} px");
        assert!(
            covered * 100 / disc >= 8,
            "the castle does not cross the moon: {covered}/{disc} px covered"
        );
        assert!(
            lit * 100 / disc >= 35,
            "the castle hides the moon rather than letting it peek out: {lit}/{disc} px still lit"
        );
    }


    // ================= LIGHTNING =================

    /// Rec.709 relative luminance and the WCAG contrast ratio, local so this test depends on nothing
    /// private elsewhere.
    fn rel_lum(c: Rgba) -> f32 {
        let f = |v: u8| {
            let x = v as f32 / 255.0;
            if x <= 0.03928 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
    }

    fn wcag(a: Rgba, b: Rgba) -> f32 {
        let (x, y) = (rel_lum(a), rel_lum(b));
        let (hi, lo) = if x > y { (x, y) } else { (y, x) };
        (hi + 0.05) / (lo + 0.05)
    }

    fn over(bg: Rgba, fg: Rgba) -> Rgba {
        let a = fg.a as f32 / 255.0;
        let m = |f: u8, b: u8| (f as f32 * a + b as f32 * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
        Rgba { r: m(fg.r, bg.r), g: m(fg.g, bg.g), b: m(fg.b, bg.b), a: 255 }
    }

    fn music_fixtures() -> Vec<(&'static str, Vec<Vec<f32>>)> {
        let parse = |csv: &str| -> Vec<Vec<f32>> {
            csv.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.split(',').filter_map(|v| v.parse::<f32>().ok()).collect())
                .collect()
        };
        vec![
            ("steady groove", parse(include_str!("../../tests/fixtures/real-music-bands.csv"))),
            ("dnb, dynamic", parse(include_str!("../../tests/fixtures/real-music-dynamic.csv"))),
            ("flat-mastered", parse(include_str!("../../tests/fixtures/real-music-flat.csv"))),
        ]
    }

    /// Drives the real trigger over a band sequence and returns how many times it fired.
    ///
    /// Goes through `Smoother` with blossom's OWN ballistics, because that is what stands between the
    /// analyser and the family in production - a trigger tested on raw band values is being tested on
    /// data the family never sees.
    fn strikes_over(rows: &[Vec<f32>], frames: usize, strength: f32) -> u32 {
        let nb = crate::dsp::bands::NUM_BANDS;
        let mut sm = crate::dsp::ballistics::Smoother::new(builtin::blossom_dusk().ballistics);
        let mut trig = crate::dsp::flourish::Trigger::default();
        let mut fires = 0;
        for i in 0..frames {
            // Palindromic, so the loop seam is not itself a transient the detector could fire on.
            let period = (rows.len() * 2).max(2);
            let k = i % period;
            let idx = if k < rows.len() { k } else { period - 1 - k };
            let row = &rows[idx.min(rows.len() - 1)];
            let mut target = [0.0f32; crate::dsp::bands::NUM_BANDS];
            for (j, v) in target.iter_mut().enumerate() {
                *v = row.get(j).copied().unwrap_or(0.0);
            }
            sm.update(&target);
            let lv = sm.levels();
            if trig.update(&lv[..STRIKE_BASS_BANDS.min(nb)], 16.7, strength) {
                fires += 1;
            }
        }
        fires
    }

    /// THE guard, and the one this project has failed three times: the trigger must actually FIRE on
    /// real music, and must NOT fire on something merely loud.
    ///
    /// Each fixture is asserted SEPARATELY and never as an aggregate. An aggregate passes while two of
    /// three fixtures give zero, which is exactly how the vaporwave family shipped lightning that never
    /// fired - and how bands 0..12 would pass here while giving 0.00/min on the steady groove.
    ///
    /// Mutation: change STRIKE_BASS_BANDS from 8 to 12 and the steady groove drops to zero. Replace the
    /// median rule with any absolute flux threshold and the same happens.
    #[test]
    fn a_bass_hit_strikes_on_every_kind_of_music_and_a_sustained_level_never_does() {
        let _g = crate::dsp::flourish::test_guard();
        // 300 seconds at 16.7ms.
        let frames = 17_964;
        let mins = frames as f32 * 16.7 / 60_000.0;
        for (name, rows) in music_fixtures() {
            assert!(!rows.is_empty(), "{name}: fixture did not load");
            // The SHIPPED rate, read from the theme rather than written here - otherwise this test
            // passes while the value that actually ships fires zero times.
            let n = strikes_over(&rows, frames, builtin::blossom_dusk().flourish);
            let per_min = n as f32 / mins;
            assert!(
                n > 0,
                "{name}: the strike NEVER fires over {mins:.0} minutes of real music - this is the \
                 failure mode this test exists for"
            );
            // An upper bound too: the minimum gap caps it at 24/min, and anything near that is a strobe
            // rather than weather.
            assert!(
                per_min < 15.0,
                "{name}: strikes far too often at {per_min:.2}/min ({n} in {mins:.0} min)"
            );
        }

        // Loud but structureless must give nothing. The median rule makes this structural rather than
        // tuned: a steady hit has a ratio of 1.0 against its own median, and the floor is above 1.0.
        for (tag, v) in [("pinned", 1.0f32), ("held loud", 0.85), ("silence", 0.0)] {
            let rows = vec![vec![v; crate::dsp::bands::NUM_BANDS]; 64];
            let n = strikes_over(&rows, 6_000, 1.0);
            assert_eq!(n, 0, "{tag}: a sustained level fired {n} strikes");
        }
        // And a metronome - the case a naive detector fires on every beat.
        for period_frames in [15usize, 20, 30, 36, 45] {
            let mut rows = Vec::new();
            for i in 0..(period_frames * 8) {
                let hit = i % period_frames == 0;
                rows.push(vec![if hit { 0.95 } else { 0.18 }; crate::dsp::bands::NUM_BANDS]);
            }
            let n = strikes_over(&rows, 6_000, 1.0);
            assert_eq!(n, 0, "a metronome at {period_frames} frames fired {n} strikes");
        }
    }

    /// Nothing steps. The firing frame draws NOTHING, the rise takes several frames, and the strike then
    /// gets out of the way rather than lingering.
    ///
    /// Mutation: use the envelope level directly instead of `strike_shape` of its age - the first value
    /// becomes 1.0 and the first assertion fails.
    #[test]
    fn the_strike_ramps_rather_than_stepping_and_then_gets_out_of_the_way() {
        assert_eq!(Blossom::strike_shape(0.0), 0.0, "the firing frame is not blank");
        assert_eq!(Blossom::strike_shape(STRIKE_MS), 0.0, "the strike does not end at zero");
        assert_eq!(Blossom::strike_shape(f32::NAN), 0.0, "NaN age leaked a brightness");
        assert_eq!(Blossom::strike_shape(f32::INFINITY), 0.0, "infinite age leaked a brightness");
        assert_eq!(Blossom::strike_shape(-1.0), 0.0, "a negative age leaked a brightness");

        for dt in [16.7f32, 28.4, 33.3, 50.0] {
            let mut env = crate::dsp::flourish::Envelope::default();
            let mut series = Vec::new();
            let lvl = env.update(true, dt, STRIKE_MS);
            series.push(Blossom::strike_shape((1.0 - lvl) * STRIKE_MS));
            for _ in 0..90 {
                let lvl = env.update(false, dt, STRIKE_MS);
                series.push(Blossom::strike_shape((1.0 - lvl) * STRIKE_MS));
            }
            assert_eq!(series[0], 0.0, "dt {dt}: the firing frame was not blank");
            let peak = series.iter().cloned().fold(0.0f32, f32::max);
            assert!(peak > 0.85, "dt {dt}: the strike never got bright, peak {peak:.3}");
            // The rise must take more than one frame at the production rate.
            if dt < 20.0 {
                assert!(
                    series[1] < 0.5,
                    "dt {dt}: the strike jumped to {:.3} in one frame - that is a step",
                    series[1]
                );
            }
            let tail = series.last().copied().unwrap_or(1.0);
            assert!(tail < 0.02, "dt {dt}: the strike never let go, tail {tail:.3}");
        }
    }

    /// The bolt must never cross the moon and never paint on castle stone, at every seed and every size.
    ///
    /// Both are load-bearing. Over the disc the core is measurably INVISIBLE - three of seven colourways
    /// put `hot` within 2.26 dL* of the moon, against the ~2.3 dL* floor this project measured for a
    /// difference being noticeable at all - and the castle's outline is its entire identity.
    ///
    /// Mutation: raise BOLT_SWING_PX to 12, or use the vaporwave family's fractional `w * 0.06` swing
    /// (22.8px at w=380), and pixels land on the disc. Delete the punch loop and they land on stone -
    /// the path crosses stone on EVERY seed, so the punch is load-bearing, not decorative.
    #[test]
    fn the_bolt_never_crosses_the_moon_and_never_paints_on_the_castle() {
        let t = builtin::blossom_dusk();
        let rows = CASTLE.len() as i32;
        let cols = CASTLE.iter().map(|r| r.len()).max().unwrap_or(0) as i32;
        let mut checked = 0;
        for (w, h) in [(380, 60), (190, 60), (120, 60), (380, 40), (380, 34)] {
            let (mx, my) = ((w as f32 * MOON_X) as i32, (h as f32 * MOON_Y) as i32);
            let cx0 = mx + CASTLE_PAST_MOON - cols;
            let cy0 = h - CASTLE_FOOT_INSET - rows;
            for seed in 0..256u32 {
                let mut fam = Blossom { bolt_seed: seed, ..Default::default() };
                fam.bolt_seed = seed;
                let layer = fam
                    .bolt_layer(w, h, &t, 1.0)
                    .unwrap_or_else(|| panic!("{w}x{h} seed {seed}: no bolt built"));
                let mut lit = 0;
                let mut lowest = -1;
                for y in 0..h {
                    for x in 0..w {
                        if layer.get(x, y).a == 0 {
                            continue;
                        }
                        lit += 1;
                        lowest = lowest.max(y);
                        let (dx, dy) = (x - mx, y - my);
                        assert!(
                            dx * dx + dy * dy > MOON_R * MOON_R,
                            "{w}x{h} seed {seed}: bolt pixel ({x},{y}) is on the moon disc"
                        );
                        let (rx, ry) = (x - cx0, y - cy0);
                        if ry >= 0 && ry < rows && rx >= 0 && rx < cols {
                            let line = CASTLE[ry as usize].as_bytes();
                            let stone = (rx as usize) < line.len() && line[rx as usize] == b'#';
                            assert!(!stone, "{w}x{h} seed {seed}: bolt pixel ({x},{y}) is on stone");
                        }
                    }
                }
                assert!(lit > 20, "{w}x{h} seed {seed}: the bolt is only {lit} px");
                // It has to actually REACH the castle, or it is a streak in the sky.
                let roof = Blossom::roof_row(w, h, mx + BOLT_ANCHOR_OFF).unwrap();
                assert_eq!(
                    lowest,
                    roof - 1,
                    "{w}x{h} seed {seed}: the bolt stops at row {lowest}, roofline is {roof}"
                );
                checked += 1;
            }
        }
        assert!(checked >= 1280, "the seed sweep barely ran: {checked}");

        // Where the castle sheds, there must be no bolt hanging in empty sky.
        for (w, h) in [(380, 33), (380, 30), (120, 28)] {
            let fam = Blossom::default();
            assert!(
                fam.bolt_layer(w, h, &t, 1.0).is_none(),
                "{w}x{h}: a bolt was drawn with no castle to strike"
            );
        }
    }

    /// A strike must do BOTH things: light the sky and mark it. Two independent metrics, so deleting
    /// either half fails.
    ///
    /// Mutation: delete the top-stop lift and (a) fails while (b) passes. Delete the `draw_over` of the
    /// bolt layer and (b) fails while (a) passes.
    #[test]
    fn a_strike_both_lights_the_sky_and_marks_it() {
        let t = builtin::blossom_dusk();
        let (w, h) = (380, 60);
        let mut fam = Blossom::default();
        let mut c = Canvas::new(w, h);
        for k in 0..60 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        // (a) the sky's own brightness, sampled where nothing else is drawn: the top rows, taking the
        // MINIMUM across the row so a petal drifting through can only be brighter, never darker.
        let sky_floor = |c: &Canvas| -> f32 {
            let mut lo = f32::MAX;
            for x in 150..300 {
                lo = lo.min(rel_lum(c.get(x, SKY_TOP_ROW)));
            }
            lo
        };
        let calm_sky = sky_floor(&c);
        let calm_bits: Vec<u32> = c.bits().to_vec();

        fam.flourish.force_next();
        let mut best_sky = calm_sky;
        let mut most_changed = 0;
        for k in 0..90 {
            fam.draw(&mut c, &t, &frame(0.6, 1.02 + k as f32 * 0.0167));
            best_sky = best_sky.max(sky_floor(&c));
            let changed = c.bits().iter().zip(calm_bits.iter()).filter(|(a, b)| a != b).count();
            most_changed = most_changed.max(changed);
        }
        assert!(
            best_sky > calm_sky * 1.20,
            "the sky never lit: calm {calm_sky:.5} -> peak {best_sky:.5}"
        );
        // (b) the bolt itself: a corridor of pixels near the anchor column must go bright.
        let mx = (w as f32 * MOON_X) as i32;
        let anchor = mx + BOLT_ANCHOR_OFF;
        let mut bolt_px = 0;
        for y in SKY_TOP_ROW..40 {
            for x in anchor - BOLT_SWING_PX - 2..=anchor + BOLT_SWING_PX + 2 {
                if rel_lum(c.get(x, y)) > 0.35 {
                    bolt_px += 1;
                }
            }
        }
        assert!(most_changed > 400, "the strike barely changed the panel: {most_changed} px");
        assert!(bolt_px > 15, "no bolt was drawn in the corridor: {bolt_px} bright px");
    }

    /// The flash must not eat the reading. The load-bearing element here is the PETAL field, and no
    /// sky-only metric can see it - so this scores the DIMMEST petal against the lit sky behind it, on
    /// every colourway and at several heights.
    ///
    /// Mutation: raise FLASH_PEAK to 1.0 and six of seven colourways fail. Lift BOTH sky stops and dusk
    /// drops 3.79 -> 3.41.
    #[test]
    fn the_dim_petals_still_read_at_the_flash_peak_on_every_colourway() {
        for t in builtin::all().into_iter().filter(|t| t.family == "blossom") {
            let (w, h) = (380, 60);
            let mut fam = Blossom::default();
            let mut c = Canvas::new(w, h);
            for k in 0..60 {
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            }
            // Score the calm frame first, then the peak, and compare the DELTA - an absolute floor would
            // fail on a colourway whose calm value is already low, which is a pre-existing property and
            // not something the flash causes.
            let dim = Rgba::from_hex(&t.lit, t.ghost.clamp(0.15, 1.0).max(0.45));
            let score = |c: &Canvas| -> f32 {
                let mut worst = f32::MAX;
                for row in [4, 12, 20, 28, 40, 52] {
                    // The sky behind, taken as the row minimum so a petal cannot flatter it.
                    let mut sky = Rgba::new(255, 255, 255, 255);
                    let mut lo = f32::MAX;
                    for x in 150..300 {
                        let px = c.get(x, row);
                        if rel_lum(px) < lo {
                            lo = rel_lum(px);
                            sky = px;
                        }
                    }
                    worst = worst.min(wcag(over(sky, dim), sky));
                }
                worst
            };
            let calm = score(&c);
            fam.flourish.force_next();
            let mut flashed = calm;
            for k in 0..8 {
                fam.draw(&mut c, &t, &frame(0.6, 1.02 + k as f32 * 0.0167));
                flashed = flashed.min(score(&c));
            }
            assert!(
                calm - flashed < 0.20,
                "{}: the flash cost the dim petal {:.2} of contrast (calm {calm:.2} -> {flashed:.2})",
                t.id,
                calm - flashed
            );
        }
    }

    /// The flash must leave NO RESIDUE: once the strike has decayed, the sky is exactly what it was.
    ///
    /// That is what makes the calm frame byte-identical to the shipped gradient - the lift is applied to
    /// the top stop only and reduces to the identity at zero, so there is nothing to leak.
    ///
    /// Mutation: implement the falloff with an extra interpolated stop instead of lifting the top one,
    /// and the calm frame stops matching. Clamp `bright` to a floor above 0 and this fails.
    #[test]
    fn the_flash_leaves_no_residue_once_the_strike_has_decayed() {
        for t in builtin::all().into_iter().filter(|t| t.family == "blossom") {
            for (w, h) in [(380, 60), (190, 60)] {
                let mut fam = Blossom::default();
                let mut c = Canvas::new(w, h);
                for k in 0..60 {
                    fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
                }
                let row_of = |c: &Canvas| -> Vec<(u8, u8, u8)> {
                    (20..(w - 40)).map(|x| {
                        let p = c.get(x, SKY_TOP_ROW);
                        (p.r, p.g, p.b)
                    }).collect()
                };
                // The row minimum is the sky; petals only brighten. Compare the whole row's darkest
                // value, which is petal-proof.
                let darkest = |c: &Canvas| -> (u8, u8, u8) {
                    row_of(c).into_iter().min_by(|a, b| {
                        let la = rel_lum(Rgba::new(a.0, a.1, a.2, 255));
                        let lb = rel_lum(Rgba::new(b.0, b.1, b.2, 255));
                        la.partial_cmp(&lb).unwrap_or(std::cmp::Ordering::Equal)
                    }).unwrap_or((0, 0, 0))
                };
                let calm = darkest(&c);
                fam.flourish.force_next();
                // Well past STRIKE_MS: 200 frames at 16.7ms is 3.34 seconds.
                for k in 0..200 {
                    fam.draw(&mut c, &t, &frame(0.6, 1.02 + k as f32 * 0.0167));
                }
                let after = darkest(&c);
                assert_eq!(
                    calm, after,
                    "{} at {w}x{h}: the sky did not return to its calm colour after the strike",
                    t.id
                );
            }
        }
    }

    /// The first drawn frame after the panel was hidden must not spend a strike.
    ///
    /// The smoother runs while the family does not draw, and the reveal gate holds the family off for
    /// 400ms, so that frame reads a whole returning track as one jump. MEASURED before the guard: it
    /// fired on 4 of 4 hide/reveal cycles on every fixture, at a gate opacity of 0.037 - guaranteed
    /// invisible, and it would spend the 2500ms minimum gap on the way.
    ///
    /// Mutation: delete the `if stale { fired = false; }` block and the first assertion fails.
    #[test]
    fn the_first_drawn_frame_after_the_panel_was_hidden_does_not_spend_a_strike() {
        let t = builtin::blossom_dusk();
        let mut fam = Blossom::default();
        let mut c = Canvas::new(380, 60);

        // Frame 1 establishes `last_seen_s`.
        fam.draw(&mut c, &t, &frame(0.6, 0.0));
        // Frame 2 arrives five seconds later in DRAWN time - the panel was hidden - and asks to fire.
        fam.flourish.force_next();
        fam.draw(&mut c, &t, &frame(0.6, 5.0));
        assert_eq!(
            fam.strike.level(),
            0.0,
            "a strike was spent on the first frame back after the panel was hidden"
        );
        // Frame 3 follows normally, and the strike must work again.
        fam.flourish.force_next();
        fam.draw(&mut c, &t, &frame(0.6, 5.0167));
        assert!(
            fam.strike.level() > 0.0,
            "the stale guard suppressed a legitimate strike on the very next frame"
        );
    }

    /// A hostile frame must not turn the sky white. This is a real mechanism, not a hypothetical:
    /// `f32::clamp` returns NaN unchanged, `NaN <= 0.0` is false, and `f32::NAN.min(255.0)` is 255 - so
    /// an unsanitised brightness reaching the flash makes 89% of the panel solid white every frame,
    /// with no error anywhere.
    #[test]
    fn a_hostile_frame_cannot_whiten_the_sky() {
        let t = builtin::blossom_dusk();
        let mut fam = Blossom::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..30 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        for bad_dt in [f32::NAN, f32::INFINITY, -1.0, 1.0e30] {
            let mut d = frame(0.6, 1.0);
            d.dt_ms = bad_dt;
            d.levels[0] = f32::NAN;
            d.levels[2] = f32::INFINITY;
            fam.draw(&mut c, &t, &d);
            let mut white = 0;
            for y in 3..56 {
                for x in 20..340 {
                    let p = c.get(x, y);
                    if p.r > 250 && p.g > 250 && p.b > 250 {
                        white += 1;
                    }
                }
            }
            assert!(white < 400, "dt {bad_dt}: {white} pixels went white - the sky was whitened");
        }
    }


    /// What the strike trigger actually does over the real-music fixtures. A measurement, not a gate.
    #[test]
    #[ignore]
    fn probe_strike_rate() {
        let _g = crate::dsp::flourish::test_guard();
        println!("enabled() = {}", crate::dsp::flourish::enabled());
        let frames = 17_964;
        let mins = frames as f32 * 16.7 / 60_000.0;
        for bands in [2usize, 3, 4, 5, 6, 8, 12, 64] {
            for strength in [0.05f32, 0.10, 0.20, 0.45, 1.0] {
                let mut out = String::new();
                for (name, rows) in music_fixtures() {
                    let nb = crate::dsp::bands::NUM_BANDS;
                    let mut sm =
                        crate::dsp::ballistics::Smoother::new(builtin::blossom_dusk().ballistics);
                    let mut trig = crate::dsp::flourish::Trigger::default();
                    let mut fires = 0;
                    for i in 0..frames {
                        let period = (rows.len() * 2).max(2);
                        let k = i % period;
                        let idx = if k < rows.len() { k } else { period - 1 - k };
                        let row = &rows[idx.min(rows.len() - 1)];
                        let mut target = [0.0f32; crate::dsp::bands::NUM_BANDS];
                        for (j, v) in target.iter_mut().enumerate() {
                            *v = row.get(j).copied().unwrap_or(0.0);
                        }
                        sm.update(&target);
                        let lv = sm.levels();
                        if trig.update(&lv[..bands.min(nb)], 16.7, strength) {
                            fires += 1;
                        }
                    }
                    out += &format!("{:>6.2} ", fires as f32 / mins);
                    let _ = name;
                }
                println!("bands {bands:>2} strength {strength:<5} -> {out}(groove dnb flat)");
            }
        }
    }

    /// Mutation: gate the branch on level, or let the bend/shake carry it off the panel.
    #[test]
    fn the_branch_is_drawn_at_every_level_and_stays_on_the_panel() {
        for gain in [0.0f32, 0.5, 1.0] {
            let (fam, c) = settled(gain, 260);
            let mut bark = 0;
            for y in 0..60 {
                for x in 0..380 {
                    let px = c.get(x, y);
                    if px.a > 0 && (px.r as i32 + px.g as i32 + px.b as i32) > 40 {
                        bark += 1;
                    }
                }
            }
            assert!(bark > 150, "at gain {gain} only {bark} lit pixels - the branch is missing");
            let y0 = fam.spine_y(0.0, 380.0, 60.0);
            let y1 = fam.spine_y(380.0 * 0.72, 380.0, 60.0);
            assert!(
                (2.0..56.0).contains(&y0) && (2.0..56.0).contains(&y1),
                "the branch left the panel at gain {gain}: spine {y0:.1}..{y1:.1}"
            );
        }
    }

    /// Run: cargo test --release dump_blossom -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_blossom() {
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
        for t in builtin::all().into_iter().filter(|t| t.family == "blossom") {
            let mut fam = Blossom::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..420 {
                fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            }
            write(format!("blossom-{}", t.id), &c);
            // FORCED, because the fixture cannot produce a strike: `frame` builds a 120bpm metronome of
            // identical hits, and a steady hit has a ratio of 1.0 against its own median while the
            // flourish floor is 1.30 - so it can never fire, and an unforced dump would show a
            // lightning-free panel on every colourway and look entirely correct.
            fam.flourish.force_next();
            for k in 420..425 {
                fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            }
            write(format!("blossom-strike-{}", t.id), &c);
        }
        // Quiet against loud, so the wind mapping is visible as a difference.
        let t = builtin::blossom_dusk();
        for (gain, tag) in [(0.15f32, "calm"), (0.95, "gale")] {
            let mut fam = Blossom::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..500 {
                fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
            }
            write(format!("blossom-wind-{tag}"), &c);
        }
        println!("wrote blossom dumps");
    }
}
