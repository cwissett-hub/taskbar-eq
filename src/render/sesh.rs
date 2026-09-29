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
//! `Theme` carries no per-family knob, so each colourway's character is a per-id [`Style`] matched on
//! the theme id (default = the shared tearing tape). The five are deliberately five DIFFERENT things,
//! not one look tinted five ways: `sesh-tape` is a heavy 8-band tear with a rolling head-switch band
//! and the word demoted to a bottom-right caption; `sesh-word` turns the tears off and makes a big
//! centred word that pulses on the bass THE meter; `sesh-vhs` gets a cold-cast panel, doubled chroma
//! bleed and a red/cyan colour-shift wobble; `sesh-red` reddens loud bands' tears; `sesh-bleached`
//! bakes in paper grain. Colour is mixed through the shared linear-light blend (`Rgba::lerp_linear`
//! / the canvas alpha blend), never by averaging sRGB bytes.

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

/// The MAXIMUM tracking bands the smoothing arrays are sized to; a colourway draws `Style::bands` of
/// them (bass at the bottom). Sized to the largest so `sesh-tape`'s fatter 8-band picture and the
/// others' 12 share one allocation-free `[f32; TRACKING_BANDS]`.
const TRACKING_BANDS: usize = 12;
/// `sesh-tape`'s band count: fewer, fatter tracking bands so the tears themselves ARE the picture.
const TAPE_BANDS: usize = 8;
/// Scanline vertical roll, pixels per second (wraps at the 2px scanline pitch).
const ROLL_PX_PER_S: f32 = 6.0;
/// Tear displacement as a fraction of width at full level (before the colourway scale).
const TEAR_FRAC: f32 = 0.18;
/// Drips preallocated to the largest cap (`sesh-word`); never grows, so `draw` allocates nothing.
const DRIPS: usize = 16;
/// Live-drip cap for every colourway except `sesh-word`, whose word IS the meter so it drips twice
/// as hard.
const DRIP_CAP_DEFAULT: usize = 8;
const DRIP_CAP_WORD: usize = 16;
/// Drip fall speed and gravity.
const DRIP_PX_PER_S: f32 = 18.0;
const DRIP_ACCEL: f32 = 40.0;
/// The word swaps on every sixth strong (bass) onset.
const WORD_SWAP_EVERY: u32 = 6;
/// How long the dropout SLAM runs, in milliseconds. Short and violent - a bass-hit slam, not a slow
/// wipe.
const DROPOUT_MS: f32 = 250.0;
/// The flourish only fires on a BASS hit: the trigger must fire AND the low bands' mean clear this.
/// The user asked for the dropout to be felt as a bass hit, not "the logo coming in every time".
const FLOURISH_BASS_MIN: f32 = 0.6;
/// The dropout slam's tear rows: this many horizontal slices across the word snap sideways by
/// `SLAM_TEAR_MIN..SLAM_TEAR_MAX` px each frame and snap straight back (no drift, no wrap).
const SLAM_TEAR_ROWS: usize = 3;
const SLAM_TEAR_MIN: i32 = 2;
const SLAM_TEAR_MAX: i32 = 6;
/// Peak static density of the slam, in percent of interior pixels. Below 100 so the word (drawn over
/// it, on a knocked-back band) stays legible instead of dissolving into same-colour noise.
const SLAM_STATIC_MAX_PCT: u64 = 62;
/// Static alpha across the word's own rows during the slam - low, so the word reads on every frame
/// while the field stays dense elsewhere.
const SLAM_WORD_ROW_ALPHA: f32 = 0.45;
/// The stamp only draws when the panel is at least this tall.
const STAMP_MIN_H: i32 = 48;
/// A strong onset needs the low bands over this - also the bass level at which `sesh-word` pulses.
const BASS_ONSET: f32 = 0.55;
/// The onset net that swaps the word and blinks REC - the same permissive flux net `vsghost` uses.
const ONSET_RATIO: f32 = 2.8;
const ONSET_REFRACTORY_MS: f32 = 200.0;

/// `sesh-tape` head-switching noise: multiply the torn-edge streak alpha by this so the noise, not
/// the word, carries the frame.
const TAPE_STREAK_ALPHA_MUL: f32 = 1.5;
/// `sesh-tape`: two rows of that noise instead of one, for a heavier tear.
const TAPE_STREAK_ROWS: i32 = 2;
/// `sesh-tape`'s head-switching band: a bright bar this many px tall, rolling slowly up the interior
/// at `HEAD_SWITCH_PX_PER_S` - the bottom-of-frame tear a mistracked VHS shows.
const HEAD_SWITCH_H: i32 = 3;
const HEAD_SWITCH_PX_PER_S: f32 = 5.0;
/// `sesh-tape`'s corner caption: the small word inset this many px from the bottom-right, like a tape
/// label rather than a centred title.
const CAPTION_MARGIN: i32 = 3;
/// `sesh-word` pulse: the word drops this many px on a bass onset and its outline thickens to 2px.
const WORD_PULSE_DROP: i32 = 2;
/// `sesh-vhs` chroma bleed: the word and stamp drawn in each plate, offset this many px, at this
/// alpha - doubled from the old ±1px/0.5 so the bleed reads as the colourway's identity.
const VHS_BLEED_PX: i32 = 2;
const VHS_BLEED_ALPHA: f32 = 0.8;
/// `sesh-vhs` colour-shift wobble: the streak rows alternate red/cyan tint with a phase drifting at
/// this rate, in Hz.
const WOBBLE_HZ: f32 = 0.3;
/// `sesh-red`: a band's torn-edge streak turns `hot` (red) once its level clears this, so the red is
/// carried by the picture and not only the word outline.
const RED_HOT_TEAR_LEVEL: f32 = 0.6;
/// `sesh-bleached` paper grain: this fraction of interior pixels get a deterministic dot one shade
/// darker than the panel, computed ONCE (no per-frame RNG) - the tell of a bleached print.
const GRAIN_FRAC: u64 = 6; // percent
const GRAIN_DARKEN: f32 = 0.12;

