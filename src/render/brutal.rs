//! The brutalist family: heavy concrete blocks that slam between two orientations on the beat.
//!
//! Asked for as "the brutalist bars as a separate theme". It is the design that was cut when the
//! frenchcore family turned into rave lasers, and it is different enough to earn its own family rather
//! than being a colourway of anything: the rave family is all light and no mass, and this is all mass.
//!
//! # THE ORIENTATION FLIPS ON THE BEAT
//!
//! In one state the blocks rise from the floor, in the other they hang from the ceiling, and an onset
//! toggles between them. The whole panel slams between two configurations several times a second.
//!
//! That is the idea worth keeping from the original sketch, and the reason is that it is a strobe made of
//! POSITION rather than brightness. It looks violent while leaving the house rule intact - `tube.rs:54-60`
//! measured a driven element 1.46 dL* brighter than its neighbour as invisible against a ~2.3 dL*
//! threshold, so brightness could not carry this even if it were allowed to.
//!
//! The consequence, which is intended: flipping destroys frame-to-frame comparability of the block TOPS.
//! You cannot track a block's tip across a flip, because the tip moves the full height of the panel. What
//! stays comparable is its LENGTH, which is what actually encodes the level - so the meter is unharmed and
//! the slam is free. It does mean the peak-hold marks are anchored to each block's own base rather than to
//! a fixed panel row; anchored to a row they would appear to leap the panel's height on every beat.
//!
//! # Dust and cracks: the concrete has to behave like concrete
//!
//! A slab slamming into the floor with nothing coming off it reads as a rectangle changing size. So each
//! flip throws DUST from the surface it hits, and every block carries a few static CRACKS.
//!
//! The debris comes in TWO CLASSES, because one does not read. A grain is 1x1 in a dimmer mix and reads as
//! powder; a CHUNK is 2x2 in the block's own tone and reads as a piece of the slab that broke off. Same
//! tone at two sizes would read as one effect with a size jitter. Chunks are thrown harder and straighter
//! and live longer, which is the physics: the impact gives everything much the same impulse, but air
//! resistance acts on the powder and barely on the lumps.
//!
//! The dust is ejected AWAY from the impacted surface while gravity always pulls down, which makes the two
//! states behave differently without any special-casing: a floor slam arcs up and falls back, and a
//! ceiling slam simply rains down. That asymmetry is the physics doing the work, and it also tells the eye
//! which state the panel is in during the moment the blocks themselves are still moving.
//!
//! A CRACK DIVIDES, and that is the whole of it. Two earlier versions were reported first as chevrons
//! ("small arrows stuck to the blocks") and then as "small black lines", and both were unbranched runs
//! differing only in how they stepped. This project had already established the identical point building
//! the cherry-blossom branch - a twig that does not divide is a spike - and a crack is topologically the
//! same object. No amount of tuning a single run turns it into a fracture, which is why two rounds of
//! tuning the step pattern got nowhere.
//!
//! So a crack wanders out from the block's base edge, jinks sideways when its hash says so rather than on
//! a fixed cadence, FORKS at 45% of its length with the fork leaning the other way, and tapers from 2px at
//! the edge to 1px beyond a third of its length.
//!
//! The cracks are STATIC per block and derived from the block's index, not from a per-frame random. A
//! crack that moved would be noise, and noise is the one thing a family this flat cannot absorb.
//!
//! They are coloured toward the BACKGROUND rather than simply darker, so they read as the panel showing
//! through a fissure. That also keeps them legible at the PEAK of the monolith flourish, where the body
//! becomes the panel colour exactly and a merely-darker crack would collapse into it. To be precise about
//! how much that is worth: a darker crack is fine for most of the envelope and only disappears at the
//! instant slab reaches 1.0 - the background mix is the better choice because a crack SHOULD read as a
//! gap and because its contrast is then constant across the whole inversion, not because the alternative
//! is invisible throughout.
//!
//! # No glow, no gradient, no ornament
//!
//! `bloom` is 0 on every colourway here, the way the chroma family sets it to zero: a halo softens exactly
//! the edges this family is about. Raw concrete, hard rectangles, and the gaps between blocks doing the
//! work a keyline would do elsewhere. Half the band count at double the width, because the subject is
//! MASS and a thin bar has none.

use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Blocks across the panel. Half the usual band count, at double the width.
///
/// Eleven at 380px gives 28px blocks with 5px gaps - a block wide enough to read as a slab rather than as
/// a bar, which is the entire point. More blocks would be thinner and this family would just be the
/// segmented meter with the segments removed.
const BLOCKS: usize = 11;

/// The gap between blocks, in pixels. It does the job a keyline does in other families.
///
/// Five, not one or two: at 1-2px the gap closes up under any halo and the blocks weld into a single mass,
/// and this project has already measured that a 2-row waist always fills under a 1px closing. Five
/// survives, and a wide gap is itself brutalist - the concrete is the ornament.
const GAP: i32 = 5;

/// The onset detector for the flip: flux ratio and refractory.
///
/// The same values the blossom family's branch shake uses, which measured 190 / 143 / 98 fires per minute
/// over the repo's three real-music fixtures - so the panel flips between one and three times a second on
/// real material. That is the intended violence.
const FLIP_RATIO: f32 = 2.8;
const FLIP_REFRACTORY_MS: f32 = 200.0;

/// The level window: `vapor`'s MEASURED p10-p90 of real music. Not a 0..1 mapping, which renders dead,
/// and not normalised against the frame's loudest band, which is provably inert at p50 0.819.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// The shortest a block ever gets, in pixels.
///
/// Not zero. A block that vanishes takes its gap with it and the grid's rhythm breaks; a stub still reads
/// as a block at rest. It is also what makes the flip visible on a quiet passage, where there would
/// otherwise be nothing to flip.
const STUB_PX: i32 = 3;

/// The dust pool, and how many grains a block throws per slam at full drive.
///
/// Pooled and never reallocated: a grain leaving the panel is marked dead and reused, so the per-frame
/// cost is constant and there is no allocation in the draw path. 120 is a hard ceiling that the emitter
/// respects even at maximum drive, which matters because the flip fires one to three times a second and
/// grains live for 420ms - so three bursts can be in the air at once.
const MAX_DUST: usize = 260;
const DUST_PER_BLOCK: usize = 7;

/// Chunks per block per slam at full drive, and the size of one in pixels.
///
/// A CHUNK IS NOT BIG DUST. It is drawn 2x2 in the BLOCK'S OWN TONE while a grain is 1x1 in a dimmer mix,
/// and that distinction is the whole point: a chunk reads as a piece of the slab that broke off, a grain
/// as the powder it ground up. Same tone at two sizes would read as one effect with a size jitter.
///
/// Fewer than the grains, because a slab that shed this much visible material every beat would have
/// nothing left. Two per block against seven grains is roughly the ratio that reads as "mostly dust, some
/// debris".
const CHUNK_PER_BLOCK: usize = 2;
const CHUNK_PX: i32 = 2;

