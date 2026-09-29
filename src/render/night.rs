//! The Night City: HUD family: a Cyberpunk 2077 combat-HUD strip.
//!
//! A black panel framed in a 1 px accent line whose top-right corner is chamfered at 45 degrees, with
//! a hatch triangle in the bottom-left corner - the two ornaments that carry the 2077 UI language
//! without lifting any trademark asset (no samurai, no wordmark, no logo geometry). Faint scanlines
//! every third row read as a CRT combat overlay.
//!
//! # The meter is the cells
//!
//! Sixteen chamfered CELLS stride across the interior, each averaging four bands and filling bottom-up
//! to its level with a 45 degree hatch capped by a solid bar. A cell's outline switches to `hot` when
//! the band sits at its peak, and a 1 px `hot` tick hangs at the decaying peak - so the strip reads as
//! a bottom-up bar meter dressed as a HUD element. A cyan SCANNER polyline traces the raw 64-band
//! spectrum across the top third with a two-frame afterimage, and a left-hand READOUT column
//! (`RAM`/`HP`/`NET`) fills the 34 px the HUD's telemetry would occupy. At a narrow panel the readouts
//! drop and the cells take the full width.
//!
//! # The panel is opaque
//!
//! Like every family this one paints an opaque panel first and clips to the rounded rect last, so the
//! Windows weather widget is covered while music plays. Every layer is drawn over that panel, and the
//! flourish re-composites over it too.
//!
//! # The flourish - a relic malfunction
//!
//! On a rare exceptional hit the whole finished frame is re-composited as an RGB split (the red and
//! blue channels slid apart by a few pixels), two red glitch bars sweep top to bottom, and the readout
//! column reads `SYSTEM` / `MALFUNCTION` / `RELIC 2.0 ERR`. It decays over 600 ms back to a clean frame.
//!
//! # Colour goes through linear light
//!
//! Colour is mixed through the shared linear-light blend (`Rgba::lerp_linear`, the canvas alpha blend),
//! never by averaging sRGB bytes. There is no rainbow colourway here - the identity is the five fixed
//! Night City palettes.

use crate::dsp::bands::NUM_BANDS;
use crate::dsp::flourish::{Envelope, Trigger};
use crate::dsp::onset::Flux;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::font3x5;
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Sixteen cells, each averaging four of the 64 bands.
const CELLS: usize = 16;
/// Gap between cells, in pixels.
const CELL_GAP: i32 = 2;
/// The frame's top-right chamfer, in pixels.
const CHAMFER_FRAME: i32 = 6;
/// Each cell's own top-right chamfer, in pixels.
const CHAMFER_CELL: i32 = 2;
/// Hatch line spacing, in pixels (the `(x + y) % HATCH_PITCH == 0` lattice).
const HATCH_PITCH: i32 = 3;
/// Width of the left readout column, in pixels.
const READOUT_W: i32 = 34;
/// The readouts only draw when the panel is at least this wide; below it the cells take the full width.
const READOUT_MIN_W: i32 = 160;
/// A scanline every this many interior rows.
const SCANLINE_EVERY: i32 = 3;
/// How long the relic malfunction runs, in milliseconds.
const MALFUNCTION_MS: f32 = 600.0;
/// Red/blue channel separation at full envelope, in pixels.
const SPLIT_PX: f32 = 3.0;
/// The two horizontal glitch bars swept during the malfunction.
const GLITCH_BARS: usize = 2;

/// The onset net that advances the NET readout and counts strong (bass) hits - the same permissive
/// flux net the flourish uses to find candidates.
const ONSET_RATIO: f32 = 2.8;
const ONSET_REFRACTORY_MS: f32 = 200.0;
/// A strong onset needs the low bands over this - it advances the NET hex.
const BASS_ONSET: f32 = 0.5;

/// The malfunction red, used for the glitch bars and (except on arasaka) the malfunction readouts.
const MALFUNCTION_RED: &str = "#FF003C";

