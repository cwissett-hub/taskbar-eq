//! The Bones / TeamSESH family: a VHS tape whose picture tears with the music, one word set in
//! pixel blackletter across it.
//!
//! The whole aesthetic is a worn cassette dub of a Bones / TeamSESH tape - horizontal scanlines with
//! a slow vertical roll, tracking tears that open with the bass, a camcorder `PLAY`/`REC` stamp, and
//! ONE word (`SESH`, `BONES`, `TEAMSESH`, ...) centred in the gothic lettering of the tapes. No
//! trademark marks: no skull, no logo geometry - the identity is carried entirely by the blackletter
//! word (see `render::gothic`) and the tape texture.
//!
//! # The two layers
//!
//! - **Tape.** The interior carries faint scanlines (every other row at `t.ghost`) rolling slowly
//!   upward. The 64 bands fold into 12 TRACKING BANDS stacked bass-at-the-bottom; a band's level
//!   tears its rows sideways, opening a widening rip about the centre with the bright head-switching
//!   noise at the torn edges. A `hot` peak-hold mark hangs at each band's largest recent tear. So the
//!   tears read bottom-up as a meter: the louder a band, the further its rip.
//! - **Word.** One word, centred, in `t.lit` with a `t.edge` outline (in `sesh-red` the outline is
//!   the one red, `hot`). It jitters a pixel on onsets, swaps every sixth strong (bass) onset, and
//!   sheds `t.lit` drips from its letters on bass hits.
//!
//! A camcorder stamp (`PLAY` + a play triangle + `HH:MM:SS`, and a blinking `REC` dot) sits over the
//! tape when the panel is tall enough.
//!
//! # The panel is opaque
//!
//! Like every family this one paints an opaque panel first and clips to the rounded rect last, so the
//! Windows weather widget is covered while music plays. Every layer - tape, word, stamp, the dropout
//! glitch - is drawn over that panel.
//!
//! # The flourish - a dropout
//!
//! On a rare exceptional hit the tape drops out: one blank frame, then a static field with the word
//! torn into shifted slices and the stamp reading `TRACKING`, decaying back to a clean picture.
//!
//! # Colourways
//!
//! `Theme` carries no per-family knob and this family adds none, so the tape-vs-word balance is a
//! per-colourway constant [`mix`] matched on the theme id. Colour is mixed through the shared
//! linear-light blend (`Rgba::lerp_linear` / the canvas alpha blend), never by averaging sRGB bytes.

use crate::dsp::bands::NUM_BANDS;
use crate::dsp::flourish::{Envelope, Trigger};
use crate::dsp::onset::Flux;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::font3x5;
use crate::render::gothic::{self, GothicSize};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// The five words the family cycles through. Every character has a gothic glyph at both sizes -
/// `every_word_char_has_a_gothic_glyph` is the guard.
pub const WORDS: [&str; 5] = ["SESH", "BONES", "TEAMSESH", "SESHOLLOWATERBOYZ", "TEAM SESH"];

/// The 64 bands fold into this many stacked tracking bands, bass at the bottom.
const TRACKING_BANDS: usize = 12;
/// Scanline vertical roll, pixels per second (wraps at the 2px scanline pitch).
const ROLL_PX_PER_S: f32 = 6.0;
/// Tear displacement as a fraction of width at full level (before the colourway `mix` scales it).
const TEAR_FRAC: f32 = 0.18;
/// Drips preallocated; never grows, so `draw` allocates nothing after the first frame.
const DRIPS: usize = 8;
/// Drip fall speed and gravity.
const DRIP_PX_PER_S: f32 = 18.0;
const DRIP_ACCEL: f32 = 40.0;
/// The word swaps on every sixth strong (bass) onset.
const WORD_SWAP_EVERY: u32 = 6;
/// How long the dropout envelope runs, in milliseconds.
const DROPOUT_MS: f32 = 550.0;
/// The stamp only draws when the panel is at least this tall.
const STAMP_MIN_H: i32 = 48;
/// A strong onset needs the low bands over this - the bass-onset threshold that drives word swaps,
/// drips and the REC blink.
const BASS_ONSET: f32 = 0.55;
/// The onset net that swaps the word and blinks REC - the same permissive flux net `vsghost` uses.
const ONSET_RATIO: f32 = 2.8;
const ONSET_REFRACTORY_MS: f32 = 200.0;

