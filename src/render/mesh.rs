//! The 3D spectrum family: extruded bars staggered into depth, where depth is TIME.
//!
//! The Winamp / Windows Media Player "bars in depth" display. Rows of solid bars marching up and to
//! the right, the nearest row bright and crisp, each row behind it dimmer and drawn first so the near
//! bars occlude it. Every row is a spectrum snapshot from a moment ago, so a transient visibly walks
//! backwards into the display.
//!
//! # Why this shape of 3D and not the obvious one
//!
//! A feasibility investigation (nine agents, three of them adversarial) killed the design it first
//! recommended - an extruded EQ curve given depth by one constant offset - and the reasoning is worth
//! keeping, because it is what makes THIS family the survivor:
//!
//! **Perspective is refused.** Depth steps and amplitude compete for the same ~48 usable rows, and
//! amplitude is the channel the eye reads as a meter. `vapor` measured the wall: at its tuned
//! `persp = 2.07`, SEVEN of sixteen depth lines collapsed onto rows 28-29, which also silently
//! disabled occlusion - lines sharing an integer row cannot occlude each other. There is a hang in it
//! too: `canvas.rs:624-650` Bresenham breaks only on reaching its endpoint, so a vertex projecting near
//! the eye saturates `as i32` to 2147483647 and one edge iterates ~2.1e9 times, measured at 294.6ms.
//! Oblique geometry with constant integer offsets has no divide, no near clip and nothing to collapse.
//!
//! **But oblique is not automatically safe, and this is the part that killed the other design.** For a
//! SWEPT SURFACE - a curve extruded by an offset - the visible depth face at column x is
//! `dy - (disp(x) - disp(x - dx))`: the offset MINUS the curve's own rise over the run. It reaches zero
//! where the curve's local slope equals `dy/dx` and inverts beyond, which is the same collapse in new
//! coordinates. Discrete boxes have no rise-over-run term, so the failure cannot occur here.
//!
//! **And the depth is not informationally empty**, which was the other fatal objection. A constant
//! offset applied to the SAME data is an affine translation - the render would carry one shape and a
//! shifted copy of it, no parallax, no more information than a drop shadow. Here each depth row holds
//! DIFFERENT data: a snapshot from further back in time. The offset is constant; the content is not.
//!
//! # Depth is time, and the ring has three properties that matter
//!
//! Learned from `vapor`, whose history ring is the closest thing in the tree:
//!
//! 1. **Rotate on an interval, not per frame.** Per-frame rotation makes the depth axis span half a
//!    second and the marching stops reading as motion.
//! 2. **PEAK-HOLD between rotations.** The ring samples at a few hertz, so a snare landing between two
//!    rotations is simply dropped unless the accumulator holds the maximum. Averaging loses it too.
//! 3. **Brightness carries DEPTH, never level.** Level is bar height - position - because `tube.rs`
//!    measured a driven element 1.46 dL* brighter than its neighbour as invisible against a ~2.3 dL*
//!    threshold. Dimming with distance is free of that trap because the eye is not being asked to read
//!    a value off it, only an ordering.

use crate::dsp::bands::NUM_BANDS;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Depth rows drawn, nearest last.
///
/// Five, and the ceiling is the vertical budget rather than taste: each row costs `DEPTH_DY` rows of
/// height that the bars then cannot use. At 5 rows the shear spends 12px of a 56px interior.
const DEPTH: usize = 5;

/// The constant oblique offset per depth row, in pixels. Up and to the right.
///
/// Integer, and constant, so there is no perspective divide and nothing that can collapse two depth
/// rows onto the same pixel row - the failure that disabled `vapor`'s occlusion.
const DEPTH_DX: i32 = 4;
const DEPTH_DY: i32 = 3;

/// Gap between bars, and the narrowest a bar may be before the family sheds detail.
const BAR_GAP: i32 = 2;
const MIN_BAR: i32 = 3;

