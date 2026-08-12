//! A flame organ: a manifold of gas nozzles along the bottom, each burning to a height set by its band.
//!
//! # What it is pretending to be
//!
//! A Rubens' tube - a perforated pipe fed with gas and driven by sound, where the flame height traces
//! the standing wave inside. It is a real instrument, which is the bar every family here has to clear,
//! and it is the only one whose reading is carried by something that looks alive.
//!
//! # Why this is cheap
//!
//! It is not a fluid simulation and does not need to be. The whole effect is the classic bottom-seeded
//! heat-diffusion buffer: every cell takes a weighted average of the three cells below it, loses a fixed
//! amount to cooling, and the bottom row is seeded from the band levels. One pass over the panel
//! interior is about 10,000 cells of a handful of float operations - the same cost class as the
//! spectrogram's history buffer, which already ships. Navier-Stokes would buy nothing a 60px-tall panel
//! could show.
//!
//! # Why the cooling SUBTRACTS rather than multiplies
//!
//! This is the one decision the family's legibility rests on. A multiplicative decay makes plume height
//! logarithmic in the seed, so a band at 1.0 would stand only a little taller than one at 0.2 and the
//! display would read as brightness rather than as height. Subtracting a constant per row makes height
//! LINEAR in the seed - `rows = seed / COOL` - so the plumes are a profile you can read across, which is
//! the same position-over-intensity rule the nixie and valve families are built on.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// Horizontal pitch of the nozzles, in pixels.
///
/// 14 gives 12 plumes at 190px and 26 at 380px. Wider and the panel reads as a handful of separate
/// fires rather than an instrument with a scale; narrower and the plumes merge into one sheet, which
/// loses the per-band reading entirely.
const NOZZLE_PITCH: i32 = 14;

/// Width of the seeded core under each nozzle, in pixels.
///
/// 5, and the first attempt at 3 is why. A 3px core plus a sideways sampling offset means most cells
/// sample the empty columns beside the plume, so it bleeds its heat away and dies within a dozen rows -
/// rendered, that read as scattered embers rather than as flames. 5 gives the column enough body to
/// survive both the diffusion and the lick, and is still far narrower than the 14px pitch, which is what
/// keeps the plumes separable.
const SEED_W: i32 = 5;

/// Heat lost per row of rise.
///
/// Heat lost per row of rise, ON TOP of what diffusion already loses.
///
/// `1.0 / COOL` is NOT the plume height, and assuming it was cost this family most of its range. The
/// three-tap average bleeds heat into the cold columns either side of a plume, so a narrow plume decays
/// far faster than this constant alone implies - the two effects were being double-counted.
///
/// Measured at 0.023: the tip moved from row 50 at silence to row 41 at a 0.70 band, which is NINE rows
/// of travel in a 51-row interior. The family's whole claim is that height is linear in the band and
/// readable as a profile across the manifold, and nine pixels does not deliver it.
///
/// 0.007 lets the sideways bleed be the dominant decay, which is the honest model: it is what actually
/// shapes the plume, and it tapers naturally because a plume's edges are always mixing with cold air.
const COOL: f32 = 0.011;

/// How much of each cell's heat comes from the cell directly below, against its two diagonal
/// neighbours.
///
/// 0.68 to the centre keeps a plume narrow enough to stay separable from its neighbour; nearer 0.33
/// (a flat average) and it spreads into a dome within a dozen rows. This is the knob that decides
/// whether the display reads as twelve flames or as one fire.
///
/// Raised from 0.55 to sharpen the edges. Side bleed is what turns a plume's boundary into a gradient,
/// and a gradient at this size reads as watercolour rather than as fire - the same reason `ZONES` below
/// posterises the field instead of drawing it continuously.
///
/// 0.62 rather than the 0.68 first tried: with the field posterised as well, that much centre bias made
/// the plumes solid wedges. Sharpening the edges and thickening the body are different things, and only
/// the first was wanted.
const CENTRE_BIAS: f32 = 0.62;

/// ADVECTION: how many rows of field a cell's heat rises per frame, at the burner and at the top.
///
/// THIS IS THE FIX FOR THE BULBOUS TIPS, and it took three attempts to get to the right mechanism, so
/// the two dead ends are recorded here rather than rediscovered:
///
/// 1. **Extra cooling with height.** At 1.4 it truncated the plumes with hard flat tops, which is worse
///    than a bulb; at 0.45 it was indistinguishable from doing nothing. Cooling decides where a plume
///    ENDS and says nothing about its shape on the way there.
/// 2. **More sideways spread with height** - the entrainment a real flame has. No visible difference at
///    a setting that took the centre bias from 0.62 to 0.38 at the tip.
///
/// Both were tuning a diffusion that had no velocity in it, which is why neither could work. Every cell
/// took its heat from the row DIRECTLY below, so heat moved up exactly one row per frame while spreading
/// sideways the whole time - it could never outrun its own spreading, and a plume that spreads as fast as
/// it climbs is a dome by construction.
///
/// A real flame stretches because the gas rises faster than it mixes, and buoyancy means it ACCELERATES
/// as it goes. So a cell now takes its heat from `RISE` rows below, growing to `RISE + RISE_GAIN` at the
/// top of the panel. The sideways mixing is unchanged; what changes is how much height it is spread over,
/// which is the taper.
///
/// 1.0 at the burner, so the base behaves as it always did and the pilot flames are unaffected. 1.9 more
/// at the top, so the tip is stretched almost three times as far as the root - enough to pull a bulb into
/// a wisp without the plume detaching into separate blobs, which is what happened past about 3.
const RISE: f32 = 1.0;
const RISE_GAIN: f32 = 1.9;

/// Peak sideways lick, in pixels of sampling offset.
///
/// What stops the plumes being static triangles - but it has to be COHERENT. The first version drew the
/// offset from a per-cell hash, which is not a lick at all: neighbouring cells sampled in different
/// directions, so the plume scattered its own heat sideways instead of leaning. Rendered, that gave
/// embers rather than flames.
///
/// Two smooth waves instead, one travelling up the plume and one slower and wider, so the column bends
/// as a body and the bends themselves drift. 0.95px of peak offset, which at a 5px core keeps most of the
/// sampling inside it.
///
/// The waves are phased PER NOZZLE. Without that they are functions of height and time alone, so every
/// plume on the panel leans the same way at the same moment - rendered, that read as a gust of wind
/// across the whole manifold rather than as twelve independent flames, which is a different and much
/// less interesting picture.
const LICK: f32 = 0.95;