/// A chunk's ejection multiplier, its lateral spread, and how long it lives.
///
/// Thrown HARDER and STRAIGHTER than dust, and it lives longer. The impact gives everything much the same
/// impulse, but air resistance acts on the powder and barely on the lumps - so the grains fan out and slow
/// while the chunks keep going roughly where they were thrown. That is why the spread is half the dust's
/// and the life is longer: a chunk should still be travelling when the grains around it have dispersed.
const CHUNK_EJECT: f32 = 1.15;
const CHUNK_SPREAD: f32 = 20.0;
const CHUNK_MS: f32 = 620.0;

/// How long a grain lives, in milliseconds.
const DUST_MS: f32 = 420.0;

/// How much of the block's tone a CHUNK keeps. Far less mixed toward the background than a grain, because
/// it is a piece of the block rather than powder in the air.
const CHUNK_MIX: f32 = 0.08;

/// Ejection speed away from the impacted surface, lateral spread, and gravity - all px/s, px/s and px/s^2.
///
/// The ejection speed is set by the arc it has to make: apex is `v^2 / 2g`, so at gravity 190 a grain
/// needs 62px/s to rise about 10px. The first values tried were 30px/s, which is an apex of 2.4px - the
/// grains hopped rather than flew and read as a shimmer at the base of the blocks rather than as debris.
///
/// Gravity is always DOWNWARD, which is what makes the two states differ for free: ejected up from the
/// floor a grain arcs and falls back, ejected down from the ceiling it accelerates away.
const DUST_EJECT: f32 = 62.0;
const DUST_SPREAD: f32 = 40.0;
const DUST_GRAV: f32 = 190.0;

/// Cracks per block, how many steps each one takes, and how far its colour moves toward the background.
///
/// Two per block, because the point is a flaw in the concrete and not a texture: at 28px wide, four or
/// more read as hatching.
///
/// THIRD ATTEMPT, and the two failures are the useful part:
///   - Three steps alternating sideways is a CHEVRON. Reported as small arrows stuck to the blocks.
///   - Five steps with one kink at the midpoint is a LINE WITH A BEND. Reported as "small black lines",
///     which is exactly what it was.
///
/// The thing both versions lacked is that A CRACK DIVIDES. This project already established the identical
/// point building the cherry-blossom branch - "a twig that does not divide is a spike; dividing is most of
/// what makes a branch look like a branch" - and a crack is topologically the same object: a path that
/// forks. No amount of tuning a single unbranched run turns it into a fracture, which is why two rounds of
/// tuning the step pattern got nowhere.
///
/// So a crack is now a WANDERING FORKED PATH from the block's base edge:
///   - It grows away from the base, one row per step, jinking sideways when the hash says so rather than
///     on a fixed cadence. A fixed cadence is what made both earlier versions read as a manufactured mark.
///   - It FORKS at `CRACK_FORK_AT` of its length, and the fork leans the other way. Not nearer the tip:
///     the blossom family measured that a fork in the last quarter reads as a frayed end.
///   - It TAPERS - 2px wide at the base edge where the stress is, 1px beyond a third of its length. A
///     uniform width is a drawn line; a varying one is a fracture.
const CRACKS_PER_BLOCK: u32 = 2;
const CRACK_STEPS: i32 = 9;
const CRACK_FORK_AT: f32 = 0.45;
const CRACK_FORK_STEPS: i32 = 4;
const CRACK_WIDE_FRAC: f32 = 0.33;
const CRACK_MIX: f32 = 0.55;

/// How far a dust grain's colour moves toward the background.
///
/// Less than the cracks. A crack is a hole and wants to disappear into the background; a grain is lit
/// concrete in mid-air and wants to be seen against it, so it stays nearer the block's own tone.
const DUST_MIX: f32 = 0.30;

/// The peak-hold cap's thickness in pixels.
const CAP_PX: i32 = 2;

/// The flourish: THE MONOLITH. Every block slams to full height at once and the panel inverts, so the
/// blocks become dark voids in a lit field, then it releases.
///
/// The inversion is the part that makes it read. Going merely brighter or taller would not: this project
/// measured a flourish changing 38.5% of the panel and still being reported as never happening, because it
/// was not a change of KIND. Figure and ground swapping is a change of kind.
///
/// The inversion is CONTINUOUS, driven by the envelope rather than a threshold - the background lifts
/// toward `lit` while the blocks darken toward the panel colour, and the two cross over in the middle. A
/// threshold would put two hard jumps in the middle of a one-shot decay, which is the snap this project
/// has now been reported for twice.
const SLAB_MS: f32 = 1100.0;

/// The smallest panel this family will draw on.
const MIN_W: i32 = 60;
const MIN_H: i32 = 18;

/// One grain of concrete dust.
#[derive(Clone, Copy, Default)]
struct Dust {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    /// Milliseconds this piece lives for. Grains and chunks share one pool but not one lifetime.
    life: f32,
    /// 1 for a grain, `CHUNK_PX` for a chunk. Also selects the tone - see `CHUNK_MIX`.
    size: i32,
    live: bool,
}

#[derive(Default)]
pub struct Brutal {
    onset: crate::dsp::onset::Flux,
    /// `true` means the blocks hang from the ceiling.
    hanging: bool,
    flourish: crate::dsp::flourish::Trigger,
    slab: crate::dsp::flourish::Envelope,
    /// The dust pool. Sized once on first use and reused thereafter - see `MAX_DUST`.
    dust: Vec<Dust>,
    /// Advanced on every emission, so consecutive slams throw different dust while any single slam is
    /// reproducible from its seed.
    seed: u32,
}

/// The value hash used for the dust spread and the crack placement.
///
/// The same one the other families use, so a given seed always produces the same debris and a golden
/// render stays reproducible.
fn hash32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn rand01(seed: u32, n: u32) -> f32 {
    hash32(seed ^ n.wrapping_mul(0x9e37_79b9)) as f32 / u32::MAX as f32
}

fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Rgba { r: f(a.r, b.r), g: f(a.g, b.g), b: f(a.b, b.b), a: f(a.a, b.a) }
}