/// How often the history ring rotates, in milliseconds. See the module docs, property 1.
///
/// 130ms gives the five depth rows a 650ms span - long enough that a transient is visibly *travelling*
/// backwards rather than blinking, short enough that the far row still belongs to the same phrase.
const ROTATE_MS: f32 = 130.0;

/// How far the furthest row is dimmed toward the panel, 0..1.
///
/// Depth cueing, and the one legitimate use of brightness here. 0.72 leaves the far row clearly
/// present; past ~0.85 it sinks into the substrate and the display looks like three rows, not five.
const DEPTH_FADE: f32 = 0.72;

/// The level window, `vapor`'s MEASURED p10-p90 of real music.
///
/// Not a 0..1 mapping, which renders dead, and not normalised against the frame's loudest band, which
/// is provably inert - that band sits at p50 0.819 so the normaliser settles near 1.1x. Four attempts
/// in that family failed exactly that way.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// Bars, at the two panel widths this app actually gets.
const BARS_WIDE: usize = 22;
const BARS_NARROW: usize = 12;

/// The flourish: the whole stack surges forward one depth step and settles back.
const SURGE_MS: f32 = 900.0;

#[derive(Default)]
pub struct Mesh {
    /// The history ring, `DEPTH` snapshots of `bars` levels. Index 0 is the NEAREST row.
    rows: Vec<Vec<f32>>,
    /// Peak-hold accumulator since the last rotation. See the module docs, property 2.
    acc: Vec<f32>,
    /// Unspent time toward the next rotation.
    due: f32,
    flourish: crate::dsp::flourish::Trigger,
    surge: crate::dsp::flourish::Envelope,
}