/// The cell layout for the last frame drawn, recomputed each frame with no allocation. The tests read
/// it back through `cell_box_for_test` / `first_cell_x`.
#[derive(Clone, Copy)]
struct Layout {
    /// `(x0, x1, y0, y1)` per cell - `x1`/`y1` exclusive. Read back only by the tests.
    #[cfg_attr(not(test), allow(dead_code))]
    cells: [(i32, i32, i32, i32); CELLS],
    /// Cell 0's left edge. Read back only by the tests.
    #[cfg_attr(not(test), allow(dead_code))]
    first_cell_x: i32,
}

impl Default for Layout {
    fn default() -> Self {
        Layout { cells: [(0, 0, 0, 0); CELLS], first_cell_x: 0 }
    }
}

pub struct Night {
    /// Fires the relic malfunction on a rare, exceptional hit. `pub(crate)` so the family tests force it.
    pub(crate) flourish: Trigger,
    /// The malfunction's one-shot decay envelope.
    malfunction: Envelope,
    /// The onset that advances the NET readout and drives the strong-hit count.
    onset: Flux,
    /// Smoothed per-cell fill, so a cell does not jitter frame to frame.
    fill: [f32; CELLS],
    /// Peak-hold per cell, decaying at `peak_fall`.
    peak: [f32; CELLS],
    /// Heals toward 100 in silence, drops with the peak mean.
    hp: f32,
    /// The NET readout, advanced on strong onsets.
    net_hex: u8,
    /// The scanner's afterimage: this frame's and the two previous frames' per-band levels.
    scan_prev: [f32; NUM_BANDS],
    scan_prev2: [f32; NUM_BANDS],
    /// The cell layout from the last frame, for the tests.
    layout: Layout,
    /// The RGB-split source, sized once per panel size and reused.
    scratch: Option<Canvas>,
    /// The glitch bars' y positions, 0..1 of the interior height.
    bars: [f32; GLITCH_BARS],
    /// splitmix64 state, for the NET hex.
    rng: u64,
}

impl Default for Night {
    fn default() -> Self {
        Night {
            flourish: Default::default(),
            malfunction: Default::default(),
            onset: Default::default(),
            fill: [0.0; CELLS],
            peak: [0.0; CELLS],
            hp: 100.0,
            net_hex: 0x2f,
            scan_prev: [0.0; NUM_BANDS],
            scan_prev2: [0.0; NUM_BANDS],
            layout: Layout::default(),
            scratch: None,
            bars: [0.0; GLITCH_BARS],
            rng: 0x1234_5678_9abc_def1,
        }
    }
}

