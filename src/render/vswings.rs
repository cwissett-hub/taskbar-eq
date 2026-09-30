//! The Virtual Self family: angel wings on Y2K chrome, where the spectrum IS the plumage.
//!
//! Asked for as a "Virtual Self" look - the early-2000s chrome-and-cherub aesthetic - and the meter that
//! carries it is a pair of wings. Thirty-two bands are fanned into two mirrored wings that meet at a
//! central root, low bands at the root and highs at the tips, so THE WINGS BEAT ON THE BASS: a kick
//! lengthens the root feathers and the whole span pulses out from the middle.
//!
//! # Why a wing is a meter and not just an ornament
//!
//! A feather is a bar rotated by its index. Reading a wing is reading a fan of bars whose LENGTHS encode
//! the spectrum - exactly the segmented meter's reading, bent into an arc. The fan runs from -70deg to
//! +70deg through the vertical, and the same fan is mirrored across the centre to make the second wing, so
//! the display is symmetric by construction and a glance still resolves "where is the energy" the way a
//! straight bar row does. Bass at the root means the loudest, most rhythmic part of most music drives the
//! biggest, most central feathers - the ones the eye lands on first.
//!
//! # The panel is opaque, and the bloom is confined to a light layer
//!
//! Like every other family, this one paints an opaque panel (dark or ice) that covers the Windows weather
//! widget while music plays - a see-through family would show the forecast through the wings. The wings,
//! grid and chrome are drawn over that panel, and a 2px bevel frames its edge.
//!
//! What that forces is where the BLOOM goes. Bloom applied to the whole canvas spreads the wings' colour
//! into a halo across the panel, and since the wings cover a large contiguous fan that halo tints nearly
//! the whole surface - which flattens the one reading the meter has, the wing LENGTH (measured as
//! how much of the panel the wings have painted over). So the bloom is confined to a light layer holding
//! only the bright accents - the hot tips, the peak dots and the flare core - which is composited over
//! crisp wing bodies. That is the exact idiom `Canvas::bloom`/`draw_over` document for a halo over an
//! opaque field, and it keeps the glow without dissolving the meter.
//!
//! # The chrome: a bevel, a grid, a lens flare
//!
//! The Y2K surface is three quiet parts around the loud wings:
//!
//! - a 2px bevel frame - light on the top and left, dark on the bottom and right - which is the whole of
//!   the "extruded chrome panel" look at this size;
//! - a single-point perspective grid receding to a horizon at 35% height, drawn in `edge` at `edge_alpha`
//!   so it reads as a floor the wings stand on rather than as a second meter. Its spacing scrolls with the
//!   music's level and an onset jumps it forward a whole cell, so the floor drifts under the beat;
//! - a lens flare - bright core, four streaks, one ring - that bursts from the wing root on a flourish and
//!   fades over 600ms. A flare is the single most Y2K thing a bright light can do.
//!
//! # Colour goes through linear light, and no rainbow
//!
//! Feathers are `lit` at the root shading to `hot` at the tip - a chrome sheen along each feather - mixed
//! through the shared linear-light blend, never by averaging sRGB bytes. There is no rainbow colourway
//! here: the identity is chrome and ice, a restrained Y2K palette, and a hue wheel would fight it.

use crate::dsp::bands::NUM_BANDS;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Feathers per wing. Every second band (`i*2`), so a wing spans the whole spectrum at half the band
/// resolution - which is what keeps a single feather wide enough to read as a feather rather than a hair.
const FEATHERS: usize = 32;

/// Half the fan angle, in degrees. The fan runs from `-FAN_DEG` (up) through `0` (out) to `+FAN_DEG`
/// (down), so 70 gives a wing that reaches almost to the vertical without the top and bottom feathers
/// collapsing onto the panel edges.
const FAN_DEG: f32 = 70.0;

/// Where the wing root sits, as a fraction of the panel interior height. Below centre, because a wing
/// hangs from a shoulder above its own midline and the downstroke feathers need the room beneath.
const ROOT_Y_FRAC: f32 = 0.62;

/// The grid's vanishing horizon, as a fraction of the interior height from the top.
const HORIZON_FRAC: f32 = 0.35;

/// Grid line counts: converging verticals and receding horizontals.
const GRID_VERTS: i32 = 9;
const GRID_HORIZ: i32 = 5;