/// The chroma-bleed plates for `sesh-vhs`: the word drawn once in each, offset a pixel, at half
/// alpha, before the main pass.
const BLEED_RED: &str = "#ff2a2a";
const BLEED_CYAN: &str = "#2ad2ff";

/// The tape-vs-word balance, per colourway. 0 = all tape, 1 = all word. Scales the tear shift, the
/// bright-streak alpha and the word-size threshold - see the module note.
fn mix(t: &Theme) -> f32 {
    match t.id.as_str() {
        "sesh-tape" => 0.25,
        "sesh-word" => 0.85,
        "sesh-vhs" => 0.5,
        "sesh-red" => 0.6,
        "sesh-bleached" => 0.5,
        _ => 0.5,
    }
}

/// A single word drip: a 1px column of `lit` falling from a letter's foot under gravity.
#[derive(Clone, Copy, Default)]
struct Drip {
    x: i32,
    y: f32,
    vy: f32,
    alive: bool,
}

pub struct Sesh {
    /// Fires the dropout on a rare, exceptional hit. `pub(crate)` so the family tests can force it.
    pub(crate) flourish: Trigger,
    /// The dropout's one-shot decay envelope.
    dropout: Envelope,
    /// The onset that swaps the word and blinks REC.
    onset: Flux,
    onset_count: u32,
    strong_count: u32,
    /// Scanline roll, wrapped at the 2px pitch.
    roll_px: f32,
    /// Real elapsed seconds, for the camcorder clock.
    elapsed_s: f32,
    /// Milliseconds left on the REC blink.
    rec_blink_ms: f32,
    /// Which word is showing.
    word_idx: usize,
    /// A word forced by a test - the one allocation, made once in the hook, never in `draw`.
    word_override: Option<String>,
    /// Test hook: draw the word layer only, for the centring test.
    tape_suppressed: bool,
    /// Smoothed tear level per tracking band.
    tear: [f32; TRACKING_BANDS],
    /// Peak-hold tear per band, decaying at `peak_fall`.
    peak: [f32; TRACKING_BANDS],
    /// The word drips, preallocated.
    drips: [Drip; DRIPS],
    /// The dropout's static/torn-word buffer, sized once per panel size and reused.
    scratch: Option<Canvas>,
    /// splitmix64 state, for the jitter, drips and static.
    rng: u64,
}

impl Default for Sesh {
    fn default() -> Self {
        Sesh {
            flourish: Default::default(),
            dropout: Default::default(),
            onset: Default::default(),
            onset_count: 0,
            strong_count: 0,
            roll_px: 0.0,
            elapsed_s: 0.0,
            rec_blink_ms: 0.0,
            word_idx: 0,
            word_override: None,
            tape_suppressed: false,
            tear: [0.0; TRACKING_BANDS],
            peak: [0.0; TRACKING_BANDS],
            drips: [Drip::default(); DRIPS],
            scratch: None,
            rng: 0x2545_f491_4f6c_dd1d,
        }
    }
}

impl Sesh {
    /// Forces a word, for the centring test. The one allocation is here, once - never in `draw`.
    #[cfg(test)]
    pub fn set_word_for_test(&mut self, s: &str) {
        self.word_override = Some(s.to_string());
    }

    /// Draws the word layer only (no tape, no stamp), for the centring test.
    #[cfg(test)]
    pub fn suppress_tape_for_test(&mut self) {
        self.tape_suppressed = true;
    }