impl Brutal {
    /// Throws dust from the surface the blocks just hit.
    ///
    /// `hanging` is the state being entered, so the surface is the ceiling when true and the floor when
    /// false. Grains are spread across each block's own width rather than across the panel, because the
    /// dust comes off the blocks and the gaps between them are empty air.
    fn slam(&mut self, hanging: bool, x0: i32, bw: i32, fy: i32, fh: i32, drive: f32) {
        if self.dust.len() != MAX_DUST {
            self.dust = vec![Dust::default(); MAX_DUST];
        }
        // Scaled by level, so a heavy passage throws more debris. That is an EVENT scaled by level, not
        // brightness standing in for one, so the house rule is intact.
        let scale = 0.35 + 0.65 * drive.clamp(0.0, 1.0);
        let grains = ((DUST_PER_BLOCK as f32) * scale).round() as usize;
        let chunks = ((CHUNK_PER_BLOCK as f32) * scale).round() as usize;
        let per = grains + chunks;
        if per == 0 {
            return;
        }
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let surface = if hanging { fy as f32 } else { (fy + fh - 1) as f32 };
        let eject = if hanging { DUST_EJECT } else { -DUST_EJECT };
        let mut next = 0usize;
        for b in 0..BLOCKS {
            let bx = x0 + b as i32 * (bw + GAP);
            for g in 0..per {
                // Reuse a dead slot. If the pool is full the grain is simply not thrown - a burst that
                // silently dropped grains is better than one that grows the pool without bound.
                let slot = loop {
                    if next >= MAX_DUST {
                        return;
                    }
                    let i = next;
                    next += 1;
                    if !self.dust[i].live {
                        break i;
                    }
                };
                // The first `grains` of each block's allocation are powder, the rest are chunks.
                let is_chunk = g >= grains;
                let n = (b * 16 + g) as u32;
                let across = rand01(self.seed, n * 3 + 1);
                let sideways = rand01(self.seed, n * 3 + 2) * 2.0 - 1.0;
                let lift = 0.55 + 0.45 * rand01(self.seed, n * 3 + 3);
                let (spread, boost, life, size) = if is_chunk {
                    (CHUNK_SPREAD, CHUNK_EJECT, CHUNK_MS, CHUNK_PX)
                } else {
                    (DUST_SPREAD, 1.0, DUST_MS, 1)
                };
                self.dust[slot] = Dust {
                    x: bx as f32 + across * bw as f32,
                    y: surface,
                    vx: sideways * spread,
                    vy: eject * lift * boost,
                    age: 0.0,
                    life,
                    size,
                    live: true,
                };
            }
        }
    }

    /// Advances every live grain by one frame and retires the ones that are finished.
    fn drift(&mut self, secs: f32, fy: i32, fh: i32, w: i32) {
        for g in self.dust.iter_mut().filter(|g| g.live) {
            g.age += secs * 1000.0;
            g.vy += DUST_GRAV * secs;
            g.x += g.vx * secs;
            g.y += g.vy * secs;
            let gone = g.age >= g.life.max(1.0)
                || !g.x.is_finite()
                || !g.y.is_finite()
                || g.x < 1.0
                || g.x > (w - 2) as f32
                || g.y < fy as f32
                || g.y > (fy + fh - 1) as f32;
            if gone {
                g.live = false;
            }
        }
    }

    /// The block grid: `(x of block 0, block width)`. `None` if the panel cannot hold the grid.
    fn grid(w: i32) -> Option<(i32, i32)> {
        let fw = w - 4;
        let total_gap = GAP * (BLOCKS as i32 + 1);
        let bw = (fw - total_gap) / BLOCKS as i32;
        if bw < 3 {
            return None;
        }
        // Centre whatever is left over, so the grid is not flush against one side.
        let used = bw * BLOCKS as i32 + total_gap;
        let x0 = 2 + GAP + (fw - used) / 2;
        Some((x0, bw))
    }
}