/// How fast the horizontal grid lines scroll, per unit of level per second. Slow: the floor drifts, it
/// does not race.
const GRID_SCROLL: f32 = 3.5;

/// The onset that jumps the grid forward a whole cell - the same permissive flux net the flourish uses to
/// find candidates, so the floor lurches on the beat without needing its own calibration.
const GRID_ONSET_RATIO: f32 = 2.8;
const GRID_ONSET_REFRACTORY_MS: f32 = 120.0;

/// Feather length: a stub plus level times this fraction of the panel WIDTH. Scaled to width so the wings
/// grow with the panel and stay legible at 128px, and clamped per-feather to the interior so nothing is
/// drawn outside the canvas - see `ray_max`.
const LEN_BASE: f32 = 6.0;
const LEN_SPAN_FRAC: f32 = 0.24;

/// Feather width at the root and at the tip, in pixels, and the outer fraction drawn in `hot`.
const WIDTH_ROOT: f32 = 3.0;
const WIDTH_TIP: f32 = 1.0;
const TIP_FRAC: f32 = 0.35;

/// How long the lens flare takes to fade, in milliseconds.
const FLARE_MS: f32 = 600.0;

/// The panel is at least this large before the family draws its wings - smaller, and it sheds rather than
/// smudging, exactly as the other families do.
const MIN_W: i32 = 60;
const MIN_H: i32 = 18;

/// The title only appears on a panel at least this tall; below it there is no room under the wings for
/// legible glyphs, so it is gated off and the wings scale to fill the space instead.
const TITLE_MIN_H: i32 = 58;

/// The bevel frame colours - the one fixed chrome in the family, shared by every colourway because a
/// bevel is a lighting effect on the panel edge, not a palette choice.
const BEVEL_LIGHT: &str = "#f4f6fa";
const BEVEL_DARK: &str = "#8a94a6";

/// The chrome the title glyphs run through, top row to bottom: white -> silver -> cobalt -> silver ->
/// white, the classic extruded-metal gradient.
const CHROME_ROWS: [&str; 5] = ["#ffffff", "#f4f6fa", "#1f5bff", "#f4f6fa", "#ffffff"];

#[derive(Default)]
pub struct Vswings {
    /// Fires the lens flare on a rare, exceptional hit - see `dsp::flourish`.
    flourish: crate::dsp::flourish::Trigger,
    /// The flare's one-shot decay envelope.
    flare: crate::dsp::flourish::Envelope,
    /// Scroll position of the receding grid, advanced by level and jumped by an onset.
    grid_phase: f32,
    /// Smoothed feather lengths, one per feather, so the wings do not jitter frame to frame - the
    /// ballistics of the bar families, applied to a fan.
    feather_len: [f32; FEATHERS],
    /// The onset that lurches the grid. A local detector rather than the flourish's private one, because
    /// the grid lurches far more often than the flare fires.
    onset: crate::dsp::onset::Flux,
    /// A persistent light layer the title is composited from, sized once per panel size and reused. The
    /// title needs a black outline UNDER a chrome gradient, which is two passes over the same pixels, so
    /// it is drawn on its own layer and laid over the wings rather than fighting them in place.
    scratch: Option<Canvas>,
}

/// The interior box the wings and grid clip to: `(x0, y0, x1, y1)`.
type Bbox = (f32, f32, f32, f32);

/// The largest `t >= 0` for which `root + t*dir` stays inside the box `[x0,x1] x [y0,y1]`.
///
/// A ray-box clip, so a feather can be grown to its musical length and then cut back to whatever the
/// panel holds along its own direction - which is how the wings scale to width and never draw outside the
/// canvas, rather than by a single global length cap that would make the near-vertical feathers stubby.
fn ray_max(root: (f32, f32), dir: (f32, f32), bbox: Bbox) -> f32 {
    let (rx, ry) = root;
    let (ux, uy) = dir;
    let (x0, y0, x1, y1) = bbox;
    let eps = 1.0e-4;
    let mut t = f32::INFINITY;
    if ux > eps {
        t = t.min((x1 - rx) / ux);
    } else if ux < -eps {
        t = t.min((x0 - rx) / ux);
    }
    if uy > eps {
        t = t.min((y1 - ry) / uy);
    } else if uy < -eps {
        t = t.min((y0 - ry) / uy);
    }
    if t.is_finite() {
        t.max(0.0)
    } else {
        0.0
    }
}