/// Heat below which nothing is drawn at all, and the width of the band over which the edge fades in.
///
/// The history here is worth keeping, because the family has been wrong in both directions. The first
/// version drew anything above 0.02 at a 10% alpha minimum, which painted a faint envelope two or three
/// pixels beyond every plume - watercolour. The second snapped the field to five discrete levels at full
/// alpha, which fixed that and introduced banding: crisp, but pixel art rather than fire.
///
/// Neither extreme is what a flame looks like. A real edge is continuous but NARROW, so this is a
/// smoothstep over `EDGE` of heat - about two pixels of falloff at this cooling rate. Continuous enough
/// to be smooth, tight enough not to be a wash.
const FLOOR: f32 = 0.11;
const EDGE: f32 = 0.10;

/// Opacity of the flame body, from its coolest visible heat to its hottest.
///
/// NEVER 1.0, and that is the difference between "a flame" and "a ghostly flame". A gas flame is a
/// luminous volume you can see through, not a solid shape - so faintness comes from ALPHA here, and the
/// ramp is free to stay pale. The previous version made faintness come from COLOUR instead, with a very
/// dark red at the cool end, which read as heavy and solid rather than insubstantial.
///
/// The distinction from the watercolour version two passes ago matters and is easy to lose: that was soft
/// EDGES plus a wide low-alpha fringe. This is a translucent BODY with the edge still tight. Softness in
/// the alpha, not in the falloff.
const BODY_ALPHA_LOW: f32 = 0.34;
const BODY_ALPHA_HIGH: f32 = 0.80;

/// Opacity of the core. Also short of 1.0, for the same reason - a fully opaque core is a hole punched in
/// the glow behind it, and the glow is most of what makes this read as light.
const CORE_ALPHA: f32 = 0.88;

/// Heat at or above which a cell is treated as the flame's core, and blooms.
///
/// Only the core goes on the bloomed layer. Blooming the whole field - which is what the first version
/// did - haloes the cool outer zones too, and a halo around an already-soft edge is precisely the
/// watercolour effect. The pattern here is the one `patchbay` uses for its cables and `tube` for its
/// cathodes: the opaque body goes straight onto the panel, and only the light blooms.
const CORE_HEAT: f32 = 0.62;

/// The warmth: how far the flames' light spills, and how strongly.
///
/// A fire feels warm because it LIGHTS THE ROOM, not because its own edges are soft - and that
/// distinction is the whole design here. The first version had no spill at all and read as cold cut-out
/// shapes; the version before that got its warmth from softening the flame itself, which read as
/// watercolour. Neither is what a fire looks like.
///
/// So the spill is a separate layer: the whole field, bloomed WIDE, composited BEFORE the crisp body.
/// `Canvas::bloom` puts its halo under existing content and the body is opaque, so the halo survives
/// only in the air around the plumes - hard edges, warm surroundings. Ordering is doing all the work.
///
/// TWO STAGES, because one is not convincing. A single bloom gives either a tight rim or a wide wash and
/// neither reads as heat: real glow is a bright halo close in plus a faint one reaching much further.
/// A single stage at radius 7 was tried and measured too timid to feel warm at all.
///
/// Both are drawn before the opaque body, so pushing them hard costs no crispness - which is the whole
/// reason the layers were separated in the first place. The colourway's own `bloom` is not used for
/// either: it is tuned for point sources like a cathode or a lamp, and this is a field of light.
const SPILL_NEAR_RADIUS: i32 = 3;
const SPILL_NEAR_ALPHA: f32 = 0.95;
const SPILL_FAR_RADIUS: i32 = 11;
const SPILL_FAR_ALPHA: f32 = 0.55;

/// How often the WIDE spill is recomputed, in frames.
///
/// Measured, because this family is the most expensive one here and it was worth knowing why before
/// guessing: a frame costs 2.81ms with both spills and 1.09ms with neither, so the glow is 61% of it -
/// and `bloom` is separable, so cost is linear in radius, which makes the radius-11 pass about
/// three-quarters of that on its own.
///
/// Recomputed every third frame and cached in between. That is a 20Hz ambient glow under a 60Hz flame,
/// and it is invisible: the wide spill is a diffuse field with no edges to alias, and what little lag it
/// gains reads as thermal inertia, which a real fire has. The NEAR spill is recomputed every frame - it
/// hugs the plume closely enough that lag there would show as the halo detaching.
///
/// Brings the family from 2.81ms a frame to about 1.9ms, which is level with `segmented`.
const SPILL_FAR_EVERY: u32 = 3;

/// How much the flames warm the panel behind them, at full drive.
///
/// The last of the three warmth cues and the most diffuse: a fire does not only halo, it raises the
/// ambient level of everything near it. A vertical gradient from the manifold upward, scaled by how hard
/// the whole manifold is burning, so a quiet passage leaves the panel cold and a loud one has the metal
/// glowing. 0.20 - enough to feel, far too little to read as a lit panel.
const EMBER_ALPHA: f32 = 0.20;

/// How brightly the plumes light the manifold they stand on.
///
/// The single most effective warmth cue for the least work, and the one a real photograph always has:
/// the metal directly under a flame is lit by it. Sampled per column from the heat just above the pipe,
/// so a tall plume lights its own nozzle brightly and a pilot flame barely at all - which also makes the
/// manifold a second, redundant reading of the spectrum.
const MANIFOLD_LIT: f32 = 0.85;

/// The flourish: a flashback, then a relight that travels along the manifold.
///
/// The gas pressure drops, every plume guts down to its pilot, and then the flame front runs back along
/// the pipe relighting each nozzle in turn with a brief overshoot before it settles. It is both the fault
/// AND the ritual, which is the doubling the VFD self-test has - and it costs nothing extra per frame,
/// because it drives the seeding that was already there rather than drawing anything new.
///
/// 1600ms in three parts. `SNUFF_FRAC` of it is the pressure loss, during which the burners are held at
/// pilot height; the rest is the front travelling the full width. The front is what makes this read as a
/// gas fault rather than as the audio dropping out - a simultaneous relight would look like a cut.
///
/// The overshoot is 1.6x, because a burner that has been starved of gas and then relit flares above its
/// steady height for a moment. That flare is the part the eye actually catches.
const FLASH_MS: f32 = 1600.0;
const SNUFF_FRAC: f32 = 0.22;
const RELIGHT_OVERSHOOT: f32 = 1.6;
/// How wide the travelling front is, as a fraction of the manifold.
///
/// 0.18 - narrow enough to read as a front moving rather than as everything brightening, wide enough that
/// it does not skip a nozzle at the panel's 12-burner resolution.
const FRONT_W: f32 = 0.18;