    /// One draw of splitmix64.
    fn next_rng(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn current_word(&self) -> &str {
        match &self.word_override {
            Some(o) => o.as_str(),
            None => WORDS[self.word_idx],
        }
    }
}

/// A finite level from `d.levels`, clamped 0..1.
fn lvl(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

impl Family for Sesh {
    fn id(&self) -> &'static str {
        "sesh"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // ---- the opaque panel ----
        let panel = Rgba::from_hex(&t.panel, 1.0);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);

        // The interior the picture lives in. `ix1`/`iy1` are exclusive.
        let (ix0, iy0) = (3i32, 4i32);
        let (ix1, iy1) = (w - 3, h - 4);
        let iw = ix1 - ix0;
        let ih = iy1 - iy0;
        if iw < 1 || ih < 1 {
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return; // too small to smudge
        }

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        self.elapsed_s += dt / 1000.0;
        if !self.elapsed_s.is_finite() {
            self.elapsed_s = 0.0;
        }
        self.roll_px = (self.roll_px + ROLL_PX_PER_S * dt / 1000.0).rem_euclid(2.0);
        if !self.roll_px.is_finite() {
            self.roll_px = 0.0;
        }

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let mixv = mix(t);

        // ---- onsets: swap the word, blink REC ----
        let onset = self.onset.update(&d.levels, dt, ONSET_RATIO, ONSET_REFRACTORY_MS);
        let bass = {
            let mut s = 0.0f32;
            for &v in &d.levels[0..8] {
                s += lvl(v);
            }
            s / 8.0
        };
        let strong = onset && bass > BASS_ONSET;
        if onset {
            self.onset_count = self.onset_count.wrapping_add(1);
        }
        if strong {
            self.strong_count = self.strong_count.wrapping_add(1);
            if self.strong_count.is_multiple_of(WORD_SWAP_EVERY) && self.word_override.is_none() {
                self.word_idx = (self.word_idx + 1) % WORDS.len();
            }
            self.rec_blink_ms = 120.0;
        }
        self.rec_blink_ms = (self.rec_blink_ms - dt).max(0.0);

        // ---- fold 64 bands into 12 tracking bands ----
        let attack = t.ballistics.attack.clamp(0.0, 1.0);
        let decay = t.ballistics.decay.clamp(0.0, 1.0);
        let peak_fall = t.ballistics.peak_fall.max(0.0);
        for b in 0..TRACKING_BANDS {
            let lo = b * NUM_BANDS / TRACKING_BANDS;
            let hi = (b + 1) * NUM_BANDS / TRACKING_BANDS;
            let mut sum = 0.0f32;
            for &v in &d.levels[lo..hi] {
                sum += lvl(v);
            }
            let target = sum / (hi - lo).max(1) as f32;
            let cur = self.tear[b];
            let kf = if target > cur { attack } else { decay };
            let mut f = cur + (target - cur) * kf;
            if !f.is_finite() {
                f = 0.0;
            }
            self.tear[b] = f;
            let mut pk = (self.peak[b] * (1.0 - peak_fall * dt).max(0.0)).max(f);
            if !pk.is_finite() {
                pk = f;
            }
            self.peak[b] = pk;
        }

        // ---- flourish: a dropout ----
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let env = self.dropout.update(fired, dt, DROPOUT_MS);
        if fired {
            // Frame 1 of the dropout: a true blank.
            c.fill_rect(ix0, iy0, iw, ih, panel);
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return;
        }

        let bbox = (ix0, iy0, ix1, iy1);
        if env > 0.15 {
            // ---- the dropout glitch: static, torn word, TRACKING ----
            self.draw_dropout(c, t, bbox, env, panel, lit);
            if h >= STAMP_MIN_H {
                self.draw_stamp(c, bbox, hot, true);
            }
        } else {
            // ---- the tape ----
            if !self.tape_suppressed {
                self.draw_tape(c, t, bbox, mixv, lit, hot);
                if h >= STAMP_MIN_H {
                    self.draw_stamp(c, bbox, hot, false);
                }
            }
            // ---- the word (and its drips) ----
            self.draw_word(c, t, bbox, mixv, onset, strong, dt, lit);
        }

        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

impl Sesh {
    /// The tape layer: rolling scanlines torn open per tracking band, bright head-switching noise at
    /// the torn edges, a `hot` peak mark hanging at each band's largest recent tear.
    fn draw_tape(&self, c: &mut Canvas, t: &Theme, bbox: (i32, i32, i32, i32), mixv: f32, lit: Rgba, hot: Rgba) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let (w, iw, ih) = (c.width(), ix1 - ix0, iy1 - iy0);
        let centre = (ix0 + ix1) / 2;
        let roll = self.roll_px as i32;
        let ghost = t.ghost.clamp(0.0, 1.0);
        let scan = Rgba::from_hex(&t.lit, ghost);
        // The dark dropout streak along each band's foot; on the bleached tape it is a dark streak on
        // a light panel, elsewhere a barely-there darkening of the near-black panel.
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let dropout = if t.id == "sesh-bleached" {
            Rgba::lerp_linear(panel, lit, 0.35)
        } else {
            Rgba::lerp_linear(panel, Rgba::new(0, 0, 0, 255), 0.5)
        };

        // Per-band tear and streak, computed once.
        let mut band_s = [0i32; TRACKING_BANDS];
        let mut band_sign = [1i32; TRACKING_BANDS];
        let mut band_streak = [0.0f32; TRACKING_BANDS];
        let mut band_peak = [0i32; TRACKING_BANDS];
        for b in 0..TRACKING_BANDS {
            let level = self.tear[b].clamp(0.0, 1.0);
            band_s[b] = (level * TEAR_FRAC * w as f32 * (1.3 - mixv)).round().max(0.0) as i32;
            band_sign[b] = if b % 2 == 0 { 1 } else { -1 };
            band_streak[b] = (level * (1.2 - mixv)).clamp(0.0, 1.0);
            band_peak[b] = (self.peak[b].clamp(0.0, 1.0) * TEAR_FRAC * w as f32 * (1.3 - mixv))
                .round()
                .max(0.0) as i32;
        }

        // Scanlines, torn open about a per-band seam.
        for y in iy0..iy1 {
            if (y + roll) % 2 != 0 {
                continue;
            }
            let b = (((iy1 - 1 - y) * TRACKING_BANDS as i32) / ih).clamp(0, TRACKING_BANDS as i32 - 1) as usize;
            let s = band_s[b];
            let gc = centre + band_sign[b] * (s / 2);
            let left_end = (gc - s).clamp(ix0, ix1);
            let right_start = (gc + s).clamp(ix0, ix1);
            if left_end > ix0 {
                c.fill_rect(ix0, y, left_end - ix0, 1, scan);
            }
            if right_start < ix1 {
                c.fill_rect(right_start, y, ix1 - right_start, 1, scan);
            }
        }

        // Per-band ornament: the dropout foot, the bright torn-edge noise, the peak mark.
        for b in 0..TRACKING_BANDS {
            let d_lo = b as i32 * ih / TRACKING_BANDS as i32;
            let d_hi = (b as i32 + 1) * ih / TRACKING_BANDS as i32;
            let y_bot = iy1 - 1 - d_lo;
            let y_top = iy1 - 1 - (d_hi - 1).max(d_lo);
            let s = band_s[b];
            let gc = centre + band_sign[b] * (s / 2);
            // Dark dropout streak along the band foot.
            c.fill_rect(ix0, y_bot, iw, 1, dropout);
            // Bright head-switching noise at the two torn edges, on the band's top row.
            if band_streak[b] > 0.01 {
                let noise = Rgba::from_hex(&t.lit, band_streak[b]);
                let lx = (gc - s).clamp(ix0, ix1 - 1);
                let rx = (gc + s).clamp(ix0, ix1 - 1);
                c.fill_rect((lx - 2).max(ix0), y_top, 3, 1, noise);
                c.fill_rect(rx, y_top, (ix1 - rx).min(3), 1, noise);
            }
            // Peak mark: a 1px hot line hanging at the largest recent tear.
            if band_peak[b] > s {
                let py = (y_top + y_bot) / 2;
                let px = (centre + band_sign[b] * band_peak[b]).clamp(ix0, ix1 - 1);
                c.fill_rect(px, py, 1, 1, hot);
            }
        }
    }

    /// The camcorder stamp: `PLAY` + a play triangle + the elapsed clock (or `TRACKING` during the
    /// dropout), with a `REC` dot top-right that blinks on strong onsets. All in `hot`.
    fn draw_stamp(&self, c: &mut Canvas, bbox: (i32, i32, i32, i32), hot: Rgba, tracking: bool) {
        let (ix0, iy0, ix1, _iy1) = bbox;
        let y = iy0 + 1;
        if tracking {
            font3x5::draw(c, ix0 + 2, y, "TRACKING", hot);
        } else {
            let mut cx = ix0 + 2;
            cx += font3x5::draw(c, cx, y, "PLAY", hot) + 2;
            // A right-pointing play triangle, three rows.
            c.fill_rect(cx, y, 1, 1, hot);
            c.fill_rect(cx, y + 1, 2, 1, hot);
            c.fill_rect(cx, y + 2, 3, 1, hot);
            c.fill_rect(cx, y + 3, 2, 1, hot);
            c.fill_rect(cx, y + 4, 1, 1, hot);
            cx += 5;
            // HH:MM:SS into a fixed buffer, no format!.
            let secs = if self.elapsed_s.is_finite() { self.elapsed_s.max(0.0) as u32 } else { 0 };
            let mut buf = [b'0'; 8];
            let (hh, mm, ss) = ((secs / 3600) % 100, (secs % 3600) / 60, secs % 60);
            buf[0] = b'0' + (hh / 10) as u8;
            buf[1] = b'0' + (hh % 10) as u8;
            buf[2] = b':';
            buf[3] = b'0' + (mm / 10) as u8;
            buf[4] = b'0' + (mm % 10) as u8;
            buf[5] = b':';
            buf[6] = b'0' + (ss / 10) as u8;
            buf[7] = b'0' + (ss % 10) as u8;
            if let Ok(s) = std::str::from_utf8(&buf) {
                font3x5::draw(c, cx, y, s, hot);
            }
        }
        // REC top-right, with a blinking dot.
        let dot_x = ix1 - 2 - 3;
        let rec_x = dot_x - 2 - font3x5::width("REC");
        font3x5::draw(c, rec_x, y, "REC", hot);
        if self.rec_blink_ms > 0.0 {
            c.fill_rect(dot_x, y + 1, 3, 3, hot);
        }
    }

    /// The word layer: one word centred in pixel blackletter, jittering on onsets, with `lit` drips
    /// shed from its feet on bass hits.
    #[allow(clippy::too_many_arguments)]
    fn draw_word(
        &mut self,
        c: &mut Canvas,
        t: &Theme,
        bbox: (i32, i32, i32, i32),
        mixv: f32,
        onset: bool,
        strong: bool,
        dt: f32,
        lit: Rgba,
    ) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let (iw, ih, h) = (ix1 - ix0, iy1 - iy0, c.height());

        let large = h >= 58 - (12.0 * mixv).round() as i32;
        let size = if large { GothicSize::Large } else { GothicSize::Small };
        let gh = if large { 13 } else { 9 };
        // Draw the randoms up front, so `next_rng` (which needs `&mut self`) does not overlap the
        // immutable borrow of `self` that the word string holds.
        let (r_jit, r_d0, r_d1) = (self.next_rng(), self.next_rng(), self.next_rng());
        let word = gothic::truncate_to_width(self.current_word(), size, iw - 8);
        if !word.is_empty() {
            let tw = gothic::text_width(word, size);
            let mut x = ix0 + (iw - tw) / 2;
            let y = iy0 + (ih - gh) / 2;
            if onset {
                x += if r_jit & 1 == 0 { 1 } else { -1 };
            }
            // The one red: sesh-red outlines the word in `hot`; every other colourway in `edge`.
            let outline = if t.id == "sesh-red" {
                Rgba::from_hex(&t.hot, t.edge_alpha)
            } else {
                Rgba::from_hex(&t.edge, t.edge_alpha)
            };
            // sesh-vhs chroma bleed: the word twice more, offset a pixel, half-alpha, BEFORE the main.
            if t.id == "sesh-vhs" {
                gothic::draw(c, x - 1, y, word, size, Rgba::from_hex(BLEED_RED, 0.5), None);
                gothic::draw(c, x + 1, y, word, size, Rgba::from_hex(BLEED_CYAN, 0.5), None);
            }
            gothic::draw(c, x, y, word, size, lit, Some(outline));

            // Drips from the word's feet on a bass hit.
            let (wx0, wx1, wy) = (x, x + tw, y + gh);
            if strong {
                let span = (wx1 - wx0).max(1) as u64;
                let spots = [wx0 + (r_d0 % span) as i32, wx0 + (r_d1 % span) as i32];
                let mut k = 0usize;
                for dr in self.drips.iter_mut() {
                    if k >= 2 {
                        break;
                    }
                    if !dr.alive {
                        *dr = Drip { x: spots[k], y: wy as f32, vy: DRIP_PX_PER_S, alive: true };
                        k += 1;
                    }
                }
            }
        }

        // Advance and draw the live drips.
        for dr in self.drips.iter_mut() {
            if !dr.alive {
                continue;
            }
            dr.vy += DRIP_ACCEL * dt / 1000.0;
            dr.y += dr.vy * dt / 1000.0;
            if !dr.y.is_finite() || dr.y as i32 >= iy1 {
                dr.alive = false;
                continue;
            }
            c.fill_rect(dr.x, dr.y as i32, 1, 2, lit);
        }
    }