/// The chroma-bleed / wobble plate colours for `sesh-vhs`. The red plate is a pink-red to match the
/// muted-violet cast the VHS colourway took from the Blunts From The Graveyard tapes.
const BLEED_RED: &str = "#ff2a6a";
const BLEED_CYAN: &str = "#2ad2ff";

/// `sesh-graveyard` (violet night-vision fog): the sickly-green tears under a moon-white word, and the
/// dim-violet picture. From the Blunts From The Graveyard compilations.
const GRAVEYARD_SCAN: &str = "#5a3a7a";
const GRAVEYARD_TEAR: &str = "#9cff6a";
/// `sesh-nightvision`: a heavier grain than the bleached tape's paper, for the phosphor-green sensor
/// noise look.
const NIGHTVISION_GRAIN_FRAC: u64 = 11;

/// Per-colourway character. Replaces the single `mix` knob that made every colourway the same black
/// panel with the same centred word: each field below turns one look into a different thing. The base
/// (`Style::base`) is the shared tearing tape; `style(t)` overrides per id.
#[derive(Clone, Copy)]
struct Style {
    /// Tracking bands actually drawn.
    bands: usize,
    /// Whether the picture tears at all (`sesh-word` turns it off - the word is the meter).
    tears: bool,
    /// Tear-shift and streak-alpha scales (were `1.3 - mix` and `1.2 - mix`).
    tear_scale: f32,
    streak_scale: f32,
    /// Extra streak-alpha multiplier and row count (`sesh-tape`'s heavier noise).
    streak_alpha_mul: f32,
    streak_rows: i32,
    /// The rolling head-switching band (`sesh-tape`).
    head_switch: bool,
    /// The word is a small bottom-right caption, never centred (`sesh-tape`).
    corner_caption: bool,
    /// Panel height at or above which the centred word goes Large.
    word_large_at: i32,
    /// The word pulses (drops + thickens) on a bass onset (`sesh-word`).
    word_pulse: bool,
    /// The word outline alpha scales with rms (`sesh-word`).
    outline_rms: bool,
    /// Live-drip cap.
    drip_cap: usize,
    /// Chroma bleed on the word and stamp: offset px (0 = none) and alpha (`sesh-vhs`).
    bleed_px: i32,
    bleed_alpha: f32,
    bleed_stamp: bool,
    /// The red/cyan colour-shift wobble on the streaks (`sesh-vhs`).
    wobble: bool,
    /// Torn-edge streaks go `hot` on loud bands (`sesh-red`).
    hot_tears: bool,
    /// Deterministic paper grain baked into the background (`sesh-bleached`, the night-vision green).
    grain: bool,
    /// Grain density in percent of interior pixels (heavier on `sesh-nightvision`).
    grain_frac: u64,
    /// Override for the scanline/picture colour (default = `t.lit`). Lets a colourway paint the
    /// picture a different colour from the word (`sesh-graveyard`: violet scanlines, white word).
    scan_hex: Option<&'static str>,
    /// Override for the torn-edge streak (tear) colour (default = `t.lit`), e.g. graveyard's sickly
    /// green tears under a moon-white word.
    tear_hex: Option<&'static str>,
}

impl Style {
    /// The shared tearing tape - a centred word over per-band tears. Every colourway starts here.
    const fn base() -> Style {
        Style {
            bands: TRACKING_BANDS,
            tears: true,
            tear_scale: 0.8,
            streak_scale: 0.7,
            streak_alpha_mul: 1.0,
            streak_rows: 1,
            head_switch: false,
            corner_caption: false,
            word_large_at: 52,
            word_pulse: false,
            outline_rms: false,
            drip_cap: DRIP_CAP_DEFAULT,
            bleed_px: 0,
            bleed_alpha: 0.0,
            bleed_stamp: false,
            wobble: false,
            hot_tears: false,
            grain: false,
            grain_frac: GRAIN_FRAC,
            scan_hex: None,
            tear_hex: None,
        }
    }
}

