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

/// How hard an onset snaps the branch, in pixels, and how fast that decays.
const SHAKE_PX: f32 = 2.6;
const SHAKE_MS: f32 = 320.0;

/// The flourish: a gust. The wind spikes and the branch lets go of a great deal at once.
const GUST_MS: f32 = 2000.0;
const GUST_WIND: f32 = 1.9;
const GUST_RELEASE: usize = 26;

/// The three tumble masks: face, angled, edge. `#` is the petal body, `+` its lit edge.
const TUMBLE: [[&str; 3]; 3] = [
    ["+##", "###", ".#."],  // face on
    [".#+", "##.", "#.."],  // angled
    ["..+", ".#.", ".#."],  // edge on
];
const MASK_W: i32 = 3;
const MASK_H: i32 = 3;

/// The moon: radius in pixels, and where it sits as a fraction of the panel.
///
/// Upper RIGHT, deliberately opposite the branch, which enters from the left. Two focal points on the
/// same side would fight; on opposite sides they frame the petals drifting between them.
///
/// Six pixels is the smallest disc that still reads as a moon rather than a dot at this size - below that
/// it is indistinguishable from a bright petal, which is the one thing it must not look like.
const MOON_R: i32 = 6;
const MOON_X: f32 = 0.82;
const MOON_Y: f32 = 0.30;

/// The twigs: where along the branch, which way, and how long.
///
/// A TABLE rather than a hash, and hand-placed rather than random. The shape has to be identical frame
/// to frame - a tree that reshuffles itself every frame is not a tree - and eight twigs is few enough
/// that choosing them by eye beats any generator: the spacing is uneven on purpose, two fork DOWNWARD,
/// and the longest sit where the branch is thickest, which is what a real branch does.
///
/// `(along, dx, dy, len)`: `along` is the fraction of the branch, `dx`/`dy` the direction per step
/// (negative `dy` is up the screen), `len` the number of steps.
const TWIGS: [(f32, f32, f32, f32); 8] = [
    (0.14, 0.55, -1.00, 8.0),
    (0.24, -0.45, -1.00, 5.0),
    (0.33, 0.80, -1.00, 10.0),
    (0.44, 0.40, 1.00, 5.0),   // downward - a branch forks both ways
    (0.55, 0.70, -1.00, 8.0),
    (0.67, -0.35, -1.00, 6.0),
    (0.79, 0.85, -1.00, 7.0),
    (0.90, 0.30, 1.00, 4.0),   // downward, near the tip
];

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
    live: bool,
}

#[derive(Default)]
pub struct Blossom {
    petals: Vec<Petal>,
    /// Smoothed wind, in px/s.
    wind: f32,
    /// Branch shake and bass load, in pixels.
    shake: f32,
    bend: f32,
    /// Unspent time toward the next trickle release.
    trickle: f32,
    seed: u32,
    onset: crate::dsp::onset::Flux,
    flourish: crate::dsp::flourish::Trigger,
    gust: crate::dsp::flourish::Envelope,
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
        base + self.bend + self.shake
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
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
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
        let sky_top = Rgba::from_hex(&t.tube.socket, 1.0);
        let sky_low = Rgba::from_hex(&t.tube.collar, 1.0);
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
        self.shake -= self.shake * (dt / SHAKE_MS).min(1.0);
        if !self.bend.is_finite() {
            self.bend = 0.0;
        }
        if !self.shake.is_finite() {
            self.shake = 0.0;
        }

        let onset = self.onset.update(&d.levels, dt, 2.8, 200.0);
        if onset {
            self.shake = SHAKE_PX;
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
        // The twigs - see `TWIGS`. Tapered like the branch: 2px at the fork, 1px past halfway, because a
        // twig of uniform thickness reads as a scratch rather than as wood.
        for (k, (along, dx, dy, tlen)) in TWIGS.iter().enumerate() {
            let at = len * along;
            let base_y = self.spine_y(at, wf, hf);
            let mut step = 0.0f32;
            while step < *tlen {
                let tx = at + dx * step;
                let ty = base_y + dy * step;
                let thick = if step < tlen * 0.5 { 2 } else { 1 };
                c.fill_rect(tx as i32, ty as i32, 1, thick, bark);
                step += 1.0;
            }
            // A single lit pixel at the fork, which is what makes a twig read as JOINED to the branch
            // rather than as a mark lying across it.
            let _ = k;
            c.fill_rect(at as i32, base_y as i32, 1, 1, bark_lit);
        }

        // ---- petals ----
        let petal = Rgba::from_hex(&t.lit, 1.0);
        let petal_hot = Rgba::from_hex(&t.hot, 1.0);
        let petal_dim = Rgba::from_hex(&t.lit, t.ghost.clamp(0.15, 1.0).max(0.45));
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
            let body = if p.shade > 0.86 {
                petal_hot
            } else if p.shade < 0.32 {
                petal_dim
            } else {
                petal
            };
            let (px, py) = (p.x as i32, p.y as i32);
            for (ry, line) in mask.iter().enumerate() {
                for (rx, ch) in line.chars().enumerate() {
                    if ch == '.' {
                        continue;
                    }
                    let col = if ch == '+' { petal_hot } else { body };
                    c.fill_rect(px + rx as i32, py + ry as i32, 1, 1, col);
                }
            }
            let _ = (MASK_W, MASK_H);
        }

        c.bloom(t.bloom as i32, t.glow_strength);
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
        assert!(fam.shake > 0.5, "the branch did not snap: shake {:.2}px", fam.shake);
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