    /// The dropout glitch, drawn over an opaque base in a reused scratch canvas and copied back: a
    /// per-pixel static field, the word torn into three shifted slices, then pasted over the panel.
    fn draw_dropout(&mut self, c: &mut Canvas, t: &Theme, bbox: (i32, i32, i32, i32), env: f32, panel: Rgba, lit: Rgba) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let (w, h, iw, ih) = (c.width(), c.height(), ix1 - ix0, iy1 - iy0);

        let mut scr = self
            .scratch
            .take()
            .filter(|s| s.width() == w && s.height() == h)
            .unwrap_or_else(|| Canvas::new(w, h));
        scr.clear();
        // An opaque base, so every pixel copied back keeps the panel opaque.
        scr.fill_rect(ix0, iy0, iw, ih, panel);

        // Per-pixel static: `lit`/`edge` roughly half and half, at alpha env*0.8.
        let a = (env * 0.8).clamp(0.0, 1.0);
        let noise_a = Rgba::from_hex(&t.lit, a);
        let noise_b = Rgba::from_hex(&t.edge, a);
        for y in iy0..iy1 {
            for x in ix0..ix1 {
                let col = if self.next_rng() & 1 == 0 { noise_a } else { noise_b };
                scr.fill_rect(x, y, 1, 1, col);
            }
        }