impl Vswings {
    /// One feather: a tapering quad from `p` along the unit direction `dir` for `len` pixels, `width.0`
    /// wide at the start and `width.1` at the end. The edge from the last point back to the first is
    /// implicit; `fill_poly` clips to the canvas, so an over-long feather is cut rather than panicking.
    fn feather(c: &mut Canvas, p: (f32, f32), dir: (f32, f32), len: f32, width: (f32, f32), col: Rgba) {
        if len < 0.5 {
            return;
        }
        let (rx, ry) = p;
        let (ux, uy) = dir;
        let (w0, w1) = width;
        let (tx, ty) = (rx + ux * len, ry + uy * len);
        // Perpendicular to the feather, for the width.
        let (px, py) = (-uy, ux);
        let (h0, h1) = (w0 * 0.5, w1 * 0.5);
        let pts = [
            ((rx + px * h0).round() as i32, (ry + py * h0).round() as i32),
            ((tx + px * h1).round() as i32, (ty + py * h1).round() as i32),
            ((tx - px * h1).round() as i32, (ty - py * h1).round() as i32),
            ((rx - px * h0).round() as i32, (ry - py * h0).round() as i32),
        ];
        c.fill_poly(&pts, col);
    }

    /// The single-point perspective floor: verticals converging to the horizon, plus horizontals whose
    /// spacing scrolls with `grid_phase`. Drawn in `edge` at `edge_alpha` so it stays quiet under the
    /// wings.
    fn draw_grid(&self, c: &mut Canvas, t: &Theme, cx: f32, bbox: Bbox) {
        let (x0, y0, x1, y1) = bbox;
        if t.edge_alpha <= 0.0 {
            return; // a colourway with no floor - the ghost
        }
        let col = Rgba::from_hex(&t.edge, t.edge_alpha.clamp(0.0, 1.0));
        let horizon = y0 + HORIZON_FRAC * (y1 - y0);
        // Verticals fan from the bottom edge up to the single vanishing point.
        for k in 0..=GRID_VERTS {
            let f = k as f32 / GRID_VERTS as f32;
            let bx = x0 + f * (x1 - x0);
            c.line(bx.round() as i32, y1.round() as i32, cx.round() as i32, horizon.round() as i32, col);
        }
        // Horizontals recede toward the horizon, bunched by a mild perspective exponent and scrolled by
        // the phase so the floor drifts under the beat.
        let phase = if self.grid_phase.is_finite() { self.grid_phase.rem_euclid(1.0) } else { 0.0 };
        for j in 0..GRID_HORIZ {
            let tl = (j as f32 + phase) / GRID_HORIZ as f32;
            let yy = horizon + tl.powf(1.7) * (y1 - horizon);
            let frac = ((yy - horizon) / (y1 - horizon)).clamp(0.0, 1.0);
            // Narrow to the vanishing point near the horizon, full width at the bottom.
            let xl = cx + (x0 - cx) * frac;
            let xr = cx + (x1 - cx) * frac;
            c.line(xl.round() as i32, yy.round() as i32, xr.round() as i32, yy.round() as i32, col);
        }
    }