impl Night {
    /// One draw of splitmix64.
    fn next_rng(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The x of cell 0's left edge as laid out on the last frame.
    #[cfg(test)]
    pub fn first_cell_x(&self) -> i32 {
        self.layout.first_cell_x
    }

    /// The `(x0, x1, y0, y1)` box of a cell from the last frame's layout.
    #[cfg(test)]
    pub fn cell_box_for_test(&self, cell: usize) -> (i32, i32, i32, i32) {
        self.layout.cells[cell.min(CELLS - 1)]
    }
}

/// The same straight colour at a new alpha, for the translucent hatch and scanner afterimage.
fn alpha(c: Rgba, a: f32) -> Rgba {
    Rgba::new(c.r, c.g, c.b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// A finite level, clamped 0..1.
fn lvl(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

impl Family for Night {
    fn id(&self) -> &'static str {
        "night"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // ---- the opaque panel ----
        let panel = Rgba::from_hex(&t.panel, 1.0);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);

        // The interior. `ix1`/`iy1` are exclusive.
        let (ix0, iy0) = (3i32, 4i32);
        let (ix1, iy1) = (w - 3, h - 4);
        let iw = ix1 - ix0;
        let ih = iy1 - iy0;
        if iw < 2 || ih < 2 {
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return; // too small to draw a HUD
        }

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let edge = Rgba::from_hex(&t.edge, t.edge_alpha.clamp(0.0, 1.0));
        let ghost = Rgba::from_hex(&t.lit, t.ghost.clamp(0.0, 1.0));
        let arasaka = t.id == "night-arasaka";
        let value_col = if arasaka { hot } else { lit };
        // The scanner reads only `zones[0].lit`; each colourway registers exactly one such zone.
        let scanner = {
            let hex = t.zones.first().map(|z| z.lit.as_str()).unwrap_or(t.lit.as_str());
            Rgba::from_hex(hex, 1.0)
        };

        // ---- onset: advance the NET readout, count strong hits ----
        let onset = self.onset.update(&d.levels, dt, ONSET_RATIO, ONSET_REFRACTORY_MS);
        let bass = {
            let mut s = 0.0f32;
            for &v in &d.levels[0..8] {
                s += lvl(v);
            }
            s / 8.0
        };
        if onset && bass > BASS_ONSET {
            self.net_hex = (self.next_rng() & 0xff) as u8;
        }

        // ---- flourish envelope ----
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let env = self.malfunction.update(fired, dt, MALFUNCTION_MS);
        if fired {
            self.bars = [0.0, 0.3];
        }
        let malfunctioning = env > 0.1;

        // ---- cell ballistics + peak hold + HP ----
        let attack = t.ballistics.attack.clamp(0.0, 1.0);
        let decay = t.ballistics.decay.clamp(0.0, 1.0);
        let peak_fall = t.ballistics.peak_fall.max(0.0);
        for i in 0..CELLS {
            let mut sum = 0.0f32;
            for &v in &d.levels[i * 4..i * 4 + 4] {
                sum += lvl(v);
            }
            let target = sum / 4.0;
            let cur = self.fill[i];
            let kf = if target > cur { attack } else { decay };
            let mut f = cur + (target - cur) * kf;
            if !f.is_finite() {
                f = 0.0;
            }
            self.fill[i] = f;
            let mut pk = (self.peak[i] - peak_fall).max(f);
            if !pk.is_finite() {
                pk = f;
            }
            self.peak[i] = pk;
        }
        let peak_mean = {
            let mut s = 0.0f32;
            for &v in &d.peaks {
                s += lvl(v);
            }
            s / NUM_BANDS as f32
        };
        let hp_target = 100.0 * (1.0 - peak_mean);
        self.hp += (hp_target - self.hp) * 0.05;
        if !self.hp.is_finite() {
            self.hp = 100.0;
        }
        self.hp = self.hp.clamp(0.0, 100.0);

        // ---- scanlines (skip the top interior row so the chamfer cut stays panel) ----
        for y in (iy0 + 1)..iy1 {
            if (y - iy0) % SCANLINE_EVERY == 0 {
                c.fill_rect(ix0, y, iw, 1, ghost);
            }
        }

        // ---- the frame, chamfered top-right ----
        self.draw_frame(c, (ix0, iy0, ix1, iy1), edge, lit);

        // ---- layout ----
        let has_readouts = w >= READOUT_MIN_W;
        let cx0 = ix0 + if has_readouts { READOUT_W } else { 0 } + 2;
        let cxr = ix1 - 2;
        let span = (cxr - cx0).max(1);
        let cell_w = ((span - (CELLS as i32 - 1) * CELL_GAP) / CELLS as i32).max(1);
        let cy0 = iy0 + if h >= 48 { ih / 3 } else { 2 };
        let cy1 = iy1 - 2;
        let mut layout = Layout { cells: [(0, 0, 0, 0); CELLS], first_cell_x: cx0 };
        for (i, cell) in layout.cells.iter_mut().enumerate() {
            let x0 = cx0 + i as i32 * (cell_w + CELL_GAP);
            *cell = (x0, x0 + cell_w, cy0, cy1);
        }
        self.layout = layout;

        // ---- readouts (normal telemetry unless malfunctioning) ----
        if has_readouts && !malfunctioning {
            self.draw_readouts(c, (ix0, iy0), h, d, edge, value_col);
        }

        // ---- the cells ----
        for i in 0..CELLS {
            let (x0, x1, y0, y1) = layout.cells[i];
            self.draw_cell(c, (x0, x1, y0, y1), self.fill[i], self.peak[i], lit, hot, edge);
        }

        // ---- the scanner ----
        let (scan_top, scan_h) = if h < 48 {
            (cy0, ((cy1 - cy0) / 3).max(1))
        } else {
            (iy0 + 2, (cy0 - (iy0 + 2)).max(1))
        };
        self.draw_scanner(c, cx0, span, scan_top, scan_h, scanner, &d.levels);

        // ---- the flourish: relic malfunction ----
        if malfunctioning {
            if has_readouts {
                self.draw_malfunction_readouts(c, (ix0, iy0), h, arasaka, hot);
            }
            self.rgb_split(c, (ix0, iy0, ix1, iy1), env);
            // Two red glitch bars sweeping top to bottom at different speeds.
            let red = Rgba::from_hex(MALFUNCTION_RED, env.clamp(0.0, 1.0));
            for k in 0..GLITCH_BARS {
                self.bars[k] = (self.bars[k] + (0.9 + 0.6 * k as f32) * dt / 1000.0 * env).rem_euclid(1.0);
                if !self.bars[k].is_finite() {
                    self.bars[k] = 0.0;
                }
                let by = iy0 + (self.bars[k] * ih as f32) as i32;
                c.fill_rect(ix0, by.min(iy1 - 1), iw, 3.min(iy1 - by), red);
            }
        }

        // Keep nothing on the rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

impl Night {
    /// The 1 px accent frame, its top-right corner cut at 45 degrees over `CHAMFER_FRAME` px, with the
    /// bottom-left hatch triangle. Drawn inside the interior, so its corner pixel is `(ix1-1, iy0)`.
    fn draw_frame(&self, c: &mut Canvas, bbox: (i32, i32, i32, i32), edge: Rgba, lit: Rgba) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let iw = ix1 - ix0;
        let ih = iy1 - iy0;
        // Top edge, stopping CHAMFER_FRAME px short of the right.
        c.fill_rect(ix0, iy0, (iw - CHAMFER_FRAME).max(0), 1, edge);
        // Bottom edge, full width.
        c.fill_rect(ix0, iy1 - 1, iw, 1, edge);
        // Left edge, full height.
        c.fill_rect(ix0, iy0, 1, ih, edge);
        // Right edge, starting CHAMFER_FRAME px down.
        c.fill_rect(ix1 - 1, iy0 + CHAMFER_FRAME, 1, (ih - CHAMFER_FRAME).max(0), edge);
        // The chamfer, joining the two stopped edges.
        c.line(ix1 - 1 - CHAMFER_FRAME, iy0, ix1 - 1, iy0 + CHAMFER_FRAME, edge);

        // Bottom-left hatch triangle: 45 degree lines HATCH_PITCH apart inside an 8x8 corner, at half
        // alpha so it reads as a filled-in HUD corner rather than a second frame.
        let hcol = alpha(lit, 0.5);
        let base = iy1 - 1;
        let mut k = 0;
        while k < 8 {
            c.line(ix0 + k, base, ix0, base - k, hcol);
            k += HATCH_PITCH;
        }
    }

    /// The left readout column: `RAM` + four block glyphs by RMS quartile, `HP` + a number, `NET` + a
    /// live hex byte. Three rows when tall, only `HP` when short.
    fn draw_readouts(&self, c: &mut Canvas, origin: (i32, i32), h: i32, d: &FrameData, edge: Rgba, value: Rgba) {
        let (ix0, iy0) = origin;
        let x = ix0 + 2;
        let rms = if d.rms_l.is_finite() { d.rms_l.clamp(0.0, 1.0) } else { 0.0 };
        let three = h >= 52;
        let (row_ram, row_hp, row_net) = (iy0 + 2, if three { iy0 + 8 } else { iy0 + 2 }, iy0 + 14);

        if three {
            // RAM + four quartile blocks.
            let lw = font3x5::draw(c, x, row_ram, "RAM", edge);
            let filled = (rms * 4.0).round() as i32;
            let bx0 = x + lw + 2;
            for i in 0..4 {
                let bx = bx0 + i * 4;
                let col = if i < filled { value } else { edge };
                c.fill_rect(bx, row_ram, 3, 5, col);
            }
        }

        // HP + number.
        let lw = font3x5::draw(c, x, row_hp, "HP", edge);
        let mut buf = [0u8; 3];
        let s = fmt_u32(self.hp.round() as u32, &mut buf);
        font3x5::draw(c, x + lw + 2, row_hp, s, value);

        if three {
            // NET + a two-digit hex byte.
            let lw = font3x5::draw(c, x, row_net, "NET", edge);
            let mut hx = [0u8; 2];
            fmt_hex(self.net_hex, &mut hx);
            if let Ok(s) = std::str::from_utf8(&hx) {
                font3x5::draw(c, x + lw + 2, row_net, s, value);
            }
        }
    }

    /// The malfunction telemetry: `SYSTEM` / `MALFUNCTION` / `RELIC 2.0 ERR` in red (white on arasaka),
    /// drawn over the composed frame so the RGB split fringes it too.
    fn draw_malfunction_readouts(&self, c: &mut Canvas, origin: (i32, i32), h: i32, arasaka: bool, hot: Rgba) {
        let (ix0, iy0) = origin;
        let x = ix0 + 2;
        let col = if arasaka { hot } else { Rgba::from_hex(MALFUNCTION_RED, 1.0) };
        if h >= 52 {
            font3x5::draw(c, x, iy0 + 2, "SYSTEM", col);
            font3x5::draw(c, x, iy0 + 8, "MALFUNCTION", col);
            font3x5::draw(c, x, iy0 + 14, "RELIC 2.0 ERR", col);
        } else {
            font3x5::draw(c, x, iy0 + 2, "MALFUNCTION", col);
        }
    }

    /// One cell: a chamfered outline (edge, or hot at peak) and a bottom-up hatch fill capped by a
    /// solid bar, with a hot peak tick. Nothing draws outside the cell box.
    #[allow(clippy::too_many_arguments)]
    fn draw_cell(&self, c: &mut Canvas, bbox: (i32, i32, i32, i32), fill: f32, peak: f32, lit: Rgba, hot: Rgba, edge: Rgba) {
        let (x0, x1, y0, y1) = bbox;
        let cw = x1 - x0;
        let chh = y1 - y0;
        if cw < 3 || chh < 3 {
            // Too small for chrome: just a lit stub so the meter still reads.
            let fh = (lvl(fill) * chh as f32).round() as i32;
            if fh >= 1 {
                c.fill_rect(x0, (y1 - fh).max(y0), cw.max(1), fh.min(chh), lit);
            }
            return;
        }
        // Hot outline only once a cell is meaningfully lit and sitting at its own peak - so a quiet
        // resting strip stays edge-coloured rather than glowing hot across every cell.
        let at_peak = (fill - peak).abs() < 0.05 && peak > 0.15;
        let ocol = if at_peak { hot } else { edge };
        // Outline with a 2 px top-right chamfer.
        c.fill_rect(x0, y0, cw - CHAMFER_CELL, 1, ocol); // top, stopping short
        c.fill_rect(x0, y1 - 1, cw, 1, ocol); // bottom
        c.fill_rect(x0, y0, 1, chh, ocol); // left
        c.fill_rect(x1 - 1, y0 + CHAMFER_CELL, 1, chh - CHAMFER_CELL, ocol); // right, starting low
        c.line(x1 - 1 - CHAMFER_CELL, y0, x1 - 1, y0 + CHAMFER_CELL, ocol); // the chamfer

        let inner_h = (chh - 2).max(0);
        let fh = (lvl(fill) * inner_h as f32).round() as i32;
        if fh >= 1 {
            let fill_top = ((y1 - 1) - fh).max(y0 + 1);
            // Hatch: the 45 degree lattice, bounded inside the outline.
            for yy in fill_top..=(y1 - 2) {
                let mut xx = x0 + 1;
                while xx < x1 - 1 {
                    if (xx + yy).rem_euclid(HATCH_PITCH) == 0 {
                        c.fill_rect(xx, yy, 1, 1, lit);
                    }
                    xx += 1;
                }
            }
            // Solid 2 px cap at the top of the fill.
            c.fill_rect(x0 + 1, fill_top, (cw - 2).max(1), 2.min((y1 - 1) - fill_top).max(1), lit);
        }
        // The peak tick.
        if peak > 0.05 {
            let py = ((y1 - 1) - (lvl(peak) * inner_h as f32).round() as i32).clamp(y0 + 1, y1 - 2);
            c.fill_rect(x0 + 1, py, (cw - 2).max(1), 1, hot);
        }
    }

    /// The scanner: a 1 px polyline tracing the raw 64-band spectrum across `span`, with the two
    /// previous frames drawn faintly behind it as an afterimage, then the history shifted.
    #[allow(clippy::too_many_arguments)]
    fn draw_scanner(&mut self, c: &mut Canvas, cx0: i32, span: i32, top: i32, height: i32, col: Rgba, levels: &[f32; NUM_BANDS]) {
        let point = |i: usize, lv: f32| -> (i32, i32) {
            let x = cx0 + (i as i32 * (span - 1)) / (NUM_BANDS as i32 - 1);
            let y = top + (height as f32 * (1.0 - lvl(lv))).round() as i32;
            (x, y)
        };
        let polyline = |c: &mut Canvas, arr: &[f32; NUM_BANDS], colour: Rgba| {
            let mut prev = point(0, arr[0]);
            for i in 1..NUM_BANDS {
                let cur = point(i, arr[i]);
                c.line(prev.0, prev.1, cur.0, cur.1, colour);
                prev = cur;
            }
        };
        // Oldest first, so the current line lands brightest on top.
        polyline(c, &self.scan_prev2, alpha(col, 0.15));
        polyline(c, &self.scan_prev, alpha(col, 0.35));
        let mut current = [0.0f32; NUM_BANDS];
        for (i, v) in current.iter_mut().enumerate() {
            *v = lvl(levels[i]);
        }
        polyline(c, &current, col);
        self.scan_prev2 = self.scan_prev;
        self.scan_prev = current;
    }

    /// Re-composites the interior as an RGB split: the finished frame copied into scratch, then drawn
    /// back with the red plate slid `dx` left and the blue plate `dx` right. Alpha stays 255, so this
    /// cannot punch a hole in the opaque panel. Bounded to the interior.
    fn rgb_split(&mut self, c: &mut Canvas, bbox: (i32, i32, i32, i32), env: f32) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let (w, h) = (c.width(), c.height());
        let iw = ix1 - ix0;
        let ih = iy1 - iy0;
        let dx = (SPLIT_PX * env).round() as i32;
        if dx == 0 {
            return; // no visible separation, and no point paying for the loop
        }
        let mut scr = self
            .scratch
            .take()
            .filter(|s| s.width() == w && s.height() == h)
            .unwrap_or_else(|| Canvas::new(w, h));
        scr.clear();
        scr.copy_region(c, (ix0, iy0), (ix0, iy0), iw, ih);
        for yy in iy0..iy1 {
            for xx in ix0..ix1 {
                let r = scr.get(xx - dx, yy).r;
                let g = scr.get(xx, yy).g;
                let b = scr.get(xx + dx, yy).b;
                c.fill_rect(xx, yy, 1, 1, Rgba::new(r, g, b, 255));
            }
        }
        self.scratch = Some(scr);
    }
}

/// Formats `v` (0..=999) as decimal into `buf` with no leading zeros, returning the slice.
fn fmt_u32(v: u32, buf: &mut [u8; 3]) -> &str {
    let v = v.min(999);
    let (h, t, o) = (v / 100, (v / 10) % 10, v % 10);
    let (start, digits): (usize, [u8; 3]) = if h > 0 {
        (0, [b'0' + h as u8, b'0' + t as u8, b'0' + o as u8])
    } else if t > 0 {
        (1, [b'0', b'0' + t as u8, b'0' + o as u8])
    } else {
        (2, [b'0', b'0', b'0' + o as u8])
    };
    *buf = digits;
    std::str::from_utf8(&buf[start..]).unwrap_or("0")
}

/// Formats `b` as two uppercase hex digits into `out`.
fn fmt_hex(b: u8, out: &mut [u8; 2]) {
    let nib = |n: u8| if n < 10 { b'0' + n } else { b'A' + (n - 10) };
    out[0] = nib(b >> 4);
    out[1] = nib(b & 0xf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::{Canvas, Rgba};
    use crate::render::{FrameData};
    use crate::themes::Theme;

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    fn frames(fam: &mut Night, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
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
        assert!(crate::render::KNOWN_FAMILIES.contains(&"night"));
        assert_eq!(crate::render::family_for("night").id(), "night");
        assert_ne!(crate::themes::family_label("night"), "night");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "night").count(), 5);
    }

    #[test]
    fn cells_fill_with_their_bands() {
        let t = theme("night-yellow");
        let mut fam = Night::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData::default();
        for v in d.levels[0..4].iter_mut() { *v = 0.9; }
        d.peaks = d.levels; d.dt_ms = 16.7;
        for k in 0..40 { d.time_s = k as f32 * 0.0167; fam.draw(&mut c, &t, &d); }
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let lit_col = Rgba::from_hex(&t.lit, 1.0);
        let is_lit = |p: Rgba| drew_over_panel(p, panel) && (p.r as i32 - lit_col.r as i32).abs() < 40 && (p.g as i32 - lit_col.g as i32).abs() < 40;
        let filled_rows = |cell: i32| -> usize {
            let (x0, x1, y0, y1) = fam.cell_box_for_test(cell as usize);
            (y0..y1).filter(|&y| (x0 + 1..x1 - 1).any(|x| is_lit(c.get(x, y)))).count()
        };
        let (_, _, y0, y1) = fam.cell_box_for_test(0);
        let h = (y1 - y0) as usize;
        assert!(filled_rows(0) * 100 >= h * 60, "cell 0 {} of {h}", filled_rows(0));
        assert!(filled_rows(8) * 100 <= h * 10, "cell 8 {} of {h}", filled_rows(8));
    }

    #[test]
    fn chamfer_is_cut() {
        let t = theme("night-yellow");
        let c = frames(&mut Night::default(), &t, 380, 48, 0.3, 5);
        let panel = Rgba::from_hex(&t.panel, 1.0);
        // The frame outline is drawn INSIDE the interior (ix1 = w-3), so its top-right corner pixel is
        // (ix1-1, iy0) = (w-4, 4), not (w-3, 4) as the brief's draft hardcoded. See the task report.
        let (fx1, fy0) = (380 - 4, 4); // frame's top-right corner pixel
        assert!(!drew_over_panel(c.get(fx1, fy0), panel), "corner should be cut");
        assert!(drew_over_panel(c.get(fx1 - 7, fy0), panel), "top edge should be drawn");
        assert!(drew_over_panel(c.get(fx1, fy0 + 7), panel), "right edge should be drawn");
    }

    #[test]
    fn readouts_are_dropped_on_a_narrow_panel() {
        let t = theme("night-yellow");
        let mut fam = Night::default();
        let _ = frames(&mut fam, &t, 128, 44, 0.3, 3);
        assert!(fam.first_cell_x() <= 3 + 6, "first cell at {}", fam.first_cell_x());
        let mut wide = Night::default();
        let _ = frames(&mut wide, &t, 380, 60, 0.3, 3);
        assert!(wide.first_cell_x() >= 3 + 34, "readouts should occupy 34px: first cell at {}", wide.first_cell_x());
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("night-corpo");
        let c = frames(&mut Night::default(), &t, 380, 48, 0.0, 10);
        assert!(lit(&c, &t) as f32 >= 0.02 * (380 * 48) as f32, "frame + cells + HP 100: {}", lit(&c, &t));
    }

    #[test]
    fn malfunction_leaves_the_panel_after_the_envelope() {
        let t = theme("night-netrunner");
        let mut a = Night::default();
        let mut b = Night::default();
        let _ = frames(&mut a, &t, 190, 48, 0.4, 5);
        let _ = frames(&mut b, &t, 190, 48, 0.4, 5);
        a.flourish.force_next();
        let ca = frames(&mut a, &t, 190, 48, 0.4, 50);
        let cb = frames(&mut b, &t, 190, 48, 0.4, 50);
        for y in 0..48 { for x in 0..190 {
            let (p, q) = (ca.get(x, y), cb.get(x, y));
            assert!((p.r as i32 - q.r as i32).abs() <= 1 && (p.g as i32 - q.g as i32).abs() <= 1 && (p.b as i32 - q.b as i32).abs() <= 1, "residue at {x},{y}");
        } }
    }

    #[test]
    fn flourish_at_narrow_size_does_not_panic() {
        for id in ["night-yellow", "night-arasaka", "night-netrunner", "night-corpo", "night-liberty"] {
            let t = theme(id);
            let mut fam = Night::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 12);
            assert!(lit(&c, &t) > 0, "{id}");
            let _ = frames(&mut Night::default(), &t, 128, 44, 0.6, 5);
        }
    }