        // The word, torn: draw it centred into the scratch, then copy three horizontal slices back
        // shifted (wrapping) so it reads as a mistracked frame.
        let large = h >= 58 - (12.0 * mix(t)).round() as i32;
        let size = if large { GothicSize::Large } else { GothicSize::Small };
        let gh = if large { 13 } else { 9 };
        let word = gothic::truncate_to_width(self.current_word(), size, iw - 8);
        if !word.is_empty() {
            let tw = gothic::text_width(word, size);
            let x = ix0 + (iw - tw) / 2;
            let y = iy0 + (ih - gh) / 2;
            gothic::draw(&mut scr, x, y, word, size, lit, None);
        }

        let shift = (0.3 * iw as f32 * env).round() as i32;
        for k in 0..3 {
            let sy = iy0 + k * ih / 3;
            let rows = if k == 2 { iy1 - sy } else { ih / 3 };
            let sh = if k == 1 { -shift } else { shift };
            let s = sh.rem_euclid(iw.max(1));
            // dest [ix0+s, ix1) <- src [ix0, ix1-s), then the wrapped remainder.
            c.copy_region(&scr, (ix0, sy), (ix0 + s, sy), iw - s, rows);
            if s > 0 {
                c.copy_region(&scr, (ix1 - s, sy), (ix0, sy), s, rows);
            }
        }
        self.scratch = Some(scr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    fn frames(fam: &mut Sesh, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
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
        assert!(crate::render::KNOWN_FAMILIES.contains(&"sesh"));
        assert_eq!(crate::render::family_for("sesh").id(), "sesh");
        assert_ne!(crate::themes::family_label("sesh"), "sesh");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "sesh").count(), 5);
    }

    /// The lit streak of each tracking band sits at the band's horizontal shift, so a louder band's
    /// streak is further from where the quiet band's is.
    #[test]
    fn louder_bands_tear_further() {
        let t = theme("sesh-tape");
        let run = |level: f32| {
            let mut fam = Sesh::default();
            fam.set_word_for_test("");
            let mut c = Canvas::new(380, 48);
            let mut d = FrameData::default();
            for v in d.levels[0..16].iter_mut() {
                *v = level;
            }
            d.peaks = d.levels;
            d.dt_ms = 16.7;
            for k in 0..30 {
                d.time_s = k as f32 * 0.0167;
                fam.draw(&mut c, &t, &d);
            }
            let panel = Rgba::from_hex(&t.panel, 1.0);
            let (mut sum, mut n) = (0.0f32, 0usize);
            for y in 32..46 {
                for x in 0..380 {
                    if drew_over_panel(c.get(x, y), panel) {
                        sum += (x as f32 - 190.0).abs();
                        n += 1;
                    }
                }
            }
            if n == 0 {
                0.0
            } else {
                sum / n as f32
            }
        };
        let quiet = run(0.2);
        let loud = run(0.9);
        assert!(loud > quiet + 0.08 * 380.0 * 0.3, "quiet {quiet} loud {loud}");
    }

    #[test]
    fn the_word_is_centred_and_whole() {
        let t = theme("sesh-word");
        let mut fam = Sesh::default();
        fam.suppress_tape_for_test();
        fam.set_word_for_test("SESHOLLOWATERBOYZ");
        let c = frames(&mut fam, &t, 190, 48, 0.3, 5);
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let (mut x0, mut x1) = (i32::MAX, i32::MIN);
        for y in 0..48 {
            for x in 0..190 {
                if drew_over_panel(c.get(x, y), panel) {
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                }
            }
        }
        assert!(x0 < x1, "nothing drawn");
        let width = x1 - x0 + 1;
        assert!(width <= 190 - 4 - 8, "word too wide: {width}");
        let centre = (x0 + x1) as f32 / 2.0;
        assert!((centre - 95.0).abs() <= 2.5, "centre {centre}");
        let ok = (1..=17).any(|n| {
            let prefix = &"SESHOLLOWATERBOYZ"[..n];
            let gw = gothic::text_width(prefix, GothicSize::Small);
            (gw + 2 - width).abs() <= 1
        });
        // The word went large at h=48 on sesh-word, so also try Large.
        let ok = ok
            || (1..=17).any(|n| {
                let prefix = &"SESHOLLOWATERBOYZ"[..n];
                let gw = gothic::text_width(prefix, GothicSize::Large);
                (gw + 2 - width).abs() <= 1
            });
        assert!(ok, "drawn width {width} matches no whole-glyph prefix");
    }

    #[test]
    fn every_word_char_has_a_gothic_glyph() {
        for w in WORDS {
            for ch in w.chars() {
                for size in [GothicSize::Small, GothicSize::Large] {
                    assert!(gothic::glyph(ch, size).is_some(), "{w:?} {ch:?}");
                }
            }
        }
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("sesh-vhs");
        let c = frames(&mut Sesh::default(), &t, 380, 48, 0.0, 10);
        assert!(lit(&c, &t) as f32 >= 0.02 * (380 * 48) as f32, "scanlines + word + stamp: {}", lit(&c, &t));
    }

    #[test]
    fn dropout_first_frame_is_blank() {
        for id in ["sesh-tape", "sesh-bleached"] {
            let t = theme(id);
            let mut fam = Sesh::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 1);
            let panel = Rgba::from_hex(&t.panel, 1.0);
            let mut painted = 0;
            for y in 4..44 {
                for x in 3..187 {
                    if drew_over_panel(c.get(x, y), panel) {
                        painted += 1;
                    }
                }
            }
            assert_eq!(painted, 0, "{id}: {painted} painted pixels on the dropout frame");
        }
    }

    #[test]
    fn fits_the_narrow_panel_and_flourish_does_not_panic() {
        for id in ["sesh-tape", "sesh-word", "sesh-vhs", "sesh-red", "sesh-bleached"] {
            let t = theme(id);
            let mut fam = Sesh::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 40);
            assert!(lit(&c, &t) > 0, "{id}");
            let _ = frames(&mut Sesh::default(), &t, 128, 44, 0.6, 5);
        }
    }

    /// Dumps for the eye test - composited over `#202020` like every other family's dump.
    ///
    /// Run: cargo test --release dump_sesh -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_sesh() {
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
        let frame = |level: f32, t_s: f32, beat: bool| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                let shape = (1.0 - f).powf(1.2) * 0.7 + 0.12;
                let wob = 1.0 + 0.3 * (t_s * 2.4 + f * 6.0).sin();
                let kick = if beat { 1.0 + 0.9 * (1.0 - f) } else { 1.0 };
                *v = ((shape * wob * kick) * level).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d.rms_l = level * 0.6;
            d.rms_r = level * 0.6;
            d
        };
        let sizes: [(&str, i32, i32); 3] = [("380x60", 380, 60), ("190x48", 190, 48), ("128x44", 128, 44)];
        for id in ["sesh-tape", "sesh-word", "sesh-vhs", "sesh-red", "sesh-bleached"] {
            let t = theme(id);
            let short = &id["sesh-".len()..];
            for (tag, level) in [("calm", 0.28f32), ("loud", 0.85)] {
                let mut fam = Sesh::default();
                let mut c = Canvas::new(380, 60);
                for k in 0..120 {
                    c.clear();
                    fam.draw(&mut c, &t, &frame(level, k as f32 * 0.0167, k % 24 == 0));
                }
                write(format!("sesh-{short}-{tag}"), &c);
            }
            // Flourish: settle, fire, capture the blank, then a mid-decay frame.
            let mut fam = Sesh::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..120 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167, k % 24 == 0));
            }
            fam.flourish.force_next();
            c.clear();
            fam.draw(&mut c, &t, &frame(0.45, 120.0 * 0.0167, false));
            write(format!("sesh-{short}-flourish"), &c);
            for k in 121..126 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.3, k as f32 * 0.0167, false));
            }
            write(format!("sesh-{short}-flourish-decay"), &c);
        }
        // The two narrow shapes, on the two colourways whose identity is the word / the tape.
        for (id, (tag, w, h)) in [("sesh-tape", sizes[1]), ("sesh-word", sizes[2])] {
            let t = theme(id);
            let mut fam = Sesh::default();
            let mut c = Canvas::new(w, h);
            for k in 0..120 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.8, k as f32 * 0.0167, k % 24 == 0));
            }
            write(format!("sesh-{}-{tag}", &id["sesh-".len()..]), &c);
        }
        println!("wrote sesh dumps to {}", dir.display());
    }

    /// The per-frame cost, steady and during the dropout, well under the 2ms budget at 380x60.
    ///
    /// Run: cargo test --release probe_sesh_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_sesh_cost() {
        let t = theme("sesh-vhs");
        let mut fam = Sesh::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = 0.6 * (1.0 - i as f32 / 96.0);
        }
        d.peaks = d.levels;
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
        println!("sesh steady: {steady:.3} ms/frame at 380x60");

        // The dropout path writes the whole interior per pixel; measure its envelope's worth of frames.
        fam.flourish.force_next();
        let m = 34;
        let t1 = std::time::Instant::now();
        for k in n..(n + m) {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        let flourish = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
        println!("sesh flourish: {flourish:.3} ms/frame at 380x60");
    }
}