/// Response window, in band-level units. Matches the other families' convention - see `patchbay`'s
/// `RESP_FLOOR` note for why this is placed on what the DSP actually produces rather than on 0..1.
const RESP_FLOOR: f32 = 0.10;
const RESP_SPAN: f32 = 0.62;

/// Nozzles at a given panel width.
fn nozzle_count(w: i32) -> usize {
    (((w - 8) / NOZZLE_PITCH).max(2) as usize).min(32)
}

/// Centre x of nozzle `i` of `n`.
fn nozzle_x(w: i32, n: usize, i: usize) -> i32 {
    let span = (w - 8) as f32 / n.max(1) as f32;
    4 + (span * (i as f32 + 0.5)) as i32
}

/// Maps a band level onto 0..=1 of plume height.
fn response(level: f32, sensitivity: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    (((level - RESP_FLOOR) / RESP_SPAN) * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

/// Deterministic 0..1 from three integers, for the flicker and the lick.
///
/// Keyed on the frame as well as the position, unlike the patchbay's brushed-metal grain which must
/// NOT crawl. Here crawling is the entire point: a fire that does not move is a triangle.
fn hash01(x: i32, y: i32, f: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add((y as u32).wrapping_mul(0x85EB_CA6B))
        .wrapping_add(f.wrapping_mul(0xC2B2_AE35));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    (h >> 8) as f32 / 16_777_216.0
}

#[derive(Default)]
pub struct Flame {
    /// Heat field over the panel interior, row 0 at the TOP. `w * h` cells.
    heat: Vec<f32>,
    /// The flourish: a flashback and a travelling relight. See `FLASH_MS`.
    flourish: crate::dsp::flourish::Trigger,
    flash: crate::dsp::flourish::Envelope,
    w: i32,
    h: i32,
    frame: u32,
    /// The wide spill, kept between frames. See `SPILL_FAR_EVERY`.
    far_glow: Option<Canvas>,
}

impl Flame {
    /// The band level driving nozzle `i`, biased toward the group's peak so one loud band in the group
    /// still lifts the plume - the same reduction the patchbay and Pantone families use.
    fn level_for(d: &FrameData, i: usize, n: usize) -> f32 {
        let len = d.levels.len();
        let n = n.max(1);
        let lo = i * len / n;
        let hi = (((i + 1) * len / n).max(lo + 1)).min(len);
        let (mut acc, mut cnt, mut peak) = (0.0f32, 0.0f32, 0.0f32);
        for v in &d.levels[lo..hi] {
            // is_finite BEFORE anything accumulates: f32::clamp does not sanitise NaN, and one poisoned
            // band would otherwise reach the heat field and stay there for the life of the process.
            if v.is_finite() {
                acc += *v;
                cnt += 1.0;
                peak = peak.max(*v);
            }
        }
        if cnt <= 0.0 {
            return 0.0;
        }
        (acc / cnt * 0.35 + peak * 0.65).clamp(0.0, 1.0)
    }

    fn at(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return 0.0;
        }
        self.heat[(y * self.w + x) as usize]
    }

    /// The field sampled at a FRACTIONAL x, interpolated between the two neighbouring cells.
    ///
    /// This is most of what separates "a flame" from "pixel art of a flame". The lean was previously
    /// applied as `lick.round()`, which snaps the sampling to whole cells - so a plume bending gradually
    /// moves in discrete one-pixel jumps and manufactures a staircase along both its edges. Nothing about
    /// the physics is stepped; the rounding was.
    ///
    /// Bilinear, in both axes.
    ///
    /// It was linear in x only while the source row was always the one directly below - interpolating
    /// vertically then would have blurred the plume along the axis it travels, for no gain. With
    /// advection the source row is fractional and several rows down (see `RISE`), so vertical
    /// interpolation is resampling a moving field rather than blurring a static one. Without it the
    /// stretch quantises to whole rows and reintroduces exactly the banding that fractional sampling in x
    /// was added to remove.
    fn at_frac(&self, x: f32, y: f32) -> f32 {
        if !x.is_finite() || !y.is_finite() {
            return 0.0;
        }
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (xi, yi) = (x0 as i32, y0 as i32);
        let top = {
            let a = self.at(xi, yi);
            let b = self.at(xi + 1, yi);
            a + (b - a) * fx
        };
        let bot = {
            let a = self.at(xi, yi + 1);
            let b = self.at(xi + 1, yi + 1);
            a + (b - a) * fx
        };
        top + (bot - top) * fy
    }

    /// One diffusion pass, then the seeding.
    ///
    /// Iterated from the TOP down, reading the row below. That order is what makes it safe in place: the
    /// row being read has not been written yet this frame, so every cell sees the previous frame's heat.
    /// Bottom-up would read cells it had already updated and the fire would shoot to the top in one
    /// frame.
    /// The seed multiplier for nozzle `i` of `n` during the flourish, or 1.0 when it is not running.
    ///
    /// Pure, and separated from `advance` so it can be tested as a shape rather than inferred from
    /// pixels: the interesting claims are that everything drops together, that the front crosses the
    /// manifold once, and that it ends with every burner back at its own level.
    ///
    /// `flash` runs 1.0 -> 0.0, so `t` here is elapsed progress, 0.0 -> 1.0.
    fn flash_gain(flash: f32, i: usize, n: usize) -> f32 {
        if flash <= 0.0 {
            return 1.0;
        }
        let t = 1.0 - flash.clamp(0.0, 1.0);
        if t < SNUFF_FRAC {
            // The pressure loss. Everything down to the pilot at once - a gas manifold does not lose
            // pressure one burner at a time.
            return 0.0;
        }
        // The front, sweeping left to right across the remainder.
        let sweep = (t - SNUFF_FRAC) / (1.0 - SNUFF_FRAC);
        let pos = (i as f32 + 0.5) / n.max(1) as f32;
        if pos > sweep {
            // Not reached yet: still out.
            return 0.0;
        }
        // Lit. How long ago the front passed, in units of the front's own width.
        let since = (sweep - pos) / FRONT_W;
        if since >= 1.0 {
            1.0
        } else {
            // The overshoot flare, decaying to the burner's steady height.
            1.0 + (RELIGHT_OVERSHOOT - 1.0) * (1.0 - since)
        }
    }

    /// `cw`/`ix` are the CANVAS width and the field's left inset, not the field's own width.
    ///
    /// The nozzle geometry has to be computed in canvas coordinates, because that is where the manifold
    /// and its stubs are drawn. Seeding from the field's own width was a real misalignment bug: the
    /// interior is 8px narrower than the canvas, and `nozzle_count` of 182 is TWELVE where
    /// `nozzle_count` of 190 is THIRTEEN - so the panel drew thirteen nozzles and only twelve of them
    /// had a flame, with the rest progressively offset from their stubs. Caught by a test asserting
    /// every burner is lit, which found burner 6 with zero heat at its mouth.
    fn advance(&mut self, d: &FrameData, sens: f32, dt: f32, cw: i32, ix: i32) {
        let (w, h) = (self.w, self.h);
        let cool = COOL * dt.clamp(0.25, 4.0);
        self.frame = self.frame.wrapping_add(1);
        for y in 0..(h - 1) {
            for x in 0..w {
                // The lick: two smooth waves, so the plume bends as a body, PHASED PER NOZZLE so the
                // plumes do not all lean together. Coherent in x and y by construction - see `LICK` for
                // what the incoherent version looked like, and for what the unphased one did.
                let fy = y as f32;
                let ft = self.frame as f32;
                // A phase per nozzle, from an irrational-ish multiple of its index so neighbours are
                // never close to in step.
                let ph = (x / NOZZLE_PITCH.max(1)) as f32 * 2.399;
                let lick = LICK
                    * ((fy * 0.28 - ft * 0.13 + ph).sin() * 0.7
                        + (fy * 0.09 + ft * 0.05 + ph * 1.7).sin() * 0.5);
                // Sampled at the FRACTIONAL offset - see `at_frac` for why rounding it was the main
                // source of the stepped look.
                let sx = x as f32 + lick;
                // ADVECTION. The source row is `rise` below, not one below, and `rise` grows toward the
                // top because buoyant gas accelerates. See `RISE`.
                let high = 1.0 - y as f32 / (h - 1).max(1) as f32;
                let rise = RISE + RISE_GAIN * high;
                let sy = y as f32 + rise;
                let below = self.at_frac(sx, sy);
                let l = self.at_frac(sx - 1.0, sy);
                let r = self.at_frac(sx + 1.0, sy);
                let side = (1.0 - CENTRE_BIAS) * 0.5;
                let mixed = below * CENTRE_BIAS + l * side + r * side;
                // Flicker in the COOLING rather than in the heat: cooling more on some cells eats holes
                // in the plume the way a real flame breaks up, where adding heat would only make it
                // sparkle.
                // Gently. 0.65-1.35 per cell per frame punched the plume full of holes on its own, which
                // is most of why the first render looked like sparks rather than fire.
                let flick = 0.88 + 0.24 * hash01(x, y, self.frame ^ 0x5BF0_3635);
                // Cooling scales with the distance RISEN, not with the frame. A cell that has just
                // travelled three rows has lost three rows' worth of heat, and without this the height
                // scale would depend on the advection rate - `seed / COOL` would stop meaning rows, which
                // is the property the whole family is built on.
                let v = mixed - cool * flick * rise;
                self.heat[(y * w + x) as usize] = if v.is_finite() { v.max(0.0) } else { 0.0 };
            }
        }
        // Seed the bottom row from the bands.
        let n = nozzle_count(cw);
        for cell in self.heat[((h - 1) * w) as usize..].iter_mut() {
            *cell = 0.0;
        }
        for i in 0..n {
            let cx = nozzle_x(cw, n, i) - ix; // canvas geometry, converted to interior coordinates
            let lvl = response(Self::level_for(d, i, n), sens);
            // A floor, so a lit burner never goes out: an unlit nozzle on a gas manifold reads as
            // broken, which is the same reason the valve row keeps a heater floor and the reel keeps
            // turning at silence.
            // The 0.20 floor is a PILOT FLAME and it has to clear `FLOOR`, or a quiet burner draws
            // nothing and the manifold reads as switched off. Posterising the field made this a real
            // regression rather than a theoretical one: at a 0.06 floor the quiet case rendered as bare
            // nozzle stubs, because 0.06 seeded heat never reaches the 0.16 that gets drawn.
            let seed = (0.20 + 0.80 * lvl) * Self::flash_gain(self.flash.level(), i, n);
            for k in 0..SEED_W {
                let x = cx - SEED_W / 2 + k;
                if x >= 0 && x < w {
                    self.heat[((h - 1) * w + x) as usize] = seed;
                }
            }
        }
    }
}

impl Family for Flame {
    fn id(&self) -> &'static str {
        "flame"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (cw, ch) = (c.width(), c.height());
        c.clear();
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, (cw - 2).max(1), (ch - 4).max(1), 3, panel);
        if cw < 40 || ch < 24 {
            return;
        }

        // The interior the heat field covers: inside the panel, and above the manifold at the bottom.
        let manifold_h = 4;
        let (ix, iy) = (4, 3);
        let (iw, ih) = (cw - 8, ch - 6 - manifold_h);
        if iw < 8 || ih < 6 {
            return;
        }
        if self.w != iw || self.h != ih {
            self.w = iw;
            self.h = ih;
            self.heat = vec![0.0; (iw * ih) as usize];
        }

        let dt = if d.dt_ms.is_finite() { (d.dt_ms / 16.7).clamp(0.25, 4.0) } else { 1.0 };
        // THE FLOURISH, advanced before the field so this frame's seeding sees it. See `FLASH_MS`.
        let fired = self.flourish.update(&d.levels, d.dt_ms, t.flourish);
        self.flash.update(fired, d.dt_ms, FLASH_MS);
        self.advance(d, t.sensitivity, dt, cw, ix);

        // TWO LAYERS, and the split is what keeps this from looking like watercolour. The cooler zones
        // are the flame's BODY and go straight onto the opaque panel at full alpha, so they have hard
        // edges. Only the core goes on the transparent layer that gets bloomed.
        //
        // Blooming everything - which the first version did - puts a halo around the soft outer zones as
        // well, and a halo on an already-soft edge is the wash itself. This is the same arrangement
        // `patchbay` uses for its cables and `tube` for its cathodes.
        //
        // `Canvas::bloom` also puts its halo UNDER existing content, so blooming a canvas that already
        // carries the opaque panel would hide it completely - the trap documented in segmented, scope,
        // vu, tube and waterfall.
        // THE EMBER WASH: the panel itself warmed by how hard the manifold is burning. Drawn first, so
        // everything else sits on top of it. See `EMBER_ALPHA`.
        // Driven by the FIELD, not by the audio, and that distinction is a bug fix rather than a
        // refinement. Keyed on the band levels, the panel stayed warm all through a flashback - the
        // flames were out but the audio was still loud, so the wash never dropped and the flourish's own
        // ink never fell below 56% of normal. The panel is warmed by the FIRE; if there is no fire there
        // is no warmth.
        let drive: f32 = {
            let n = (iw * ih) as f32;
            self.heat.iter().filter(|v| v.is_finite()).sum::<f32>() / n.max(1.0) * 3.2
        };
        if drive > 0.02 {
            let a = (drive * EMBER_ALPHA).clamp(0.0, 1.0);
            c.vertical_gradient(
                2,
                iy,
                cw - 4,
                ih + manifold_h,
                &[
                    (0.0, Rgba::from_hex(&t.lit, 0.0)),
                    (0.55, Rgba::from_hex(&t.lit, a * 0.45)),
                    (1.0, Rgba::from_hex(&t.lit, a)),
                ],
                true,
            );
        }

        // THE SPILL, before the crisp body so the body lands on top of it. See `SPILL_NEAR_RADIUS`.
        //
        // Built from the whole field rather than only the core: the cool outer zones are most of the light
        // a fire throws, and a spill built from the core alone leaves the tall plumes glowing and the low
        // ones cold.
        let mut spill = Canvas::new(cw, ch);
        for y in 0..ih {
            for x in 0..iw {
                let v = self.at(x, y);
                if v < FLOOR * 0.6 {
                    continue;
                }
                let frac = x as f32 / iw as f32;
                spill.fill_rect(
                    ix + x,
                    iy + y,
                    1,
                    1,
                    super::tint(t, frac, d.time_s, false, &t.lit, v.clamp(0.0, 1.0)),
                );
            }
        }
        // Far first, then near on top of it: the wide faint field, then the bright close halo.
        //
        // The far pass is cached and refreshed every `SPILL_FAR_EVERY` frames - it is 45% of this
        // family's whole frame cost on its own. Also refreshed whenever the cached canvas is the wrong
        // size, which is how a resize is handled: a stale glow at the old width would be drawn into the
        // corner of the new one.
        let stale = self
            .far_glow
            .as_ref()
            .map(|g| g.width() != cw || g.height() != ch)
            .unwrap_or(true);
        if stale || self.frame % SPILL_FAR_EVERY == 0 {
            let mut far = spill.clone();
            far.bloom(SPILL_FAR_RADIUS, SPILL_FAR_ALPHA);
            self.far_glow = Some(far);
        }
        if let Some(far) = self.far_glow.as_ref() {
            c.draw_over(far);
        }
        let mut near = spill.clone();
        near.bloom(SPILL_NEAR_RADIUS, SPILL_NEAR_ALPHA);
        c.draw_over(&near);

        // Built once a frame, not per pixel: 64 interpolations against ~10,000 lookups.
        let ramp = super::waterfall::ramp_stops(t);
        let mut core = Canvas::new(cw, ch);
        for y in 0..ih {
            for x in 0..iw {
                let v = self.at(x, y);
                if v < FLOOR {
                    continue;
                }
                // CONTINUOUS, through the colourway's own multi-stop ramp. `Theme::zones` is already
                // "(position, colour)" and the spectrogram family builds a heat ramp from it the same
                // way, so a flame colourway declares its stops - deep red, orange, yellow, white - and
                // needs nothing new in the schema. The builder is shared with that family rather than
                // copied, for the reason the two onset detectors were merged.
                let mut col = super::waterfall::ramp_at(&ramp, v.clamp(0.0, 1.0));
                // A rainbow colourway takes its hue from POSITION, so the ramp cannot supply the colour -
                // it only supplies how hot the cell is. Without this the body was drawn from the neutral
                // ramp and only the spill was tinted, which left the rainbow colourway looking washed out
                // beside the other six while its glow was correctly coloured.
                //
                // The hue comes from the tint; the ramp's own lightness still decides how far toward white
                // the cell has burned, so a rainbow flame still goes white-hot at its base.
                if t.rainbow > 0.0 {
                    let frac = x as f32 / iw.max(1) as f32;
                    let hue = super::tint(t, frac, d.time_s, false, &t.lit, 1.0);
                    let toward_white = v.clamp(0.0, 1.0).powf(1.6);
                    let mix = |h: u8, w: u8| -> u8 {
                        (h as f32 + (w as f32 - h as f32) * toward_white).round() as u8
                    };
                    col = Rgba::new(mix(hue.r, 255), mix(hue.g, 255), mix(hue.b, 255), col.a);
                }
                // Two alphas multiplied, and they do different jobs. The smoothstep is the EDGE - a tight
                // anti-aliased boundary over `EDGE` of heat. The body term is the TRANSLUCENCY, rising
                // with heat across the whole range so the flame is a luminous volume rather than a solid
                // shape. See `BODY_ALPHA_LOW`.
                let e = ((v - FLOOR) / EDGE).clamp(0.0, 1.0);
                let edge = e * e * (3.0 - 2.0 * e);
                let body = BODY_ALPHA_LOW + (BODY_ALPHA_HIGH - BODY_ALPHA_LOW) * v.clamp(0.0, 1.0);
                let a = edge * body;
                if v >= CORE_HEAT {
                    // The core goes on the bloomed layer, so the hottest part of the flame is what throws
                    // light. Held at the ramp's top rather than a flat `hot`, so a colourway whose stops
                    // run to white gets white and one running to pale blue gets that.
                    core.fill_rect(
                        ix + x,
                        iy + y,
                        1,
                        1,
                        Rgba::new(col.r, col.g, col.b, (CORE_ALPHA * 255.0) as u8),
                    );
                } else {
                    c.fill_rect(
                        ix + x,
                        iy + y,
                        1,
                        1,
                        Rgba::new(col.r, col.g, col.b, (a * 255.0) as u8),
                    );
                }
            }
        }

        if t.bloom > 0.0 {
            let mut glow = core.clone();
            // Half the colourway's bloom. At full radius the core's halo reaches back over the crisp body
            // and undoes the whole point of separating them.
            glow.bloom((t.bloom * 0.5).max(0.0) as i32, t.glow_strength.clamp(0.0, 1.0));
            c.draw_over(&glow);
        }
        c.draw_over(&core);

        // The manifold, over the flames' feet so the plumes read as leaving it. Drawn from the valve
        // row's chassis colours for the same reason the patchbay borrows them: a gas pipe and a valve
        // chassis are the same milled metal, and reusing them keeps this family off the theme schema.
        let my = ch - 3 - manifold_h;
        c.fill_rect(2, my, cw - 4, manifold_h, Rgba::from_hex(&t.tube.chassis_bottom, 0.95));
        c.fill_rect(2, my, cw - 4, 1, Rgba::from_hex(&t.tube.glass, 0.16));

        // LIT BY ITS OWN FLAMES, per column. The cheapest warmth cue there is and the one every
        // photograph of a fire has: the metal under a flame is lit by it. Sampled from the heat in the
        // bottom rows of the field, so a tall plume lights its nozzle brightly and a pilot flame barely
        // at all - which incidentally makes the manifold a second reading of the spectrum.
        for x in 0..(cw - 4) {
            let fx = x - (ix - 2);
            if fx < 0 || fx >= iw {
                continue;
            }
            let lit_by = (self.at(fx, ih - 1) + self.at(fx, ih - 2) + self.at(fx, ih - 3)) / 3.0;
            if lit_by < 0.04 {
                continue;
            }
            let frac = fx as f32 / iw as f32;
            let a = (lit_by * MANIFOLD_LIT).clamp(0.0, 0.9);
            // The top two rows only: light falls on the face pointing at the flame, not down the side of
            // the pipe. Drawn as a tint over the metal rather than replacing it, so the pipe still reads
            // as metal that is lit and not as a coloured bar.
            for k in 0..2 {
                c.fill_rect(2 + x, my + k, 1, 1, super::tint(t, frac, d.time_s, false, &t.lit, a * (1.0 - k as f32 * 0.45)));
            }
        }

        let n = nozzle_count(cw);
        let collar = Rgba::from_hex(&t.tube.collar, 0.85);
        for i in 0..n {
            let x = nozzle_x(cw, n, i);
            // A stub standing proud of the pipe, so the row reads as nozzles rather than as perforations.
            c.fill_rect(x - 1, my - 1, 3, 2, collar);
        }

        c.clip_to_rounded_rect(1, 2, cw - 2, ch - 4, 3);
        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, cw - 2, 1, e);
        c.fill_rect(1, ch - 3, cw - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn flat(level: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
        for v in d.levels.iter_mut() {
            *v = level;
        }
        d.peaks = d.levels;
        d
    }

    /// Total light in the panel interior. The family's reading is plume HEIGHT, so this is a proxy used
    /// only where the claim really is "more or less fire", never where it is "taller or shorter".
    fn ink(c: &Canvas) -> f64 {
        let mut acc = 0.0;
        for y in 0..c.height() {
            for x in 0..c.width() {
                let p = c.get(x, y);
                acc += (0.2126 * p.r as f64 + 0.7152 * p.g as f64 + 0.0722 * p.b as f64)
                    * (p.a as f64 / 255.0);
            }
        }
        acc
    }

    fn lum(p: Rgba) -> f64 {
        0.2126 * p.r as f64 + 0.7152 * p.g as f64 + 0.0722 * p.b as f64
    }

    /// The topmost row above nozzle `i` that is brighter than the same pixel with no fire at all.
    ///
    /// DIFFERENTIAL, against a silence baseline, and two simpler instruments had to be discarded first:
    ///
    /// - **alpha** is useless on this family. `panel_alpha` is 1.0, so every in-bounds pixel has alpha
    ///   255 - a threshold on `.a` matched row 2 at every drive level, including silence. `scope.rs`
    ///   documents this exact trap for the same reason.
    /// - an ABSOLUTE luminance threshold under-reads a translucent flame. At `a > 40 && sum > 150` the
    ///   measured tip plateaued at row 42 across both 0.45 and 0.55 and then jumped to 24 at 0.70, which
    ///   looked like a physics problem and was not: the field's own height over those three levels is a
    ///   clean 26, 18, 7. The faint upper body simply sat under the threshold until it brightened.
    ///
    /// Comparing against silence removes the panel, the ember wash and the bezel in one step, because all
    /// three are in both frames.
    ///
    /// The margin is 6 luminance, lowered from 14 when advection landed. Not to make a test pass: the
    /// stretch makes a plume's upper half genuinely fainter, so at 14 the measure reported 13 rows of
    /// travel where the field had 36 - it was drawing the line partway up a plume that carries on. 6 is
    /// still a difference the eye finds on a dark panel, and silence measures as no plume at all, which is
    /// the check that stops it becoming a noise detector.
    fn tip(c: &Canvas, silence: &Canvas, i: usize) -> Option<i32> {
        let n = nozzle_count(c.width());
        let cx = nozzle_x(c.width(), n, i);
        (0..c.height()).find(|y| {
            (cx - 3..=cx + 3).any(|x| lum(c.get(x, *y)) > lum(silence.get(x, *y)) + 6.0)
        })
    }

    fn settled(t: &Theme, d: &FrameData, frames: usize) -> (Flame, Canvas) {
        let mut f = Flame::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..frames {
            f.draw(&mut c, t, d);
        }
        (f, c)
    }

    /// Run: cargo test --release probe_flame_heights -- --ignored --nocapture
    ///
    /// The height curve and the flashback's drain, on the two measures that survived scrutiny: the heat
    /// field's own top row, and a pixel tip taken as a DIFFERENCE against silence. Absolute-luminance and
    /// alpha thresholds were both tried and are recorded as failures on `tip` - alpha in particular is
    /// vacuous here, because an opaque panel makes every in-bounds pixel alpha 255.
    #[test]
    #[ignore]
    fn probe_flame_heights() {
        let t = builtin::flame_sodium();
        let silence = settled(&t, &flat(0.0), 120).1;
        println!("  level   field top   tip vs silence   ink");
        for lvl in [0.0f32, 0.12, 0.18, 0.30, 0.45, 0.55, 0.70] {
            let (f, c) = settled(&t, &flat(lvl), 120);
            let fld = (0..f.h).find(|y| (0..f.w).any(|x| f.at(x, *y) >= FLOOR));
            println!(
                "  {lvl:5.2}   {:>9?}   {:>14?}   {:>6.0}",
                fld,
                tip(&c, &silence, 5),
                ink(&c)
            );
        }
        let mut t2 = builtin::flame_sodium();
        t2.flourish = 0.0;
        let d = flat(0.45);
        let mut f = Flame::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..120 {
            f.draw(&mut c, &t2, &d);
        }
        let before = ink(&c);
        f.flourish.force_next();
        print!("  flashback ink/before:");
        for k in 1..=30 {
            f.draw(&mut c, &t2, &d);
            if k % 3 == 0 {
                print!(" f{k}={:.2}", ink(&c) / before);
            }
        }
        println!();
    }

    #[test]
    fn a_louder_band_burns_a_taller_plume() {
        // The family's whole reason to exist, and the reason the cooling subtracts rather than multiplies:
        // height has to be readable as a profile. Measured as the tip ROW, not as brightness - the same
        // position-over-intensity rule the nixie family is built on.
        let t = builtin::flame_sodium();
        let silence = settled(&t, &flat(0.0), 120).1;
        // Monotone across the range the DSP actually produces - 0.15 to 0.65 per active band - and not
        // merely different at the two ends. A plateau in the middle is where a height cue dies, and the
        // middle is where music lives.
        let mut tips = Vec::new();
        for lvl in [0.12f32, 0.20, 0.30, 0.42, 0.55, 0.68] {
            let c = settled(&t, &flat(lvl), 120).1;
            tips.push(tip(&c, &silence, 5).unwrap_or(59));
        }
        for w in tips.windows(2) {
            assert!(w[1] <= w[0], "the plume got SHORTER as the band got louder: {tips:?}");
        }
        assert!(
            tips[0] - tips[tips.len() - 1] >= 18,
            "the height cue is too compressed to read as a profile: {tips:?} - only {} rows of travel",
            tips[0] - tips[tips.len() - 1]
        );
    }

    #[test]
    fn every_burner_stays_lit_at_silence() {
        // A gas manifold with a burner out reads as broken, not as quiet - the same reason the valve row
        // keeps a heater floor and the reel keeps turning. This regressed once for real: raising the draw
        // floor to sharpen the edges left the pilot seed below it, and the quiet case rendered as bare
        // nozzle stubs.
        let t = builtin::flame_sodium();
        // ASSERTED ON THE HEAT FIELD, not on pixels, and three pixel-level instruments were tried and
        // discarded first - each defeated by something the family does deliberately:
        //
        // - a difference against a fire-free reference frame: there is no such frame. Zeroing the field
        //   and drawing again re-seeds it on that very draw, because seeding is part of `advance`, so it
        //   compared fire against fire and called every burner out.
        // - a difference against the GAP beside each nozzle: the spill fills the gaps with warmth on
        //   purpose, so the contrast is tiny, and for burner 3 of 13 the gap measured BRIGHTER than the
        //   nozzle (11.1 against 10.4) because both neighbours' plumes reach into it.
        // - a difference against the TOP of the same column: the ember wash is a bottom-bright gradient,
        //   so it would report a lit burner over an unlit one.
        //
        // A pilot flame IS the seed floor, so the field is where the claim lives. That the field reaches
        // the screen at all is covered by `a_louder_band_burns_a_taller_plume`, which measures pixels
        // against a silence baseline and passes.
        // MEASURED OVER A WINDOW, because a pilot flame flickers. Asserting a minimum plume height in one
        // settled frame failed on burner 3 of 13 with 2 rows against the others' 3 or more - not a dead
        // burner, but that nozzle's lick phase shearing a plume only three rows tall at the moment the
        // frame was taken. "Lit" for a flickering flame means it produces a plume across a span of
        // frames, not in every single one.
        let (mut f, mut c) = settled(&t, &flat(0.0), 120);
        let n = nozzle_count(190);
        let mut best = vec![0usize; n];
        let mut mouth = vec![0.0f32; n];
        for _ in 0..24 {
            f.draw(&mut c, &t, &flat(0.0));
            for i in 0..n {
                let cx = nozzle_x(190, n, i) - 4;
                mouth[i] = mouth[i].max(f.at(cx, f.h - 1));
                best[i] = best[i].max((0..f.h).filter(|y| f.at(cx, *y) >= FLOOR).count());
            }
        }
        for i in 0..n {
            assert!(
                mouth[i] >= FLOOR,
                "burner {i} of {n} is out at silence: {:.3} of heat at the mouth, below the {FLOOR} that                  gets drawn. A manifold with a burner out reads as broken, not as quiet",
                mouth[i]
            );
            assert!(
                best[i] >= 3,
                "burner {i} of {n} has heat but never a plume: {} rows above FLOOR at its tallest over 24                  frames",
                best[i]
            );
        }
    }

    #[test]
    fn the_flourish_snuffs_every_burner_then_relights_them_left_to_right() {
        // Asserted on `flash_gain` directly, because the claims are about a SHAPE in time and pixels would
        // only blur them: everything drops together, the front crosses once, and it ends with every burner
        // back at its own level. The pixel-level consequence is covered by the two tests above plus the
        // "comes back" test below.
        const N: usize = 12;
        // Not firing: no effect at all.
        for i in 0..N {
            assert_eq!(Flame::flash_gain(0.0, i, N), 1.0, "no flourish must not touch the seeding");
        }
        // The pressure loss: everything out at once, whatever its position.
        let early = 1.0 - SNUFF_FRAC * 0.5;
        for i in 0..N {
            assert_eq!(
                Flame::flash_gain(early, i, N),
                0.0,
                "burner {i} must go out during the pressure loss - a manifold does not lose pressure one \
                 burner at a time"
            );
        }
        // Mid-sweep: a prefix is lit and the rest is not, and the boundary moves monotonically.
        let lit_count = |flash: f32| (0..N).filter(|i| Flame::flash_gain(flash, *i, N) > 0.0).count();
        let mut prev = 0;
        for step in 0..=20 {
            let t = SNUFF_FRAC + (1.0 - SNUFF_FRAC) * step as f32 / 20.0;
            let k = lit_count(1.0 - t);
            assert!(k >= prev, "the relight front went BACKWARDS at t={t:.2}: {prev} lit, then {k}");
            prev = k;
        }
        assert_eq!(prev, N, "the front must reach the last burner");
        // And the flare: a burner just reached is brighter than its steady self.
        let just_lit = 1.0 - (SNUFF_FRAC + (1.0 - SNUFF_FRAC) * 0.5);
        let i = (N as f32 * 0.5) as usize - 1;
        assert!(
            Flame::flash_gain(just_lit, i, N) > 1.0,
            "a burner relighting must overshoot its steady height - the flare is the part the eye catches"
        );
    }

    #[test]
    fn the_manifold_comes_back_to_normal_after_the_flourish() {
        // Byte-identical, which also proves the flourish leaves no residue in the heat field.
        let mut t = builtin::flame_sodium();
        t.flourish = 0.0;
        let d = flat(0.4);
        let mut a = Flame::default();
        let mut ca = Canvas::new(190, 60);
        let mut b = Flame::default();
        let mut cb = Canvas::new(190, 60);
        for _ in 0..120 {
            a.draw(&mut ca, &t, &d);
            b.draw(&mut cb, &t, &d);
        }
        b.flourish.force_next();
        // 1600ms is 96 frames; 200 is comfortably past it plus the field settling again.
        for _ in 0..200 {
            a.draw(&mut ca, &t, &d);
            b.draw(&mut cb, &t, &d);
        }
        assert_eq!(ca.bits(), cb.bits(), "the manifold did not return to its steady state");
    }

    #[test]
    fn the_flourish_visibly_takes_the_fire_away_and_gives_it_back() {
        // The end-to-end check, on total light rather than height: during the pressure loss there is
        // genuinely LESS fire, which is the one moment where an ink measure is the right instrument.
        let mut t = builtin::flame_sodium();
        t.flourish = 0.0;
        let d = flat(0.45);
        let mut f = Flame::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..120 {
            f.draw(&mut c, &t, &d);
        }
        let before = ink(&c);
        f.flourish.force_next();
        // ~10 frames in is inside the 22% snuff, and the field takes a few frames to drain.
        // 18 frames in, which is inside the 22% snuff window (352ms = 21 frames) and past the few frames
        // the field takes to drain - heat already aloft keeps rising, which is correct behaviour and the
        // reason 12 frames was too early. Measured curve: 0.89 at 3 frames, 0.66 at 9, 0.42 at 18.
        for _ in 0..18 {
            f.draw(&mut c, &t, &d);
        }
        let during = ink(&c);
        assert!(
            during < before * 0.55,
            "the flashback did not take the fire away: {during:.0} against {before:.0} before"
        );
    }

    /// Run: cargo test --release probe_flame_cost -- --ignored --nocapture
    ///
    /// What a frame of this costs, against families that already ship. Worth measuring rather than
    /// estimating, because this app has a history of costing someone else's framerate: the warmth is two
    /// full-canvas blooms per frame on top of the diffusion pass, and "it is only a blur" is exactly the
    /// kind of assumption that turned out to be wrong about the UIA cache.
    ///
    /// `Canvas::bloom` is separable - two 1-D passes - so its cost is linear in radius, not quadratic.
    /// That is the reason a radius of 11 is affordable at all.
    #[test]
    #[ignore]
    fn probe_flame_cost() {
        let d = flat(0.45);
        let cases: [(&str, Box<dyn Fn() -> Box<dyn Family>>); 4] = [
            ("flame (2 blooms + field)", Box::new(|| Box::new(Flame::default()))),
            ("waterfall", Box::new(|| Box::new(crate::render::waterfall::Waterfall::default()))),
            ("tube", Box::new(|| Box::new(crate::render::tube::Tube::default()))),
            ("segmented", Box::new(|| Box::new(crate::render::segmented::Segmented::default()))),
        ];
        println!("  {:<26} {:>10} {:>12}", "family", "ms/frame", "% of 16.7ms");
        for (name, make) in &cases {
            let t = match *name {
                "flame (2 blooms + field)" => builtin::tube_soviet(),
                _ => builtin::all()
                    .into_iter()
                    .find(|th| {
                        th.family
                            == match *name {
                                "waterfall" => "waterfall",
                                "tube" => "tube",
                                _ => "segmented",
                            }
                    })
                    .unwrap(),
            };
            let mut f = make();
            let mut c = Canvas::new(190, 60);
            // Warm up, so buffer allocation and first-touch page faults are not in the figure.
            for _ in 0..60 {
                f.draw(&mut c, &t, &d);
            }
            let n = 300;
            let t0 = std::time::Instant::now();
            for _ in 0..n {
                f.draw(&mut c, &t, &d);
            }
            let ms = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
            println!("  {name:<26} {ms:>10.3} {:>11.1}%", ms / 16.7 * 100.0);
        }
    }

    /// Run: cargo test --release dump_flame -- --ignored --nocapture
    ///
    /// A prototype dump, written before any assertions, because "does this look like fire" is not a
    /// question a test can answer and building verification machinery around a look nobody has approved
    /// yet is how effort gets wasted.
    #[test]
    #[ignore]
    fn dump_flame() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        // One row per colourway, all at the same drive, so the seven can be compared directly - which is
        // the question now that the shape is settled.
        let themes: Vec<Theme> = builtin::all().into_iter().filter(|t| t.family == "flame").collect();
        assert!(themes.len() >= 5, "the family needs five colourways, has {}", themes.len());
        let (w, h) = (190i32, 60i32);
        let mut rows = Vec::new();
        let mut d = flat(0.0);
        // A comb, so both a tall plume and a short one appear in every row.
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = if (i / 5) % 2 == 0 { 0.58 } else { 0.20 };
        }
        d.peaks = d.levels;
        for t in &themes {
            let mut f = Flame::default();
            let mut c = Canvas::new(w, h);
            for _ in 0..120 {
                f.draw(&mut c, t, &d);
            }
            rows.push(c);
        }
        let (ow, oh) = (w, h * rows.len() as i32 + 4 * (rows.len() as i32 - 1));
        let mut out = vec![22u8; (ow * oh * 4) as usize];
        for (ri, shot) in rows.iter().enumerate() {
            for y in 0..h {
                for x in 0..w {
                    let px = shot.get(x, y);
                    let a = px.a as f32 / 255.0;
                    let o = (((ri as i32 * (h + 4) + y) * ow + x) * 4) as usize;
                    for (k, ch8) in [px.r, px.g, px.b].iter().enumerate() {
                        out[o + k] = (*ch8 as f32 + 22.0 * (1.0 - a)).min(255.0) as u8;
                    }
                    out[o + 3] = 255;
                }
            }
        }
        let path = dir.join(format!("flame-{ow}x{oh}.rgba"));
        std::fs::write(&path, &out).unwrap();
        let names: Vec<&str> = themes.iter().map(|t| t.name.as_str()).collect();
        println!("wrote {} ({ow}x{oh}) - rows: {}", path.display(), names.join(", "));
    }

}