    /// The lens flare at the wing root, drawn CRISP on the main canvas and NEVER bloomed - a bloomed core
    /// is precisely the soft disc that swallowed the wings in the first cut. The SHAPE carries it:
    ///
    /// - a small bright core (a radial gradient a few px across, scaled with height);
    /// - four 1px streaks that fade along their length - a fixed-length cross, so the fade lives in the
    ///   brightness, not in the reach, and the cross stays a cross as it decays;
    /// - a 1px ring that EXPANDS from the root as the envelope decays - small and bright at the peak,
    ///   wide and faint on the way out, the shockwave of the burst.
    ///
    /// Alpha is the envelope throughout.
    fn draw_flare(&self, c: &mut Canvas, t: &Theme, center: (f32, f32), size: (i32, i32), flare: f32) {
        let a = flare.clamp(0.0, 1.0);
        if a <= 0.004 {
            return;
        }
        let (cx, cy) = center;
        let (w, h) = size;
        let (fx, fy) = (cx.round() as i32, cy.round() as i32);
        let core = Rgba::from_hex(&t.hot, 1.0);
        let col = |k: f32| Rgba::new(core.r, core.g, core.b, (k.clamp(0.0, 1.0) * 255.0).round() as u8);

        // The core: a few px across, softly falling.
        let r_core = (5.0 + h as f32 / 30.0).round() as i32;
        c.radial_gradient(fx, fy, 1, r_core, &[(0.0, col(a)), (1.0, col(0.0))]);

        // Four 1px streaks, brightest at the core, fading to nothing at the tip. Length is fixed so the
        // cross is the dominant read at any envelope level.
        let reach = (0.25 * w as f32).round() as i32;
        for (sx, sy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for s in 1..=reach {
                let f = s as f32 / reach as f32;
                c.fill_rect(fx + sx * s, fy + sy * s, 1, 1, col(a * (1.0 - f)));
            }
        }

        // The expanding ring: radius 4px at the peak out to ~0.3h as it fades.
        let r_ring = (4.0 + (0.3 * h as f32 - 4.0).max(0.0) * (1.0 - a)).round() as i32;
        if r_ring >= 3 {
            let ring = col(a * 0.9);
            let steps = (r_ring as f32 * 6.5).round() as i32 + 8;
            for i in 0..steps {
                let ang = i as f32 / steps as f32 * std::f32::consts::TAU;
                let px = fx + (r_ring as f32 * ang.cos()).round() as i32;
                let py = fy + (r_ring as f32 * ang.sin()).round() as i32;
                c.fill_rect(px, py, 1, 1, ring);
            }
        }
    }

    /// "VIRTUAL SELF" in a 3x5 chrome font with a 1px black outline, composited from `scratch`.
    ///
    /// A private glyph table rather than the canvas's `text_3x5`, because that font deliberately omits
    /// V, T, S and E as ambiguous at 3x5 in its own context - here the string is fixed and known, so the
    /// letters can be drawn unambiguously and the whole title reads.
    fn draw_title(c: &mut Canvas, s: &mut Canvas, w: i32) {
        const TITLE: &str = "VIRTUAL SELF";
        let tw = TITLE.chars().count() as i32 * 4 - 1;
        if w < tw + 6 {
            return; // no room to centre it without clipping glyphs
        }
        // `s` is the reused light layer, already at panel size; clear it for a fresh title pass.
        s.clear();
        let black = Rgba::new(0, 0, 0, 255);
        let x0 = (w - tw) / 2;
        let y0 = 2;
        let chrome: [Rgba; 5] = std::array::from_fn(|i| Rgba::from_hex(CHROME_ROWS[i], 1.0));
        // Pass 1: the black outline, plotted around every set pixel.
        for (ci, ch) in TITLE.chars().enumerate() {
            let rows = title_glyph(ch);
            for (dy, row) in rows.iter().enumerate() {
                for dx in 0..3 {
                    if row & (0b100 >> dx) != 0 {
                        let px = x0 + ci as i32 * 4 + dx;
                        let py = y0 + dy as i32;
                        for oy in -1..=1 {
                            for ox in -1..=1 {
                                s.fill_rect(px + ox, py + oy, 1, 1, black);
                            }
                        }
                    }
                }
            }
        }
        // Pass 2: the chrome gradient, on top of its own outline.
        for (ci, ch) in TITLE.chars().enumerate() {
            let rows = title_glyph(ch);
            for (dy, row) in rows.iter().enumerate() {
                for dx in 0..3 {
                    if row & (0b100 >> dx) != 0 {
                        let px = x0 + ci as i32 * 4 + dx;
                        let py = y0 + dy as i32;
                        s.fill_rect(px, py, 1, 1, chrome[dy]);
                    }
                }
            }
        }
        c.draw_over(s);
    }
}

/// One glyph of the title font, five rows of three bits with bit 2 leftmost. Unknown characters are a
/// blank cell, so the string can only ever degrade to a gap.
fn title_glyph(ch: char) -> [u8; 5] {
    match ch {
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        _ => [0, 0, 0, 0, 0],
    }
}