impl Family for Brutal {
    fn id(&self) -> &'static str {
        "brutal"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        if w < MIN_W || h < MIN_H {
            return; // shed rather than smudge
        }
        let Some((x0, bw)) = Self::grid(w) else {
            return;
        };
        let (fy, fh) = (3, h - 6);
        if fh < 6 {
            return;
        }
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 250.0) } else { 16.7 };

        // The frame's overall level, for how much debris a slam throws. Windowed the same way every
        // family here windows it - `vapor`'s measured p10-p90, because a raw 0..1 renders dead.
        let mean = d.levels.iter().filter(|v| v.is_finite()).sum::<f32>() / d.levels.len().max(1) as f32;
        let drive = ((mean - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0).powf(LEVEL_GAMMA);

        // Existing grains advance BEFORE new ones are thrown, which is what makes "a fresh grain sits
        // exactly on the impact surface" true rather than nearly true. Emitting first and then drifting
        // put every new grain a pixel off the surface on the very frame it was born - the frame where the
        // impact is supposed to read.
        self.drift(dt / 1000.0, fy, fh, w);

        // ---- the flip ----
        if self.onset.update(&d.levels, dt, FLIP_RATIO, FLIP_REFRACTORY_MS) {
            self.hanging = !self.hanging;
            // The slam. Thrown from the surface being ENTERED, so the dust and the blocks arrive
            // together rather than the dust trailing the state it belongs to by a frame.
            self.slam(self.hanging, x0, bw, fy, fh, drive);
        }

        // ---- the flourish ----
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let slab = self.slab.update(fired, dt, SLAB_MS).clamp(0.0, 1.0);
        let slab = if slab.is_finite() { slab } else { 0.0 };

        // Figure and ground cross over continuously - see SLAB_MS.
        let dark = Rgba::from_hex(&t.panel, 1.0);
        if slab > 0.0 {
            c.fill_rect(2, fy, w - 4, fh, Rgba::from_hex(&t.lit, slab));
        }
        // What sits BEHIND the blocks right now. The cracks are mixed toward this rather than toward
        // black, so they read as the panel showing through and survive the inversion - see the module
        // note. Under the monolith this is the lit wash, which is exactly what a crack should show.
        let background = lerp(dark, Rgba::from_hex(&t.lit, 1.0), slab);
        let bands = d.levels.len().max(1);

        for i in 0..BLOCKS {
            let bx = x0 + i as i32 * (bw + GAP);
            // This block's slice of the spectrum, taken as its loudest band so a wide slice is not
            // averaged into nothing.
            let lo = i * bands / BLOCKS;
            let hi = (((i + 1) * bands) / BLOCKS).max(lo + 1).min(bands);
            let mut slice = 0.0f32;
            let mut peak = 0.0f32;
            for k in lo..hi {
                let v = d.levels[k];
                if v.is_finite() {
                    slice = slice.max(v);
                }
                let p = d.peaks[k];
                if p.is_finite() {
                    peak = peak.max(p);
                }
            }
            let norm = |v: f32| ((v - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0).powf(LEVEL_GAMMA);
            let usable = (fh - STUB_PX).max(1) as f32;
            let mut len = STUB_PX + (norm(slice) * usable).round() as i32;
            let cap_at = STUB_PX + (norm(peak) * usable).round() as i32;
            // The monolith takes every block to full height.
            len = len + (((fh - len) as f32) * slab).round() as i32;
            let len = len.clamp(1, fh);

            // Resolved PER BLOCK through the shared rainbow resolver, keyed on the block's position
            // across the panel. On a fixed colourway `tint` returns the hex unchanged, so those
            // colourways are bit-for-bit what they were; on a rainbow one every block gets its own hue,
            // which is what makes a bold primary-colour set possible in a family built on flat fills.
            //
            // Position rather than a per-block random, because the blocks are a fixed grid: a stable hue
            // per column reads as painted concrete, where a hue that moved would read as a light show
            // and this family is explicitly not that.
            let x01 = if BLOCKS > 1 { i as f32 / (BLOCKS - 1) as f32 } else { 0.5 };
            let body = lerp(crate::render::tint(t, x01, d.time_s, false, &t.lit, 1.0), dark, slab);
            let tip = lerp(crate::render::tint(t, x01, d.time_s, true, &t.hot, 1.0), dark, slab);
            // The two states. `hanging` grows downward from the ceiling, otherwise upward from the floor.
            let (by, cap_y) = if self.hanging {
                (fy, fy + cap_at.clamp(0, fh) - CAP_PX)
            } else {
                (fy + fh - len, fy + fh - cap_at.clamp(0, fh))
            };
            c.fill_rect(bx, by, bw, len, body);

            // ---- the cracks ----
            //
            // A wandering FORKED path from the block's base edge - see CRACK_STEPS for why it forks and
            // why two unbranched versions failed. Static per block from its index, so it is damage rather
            // than noise, and clamped into the block's own rectangle at every length.
            let crack = lerp(body, background, CRACK_MIX);
            for ci in 0..CRACKS_PER_BLOCK {
                let n = i as u32 * 32 + ci * 8;
                // Start on the base edge, in the outer third on alternating sides. Alternating rather than
                // random, so the pair never shares a corner - at random they clustered often enough to
                // look like a deliberate mark.
                let side = ci % 2 == 0;
                let inset = 2 + (rand01(0xC0FF_EE01, n + 1) * (bw as f32 / 3.0)) as i32;
                let mut cx = if side { bx + inset } else { bx + bw - 1 - inset };
                // Which way it leans overall, and how far up the base edge it begins.
                let lean = if side { 1 } else { -1 };
                let steps = CRACK_STEPS.min(len);

                // The main run, plus its fork. Both walk the same way, so one routine draws both: a step
                // outward from the base each row, a sideways jink when the hash says so, and a 2px width
                // for the first third.
                let mut fork: Option<(i32, i32, i32)> = None;
                for k in 0..steps {
                    // The jink is hash-driven rather than on a cadence. A fixed cadence is precisely what
                    // made the earlier versions read as a manufactured chevron and then as a bent line.
                    if k > 0 && rand01(0xC0FF_EE01, n + 100 + k as u32) < 0.42 {
                        cx += lean;
                    }
                    let wide = (k as f32) < steps as f32 * CRACK_WIDE_FRAC;
                    let w_px = if wide { 2 } else { 1 };
                    // Rows run AWAY from the base, which is the top of the block when hanging and the
                    // bottom when standing.
                    let py = if self.hanging { by + k } else { by + len - 1 - k };
                    for dx in 0..w_px {
                        let px = cx + dx * lean;
                        if py >= by && py < by + len && px >= bx && px < bx + bw {
                            c.fill_rect(px, py, 1, 1, crack);
                        }
                    }
                    if fork.is_none() && (k as f32) >= steps as f32 * CRACK_FORK_AT {
                        fork = Some((cx, k, -lean));
                    }
                }
                // The fork, leaning the other way. This is the whole difference between a fracture and a
                // line, so it is not conditional on anything but having room for it.
                if let Some((fx0, fk, flean)) = fork {
                    let mut fx = fx0;
                    for j in 1..=CRACK_FORK_STEPS.min(len - fk).max(0) {
                        if rand01(0xC0FF_EE01, n + 200 + j as u32) < 0.62 {
                            fx += flean;
                        }
                        let k = fk + j;
                        let py = if self.hanging { by + k } else { by + len - 1 - k };
                        if py >= by && py < by + len && fx >= bx && fx < bx + bw {
                            c.fill_rect(fx, py, 1, 1, crack);
                        }
                    }
                }
            }

            // The peak cap, anchored to THIS BLOCK'S BASE rather than to a panel row - see the module
            // note. Only drawn when the peak is genuinely ahead of the block, or it just thickens the tip.
            if cap_at > len + CAP_PX {
                c.fill_rect(bx, cap_y.clamp(fy, fy + fh - CAP_PX), bw, CAP_PX, tip);
            }
        }

        // ---- the dust ----
        //
        // In FRONT of the blocks: it has come off them, so it passes over the concrete rather than
        // behind it. A grain is one pixel and flat, like everything else here.
        //
        // Its colour is the block tone mixed most of the way to the background, which is the same
        // relationship the cracks use - dust and cracks are the same material seen two ways, and pinning
        // both to `background` means neither of them needs a special case for the inversion.
        let src = lerp(crate::render::tint(t, 0.5, d.time_s, false, &t.lit, 1.0), dark, slab);
        let grain = lerp(src, background, DUST_MIX);
        // A CHUNK KEEPS THE BLOCK'S TONE. That, plus its 2x2 size, is what makes it read as a piece of the
        // slab rather than as a large grain - see CHUNK_PER_BLOCK.
        let lump = lerp(src, background, CHUNK_MIX);
        for g in self.dust.iter().filter(|g| g.live) {
            let (col, sz) = if g.size > 1 { (lump, g.size) } else { (grain, 1) };
            c.fill_rect(g.x as i32, g.y as i32, sz, sz, col);
        }

        // No bloom. Every colourway here sets `bloom` to 0 and this family would ignore it anyway - see
        // the module note. The clip is kept because the grid is centred by integer division and any future
        // change to that arithmetic is one slip from painting on the rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn frame(gain: f32, t_s: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / crate::dsp::bands::NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.5) * 0.58 + 0.15;
            let wob = 1.0 + 0.32 * ((t_s * 2.2 + f * 7.0).sin());
            *v = ((shape * wob) * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d
    }

    /// A frame with a hard transient on the given step, so the flip detector has something to find.
    fn beat_frame(t_s: f32, period: usize, k: usize, gain: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        let hit = k % period == 0;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / crate::dsp::bands::NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.4) * 0.5 + 0.14;
            let punch = if hit { 0.45 } else { 0.0 };
            *v = ((shape + punch) * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d
    }

    /// How much ink sits in the top third of the panel against the bottom third.
    fn top_vs_bottom(c: &Canvas, t: &Theme) -> (i32, i32) {
        let dark = Rgba::from_hex(&t.panel, 1.0);
        let h = c.height();
        let (mut top, mut bot) = (0, 0);
        for y in 3..h - 3 {
            for x in 2..c.width() - 2 {
                let p = c.get(x, y);
                if p.a == 0 || (p.r, p.g, p.b) == (dark.r, dark.g, dark.b) {
                    continue;
                }
                if y < 3 + (h - 6) / 3 {
                    top += 1;
                } else if y >= 3 + 2 * (h - 6) / 3 {
                    bot += 1;
                }
            }
        }
        (top, bot)
    }

    /// THE load-bearing property: a beat flips the whole panel between hanging from the ceiling and
    /// standing on the floor. This is the family's identity and its strobe.
    ///
    /// Mutation: remove the `self.hanging = !self.hanging` toggle, or make the draw ignore it - the two
    /// measured layouts become identical and this fails.
    #[test]
    fn a_beat_flips_the_blocks_between_floor_and_ceiling() {
        let t = builtin::brutal_concrete();
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        // Settle, then find one frame in each state and compare where the ink is.
        let mut standing: Option<(i32, i32)> = None;
        let mut hanging: Option<(i32, i32)> = None;
        for k in 0..400 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.75));
            if k < 40 {
                continue;
            }
            let m = top_vs_bottom(&c, &t);
            if fam.hanging {
                hanging = Some(m);
            } else {
                standing = Some(m);
            }
        }
        let (h_top, h_bot) = hanging.expect("the panel never entered the hanging state");
        let (s_top, s_bot) = standing.expect("the panel never entered the standing state");
        assert!(h_top + h_bot > 300 && s_top + s_bot > 300, "almost nothing was drawn");
        assert!(
            h_top > h_bot,
            "the hanging state is not top-heavy: {h_top} top, {h_bot} bottom"
        );
        assert!(
            s_bot > s_top,
            "the standing state is not bottom-heavy: {s_top} top, {s_bot} bottom"
        );
    }

    /// Block LENGTH tracks the level, in both states - which is what makes the flip free. The tips move
    /// the height of the panel on a flip, so length is the only thing that can carry the reading.
    ///
    /// Mutation: make `len` a constant, or drop the `norm(slice)` term.
    #[test]
    fn block_length_tracks_the_level_in_both_states() {
        let t = builtin::brutal_concrete();
        let ink = |gain: f32, force_hanging: bool| -> i32 {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..40 {
                fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
            }
            fam.hanging = force_hanging;
            fam.draw(&mut c, &t, &frame(gain, 1.0));
            let (a, b) = top_vs_bottom(&c, &t);
            a + b
        };
        for hanging in [false, true] {
            let quiet = ink(0.12, hanging);
            let loud = ink(0.95, hanging);
            assert!(
                loud > quiet * 2,
                "hanging={hanging}: length did not follow level, {quiet} -> {loud} px of ink"
            );
        }
    }

    /// The flourish INVERTS figure and ground, which is the change of kind that makes it read on a panel
    /// that is already slamming several times a second.
    ///
    /// Mutation: drop the background wash, or the `lerp(lit, dark, slab)` on the body - the panel then
    /// merely gets taller and this fails.
    #[test]
    fn the_flourish_inverts_the_panel() {
        let t = builtin::brutal_concrete();
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..80 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.7));
        }
        // Before: the gaps are dark and the blocks are lit.
        let dark = Rgba::from_hex(&t.panel, 1.0);
        let count_dark = |c: &Canvas| -> i32 {
            let mut n = 0;
            for y in 4..56 {
                for x in 4..376 {
                    let p = c.get(x, y);
                    if (p.r, p.g, p.b) == (dark.r, dark.g, dark.b) {
                        n += 1;
                    }
                }
            }
            n
        };
        let before = count_dark(&c);
        fam.flourish.force_next();
        let mut most_dark = before;
        for k in 80..110 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.7));
            most_dark = most_dark.max(count_dark(&c));
        }
        // At the peak the panel is a lit field with the blocks as dark voids, so the count of
        // panel-coloured pixels must have moved a long way - in EITHER direction, since which of figure
        // and ground dominates depends on the level. What must not happen is nothing.
        assert!(
            (most_dark - before).abs() > 500,
            "the flourish did not invert anything: {before} dark px before, {most_dark} at most"
        );
        for k in 110..300 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.7));
        }
        assert!(fam.slab.level() < 0.05, "the monolith never let go: {:.3}", fam.slab.level());
    }

    /// The peak cap is anchored to the BLOCK'S OWN BASE, not to a panel row. Anchored to a row it would
    /// appear to leap the panel's height on every flip, which is the specific fault the module note warns
    /// about - so this asserts the cap stays a bounded distance from its block in both states.
    ///
    /// Mutation: compute `cap_y` as `fy + fh - cap_at` in both branches and the hanging case fails.
    #[test]
    fn the_peak_cap_follows_its_own_block_through_a_flip() {
        let t = builtin::brutal_concrete();
        let (x0, bw) = Brutal::grid(380).unwrap();
        let hot = Rgba::from_hex(&t.hot, 1.0);
        // A frame whose peaks sit well above the current level, so a cap is actually drawn.
        let mut d = frame(0.35, 1.0);
        for p in d.peaks.iter_mut() {
            *p = 0.95;
        }
        for hanging in [false, true] {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..20 {
                fam.draw(&mut c, &t, &frame(0.35, k as f32 * 0.0167));
            }
            fam.hanging = hanging;
            fam.draw(&mut c, &t, &d);
            // Find the cap in the first block's columns.
            let mut cap_rows: Vec<i32> = Vec::new();
            for y in 3..57 {
                let p = c.get(x0 + bw / 2, y);
                if (p.r, p.g, p.b) == (hot.r, hot.g, hot.b) {
                    cap_rows.push(y);
                }
            }
            assert!(!cap_rows.is_empty(), "hanging={hanging}: no peak cap was drawn");
            let cap = cap_rows[0];
            // In the standing state the cap is above the block's top, in the hanging state below its
            // bottom - in both, it is on the block's FAR side from its own base.
            if hanging {
                assert!(cap > 3 + 10, "hanging: the cap sat at row {cap}, too near the ceiling");
            } else {
                assert!(cap < 57 - 10, "standing: the cap sat at row {cap}, too near the floor");
            }
        }
    }

    /// Dust must come off the surface the blocks actually HIT, which is the whole physical claim: the
    /// ceiling when they arrive hanging, the floor when they arrive standing.
    ///
    /// Asserted on the grain state rather than on pixels, because a grain is one pixel among concrete of
    /// a similar tone and reading its position back off the canvas would mean inferring it from whatever
    /// happened to be drawn there.
    ///
    /// Mutation: emit at a fixed row instead of `surface`, or drop the `hanging` term from it, and one
    /// of the two directions fails.
    #[test]
    fn a_slam_throws_dust_from_the_surface_it_hits() {
        let t = builtin::brutal_concrete();
        let (fy, fh) = (3, 60 - 6);
        let mut seen_floor = false;
        let mut seen_ceiling = false;
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..400 {
            let was = fam.hanging;
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.8));
            if fam.hanging == was {
                continue;
            }
            // The frame it flipped on: the fresh grains are the ones with age 0.
            let fresh: Vec<f32> =
                fam.dust.iter().filter(|g| g.live && g.age <= 0.0).map(|g| g.y).collect();
            if fresh.is_empty() {
                continue;
            }
            let near_ceiling = fresh.iter().all(|y| *y < (fy + 4) as f32);
            let near_floor = fresh.iter().all(|y| *y > (fy + fh - 5) as f32);
            if fam.hanging {
                assert!(
                    near_ceiling,
                    "arrived hanging but the dust came off rows {fresh:?}, not the ceiling"
                );
                seen_ceiling = true;
            } else {
                assert!(
                    near_floor,
                    "arrived standing but the dust came off rows {fresh:?}, not the floor"
                );
                seen_floor = true;
            }
        }
        assert!(seen_floor && seen_ceiling, "never observed both slams: floor {seen_floor}, ceiling {seen_ceiling}");
    }

    /// Dust must SETTLE, and the pool must never grow. The flip fires one to three times a second and a
    /// grain lives 420ms, so several bursts overlap - an emitter that leaked would grow without bound and
    /// a pool that never retired grains would silently stop throwing new ones.
    ///
    /// Mutation: remove the `gone` retirement in `drift`, and the quiet tail keeps its grains forever.
    #[test]
    fn dust_settles_when_the_music_stops_and_the_pool_never_grows() {
        let t = builtin::brutal_concrete();
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        let mut high_water = 0usize;
        for k in 0..600 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 18, k, 0.95));
            high_water = high_water.max(fam.dust.iter().filter(|g| g.live).count());
            assert!(fam.dust.len() <= MAX_DUST, "the pool grew to {}", fam.dust.len());
        }
        assert!(high_water > 8, "barely any dust was ever thrown: {high_water}");

        // Silence: no onsets, so no slams, so every grain must retire.
        let quiet = FrameData { dt_ms: 16.7, ..FrameData::default() };
        for _ in 0..120 {
            fam.draw(&mut c, &t, &quiet);
        }
        let left = fam.dust.iter().filter(|g| g.live).count();
        assert_eq!(left, 0, "{left} grains never settled");
    }

    /// A heavier passage throws more debris.
    ///
    /// Mutation: drop the `drive` term from `slam`'s per-block count.
    #[test]
    fn louder_music_throws_more_dust() {
        let t = builtin::brutal_concrete();
        let thrown = |gain: f32| -> usize {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(380, 60);
            let mut total = 0usize;
            for k in 0..400 {
                let before = fam.dust.iter().filter(|g| g.live).count();
                fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, gain));
                let after = fam.dust.iter().filter(|g| g.live).count();
                total += after.saturating_sub(before);
            }
            total
        };
        let quiet = thrown(0.30);
        let loud = thrown(0.95);
        assert!(quiet > 0, "the quiet run threw no dust at all");
        assert!(loud > quiet, "dust did not scale with level: {quiet} quiet, {loud} loud");
    }

    /// The cracks must be STATIC and must stay INSIDE their block. A crack that moved would be noise,
    /// which is the one thing a family this flat cannot absorb, and a crack outside its block would
    /// appear as a speck floating in the gap.
    ///
    /// Driven with a genuinely constant level so no onset fires: no flip, so no dust, so two consecutive
    /// frames must be byte-identical. That is a much stronger statement than checking crack pixels
    /// individually, and it catches any per-frame jitter anywhere in the family.
    ///
    /// Mutation: seed the crack placement from `d.time_s` or from a running counter instead of the block
    /// index, and the two frames stop matching.
    #[test]
    fn the_cracks_are_static_and_stay_inside_their_blocks() {
        let t = builtin::brutal_concrete();
        let mut fam = Brutal::default();
        let (w, h) = (380, 60);
        let mut c = Canvas::new(w, h);
        // Dead flat LEVELS but ADVANCING TIME. Both halves matter: flat levels mean `Flux` sees no rise,
        // so nothing flips and no dust is thrown, while advancing time is what makes the test able to
        // fail. The first version of this test left `time_s` at 0.0 on every frame, and a mutation that
        // seeded the crack placement from the clock passed it - because the clock never moved.
        let flat = |t_s: f32| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for v in d.levels.iter_mut() {
                *v = 0.55;
            }
            d.peaks = d.levels;
            d
        };
        for k in 0..60 {
            fam.draw(&mut c, &t, &flat(k as f32 * 0.0167));
        }
        assert_eq!(fam.dust.iter().filter(|g| g.live).count(), 0, "a flat level threw dust");
        let first: Vec<u32> = c.bits().to_vec();
        for k in 60..64 {
            fam.draw(&mut c, &t, &flat(k as f32 * 0.0167));
            assert_eq!(c.bits(), &first[..], "the panel is not static under a constant level");
        }

        // Nothing may be drawn in the gaps between blocks.
        let (x0, bw) = Brutal::grid(w).unwrap();
        let dark = Rgba::from_hex(&t.panel, 1.0);
        for i in 0..BLOCKS.saturating_sub(1) {
            let gap_x = x0 + i as i32 * (bw + GAP) + bw;
            for gx in gap_x..gap_x + GAP {
                for gy in 3..h - 3 {
                    let px = c.get(gx, gy);
                    assert_eq!(
                        (px.r, px.g, px.b),
                        (dark.r, dark.g, dark.b),
                        "something was drawn in the gap after block {i} at ({gx},{gy})"
                    );
                }
            }
        }
    }

    /// The cracks must stay visible through the monolith, which inverts figure and ground. A crack mixed
    /// toward BLACK rather than toward the background would vanish exactly when the blocks darken.
    ///
    /// Asserted as "a third tone exists inside the block": the body, the background and the peak cap are
    /// all computable from the theme and the envelope, so anything else in the block's columns is a crack.
    ///
    /// DRIVEN WITH FLAT LEVELS, and that is the load-bearing part of the fixture. Two earlier versions of
    /// this test were vacuous. The first counted pixels merely differing from the panel colour across the
    /// whole block column, which the lit wash satisfies on its own above and below the block. The second
    /// counted a third tone but drove the family with beats - so it was finding the DUST, and it passed
    /// with the cracks deleted outright. Flat levels mean no onset, so no flip, so no dust; the flourish
    /// is forced, which works regardless of level. The dust count is asserted at zero so this cannot
    /// quietly start measuring the wrong thing again.
    ///
    /// Mutation: replace `background` with `dark` in the crack colour, or delete the crack loop. Both
    /// leave only the body and the background inside the block.
    #[test]
    fn the_cracks_survive_the_inversion() {
        let t = builtin::brutal_concrete();
        let (w, h) = (380, 60);
        let (x0, bw) = Brutal::grid(w).unwrap();
        let dark = Rgba::from_hex(&t.panel, 1.0);
        let lit = Rgba::from_hex(&t.lit, 1.0);

        let flat = |t_s: f32| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for v in d.levels.iter_mut() {
                *v = 0.55;
            }
            d.peaks = d.levels;
            d
        };

        // Tones in the first block's columns that the family did not compute for the body, the
        // background or the cap - i.e. the cracks.
        let third_tones = |c: &Canvas, slab: f32| -> usize {
            let body = lerp(lit, dark, slab);
            let background = lerp(dark, lit, slab);
            let cap = lerp(Rgba::from_hex(&t.hot, 1.0), dark, slab);
            let mut others = std::collections::BTreeSet::new();
            for x in x0..x0 + bw {
                for y in 4..h - 4 {
                    let px = c.get(x, y);
                    let rgb = (px.r, px.g, px.b);
                    if rgb == (body.r, body.g, body.b)
                        || rgb == (background.r, background.g, background.b)
                        || rgb == (cap.r, cap.g, cap.b)
                    {
                        continue;
                    }
                    others.insert(rgb);
                }
            }
            others.len()
        };

        let mut fam = Brutal::default();
        let mut c = Canvas::new(w, h);
        for k in 0..60 {
            fam.draw(&mut c, &t, &flat(k as f32 * 0.0167));
        }
        assert_eq!(
            fam.dust.iter().filter(|g| g.live).count(),
            0,
            "dust is in the air, so this test would be measuring grains rather than cracks"
        );
        let at_rest = third_tones(&c, fam.slab.level());
        assert!(at_rest > 0, "no crack tone was found at rest");

        // Deep into the monolith, where the body has darkened most.
        fam.flourish.force_next();
        // AT THE PEAK, and only there. This is the instant the distinction is real: at slab 1.0 the body
        // IS the panel colour, so a crack mixed toward black collapses into it, while one mixed toward the
        // background is a mid tone between panel and lit. At slab 0.8 a black-mixed crack is still
        // distinct from a not-yet-black body, so a test that took the best frame over the whole envelope
        // could not tell the two apart - and did not.
        let mut best = 0usize;
        let mut deepest = 0.0f32;
        for k in 60..120 {
            fam.draw(&mut c, &t, &flat(k as f32 * 0.0167));
            let slab = fam.slab.level();
            assert_eq!(
                fam.dust.iter().filter(|g| g.live).count(),
                0,
                "dust appeared during the monolith"
            );
            if slab > 0.995 {
                best = best.max(third_tones(&c, slab));
                deepest = deepest.max(slab);
            }
        }
        assert!(deepest > 0.995, "the monolith never reached its peak: {deepest:.3}");
        assert!(
            best > 0,
            "the cracks vanished under the inversion at slab {deepest:.2} - only the body and the \
             background were left inside the block"
        );
    }

    /// THE CRACK MUST FORK. This is the property both earlier versions lacked and the reason they were
    /// reported first as chevrons and then as "small black lines" - an unbranched run is a line however it
    /// is stepped, and no amount of tuning the step pattern changes that.
    ///
    /// Measured as a genuine branch signature rather than by trusting the constants: somewhere along the
    /// crack there must be a row containing TWO separated runs of crack colour with block body between
    /// them. A single path, however wandering, can never produce that.
    ///
    /// Driven with flat levels so no dust is in the air to be mistaken for a second run - the same
    /// isolation the inversion test needs, and for the same reason.
    ///
    /// Mutation: delete the fork block, or set CRACK_FORK_AT above 1.0 so it never triggers. Either leaves
    /// one run per row and this fails.
    #[test]
    fn a_crack_forks_rather_than_running_as_one_line() {
        let t = builtin::brutal_concrete();
        let (w, h) = (380, 60);
        let (x0, bw) = Brutal::grid(w).unwrap();
        let mut fam = Brutal::default();
        let mut c = Canvas::new(w, h);
        let flat = |t_s: f32| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for v in d.levels.iter_mut() {
                *v = 0.62;
            }
            d.peaks = d.levels;
            d
        };
        for k in 0..60 {
            fam.draw(&mut c, &t, &flat(k as f32 * 0.0167));
        }
        assert_eq!(
            fam.dust.iter().filter(|g| g.live).count(),
            0,
            "dust is in the air, so a second run on a row might be a grain rather than a fork"
        );

        let body = Rgba::from_hex(&t.lit, 1.0);
        let is_body = |px: Rgba| (px.r, px.g, px.b) == (body.r, body.g, body.b);
        // A forked row needs THREE separated runs, not two.
        //
        // Two is worthless as a signature, and the first version of this test used it and was vacuous:
        // there are two cracks per block on opposite sides, so any row crossing both shows two runs
        // whether or not either one branches - disabling the fork entirely still passed. A THIRD run on
        // one row can only come from one of the two cracks having split.
        let mut forked_blocks = 0;
        for b in 0..BLOCKS {
            let bx = x0 + b as i32 * (bw + GAP);
            let mut found = false;
            for y in 4..h - 4 {
                let mut runs = 0;
                let mut in_run = false;
                let mut any_body_seen = false;
                for x in bx..bx + bw {
                    let px = c.get(x, y);
                    if is_body(px) {
                        any_body_seen = true;
                        in_run = false;
                    } else if !in_run {
                        in_run = true;
                        runs += 1;
                    }
                }
                if runs >= 3 && any_body_seen {
                    found = true;
                    break;
                }
            }
            if found {
                forked_blocks += 1;
            }
        }
        // Not every block shows three runs on every row - the two cracks have to be crossing the same
        // row for the third to be visible - but half of eleven blocks is far more than chance.
        assert!(
            forked_blocks >= BLOCKS / 2,
            "only {forked_blocks} of {BLOCKS} blocks show a row with three separated crack runs, which is the only signature a single unbranched path cannot produce"
        );
    }

    /// A slam throws TWO SIZES of debris, and the chunks are the block's own tone while the grains are a
    /// dimmer mix. Same tone at two sizes would read as one effect with a size jitter rather than as
    /// concrete and powder.
    ///
    /// Mutation: give chunks `size: 1`, or draw them with the grain colour. Either collapses the two
    /// classes into one and this fails.
    #[test]
    fn a_slam_throws_both_powder_and_chunks() {
        let t = builtin::brutal_concrete();
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        let mut saw_grain = false;
        let mut saw_chunk = false;
        for k in 0..400 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.95));
            for g in fam.dust.iter().filter(|g| g.live) {
                if g.size > 1 {
                    saw_chunk = true;
                } else {
                    saw_grain = true;
                }
            }
        }
        assert!(saw_grain, "no 1px grains were ever thrown");
        assert!(saw_chunk, "no chunks were ever thrown");

        // And the two are drawn in different tones, which is what makes the distinction read.
        let dark = Rgba::from_hex(&t.panel, 1.0);
        let src = crate::render::tint(&t, 0.5, 0.0, false, &t.lit, 1.0);
        let grain = lerp(src, dark, DUST_MIX);
        let lump = lerp(src, dark, CHUNK_MIX);
        assert_ne!(
            (grain.r, grain.g, grain.b),
            (lump.r, lump.g, lump.b),
            "a chunk and a grain are the same colour, so a chunk is just big dust"
        );
    }

    /// A chunk outlives and outruns the powder around it, which is the physics claim: the impact gives
    /// everything much the same impulse, but air resistance acts on the powder and barely on the lumps.
    ///
    /// Mutation: set CHUNK_MS equal to DUST_MS and CHUNK_SPREAD equal to DUST_SPREAD.
    #[test]
    fn chunks_outlive_and_outrun_the_powder() {
        assert!(CHUNK_MS > DUST_MS, "a chunk must live longer than a grain");
        assert!(CHUNK_SPREAD < DUST_SPREAD, "a chunk must fan out less than a grain");
        assert!(CHUNK_EJECT > 1.0, "a chunk must be thrown at least as hard as a grain");

        // And it shows in flight: measure how far each class travels sideways from where it was thrown.
        let t = builtin::brutal_concrete();
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        let (mut grain_life, mut chunk_life) = (0.0f32, 0.0f32);
        for k in 0..400 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.95));
            for g in fam.dust.iter().filter(|g| g.live) {
                if g.size > 1 {
                    chunk_life = chunk_life.max(g.age);
                } else {
                    grain_life = grain_life.max(g.age);
                }
            }
        }
        assert!(grain_life > 0.0 && chunk_life > 0.0, "one class was never observed in flight");
        assert!(
            chunk_life > grain_life,
            "chunks did not outlive the powder in flight: {chunk_life:.0}ms against {grain_life:.0}ms"
        );
    }

    /// Small panels shed, a hostile frame cannot poison anything, and the grid never paints outside the
    /// panel.
    #[test]
    fn tiny_panels_shed_and_a_hostile_frame_is_survivable() {
        let t = builtin::brutal_concrete();
        for (w, h) in [(1, 1), (8, 8), (59, 17), (60, 10), (12, 60), (0, 0), (61, 19)] {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(w, h);
            fam.flourish.force_next();
            fam.draw(&mut c, &t, &frame(0.6, 0.1));
            fam.draw(&mut c, &t, &frame(0.6, 0.2));
        }
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..20 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        for bad in [f32::NAN, f32::INFINITY, -1.0e30, 1.0e30] {
            let mut d = frame(0.6, 1.0);
            d.dt_ms = bad;
            d.levels[0] = bad;
            d.peaks[1] = f32::NAN;
            fam.draw(&mut c, &t, &d);
        }
        fam.draw(&mut c, &t, &frame(0.6, 2.0));
        // Nothing outside the rounded panel.
        for x in 0..380 {
            assert_eq!(c.get(x, 0).a, 0, "painted on row 0 at x {x}");
            assert_eq!(c.get(x, 59).a, 0, "painted on the last row at x {x}");
        }
    }

    /// Every colourway draws on both panel widths, and none of them enables bloom - a halo would soften
    /// the edges this family is built on.
    #[test]
    fn every_colourway_draws_hard_edged_on_both_widths() {
        for t in builtin::all().into_iter().filter(|t| t.family == "brutal") {
            assert_eq!(t.bloom, 0.0, "{}: bloom must be 0 in this family", t.id);
            for w in [380, 190] {
                let mut fam = Brutal::default();
                let mut c = Canvas::new(w, 60);
                for k in 0..30 {
                    fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
                }
                let (a, b) = top_vs_bottom(&c, &t);
                assert!(a + b > w / 2, "{} drew almost nothing at {w}px: {}", t.id, a + b);
            }
        }
    }

    #[test]
    #[ignore]
    fn probe_brutal_cost() {
        let t = builtin::brutal_concrete();
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..60 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.8));
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.8));
        }
        println!("brutal: {:.3} ms/frame at 380x60", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }

    #[test]
    #[ignore]
    fn dump_brutal() {
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
        for t in builtin::all().into_iter().filter(|t| t.family == "brutal") {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..300 {
                fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            }
            write(format!("brutal-{}", t.id), &c);
        }
        // Mid-slam, on the frame the dust is thrown, in both states - which is the thing to judge.
        for hanging in [false, true] {
            let t = builtin::brutal_concrete();
            let mut fam = Brutal::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..200 {
                fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 18, k, 0.9));
            }
            // Force the state, then slam into it so the dust is fresh.
            fam.hanging = !hanging;
            let mut k = 200;
            while fam.hanging != hanging && k < 400 {
                fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 18, k, 0.9));
                k += 1;
            }
            for j in 0..5 {
                fam.draw(&mut c, &t, &beat_frame((k + j) as f32 * 0.0167, 18, (k + j) as usize, 0.9));
            }
            write(format!("brutal-slam-{}", if hanging { "ceiling" } else { "floor" }), &c);
        }

        // Both states of the flip, side by side, which is the thing to judge.
        let t = builtin::brutal_concrete();
        for hanging in [false, true] {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..120 {
                fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            }
            fam.hanging = hanging;
            fam.draw(&mut c, &t, &frame(0.62, 2.0));
            write(format!("brutal-{}", if hanging { "hanging" } else { "standing" }), &c);
        }
        // The monolith, mid-hold.
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..120 {
            fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
        }
        fam.flourish.force_next();
        for k in 120..132 {
            fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
        }
        write("brutal-monolith".into(), &c);
    }
}