/// The per-colourway style, matched on the theme id. Default (an unknown id) is the shared base.
fn style(t: &Theme) -> Style {
    let base = Style::base();
    match t.id.as_str() {
        // The tape: fewer fatter bands, heavier two-row noise, a rolling head-switch band, and the
        // word demoted to a bottom-right caption - the tears are the picture.
        "sesh-tape" => Style {
            bands: TAPE_BANDS,
            tear_scale: 1.05,
            streak_scale: 0.95,
            streak_alpha_mul: TAPE_STREAK_ALPHA_MUL,
            streak_rows: TAPE_STREAK_ROWS,
            head_switch: true,
            corner_caption: true,
            ..base
        },
        // The word: no tears, a big centred word that pulses on the bass and drips hard, over clean
        // rolling scanlines. The word is the meter.
        "sesh-word" => Style {
            tears: false,
            word_large_at: 44,
            word_pulse: true,
            outline_rms: true,
            drip_cap: DRIP_CAP_WORD,
            ..base
        },
        // The VHS: a cold-cast panel (in builtin), doubled chroma bleed on word AND stamp, and a slow
        // red/cyan colour-shift wobble on the streaks.
        "sesh-vhs" => Style {
            bleed_px: VHS_BLEED_PX,
            bleed_alpha: VHS_BLEED_ALPHA,
            bleed_stamp: true,
            wobble: true,
            ..base
        },
        // The one red, plus the loud bands' tears go hot so the red is in the picture too.
        "sesh-red" => Style { tear_scale: 0.7, streak_scale: 0.6, hot_tears: true, ..base },
        // The inverted tape, plus faint baked-in paper grain.
        "sesh-bleached" => Style { grain: true, ..base },
        // Violet graveyard fog: a dim-violet picture, a moon-white word (green outline), sickly-green
        // tears. The tears carry the night-vision colour without recolouring the word or scanlines.
        "sesh-graveyard" => Style {
            scan_hex: Some(GRAVEYARD_SCAN),
            tear_hex: Some(GRAVEYARD_TEAR),
            ..base
        },
        // Phosphor-green night vision: everything green (word, scanlines, tears all `lit`), a heavier
        // grain for sensor noise, and the red REC/stamp from `hot`.
        "sesh-nightvision" => Style { grain: true, grain_frac: NIGHTVISION_GRAIN_FRAC, ..base },
        _ => base,
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
    /// Test hook: pin the slam's jitter and tear offsets to zero, so the word sits at a deterministic
    /// position for a pixel-for-pixel comparison of the knock-back plate.
    slam_pinned: bool,
    /// Test hook: suppress the slam's static field, so a slam frame renders as the clean plate + word
    /// alone - the reference the legibility test compares a real (static-on) slam frame against.
    slam_no_static: bool,
    /// Smoothed tear level per tracking band.
    tear: [f32; TRACKING_BANDS],
    /// Peak-hold tear per band, decaying at `peak_fall`.
    peak: [f32; TRACKING_BANDS],
    /// The word drips, preallocated.
    drips: [Drip; DRIPS],
    /// The dropout's static/torn-word buffer, sized once per panel size and reused.
    scratch: Option<Canvas>,
    /// `sesh-bleached`'s paper-grain dot positions, computed ONCE for the current interior size and
    /// reused every frame - deterministic, so there is no per-frame RNG.
    grain: Vec<(i32, i32)>,
    grain_dim: (i32, i32),
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
            slam_pinned: false,
            slam_no_static: false,
            tear: [0.0; TRACKING_BANDS],
            peak: [0.0; TRACKING_BANDS],
            drips: [Drip::default(); DRIPS],
            scratch: None,
            grain: Vec::new(),
            grain_dim: (0, 0),
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

    /// Pins the slam's word jitter and tear offsets to zero, so the plate can be compared
    /// pixel-for-pixel against a static-suppressed reference at the same geometry.
    #[cfg(test)]
    pub fn pin_slam_jitter_for_test(&mut self) {
        self.slam_pinned = true;
    }

    /// Suppresses the slam's static field, so a slam frame renders as the clean plate + word alone.
    #[cfg(test)]
    pub fn suppress_slam_static_for_test(&mut self) {
        self.slam_no_static = true;
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

/// A deterministic 2D hash, for the baked paper grain. Pure - no state, so the pattern is identical
/// every run and needs no per-frame RNG.
fn hash2(x: i32, y: i32) -> u64 {
    let mut z = (x as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xD1B5_4A32_D192_ED03);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
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
        let st = style(t);

        // Paper grain, baked once and laid under everything. Painted BEFORE the flourish's blank
        // frame overwrites the interior, so the dropout stays truly blank.
        if st.grain {
            if self.grain_dim != (iw, ih) {
                self.grain.clear();
                for y in iy0..iy1 {
                    for x in ix0..ix1 {
                        if hash2(x, y) % 100 < st.grain_frac {
                            self.grain.push((x, y));
                        }
                    }
                }
                self.grain_dim = (iw, ih);
            }
            let grain_col = Rgba::lerp_linear(panel, Rgba::new(0, 0, 0, 255), GRAIN_DARKEN);
            for &(x, y) in &self.grain {
                c.fill_rect(x, y, 1, 1, grain_col);
            }
        }

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
        for b in 0..st.bands {
            let lo = b * NUM_BANDS / st.bands;
            let hi = (b + 1) * NUM_BANDS / st.bands;
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

        // ---- flourish: a bass-hit dropout ----
        // The trigger finds the exceptional hit; sesh only DROPS OUT when that hit is also a bass hit,
        // so the slam is felt on the low end rather than firing on any transient. A forced test fire
        // bypasses the gate (see `Trigger::was_forced`).
        let triggered = self.flourish.update(&d.levels, dt, t.flourish);
        let bass_gate = triggered && bass >= FLOURISH_BASS_MIN;
        #[cfg(test)]
        let fired = bass_gate || (triggered && self.flourish.was_forced());
        #[cfg(not(test))]
        let fired = bass_gate;
        let env = self.dropout.update(fired, dt, DROPOUT_MS);
        if fired {
            // Frame 1 of the dropout: a true blank.
            c.fill_rect(ix0, iy0, iw, ih, panel);
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return;
        }

        let rms = (((d.rms_l + d.rms_r) * 0.5).max(0.0)).min(1.0);
        let rms = if rms.is_finite() { rms } else { 0.0 };

        let bbox = (ix0, iy0, ix1, iy1);
        if env > 0.15 {
            // ---- the dropout glitch: static, torn word, TRACKING ----
            self.draw_dropout(c, t, &st, bbox, env);
            if h >= STAMP_MIN_H {
                self.draw_stamp_all(c, &st, bbox, hot, true);
            }
        } else {
            // ---- the tape ----
            if !self.tape_suppressed {
                self.draw_tape(c, t, &st, bbox, lit, hot);
                if h >= STAMP_MIN_H {
                    self.draw_stamp_all(c, &st, bbox, hot, false);
                }
            }
            // ---- the word (and its drips) ----
            self.draw_word(c, t, &st, bbox, onset, bass, rms, dt, lit);
        }

        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

impl Sesh {
    /// The tape layer: rolling scanlines torn open per tracking band, bright head-switching noise at
    /// the torn edges, a `hot` peak mark hanging at each band's largest recent tear. `sesh-word` turns
    /// the tears off (clean scanlines only); `sesh-tape` adds a rolling head-switching band; `sesh-vhs`
    /// wobbles the streak colours red/cyan; `sesh-red` reddens the streaks of loud bands.
    fn draw_tape(&self, c: &mut Canvas, t: &Theme, st: &Style, bbox: (i32, i32, i32, i32), lit: Rgba, hot: Rgba) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let (w, iw, ih) = (c.width(), ix1 - ix0, iy1 - iy0);
        let bands = st.bands;
        let centre = (ix0 + ix1) / 2;
        let roll = self.roll_px as i32;
        let ghost = t.ghost.clamp(0.0, 1.0);
        let scan_hex = st.scan_hex.unwrap_or(t.lit.as_str());
        let scan = Rgba::from_hex(scan_hex, ghost);
        let tear_hex = st.tear_hex.unwrap_or(t.lit.as_str());
        // The dark dropout streak along each band's foot; on the bleached tape it is a dark streak on
        // a light panel, elsewhere a barely-there darkening of the near-black panel.
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let dropout = if t.id == "sesh-bleached" {
            Rgba::lerp_linear(panel, lit, 0.35)
        } else {
            Rgba::lerp_linear(panel, Rgba::new(0, 0, 0, 255), 0.5)
        };

        // Per-band tear and streak, computed once. With tears off the seam collapses to the centre so
        // the scanlines run clean and unbroken.
        let mut band_s = [0i32; TRACKING_BANDS];
        let mut band_sign = [1i32; TRACKING_BANDS];
        let mut band_streak = [0.0f32; TRACKING_BANDS];
        let mut band_peak = [0i32; TRACKING_BANDS];
        for b in 0..bands {
            let level = self.tear[b].clamp(0.0, 1.0);
            band_s[b] = if st.tears {
                (level * TEAR_FRAC * w as f32 * st.tear_scale).round().max(0.0) as i32
            } else {
                0
            };
            band_sign[b] = if b % 2 == 0 { 1 } else { -1 };
            band_streak[b] = if st.tears { (level * st.streak_scale).clamp(0.0, 1.0) } else { 0.0 };
            band_peak[b] = if st.tears {
                (self.peak[b].clamp(0.0, 1.0) * TEAR_FRAC * w as f32 * st.tear_scale).round().max(0.0) as i32
            } else {
                0
            };
        }

        // Scanlines, torn open about a per-band seam.
        for y in iy0..iy1 {
            if (y + roll) % 2 != 0 {
                continue;
            }
            let b = (((iy1 - 1 - y) * bands as i32) / ih).clamp(0, bands as i32 - 1) as usize;
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

        // The colour-shift wobble phase (sesh-vhs), drifting slowly so the tint banding rolls upward.
        let wobble_phase = self.elapsed_s * WOBBLE_HZ * ih as f32;

        // Per-band ornament: the dropout foot, the bright torn-edge noise, the peak mark.
        for b in 0..bands {
            let d_lo = b as i32 * ih / bands as i32;
            let d_hi = (b as i32 + 1) * ih / bands as i32;
            let y_bot = iy1 - 1 - d_lo;
            let y_top = iy1 - 1 - (d_hi - 1).max(d_lo);
            let s = band_s[b];
            let gc = centre + band_sign[b] * (s / 2);
            // Dark dropout streak along the band foot.
            c.fill_rect(ix0, y_bot, iw, 1, dropout);
            // Bright head-switching noise at the two torn edges. `sesh-tape` draws it heavier (x1.5)
            // and over two rows; `sesh-red` reddens it on loud bands; `sesh-vhs` tints it red/cyan.
            if band_streak[b] > 0.01 {
                let a = (band_streak[b] * st.streak_alpha_mul).clamp(0.0, 1.0);
                let level = self.tear[b].clamp(0.0, 1.0);
                let mut noise = if st.hot_tears && level > RED_HOT_TEAR_LEVEL {
                    Rgba::from_hex(&t.hot, a)
                } else {
                    Rgba::from_hex(tear_hex, a)
                };
                if st.wobble {
                    let tint = if ((y_top as f32 + wobble_phase) as i32).rem_euclid(2) == 0 {
                        Rgba::from_hex(BLEED_RED, a)
                    } else {
                        Rgba::from_hex(BLEED_CYAN, a)
                    };
                    noise = Rgba::lerp_linear(noise, tint, 0.6);
                }
                let lx = (gc - s).clamp(ix0, ix1 - 1);
                let rx = (gc + s).clamp(ix0, ix1 - 1);
                for r in 0..st.streak_rows {
                    let ry = (y_top + r).min(iy1 - 1);
                    c.fill_rect((lx - 2).max(ix0), ry, 3, 1, noise);
                    c.fill_rect(rx, ry, (ix1 - rx).min(3), 1, noise);
                }
            }
            // Peak mark: a 1px hot line hanging at the largest recent tear.
            if band_peak[b] > s {
                let py = (y_top + y_bot) / 2;
                let px = (centre + band_sign[b] * band_peak[b]).clamp(ix0, ix1 - 1);
                c.fill_rect(px, py, 1, 1, hot);
            }
        }

        // The rolling head-switching band (sesh-tape): a bright bar rolling slowly up the interior,
        // the bottom-of-frame tear of a mistracked VHS. Drawn last so it reads over the scanlines.
        if st.head_switch && ih > HEAD_SWITCH_H {
            let travel = (ih - HEAD_SWITCH_H).max(1) as f32;
            // Phase-offset half a span so the bar starts mid-interior rather than pinned to the foot,
            // then rolls up and wraps.
            let up = (self.elapsed_s * HEAD_SWITCH_PX_PER_S + travel * 0.5).rem_euclid(travel);
            let hy = iy1 - HEAD_SWITCH_H - up.round() as i32;
            let bar = Rgba::from_hex(&t.lit, (ghost + 0.5).min(1.0));
            for r in 0..HEAD_SWITCH_H {
                let ry = (hy + r).clamp(iy0, iy1 - 1);
                c.fill_rect(ix0, ry, iw, 1, bar);
            }
        }
    }

    /// The camcorder stamp, with any chroma bleed (sesh-vhs): the red and cyan plates offset, then the
    /// `hot` plate on top.
    fn draw_stamp_all(&self, c: &mut Canvas, st: &Style, bbox: (i32, i32, i32, i32), hot: Rgba, tracking: bool) {
        if st.bleed_px > 0 && st.bleed_stamp {
            self.draw_stamp(c, bbox, Rgba::from_hex(BLEED_RED, st.bleed_alpha), -st.bleed_px, tracking);
            self.draw_stamp(c, bbox, Rgba::from_hex(BLEED_CYAN, st.bleed_alpha), st.bleed_px, tracking);
        }
        self.draw_stamp(c, bbox, hot, 0, tracking);
    }

    /// The camcorder stamp: `PLAY` + a play triangle + the elapsed clock (or `TRACKING` during the
    /// dropout), with a `REC` dot top-right that blinks on strong onsets. `dx` offsets the whole stamp
    /// for the chroma-bleed plates.
    fn draw_stamp(&self, c: &mut Canvas, bbox: (i32, i32, i32, i32), col: Rgba, dx: i32, tracking: bool) {
        let (ix0, iy0, ix1, _iy1) = bbox;
        let (ix0, ix1) = (ix0 + dx, ix1 + dx);
        let hot = col;
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

    /// The word layer: one word in pixel blackletter. `sesh-tape` shrinks it to a small bottom-right
    /// CAPTION (the tears are the picture); every other colourway centres it. `sesh-word` forces it
    /// Large, PULSES it on a bass onset (drops + a 2px outline) and scales the outline alpha with rms;
    /// `sesh-vhs` bleeds it in red/cyan; `sesh-red` outlines it in `hot`.
    #[allow(clippy::too_many_arguments)]
    fn draw_word(
        &mut self,
        c: &mut Canvas,
        t: &Theme,
        st: &Style,
        bbox: (i32, i32, i32, i32),
        onset: bool,
        bass: f32,
        rms: f32,
        dt: f32,
        lit: Rgba,
    ) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let (iw, ih, h) = (ix1 - ix0, iy1 - iy0, c.height());

        // The caption is always Small; a centred word goes Large above the colourway's threshold.
        let large = !st.corner_caption && h >= st.word_large_at;
        let size = if large { GothicSize::Large } else { GothicSize::Small };
        let gh = if large { 13 } else { 9 };
        let pulse = st.word_pulse && bass > BASS_ONSET;
        // Draw the randoms up front, so `next_rng` (which needs `&mut self`) does not overlap the
        // immutable borrow of `self` that the word string holds.
        let (r_jit, r_d0, r_d1) = (self.next_rng(), self.next_rng(), self.next_rng());
        let word = gothic::truncate_to_width(self.current_word(), size, iw - 8);
        if !word.is_empty() {
            let tw = gothic::text_width(word, size);
            let (mut x, y) = if st.corner_caption {
                // Bottom-right, like a tape label.
                (ix1 - tw - CAPTION_MARGIN, iy1 - gh - CAPTION_MARGIN)
            } else {
                let mut y = iy0 + (ih - gh) / 2;
                if pulse {
                    y += WORD_PULSE_DROP; // the word drops on the bass
                }
                (ix0 + (iw - tw) / 2, y)
            };
            if onset {
                x += if r_jit & 1 == 0 { 1 } else { -1 };
            }
            // The one red: sesh-red outlines the word in `hot`; every other colourway in `edge`. The
            // word colourway scales the outline alpha with rms so the outline itself is a meter.
            let out_a = if st.outline_rms { (t.edge_alpha * (0.4 + rms)).clamp(0.0, 1.0) } else { t.edge_alpha };
            let outline = if t.id == "sesh-red" {
                Rgba::from_hex(&t.hot, out_a)
            } else {
                Rgba::from_hex(&t.edge, out_a)
            };
            // sesh-vhs chroma bleed: the word in each plate, offset, BEFORE the main pass.
            if st.bleed_px > 0 {
                gothic::draw(c, x - st.bleed_px, y, word, size, Rgba::from_hex(BLEED_RED, st.bleed_alpha), None);
                gothic::draw(c, x + st.bleed_px, y, word, size, Rgba::from_hex(BLEED_CYAN, st.bleed_alpha), None);
            }
            // A pulsing word thickens its outline to 2px: an extra outline ring at radius 2.
            if pulse {
                for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                    gothic::draw(c, x + dx, y + dy, word, size, outline, None);
                }
            }
            gothic::draw(c, x, y, word, size, lit, Some(outline));

            // Drips from the word's feet on a bass hit (an onset over the bass threshold), capped per
            // colourway - `sesh-word` drips twice as hard.
            let (wx0, wx1, wy) = (x, x + tw, y + gh);
            let strong = onset && bass > BASS_ONSET;
            if strong {
                let span = (wx1 - wx0).max(1) as u64;
                let spots = [wx0 + (r_d0 % span) as i32, wx0 + (r_d1 % span) as i32];
                let mut k = 0usize;
                for dr in self.drips.iter_mut().take(st.drip_cap) {
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

    /// The dropout SLAM: the word sits centred (jittering ±1px in place, outline thickened) on a
    /// static field whose density decays with the envelope, and two-or-three horizontal tear rows
    /// across the word snap sideways by a few px per frame and snap straight back. Nothing travels
    /// laterally and nothing wraps - the word never slides in from a side. Built in a reused scratch
    /// canvas over an opaque base, then copied back in strips (in place except the tear rows).
    fn draw_dropout(&mut self, c: &mut Canvas, t: &Theme, st: &Style, bbox: (i32, i32, i32, i32), env: f32) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let (w, h, iw, ih) = (c.width(), c.height(), ix1 - ix0, iy1 - iy0);
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let lit = Rgba::from_hex(&t.lit, 1.0);

        let mut scr = self
            .scratch
            .take()
            .filter(|s| s.width() == w && s.height() == h)
            .unwrap_or_else(|| Canvas::new(w, h));
        scr.clear();
        // An opaque base, so every pixel copied back keeps the panel opaque.
        scr.fill_rect(ix0, iy0, iw, ih, panel);

        // Word metrics first (no RNG), so the static can be dimmed across the word's rows.
        let large = !st.corner_caption && h >= st.word_large_at;
        let size = if large { GothicSize::Large } else { GothicSize::Small };
        let gh = if large { 13 } else { 9 };
        let word_y = iy0 + (ih - gh) / 2;
        let (band_lo, band_hi) = ((word_y - 2).max(iy0), (word_y + gh + 2).min(iy1));

        // Static field whose DENSITY decays with the envelope: at the peak the field is dense, and as
        // the slam decays it thins out rather than merely dimming. Capped below full everywhere, and
        // dropped to a low alpha across the WORD'S ROWS so the word reads at every slam frame.
        let density = if self.slam_no_static {
            0
        } else {
            (env * SLAM_STATIC_MAX_PCT as f32).clamp(0.0, SLAM_STATIC_MAX_PCT as f32) as u64
        };
        let full_a = Rgba::from_hex(&t.lit, 0.9);
        let full_b = Rgba::from_hex(&t.edge, 0.9);
        let dim_a = Rgba::from_hex(&t.lit, SLAM_WORD_ROW_ALPHA);
        let dim_b = Rgba::from_hex(&t.edge, SLAM_WORD_ROW_ALPHA);
        for y in iy0..iy1 {
            let dim = y >= band_lo && y < band_hi;
            for x in ix0..ix1 {
                let r = self.next_rng();
                if r % 100 < density {
                    let hi = r & 0x100 == 0;
                    let col = match (dim, hi) {
                        (true, true) => dim_a,
                        (true, false) => dim_b,
                        (false, true) => full_a,
                        (false, false) => full_b,
                    };
                    scr.fill_rect(x, y, 1, 1, col);
                }
            }
        }

        // The word, centred, jittering ±1px IN PLACE, with a thickened (2px) outline, on a WIDE fully
        // opaque panel plate so it punches cleanly through the static on every frame (the plate is
        // panel-coloured, so on a dark colourway it just reads as static parting around the word).
        let (jx, jy) = if self.slam_pinned {
            (0, 0)
        } else {
            ((self.next_rng() % 3) as i32 - 1, (self.next_rng() % 3) as i32 - 1)
        };
        let word = gothic::truncate_to_width(self.current_word(), size, iw - 8);
        if !word.is_empty() {
            let tw = gothic::text_width(word, size);
            let x = ix0 + (iw - tw) / 2 + jx;
            let y = word_y + jy;
            let kb = Rgba::from_hex(&t.panel, 1.0);
            scr.fill_rect((x - 7).max(ix0), (y - 4).max(iy0), (tw + 14).min(iw), (gh + 8).min(ih), kb);
            let outline = Rgba::from_hex(&t.edge, t.edge_alpha);
            for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                gothic::draw(&mut scr, x + dx, y + dy, word, size, outline, None);
            }
            gothic::draw(&mut scr, x, y, word, size, lit, Some(outline));
        }

        // Copy the whole interior back in place - no shift, so the picture stays put.
        c.copy_region(&scr, (ix0, iy0), (ix0, iy0), iw, ih);
        // Then re-copy a few tear rows across the word, each offset a few px THIS frame only (fresh
        // random each frame => it snaps back rather than drifting), no wrap: the shifted-out edge keeps
        // the in-place copy underneath, which reads as a torn seam.
        let span = SLAM_TEAR_MAX - SLAM_TEAR_MIN + 1;
        for k in 0..SLAM_TEAR_ROWS {
            let sy = (word_y + jy + (k as i32) * gh / SLAM_TEAR_ROWS as i32).clamp(iy0, iy1 - 1);
            let rows = (gh / SLAM_TEAR_ROWS as i32).max(2).min(iy1 - sy);
            let r = self.next_rng();
            let mag = SLAM_TEAR_MIN + (r % span as u64) as i32;
            let off = if self.slam_pinned {
                0
            } else if r & 0x100 == 0 {
                mag
            } else {
                -mag
            };
            if off > 0 {
                c.copy_region(&scr, (ix0, sy), (ix0 + off, sy), iw - off, rows);
            } else if off < 0 {
                let a = -off;
                c.copy_region(&scr, (ix0 + a, sy), (ix0, sy), iw - a, rows);
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
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "sesh").count(), 7);
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
        for id in [
            "sesh-tape",
            "sesh-word",
            "sesh-vhs",
            "sesh-red",
            "sesh-bleached",
            "sesh-graveyard",
            "sesh-nightvision",
        ] {
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
        let ids = [
            "sesh-tape",
            "sesh-word",
            "sesh-vhs",
            "sesh-red",
            "sesh-bleached",
            "sesh-graveyard",
            "sesh-nightvision",
        ];
        for id in ids {
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
            // Flourish sequence: settle, fire, capture the blank (frame 1), the slam peak (frame 2),
            // then a mid-decay frame - so the eye can confirm the word STAYS PUT and never slides in.
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
            c.clear();
            fam.draw(&mut c, &t, &frame(0.3, 121.0 * 0.0167, false));
            write(format!("sesh-{short}-slam"), &c);
            for k in 122..127 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.3, k as f32 * 0.0167, false));
            }
            write(format!("sesh-{short}-flourish-decay"), &c);
        }
        // The tape/word pair at the narrow 190x48, to judge both at a small size side by side.
        let _ = sizes[2];
        for id in ["sesh-tape", "sesh-word"] {
            let (tag, w, h) = sizes[1];
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

    /// The colourways must read as DIFFERENT things, not one look tinted several ways. Renders each
    /// loud at 380x60 for 30 frames off identical input and asserts every PAIR differs in at least
    /// 15% of interior pixels by a `drew_over_panel`-style channel delta between the two frames. This
    /// is the guard that would have caught the v0.3.0 complaint that "all the sesh themes look the
    /// same except the red one".
    #[test]
    fn the_seven_colourways_are_visibly_different() {
        let ids = [
            "sesh-tape",
            "sesh-word",
            "sesh-vhs",
            "sesh-red",
            "sesh-bleached",
            "sesh-graveyard",
            "sesh-nightvision",
        ];
        let (w, h) = (380i32, 60i32);
        let canvases: Vec<Canvas> = ids
            .iter()
            .map(|id| {
                let t = theme(id);
                frames(&mut Sesh::default(), &t, w, h, 0.85, 30)
            })
            .collect();
        let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
        let interior = ((ix1 - ix0) * (iy1 - iy0)) as f32;
        let mut too_similar = Vec::new();
        for a in 0..ids.len() {
            for b in (a + 1)..ids.len() {
                let (ca, cb) = (&canvases[a], &canvases[b]);
                let mut diff = 0usize;
                for y in iy0..iy1 {
                    for x in ix0..ix1 {
                        let (pa, pb) = (ca.get(x, y), cb.get(x, y));
                        let d = (pa.r as i32 - pb.r as i32).abs()
                            + (pa.g as i32 - pb.g as i32).abs()
                            + (pa.b as i32 - pb.b as i32).abs();
                        if d > 24 {
                            diff += 1;
                        }
                    }
                }
                let frac = diff as f32 / interior;
                if frac < 0.15 {
                    too_similar.push(format!("{} vs {}: {:.1}%", ids[a], ids[b], frac * 100.0));
                }
            }
        }
        assert!(
            too_similar.is_empty(),
            "these colourway pairs differ in <15% of interior pixels: {too_similar:?}"
        );
    }

    /// The slam keeps the word legible on EVERY frame, and this is SHAPE-AWARE, not a pixel count.
    ///
    /// Inside the knock-back plate rectangle only, it compares a real (static-on) mid-decay slam frame
    /// against a static-SUPPRESSED reference at the same pinned geometry: if the plate is genuinely
    /// clean panel + word, the two are pixel-identical there; if static leaks into the plate (a too-
    /// small or too-transparent plate) the leaked noise diverges from the clean reference. Both frames
    /// pin jitter and tear offsets so the word sits at the same place. Asserts >=90% of plate pixels
    /// match. A plain "count near-lit pixels in the band" test is vacuous here - the static is `lit`
    /// at 0.9 over near-black, so noise reads as word; this compares SHAPE instead.
    #[test]
    fn the_slam_keeps_the_word_legible() {
        let t = theme("sesh-word");
        let (w, h) = (380i32, 60i32);

        // A mid-decay slam frame, `pin`ned, optionally with the static suppressed (the reference).
        let slam_frame = |no_static: bool| -> Canvas {
            let mut fam = Sesh::default();
            fam.pin_slam_jitter_for_test();
            if no_static {
                fam.suppress_slam_static_for_test();
            }
            let _ = frames(&mut fam, &t, w, h, 0.5, 5);
            fam.flourish.force_next();
            let mut c = Canvas::new(w, h);
            let mut d = FrameData::default();
            for (i, v) in d.levels.iter_mut().enumerate() {
                *v = 0.3 * (1.0 - i as f32 / 96.0);
            }
            d.peaks = d.levels;
            d.rms_l = 0.3;
            d.rms_r = 0.3;
            d.dt_ms = 16.7;
            fam.draw(&mut c, &t, &d); // the forced blank frame
            for _ in 0..7 {
                fam.draw(&mut c, &t, &d); // into the mid-decay of the ~250ms slam
            }
            c
        };
        let reference = slam_frame(true); // clean plate + word
        let slam = slam_frame(false); // plate + word + a dense static field around it

        // The knock-back plate rectangle, computed exactly as `draw_dropout` does (jitter pinned to 0).
        let (gh, iy0, iy1) = (13, 4, h - 4); // sesh-word is Large (gh=13) at h=60
        let (iw, ih) = (w - 6, iy1 - iy0);
        let word_y = iy0 + (ih - gh) / 2;
        let tw = gothic::text_width("SESH", GothicSize::Large);
        let x = 3 + (iw - tw) / 2;
        let (px0, py0) = ((x - 7).max(3), (word_y - 4).max(iy0));
        let (pw, ph) = ((tw + 14).min(iw), (gh + 8).min(ih));

        let close = |a: Rgba, b: Rgba| {
            (a.r as i32 - b.r as i32).abs()
                + (a.g as i32 - b.g as i32).abs()
                + (a.b as i32 - b.b as i32).abs()
                <= 24
        };
        let (mut match_n, mut total) = (0i32, 0i32);
        for y in py0..(py0 + ph).min(iy1) {
            for x in px0..(px0 + pw).min(w - 3) {
                total += 1;
                if close(slam.get(x, y), reference.get(x, y)) {
                    match_n += 1;
                }
            }
        }
        assert!(total > 200, "plate rectangle too small to be a real test: {total} px");
        let frac = match_n as f32 / total as f32;
        assert!(
            frac >= 0.90,
            "only {:.1}% of plate pixels match the clean word-alone reference - static is leaking into the plate",
            frac * 100.0
        );
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