fn resp(level: f32, sensitivity: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    let x = ((level - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0);
    (x.powf(LEVEL_GAMMA) * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

/// Mixes `c` toward `panel` by `amount` - the depth cue.
fn recede(c: Rgba, panel: Rgba, amount: f32) -> Rgba {
    let a = amount.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 * (1.0 - a) + y as f32 * a).round().clamp(0.0, 255.0) as u8;
    Rgba::new(mix(c.r, panel.r), mix(c.g, panel.g), mix(c.b, panel.b), c.a)
}

impl Mesh {
    fn fit(&mut self, bars: usize) {
        if self.acc.len() != bars {
            self.acc = vec![0.0; bars];
            self.rows = vec![vec![0.0; bars]; DEPTH];
        }
        if self.rows.len() != DEPTH {
            self.rows = vec![vec![0.0; bars]; DEPTH];
        }
    }

    /// Peak-holds this frame into the accumulator, and rotates the ring when its interval elapses.
    fn advance(&mut self, d: &FrameData, sensitivity: f32, dt: f32, bars: usize) {
        for i in 0..bars {
            let lo = (i * NUM_BANDS) / bars;
            let hi = (((i + 1) * NUM_BANDS) / bars).clamp(lo + 1, NUM_BANDS);
            let band = d.levels[lo..hi].iter().copied().fold(0.0f32, f32::max);
            let v = resp(band, sensitivity);
            if v > self.acc[i] {
                self.acc[i] = v;
            }
        }
        self.due += dt;
        if !self.due.is_finite() {
            self.due = 0.0;
        }
        // Bounded, so a long stall cannot rotate the whole ring in one frame and flush the history.
        self.due = self.due.min(ROTATE_MS * DEPTH as f32);
        while self.due >= ROTATE_MS {
            self.due -= ROTATE_MS;
            self.rows.rotate_right(1);
            self.rows[0] = self.acc.clone();
            // Reset to the CURRENT frame rather than to zero, so the new nearest row starts from where
            // the music actually is instead of from silence.
            for i in 0..bars {
                self.acc[i] = 0.0;
            }
        }
    }
}

impl Family for Mesh {
    fn id(&self) -> &'static str {
        "mesh"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let surge = self.surge.update(fired, dt, SURGE_MS);

        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);

        // The shear costs height and width before anything is drawn.
        let shear_x = DEPTH_DX * (DEPTH as i32 - 1);
        let shear_y = DEPTH_DY * (DEPTH as i32 - 1);
        let bars = if w >= 300 { BARS_WIDE } else { BARS_NARROW };
        let avail_w = (w - 4) - shear_x;
        let pitch = avail_w / bars as i32;
        let bar_w = pitch - BAR_GAP;
        let base_y = h - 4;
        let max_bar = (h - 6) - shear_y;
        if bar_w < MIN_BAR || max_bar < 6 || pitch < MIN_BAR + BAR_GAP {
            return; // shed rather than smudge
        }
        self.fit(bars);
        self.advance(d, t.sensitivity, dt, bars);

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let key = Rgba::from_hex(&t.panel, 1.0);

        // BACK TO FRONT. The painter's algorithm is the whole occlusion story: a near bar is drawn
        // after the row behind it, over the top of it, with its own keyline. No z-buffer, no sorting
        // beyond this loop order.
        for depth in (0..DEPTH).rev() {
            let far = depth as f32 / (DEPTH - 1) as f32;
            // The flourish pulls the whole stack one depth step nearer, then lets it settle.
            let pull = surge * DEPTH_DY as f32;
            let dx = DEPTH_DX * depth as i32;
            let dy = (DEPTH_DY * depth as i32) as f32 - pull;
            let body = recede(if surge > 0.01 && depth == 0 { hot } else { lit }, panel, far * DEPTH_FADE);
            let face = recede(body, key, 0.42);

            for i in 0..bars {
                let level = self.rows[depth][i];
                let bar_h = (level * max_bar as f32).round() as i32;
                if bar_h <= 0 {
                    continue;
                }
                let x = 2 + dx + i as i32 * pitch;
                let y = base_y - dy.round() as i32 - bar_h;

                // Keyline first, one pixel all round: the same trick the dolphin family needed. Without
                // it adjacent depth rows in the same hue have nothing between them and the stack reads
                // as one lumpy surface instead of as five rows.
                c.fill_rect(x - 1, y - 1, bar_w + 2, bar_h + 2, key);
                // The front face, then the lit top - two tones is what makes a box read as a box at
                // this size, and the top face is where the eye reads the height from.
                c.fill_rect(x, y, bar_w, bar_h, face);
                c.fill_rect(x, y, bar_w, 2.min(bar_h), body);
            }
        }

        c.bloom(t.bloom as i32, t.glow_strength);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    /// A moving, bass-weighted spectrum with an occasional transient, so the depth rows differ.
    fn frame(gain: f32, t_s: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        // A snare every 0.6s, one band wide, to prove a transient walks backwards into the display.
        let hit = ((t_s / 0.6).fract() < 0.06) as i32 as f32;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.5) * 0.62 + 0.14;
            let wobble = 1.0 + 0.35 * ((t_s * 2.3 + f * 7.0).sin());
            let snare = if (0.42..0.52).contains(&f) { hit * 0.55 } else { 0.0 };
            *v = ((shape * wobble + snare) * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = 0.30 * gain;
        d.rms_r = 0.27 * gain;
        d
    }

    /// Run: cargo test --release dump_mesh -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_mesh() {
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

        for t in builtin::all().into_iter().filter(|t| t.family == "mesh") {
            for (w, h, tag) in [(380, 60, "380"), (190, 60, "190")] {
                let mut fam = Mesh::default();
                let mut c = Canvas::new(w, h);
                for k in 0..140 {
                    fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
                }
                write(format!("mesh-{}-{tag}", t.id), &c);
            }
        }

        // A filmstrip, so the backwards march of a transient is visible as a sequence.
        let t = builtin::mesh_wmp_cyan();
        let mut fam = Mesh::default();
        let mut c = Canvas::new(380, 60);
        let mut shot = 0;
        for k in 0..200 {
            fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            if k >= 60 && (k - 60) % 8 == 0 && shot < 6 {
                write(format!("mesh-march-{shot}"), &c);
                shot += 1;
            }
        }
        println!("wrote mesh dumps to {}", dir.display());
    }
}
