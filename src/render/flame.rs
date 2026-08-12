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
/// Sets the height scale directly: a seed of 1.0 climbs `1.0 / COOL` rows before it dies.
///
/// 0.023 reaches about 43 rows against an interior of 51, which leaves the top eighth clear. 0.018 was
/// tried and reached the ceiling: a plume that runs out of panel spreads sideways along the top row
/// instead of tapering, and rendered, the loud case grew flat horizontal caps that read as a fault in the
/// drawing rather than as fire.
const COOL: f32 = 0.023;

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

/// TWO ATTEMPTS AT THE BULBOUS TIPS, BOTH REVERTED, recorded so they are not repeated.
///
/// The plumes swell into a bulb near their tops rather than tapering to a wisp. It appeared as soon as
/// fractional sampling stopped the diffusion leaking heat sideways into the staircase - the heat it had
/// been losing was suddenly being kept, and it piles up where the rise stalls.
///
/// 1. **Extra cooling with height.** At 1.4 it truncated the plumes with hard flat tops, which is worse
///    than the bulbs; at 0.45 it was indistinguishable from doing nothing. Cooling decides where a plume
///    ENDS and says nothing about its shape on the way there.
/// 2. **More sideways spread with height** (falling centre bias), which is the entrainment a real flame
///    has. At 0.24 - taking the bias from 0.62 to 0.38 at the tip - it made no visible difference either.
///
/// Both were removed rather than left in at a setting that does nothing, which is the same fault as the
/// inert `brightness` and `saturation` config fields this project documents.
///
/// The likely real cause is that there is no VELOCITY here: heat only diffuses, so it cannot outrun its
/// own spreading, and a genuine taper needs the gas to rise faster than it mixes. That is an advection
/// term, a bigger change, and worth doing deliberately rather than by tuning a constant.

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
const FLOOR: f32 = 0.13;
const EDGE: f32 = 0.085;

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
    /// Linear in x only, not bilinear. The vertical neighbour is always the row directly below, which is
    /// exactly what the diffusion means, and interpolating vertically as well would blur the plume along
    /// the axis it is travelling - the one direction where the sharpness is real.
    fn at_frac(&self, x: f32, y: i32) -> f32 {
        if !x.is_finite() {
            return 0.0;
        }
        let x0 = x.floor();
        let f = x - x0;
        let a = self.at(x0 as i32, y);
        let b = self.at(x0 as i32 + 1, y);
        a + (b - a) * f
    }

    /// One diffusion pass, then the seeding.
    ///
    /// Iterated from the TOP down, reading the row below. That order is what makes it safe in place: the
    /// row being read has not been written yet this frame, so every cell sees the previous frame's heat.
    /// Bottom-up would read cells it had already updated and the fire would shoot to the top in one
    /// frame.
    fn advance(&mut self, d: &FrameData, sens: f32, dt: f32) {
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
                let below = self.at_frac(sx, y + 1);
                let l = self.at_frac(sx - 1.0, y + 1);
                let r = self.at_frac(sx + 1.0, y + 1);
                let side = (1.0 - CENTRE_BIAS) * 0.5;
                let mixed = below * CENTRE_BIAS + l * side + r * side;
                // Flicker in the COOLING rather than in the heat: cooling more on some cells eats holes
                // in the plume the way a real flame breaks up, where adding heat would only make it
                // sparkle.
                // Gently. 0.65-1.35 per cell per frame punched the plume full of holes on its own, which
                // is most of why the first render looked like sparks rather than fire.
                let flick = 0.88 + 0.24 * hash01(x, y, self.frame ^ 0x5BF0_3635);
                let v = mixed - cool * flick;
                self.heat[(y * w + x) as usize] = if v.is_finite() { v.max(0.0) } else { 0.0 };
            }
        }
        // Seed the bottom row from the bands.
        let n = nozzle_count(w);
        for cell in self.heat[((h - 1) * w) as usize..].iter_mut() {
            *cell = 0.0;
        }
        for i in 0..n {
            let cx = nozzle_x(w, n, i) - 4; // interior coordinates
            let lvl = response(Self::level_for(d, i, n), sens);
            // A floor, so a lit burner never goes out: an unlit nozzle on a gas manifold reads as
            // broken, which is the same reason the valve row keeps a heater floor and the reel keeps
            // turning at silence.
            // The 0.20 floor is a PILOT FLAME and it has to clear `FLOOR`, or a quiet burner draws
            // nothing and the manifold reads as switched off. Posterising the field made this a real
            // regression rather than a theoretical one: at a 0.06 floor the quiet case rendered as bare
            // nozzle stubs, because 0.06 seeded heat never reaches the 0.16 that gets drawn.
            let seed = 0.20 + 0.80 * lvl;
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
        self.advance(d, t.sensitivity, dt);

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
        let drive: f32 = {
            let n = nozzle_count(cw);
            (0..n).map(|i| response(Self::level_for(d, i, n), t.sensitivity)).sum::<f32>() / n as f32
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
                let col = super::waterfall::ramp_at(&ramp, v.clamp(0.0, 1.0));
                // Smoothstep alpha over a narrow band, so the edge is anti-aliased rather than either
                // banded or washed. See `EDGE`.
                let e = ((v - FLOOR) / EDGE).clamp(0.0, 1.0);
                let a = e * e * (3.0 - 2.0 * e);
                if v >= CORE_HEAT {
                    // The core goes on the bloomed layer, so the hottest part of the flame is what throws
                    // light. Held at the ramp's top rather than a flat `hot`, so a colourway whose stops
                    // run to white gets white and one running to pale blue gets that.
                    core.fill_rect(ix + x, iy + y, 1, 1, Rgba::new(col.r, col.g, col.b, 255));
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
        // Three drive levels, and a comb so the per-band reading can be judged as well as the look.
        let cases: [(&str, Box<dyn Fn(usize) -> f32>); 4] = [
            ("quiet", Box::new(|_| 0.16)),
            ("normal", Box::new(|_| 0.34)),
            ("loud", Box::new(|_| 0.62)),
            ("comb", Box::new(|i| if (i / 6) % 2 == 0 { 0.60 } else { 0.14 })),
        ];
        // A sodium-flame ramp, authored here because the colourways do not exist yet. Deep red at the
        // cool tips through orange and yellow to white at the base, which is the luminance progression a
        // real flame has and the thing the posterised version could not express.
        let mut t = builtin::tube_soviet();
        t.zones = vec![
            crate::themes::Zone { upto: 0.28, lit: "#7a1500".into(), hot: "#7a1500".into() },
            crate::themes::Zone { upto: 0.52, lit: "#e04a06".into(), hot: "#e04a06".into() },
            crate::themes::Zone { upto: 0.74, lit: "#ff9a1f".into(), hot: "#ff9a1f".into() },
            crate::themes::Zone { upto: 0.90, lit: "#ffd76a".into(), hot: "#ffd76a".into() },
            crate::themes::Zone { upto: 1.00, lit: "#fff6e0".into(), hot: "#fff6e0".into() },
        ];
        let (w, h) = (190i32, 60i32);
        let mut rows = Vec::new();
        for (_label, f) in &cases {
            let mut fl = Flame::default();
            let mut c = Canvas::new(w, h);
            let mut d = flat(0.0);
            for (i, v) in d.levels.iter_mut().enumerate() {
                *v = f(i);
            }
            d.peaks = d.levels;
            // 90 frames, so the field has filled and is in steady state rather than still climbing.
            for _ in 0..90 {
                fl.draw(&mut c, &t, &d);
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
        println!("wrote {} ({ow}x{oh}) - rows: quiet, normal, loud, comb", path.display());
    }
}