    /// Dumps for the eye test - composited over `#202020` like every other family's dump.
    ///
    /// Run: cargo test --release dump_night -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_night() {
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
        // A shaped spectrum that beats, with a strong bass kick every so often.
        let frame = |level: f32, t_s: f32, beat: bool| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                let shape = (1.0 - f).powf(1.1) * 0.72 + 0.10;
                let wob = 1.0 + 0.32 * (t_s * 2.4 + f * 6.0).sin();
                let kick = if beat { 1.0 + 0.9 * (1.0 - f) } else { 1.0 };
                *v = ((shape * wob * kick) * level).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d.rms_l = level * 0.6;
            d.rms_r = level * 0.6;
            d
        };
        for id in ["night-yellow", "night-arasaka", "night-netrunner", "night-corpo", "night-liberty"] {
            let t = theme(id);
            let short = &id["night-".len()..];
            for (tag, level) in [("calm", 0.28f32), ("loud", 0.85)] {
                let mut fam = Night::default();
                let mut c = Canvas::new(380, 60);
                for k in 0..120 {
                    c.clear();
                    fam.draw(&mut c, &t, &frame(level, k as f32 * 0.0167, k % 24 == 0));
                }
                write(format!("night-{short}-{tag}"), &c);
            }
            // Flourish: settle, fire, capture a mid-decay frame, then a later decay frame.
            let mut fam = Night::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..120 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167, k % 24 == 0));
            }
            fam.flourish.force_next();
            c.clear();
            fam.draw(&mut c, &t, &frame(0.45, 120.0 * 0.0167, false));
            write(format!("night-{short}-flourish"), &c);
            for k in 121..128 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.35, k as f32 * 0.0167, false));
            }
            write(format!("night-{short}-flourish-decay"), &c);
        }
        // The classic at the two narrow shapes.
        for (tag, w, h) in [("190x48", 190, 48), ("128x44", 128, 44)] {
            let t = theme("night-yellow");
            let mut fam = Night::default();
            let mut c = Canvas::new(w, h);
            for k in 0..120 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.8, k as f32 * 0.0167, k % 24 == 0));
            }
            write(format!("night-yellow-{tag}"), &c);
        }
        println!("wrote night dumps to {}", dir.display());
    }

    /// The per-frame cost, steady and during the malfunction (the RGB split is the expensive path),
    /// well under the 2ms budget at 380x60.
    ///
    /// Run: cargo test --release probe_night_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_night_cost() {
        let t = theme("night-yellow");
        let mut fam = Night::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = 0.6 * (1.0 - i as f32 / 96.0);
        }
        d.peaks = d.levels;
        d.rms_l = 0.4;
        for _ in 0..60 {
            fam.draw(&mut c, &t, &d);
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        let steady = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("night steady: {steady:.3} ms/frame at 380x60");

        // The malfunction path recomposites the whole interior per pixel; measure its envelope.
        fam.flourish.force_next();
        let m = 36;
        let t1 = std::time::Instant::now();
        for k in n..(n + m) {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        let flourish = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
        println!("night flourish: {flourish:.3} ms/frame at 380x60");
    }
}