impl Family for Vswings {
    fn id(&self) -> &'static str {
        "vswings"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        if w < MIN_W || h < MIN_H {
            return; // shed rather than smudge
        }
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 250.0) } else { 16.7 };

        // The interior the wings and grid live in - inside the 2px bevel, so nothing overdraws the frame.
        let (ix0, iy0) = (2.0f32, 2.0f32);
        let (ix1, iy1) = ((w - 3) as f32, (h - 3) as f32);
        let cx = w as f32 / 2.0;
        let root_y = iy0 + ROOT_Y_FRAC * (iy1 - iy0);

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);

        // ---- the opaque panel ----
        //
        // This family covers the widget like the others - see the module note. Filled first; a
        // `clip_to_rounded_rect` at the very end keeps anything (bevel included) off the rounded corners.
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);

        // The grid scroll is advanced here but DRAWN after the bloom - see the ordering note below.
        let rms = if d.rms_l.is_finite() { d.rms_l.clamp(0.0, 1.0) } else { 0.0 };
        let onset = self.onset.update(&d.levels, dt, GRID_ONSET_RATIO, GRID_ONSET_REFRACTORY_MS);
        self.grid_phase += rms * (dt / 1000.0) * GRID_SCROLL;
        if onset {
            self.grid_phase += 1.0 / GRID_HORIZ as f32;
        }
        if !self.grid_phase.is_finite() {
            self.grid_phase = 0.0;
        }

        // ---- the wings ----
        //
        // The same fan, mirrored across the centre: `side = +1` is the right wing, `-1` the left, so the
        // display is symmetric by construction and `wings_are_mirrored_about_the_centre` holds.

        // A light layer for the bright, BLOOMING marks - the hot tips, the peak dots and the flare. The
        // crisp wing BODIES stay on the main canvas and carry the meter's reading; only the accents glow.
        //
        // This split is load-bearing, not decorative. Bloom applied to the whole canvas washes this
        // family flat: the wings cover a large contiguous fan, and a two-pass blur of it lifts nearly
        // every pixel above the alpha floor (measured at 97% of the panel lit at any level), which
        // destroys the one reading the meter has - length. So the bodies are never bloomed; only the
        // small bright accents are, on their own layer, and the halo is composited over the crisp wings.
        // This is the exact idiom `Canvas::bloom`/`draw_over` document for a halo over an opaque field.
        let mut layer = self
            .scratch
            .take()
            .filter(|s| s.width() == w && s.height() == h)
            .unwrap_or_else(|| Canvas::new(w, h));
        layer.clear();

        for i in 0..FEATHERS {
            let band = (i * 2).min(NUM_BANDS - 1);
            let level_i = if d.levels[band].is_finite() { d.levels[band].clamp(0.0, 1.0) } else { 0.0 };
            let peak_i = if d.peaks[band].is_finite() { d.peaks[band].clamp(0.0, 1.0) } else { 0.0 };
            // `d.levels` has already been through `Smoother::new(theme.ballistics)`
            // upstream in `Ticker::tick` - re-smoothing it here with the SAME ballistics is a second
            // low-pass pass, not a second effect. Length tracks the already-smoothed level directly.
            let target = LEN_BASE + level_i * (w as f32 * LEN_SPAN_FRAC);
            let mut smoothed = target;
            if !smoothed.is_finite() {
                smoothed = LEN_BASE;
            }
            self.feather_len[i] = smoothed;
            let peak_len = LEN_BASE + peak_i * (w as f32 * LEN_SPAN_FRAC);

            let theta = (-FAN_DEG + 2.0 * FAN_DEG * (i as f32 / (FEATHERS - 1) as f32))
                * std::f32::consts::PI
                / 180.0;
            let (ct, st) = (theta.cos(), theta.sin());
            for side in [1.0f32, -1.0] {
                let (ux, uy) = (side * ct, st);
                let cap = ray_max((cx, root_y), (ux, uy), (ix0, iy0, ix1, iy1));
                let len = smoothed.min(cap);
                // Body: `lit`, tapering root to tip, crisp on the main canvas.
                Self::feather(c, (cx, root_y), (ux, uy), len, (WIDTH_ROOT, WIDTH_TIP), lit);
                // Tip: the outer fraction in `hot`, on the light layer so it glows - the chrome
                // highlight along the feather.
                let tip_start = len * (1.0 - TIP_FRAC);
                let w_mid = WIDTH_ROOT + (WIDTH_TIP - WIDTH_ROOT) * (1.0 - TIP_FRAC);
                Self::feather(
                    &mut layer,
                    (cx + ux * tip_start, root_y + uy * tip_start),
                    (ux, uy),
                    len - tip_start,
                    (w_mid, WIDTH_TIP),
                    hot,
                );
                // Peak-hold dot, where the peak sits ahead of the current length.
                let plen = peak_len.min(cap);
                if plen > len + 1.0 {
                    let (dx, dy) = (cx + ux * plen, root_y + uy * plen);
                    layer.fill_rect(dx.round() as i32 - 1, dy.round() as i32 - 1, 2, 2, hot);
                }
            }
        }

        // ---- advance the flourish envelope (the flare itself is drawn crisp, below the bevel) ----
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let flare = self.flare.update(fired, dt, FLARE_MS);

        // ---- bloom the accents and lay the halo over the crisp wings ----
        if t.bloom > 0.0 {
            layer.bloom(t.bloom as i32, t.glow_strength);
        }
        c.draw_over(&layer);

        // ---- the receding floor grid, quiet and un-bloomed, over the wings ----
        self.draw_grid(c, t, cx, (ix0, iy0, ix1, iy1));

        // ---- the flourish: a lens flare from the wing root, crisp and never bloomed ----
        self.draw_flare(c, t, (cx, root_y), (w, h), flare);

        // ---- the chrome bevel frame ----
        //
        // The extruded-panel edge: light on the top and left, dark on the bottom and right. Drawn INSIDE
        // the panel rect (at x/y 1..) so the final clip keeps it - the panel fill starts at x=1,y=2.
        let bevel_l = Rgba::from_hex(BEVEL_LIGHT, 1.0);
        let bevel_d = Rgba::from_hex(BEVEL_DARK, 1.0);
        c.fill_rect(1, 2, w - 2, 2, bevel_l);
        c.fill_rect(1, 2, 2, h - 4, bevel_l);
        c.fill_rect(1, h - 4, w - 2, 2, bevel_d);
        c.fill_rect(w - 3, 2, 2, h - 4, bevel_d);

        // ---- the title, only where it fits, on top of everything ----
        if h >= TITLE_MIN_H {
            Self::draw_title(c, &mut layer, w);
        }

        // Keep nothing on the rounded corners - the panel, the bevel and the wings are all authored
        // inside this rect, and this makes that a guarantee rather than an assumption.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);

        // The light layer is kept for reuse next frame - see the allocation note above.
        self.scratch = Some(layer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    fn frames(fam: &mut Vswings, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
        let mut c = Canvas::new(w, h);
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = level * (1.0 - i as f32 / 96.0);
        }
        d.peaks = d.levels;
        d.rms_l = level;
        d.rms_r = level;
        d.dt_ms = 16.7;
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, t, &d);
        }
        c
    }
    /// Pixels the family DREW OVER the panel - opaque and differing from the panel colour by more than a
    /// rounding margin. The panel is opaque, so "lit" is paint on top of it, not raw alpha.
    fn drew_over_panel(px: Rgba, panel: Rgba) -> bool {
        let d = (px.r as i32 - panel.r as i32).abs()
            + (px.g as i32 - panel.g as i32).abs()
            + (px.b as i32 - panel.b as i32).abs();
        px.a > 8 && d > 24
    }
    fn lit(c: &Canvas, t: &Theme) -> usize {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let mut n = 0;
        for y in 0..c.height() {
            for x in 0..c.width() {
                if drew_over_panel(c.get(x, y), panel) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"vswings"));
        assert_eq!(crate::render::family_for("vswings").id(), "vswings");
        assert_ne!(crate::themes::family_label("vswings"), "vswings");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "vswings").count(), 5);
    }

    #[test]
    fn wings_are_mirrored_about_the_centre() {
        let t = theme("vswings-particle-arts");
        let c = frames(&mut Vswings::default(), &t, 380, 48, 0.6, 30);
        // Painted-over-panel count in the left half vs the right half within 10%: the two wings are a
        // mirror pair.
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let (mut l, mut r) = (0usize, 0usize);
        for y in 0..48 {
            for x in 0..380 {
                if drew_over_panel(c.get(x, y), panel) {
                    if x < 190 {
                        l += 1
                    } else {
                        r += 1
                    }
                }
            }
        }
        assert!((l as f32 - r as f32).abs() < 0.10 * l.max(r) as f32, "left {l} right {r}");
    }

    #[test]
    fn louder_bass_makes_longer_root_feathers() {
        let t = theme("vswings-particle-arts");
        let quiet = frames(&mut Vswings::default(), &t, 380, 48, 0.2, 30);
        let loud = frames(&mut Vswings::default(), &t, 380, 48, 0.8, 30);
        assert!(lit(&loud, &t) > lit(&quiet, &t) + 200, "quiet {} loud {}", lit(&quiet, &t), lit(&loud, &t));
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("vswings-eon-break");
        let c = frames(&mut Vswings::default(), &t, 380, 48, 0.0, 10);
        assert!(lit(&c, &t) as f32 >= 0.02 * (380 * 48) as f32, "grid should still show: {}", lit(&c, &t));
    }

    #[test]
    fn fits_the_narrow_panel_and_flourish_does_not_panic() {
        for id in ["vswings-particle-arts", "vswings-eon-break", "vswings-angel-voices", "vswings-utopia", "vswings-ghost"] {
            let t = theme(id);
            let mut fam = Vswings::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 40);
            assert!(lit(&c, &t) > 0, "{id}");
        }
    }

    #[test]
    #[ignore]
    fn probe_vswings_cost() {
        let t = theme("vswings-eon-break");
        let mut fam = Vswings::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = 0.6 * (1.0 - i as f32 / 96.0);
        }
        d.peaks = d.levels;
        d.rms_l = 0.3;
        for _ in 0..60 {
            fam.draw(&mut c, &t, &d);
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        println!("vswings: {:.3} ms/frame at 380x60", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }

    /// Dumps for the eye test - composited over `#202020` like every other family's dump.
    ///
    /// Run: cargo test --release dump_vswings -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_vswings() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let bg = Rgba::from_hex("#202020", 1.0);
        let write = |name: String, c: &Canvas| {
            let mut out = Vec::with_capacity((c.width() * c.height() * 4) as usize);
            for y in 0..c.height() {
                for x in 0..c.width() {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for (ch, base) in [(px.r, bg.r), (px.g, bg.g), (px.b, bg.b)] {
                        out.push((ch as f32 * a + base as f32 * (1.0 - a)).round() as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("{name}.rgba")), &out).unwrap();
        };
        // A shaped spectrum that beats: bass-heavy with a wobble, so the root feathers dominate.
        let frame = |level: f32, t_s: f32| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                let shape = (1.0 - f).powf(1.4) * 0.7 + 0.12;
                let wob = 1.0 + 0.3 * (t_s * 2.4 + f * 6.0).sin();
                *v = ((shape * wob) * level).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d.rms_l = level * 0.5;
            d.rms_r = level * 0.5;
            d
        };
        for id in ["vswings-particle-arts", "vswings-eon-break", "vswings-angel-voices", "vswings-utopia", "vswings-ghost"] {
            let t = theme(id);
            // Calm and loud, at the real wide size.
            // Production builds a FRESH canvas every frame (main.rs), so the dump must clear before each
            // draw - otherwise a transparent family's glow accumulates across frames and reads as a blob
            // that never happens on screen. The family's own state (smoothing, phase) still settles.
            for (tag, level) in [("calm", 0.28f32), ("loud", 0.85)] {
                let mut fam = Vswings::default();
                let mut c = Canvas::new(380, 60);
                for k in 0..90 {
                    c.clear();
                    fam.draw(&mut c, &t, &frame(level, k as f32 * 0.0167));
                }
                write(format!("vswings-{}-{tag}", &t.id["vswings-".len()..]), &c);
            }
            // Flourish: settle, fire, then capture TWO frames - one at the envelope peak and one at
            // ~40% decay (the ring has expanded by then). Each is a fresh frame (clear each), post-hit
            // music level dropped to 0.3.
            let mut fam = Vswings::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..90 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            }
            fam.flourish.force_next();
            // Peak: two frames after the fire, flare ~0.94.
            for k in 90..93 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.45, k as f32 * 0.0167));
            }
            write(format!("vswings-{}-flourish", &t.id["vswings-".len()..]), &c);
            // ~40% decay: 0.6 of the 600ms decay has elapsed by ~22 frames after the fire.
            for k in 93..113 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.3, k as f32 * 0.0167));
            }
            write(format!("vswings-{}-flourish-decay", &t.id["vswings-".len()..]), &c);
        }
        // One at the awkward mid size, to prove it still reads.
        let t = theme("vswings-eon-break");
        let mut fam = Vswings::default();
        let mut c = Canvas::new(190, 48);
        for k in 0..90 {
            c.clear();
            fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
        }
        write("vswings-eon-break-190x48".into(), &c);
    }
}
