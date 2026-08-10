use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::{Texture, Theme};

#[derive(Default)]
pub struct Scope {
    trace: Option<Canvas>,
    trail: Option<Canvas>,
    /// Slow-following peak of |waveform|, for auto-ranging. See `update_peak`.
    peak: f32,
    /// The flourish: the sweep loses trigger lock. See `dsp::flourish` and `UNLOCK_MS`.
    flourish: crate::dsp::flourish::Trigger,
    unlock: crate::dsp::flourish::Envelope,
    /// Sample offset added to the triggered start while lock is lost, accumulated across frames so
    /// the trace slides continuously rather than jumping to a fresh random phase each frame. A jump
    /// would read as noise; a slide reads as a sweep running at the wrong rate, which is the fault.
    drift: f32,
}

/// Deflection the auto-ranger aims for, as a fraction of the available amplitude.
///
/// Below 1.0 so an unusually loud transient has somewhere to go before the soft knee
/// engages.
const TARGET_DEFLECTION: f32 = 0.80;

/// Quietest peak the auto-ranger will still normalise against.
///
/// Without a floor, near-silence divides by a tiny peak and amplifies the noise between
/// tracks into a full-height trace.
const PEAK_FLOOR: f32 = 0.05;

/// Peak-follower ballistics, per frame at ~60fps.
///
/// Fast attack catches a transient in ~3 frames so a drum hit does not blow through the
/// top of the screen; a much slower release means the range does not visibly pump
/// between beats. Release is deliberately ~50x slower than attack.
const PEAK_ATTACK: f32 = 0.30;
const PEAK_RELEASE: f32 = 0.006;

/// Fraction of full deflection below which the signal is passed through untouched.
///
/// Above the knee the excess is compressed asymptotically toward 1.0. A plain `tanh` over
/// the whole range was the bug being fixed here: it starts bending immediately, so at the
/// old gain of 3.2 any sample above 0.3 was already at 74% of full height and anything
/// above 0.5 was pinned at the rails - every waveform, loud or quiet, collapsed into the
/// same full-height block. Staying linear under the knee preserves the shape that makes
/// it read as a waveform at all.
const SOFT_KNEE: f32 = 0.70;

/// Samples the trigger search may consume, leaving the rest as the drawable window.
///
/// A real scope syncs its sweep to a zero crossing so a periodic wave stands still on
/// screen. Without that the 256-sample window starts at an arbitrary phase every frame,
/// the trace slides sideways, and persistence smears several DIFFERENT shapes over each
/// other instead of reinforcing one - which is what turned the afterglow into a hatched
/// block rather than a glowing trace.
///
/// Must cover a full period of the lowest frequency worth locking, or the search finds no
/// rising crossing and the frame silently draws untriggered at an arbitrary phase. At 64
/// that happened on well over half of all frames - measured leaving frame-to-frame drawn
/// difference at 0.0735 against 0.0860 for no trigger at all, i.e. very nearly useless.
/// 128 samples is 2.7ms, so everything above ~375Hz locks; on a steady tone the same
/// measurement drops to 0.0012, a 72x improvement. Content that genuinely is not periodic
/// across a 5ms window cannot be stabilised by any trigger and improves only ~25%.
const TRIGGER_SEARCH: usize = 128;

/// Samples actually drawn, after the trigger offset is taken off the front.
const DRAW_WINDOW: usize = 256 - TRIGGER_SEARCH;

/// How long the sweep free-runs during the flourish, and how fast it slides while it does.
///
/// THE FLOURISH IS THIS FAMILY'S OWN WORST BUG, ON PURPOSE. `TRIGGER_SEARCH` above records what an
/// untriggered sweep looks like: the window starts at an arbitrary phase every frame, the trace
/// slides sideways, and the persistence smears several different shapes over each other instead of
/// reinforcing one. That was measured at 0.0860 frame-to-frame difference against 0.0012 locked - a
/// 72x change - which makes it both unmistakable on screen and cheap to assert on.
///
/// So the effect is not a new drawing routine at all; it is the trigger being switched off for
/// 1400ms. Nothing else in the family needs to know.
///
/// 1400ms because the slide has to be legible as a slide: shorter and it reads as one frame of
/// glitch rather than a sweep out of sync.
///
/// The drift rate is scaled by the envelope, and `Envelope` decays LINEARLY, so the slide decelerates
/// as lock is recovered rather than stopping dead. That makes the total travel the integral, not the
/// product: 0.18 samples/ms over the area under a 1400ms linear ramp is 0.18 x 700 = ~126 samples,
/// which is one full sweep window - the trace crosses the screen about once and settles. At the top of
/// the envelope that is 0.18 x 16.7 = 3.0 samples a frame, which at this width is ~4px, and 4px a
/// frame is what the test measures. Faster read as tearing; slower did not read as movement at all.
const UNLOCK_MS: f32 = 1400.0;
const DRIFT_PER_MS: f32 = 0.18;

/// Half-width of the smoothing kernel applied before stroking.
///
/// 256 samples at 48kHz is ~5ms, so everything above roughly 1kHz completes many cycles
/// across the screen. Point-sampling that into one vertical span per column renders treble
/// as a solid block of hash, not a waveform. A short moving average keeps the shape the
/// eye reads as a wave and drops the content too fine to draw at this size. Deliberately
/// small: widen it and the trace goes limp and sinusoidal.
const SMOOTH_TAPS: usize = 2;

/// Limits `x` to +/-1 while leaving everything below `SOFT_KNEE` exactly as it was.
fn soft_clip(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }
    let a = x.abs();
    if a <= SOFT_KNEE {
        return x;
    }
    let headroom = 1.0 - SOFT_KNEE;
    let compressed = SOFT_KNEE + headroom * ((a - SOFT_KNEE) / headroom).tanh();
    if x < 0.0 {
        -compressed
    } else {
        compressed
    }
}

impl Scope {

    /// Draws the waveform polyline into `buf` after decaying what was there.
    ///
    /// Deliberately does NOT bloom `buf` itself. `Canvas::bloom` composites its
    /// halo *underneath* whatever in the buffer isn't already fully opaque, so
    /// calling it every frame directly on the persisted buffer keeps adding
    /// another layer of halo into the still-translucent areas on top of
    /// whatever halo the previous frame already baked in - measured making the
    /// summed luminance at a fixed row GROW frame over frame (4499 -> 5701 over
    /// one decay step) instead of decay. Blooming happens once, on a disposable
    /// clone, at compose time in `draw` - see there.
    /// Advances the peak follower and returns the gain to draw this frame with.
    ///
    /// Auto-ranging rather than a fixed gain because the two are not interchangeable
    /// here: raw PCM spans roughly 0.02 to 0.9 depending on the track and the system
    /// volume, and no single multiplier makes a quiet passage visible without pinning a
    /// loud one against the graticule. A scope is the wrong instrument to read level off
    /// anyway - that is what the VU family is for - so it optimises for showing the
    /// SHAPE of the wave at any volume.
    fn update_peak(&mut self, wave: &[f32; 256]) -> f32 {
        let mut frame_peak = 0.0f32;
        for &s in wave.iter() {
            if s.is_finite() {
                frame_peak = frame_peak.max(s.abs());
            }
        }
        let k = if frame_peak > self.peak {
            PEAK_ATTACK
        } else {
            PEAK_RELEASE
        };
        self.peak += (frame_peak - self.peak) * k;
        if !self.peak.is_finite() {
            self.peak = 0.0;
        }
        TARGET_DEFLECTION / self.peak.max(PEAK_FLOOR)
    }

    /// Smooths the waveform and returns it with the index the sweep should start at.
    ///
    /// Returned as an owned buffer so both persistence layers stroke the exact same
    /// samples; recomputing per layer would let the trail and trace trigger on different
    /// offsets and drift apart.
    fn prepare(d: &FrameData) -> ([f32; 256], usize) {
        let mut s = [0.0f32; 256];
        for i in 0usize..256 {
            let (mut acc, mut n) = (0.0f32, 0.0f32);
            for k in i.saturating_sub(SMOOTH_TAPS)..(i + SMOOTH_TAPS + 1).min(256) {
                let v = d.waveform[k];
                if v.is_finite() {
                    acc += v;
                    n += 1.0;
                }
            }
            s[i] = if n > 0.0 { acc / n } else { 0.0 };
        }

        // Trigger on the first rising crossing of zero, requiring a minimum slope so
        // noise riding on a quiet passage does not trigger on its own dither and
        // reintroduce the sideways jitter this exists to remove.
        //
        // Detected as a sign change between neighbours rather than via an armed/not-armed
        // state machine. The state-machine version demanded the window first dip below a
        // negative threshold BEFORE the crossing, which frequently cannot happen inside
        // the search window, so it fell through to offset 0 - an untriggered frame at
        // arbitrary phase. It failed even on a pure sine, which is what gave it away.
        const MIN_SLOPE: f32 = 0.004;
        let mut start = 0;
        for i in 0..TRIGGER_SEARCH {
            if s[i] <= 0.0 && s[i + 1] > 0.0 && (s[i + 1] - s[i]) >= MIN_SLOPE {
                start = i;
                break;
            }
        }
        (s, start)
    }

    #[allow(clippy::too_many_arguments)]
    fn stroke_into(
        buf: &mut Canvas,
        wave: &[f32; 256],
        start: usize,
        colour: Rgba,
        fade: f32,
        gain: f32,
        // Some(..) only for a rainbow colourway, in which case the trace is coloured per COLUMN so
        // the hue sweeps along it. Passed rather than resolved inside because the trail layer wants
        // its own colour and must not be re-tinted.
        rainbow: Option<(&Theme, f32)>,
    ) {
        // Decay what is already there. Scaling alpha keeps the buffer transparent,
        // which is what lets the panel show through the trail.
        let decay = (1.0 - fade.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        let (w, h) = (buf.width(), buf.height());
        for y in 0..h {
            for x in 0..w {
                let p = buf.get(x, y);
                if p.a == 0 {
                    continue;
                }
                let a = (p.a as f32 * decay) as u8;
                // `fill_rect` always blends source-over onto whatever is already
                // there (see Canvas::blend_over), and it no-ops entirely when the
                // source alpha is 0 (Canvas::fill_rect's own early return). So
                // writing the decayed colour straight over the still-fully-opaque
                // old pixel blends toward the OLD (undecayed) value instead of
                // replacing it - alpha barely moves and the trace never fades.
                // `punch_rect` writes zero unconditionally, so clearing first and
                // then filling the decayed alpha onto a genuinely transparent
                // pixel makes the write an overwrite, not a blend.
                buf.punch_rect(x, y, 1, 1);
                if a > 2 {
                    buf.fill_rect(x, y, 1, 1, Rgba::new(p.r, p.g, p.b, a));
                }
            }
        }

        // Stroke the new trace: one vertical span per column, joining consecutive
        // samples so a steep slope stays continuous instead of dotting.
        let mid = h / 2;
        let amp = (h as f32 * 0.38) as i32;
        let x0 = 5;
        let span = (w - 10).max(1);
        let mut prev_y: Option<i32> = None;
        for px in 0..span {
            let i = start + (px as usize * (DRAW_WINDOW - 1)) / span.max(1) as usize;
            let w = soft_clip(wave[i.min(255)] * gain);
            let y = mid - (w * amp as f32) as i32;
            let y = y.clamp(0, h - 1);
            let colour = match rainbow {
                Some((t, time_s)) => {
                    let x01 = px as f32 / span.max(1) as f32;
                    super::tint(t, x01, time_s, false, &t.lit, colour.a as f32 / 255.0)
                }
                None => colour,
            };
            let (lo, hi) = match prev_y {
                Some(p) if p < y => (p, y),
                Some(p) => (y, p),
                None => (y, y),
            };
            buf.fill_rect(x0 + px, lo, 1, (hi - lo + 1).max(1), colour);
            prev_y = Some(y);
        }
    }
}

impl Family for Scope {
    fn id(&self) -> &'static str {
        "scope"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // Reallocate the persistence buffers when the widget rect changes width.
        let stale = self
            .trace
            .as_ref()
            .map(|b| b.width() != w || b.height() != h)
            .unwrap_or(true);
        if stale {
            self.trace = Some(Canvas::new(w, h));
            self.trail = None;
        }
        if t.dual.is_some() && self.trail.is_none() {
            self.trail = Some(Canvas::new(w, h));
        } else if t.dual.is_none() {
            self.trail = None;
        }

        // One gain and one triggered, smoothed waveform for both buffers, resolved
        // before either is stroked, so the slow trail and the fast trace stay registered
        // with each other instead of drifting apart.
        let (wave, start) = Self::prepare(d);
        let gain = self.update_peak(&wave) * t.sensitivity.max(0.0);

        // THE FLOURISH: trigger loss. The sweep free-runs, so the trace slides and the phosphor
        // smears every phase it passes through. See `UNLOCK_MS`.
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let unlock = self.unlock.update(fired, dt, UNLOCK_MS);
        let start = if unlock > 0.01 {
            self.drift += DRIFT_PER_MS * dt * unlock;
            // Bounded to the sweep window, because `stroke_into` reads up to
            // `wave[start + DRAW_WINDOW - 1]` behind a `.min(255)`: an unbounded offset does not
            // panic, it silently pins every column at sample 255 and draws the trace as a flat
            // horizontal line. Repeated triggers are what get it there - `drift` only resets once the
            // envelope expires, so a hit arriving mid-slide keeps accumulating onto it.
            //
            // Wrapping rather than clamping is a modelling choice, not a safety one. Checked: with
            // start <= 127 and the window 127 wide, even a clamp at TRIGGER_SEARCH tops out at index
            // 254, comfortably in bounds. Wrap because a free-running sweep keeps running - it does
            // not stop at the edge of the buffer.
            self.drift %= (TRIGGER_SEARCH + 1) as f32;
            (start + self.drift as usize) % (TRIGGER_SEARCH + 1)
        } else {
            self.drift = 0.0;
            start
        };

        // Slow trail first (drawn underneath), then the fast trace.
        if let (Some((trail_hex, trail_fade)), Some(trail)) = (t.dual.clone(), self.trail.as_mut()) {
            let c = Rgba::from_hex(&trail_hex, 1.0);
            // The dual-layer trail keeps its own phosphor colour even under a rainbow - its whole
            // point is being a DIFFERENT colour from the trace.
            Self::stroke_into(trail, &wave, start, c, trail_fade, gain, None);
        }
        if let Some(trace) = self.trace.as_mut() {
            let c = Rgba::from_hex(&t.lit, 1.0);
            let rb = if t.rainbow > 0.0 { Some((t, d.time_s)) } else { None };
            Self::stroke_into(trace, &wave, start, c, t.fade, gain, rb);
        }

        // Compose: panel, graticule, trail, trace, bezel.
        c.clear();
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&t.panel, t.panel_alpha));

        // Scanlines, when the colourway asks for them. The scope family ignored
        // `texture` entirely before this, so setting the field on a scope theme was
        // inert - and a colourway that declares a texture it does not get is the same
        // fake distinctness the bloom spread was.
        if matches!(t.texture, Texture::Scanlines) {
            // Every other row, dark rather than lit: a scanline is the ABSENCE of
            // phosphor, so it must darken the panel, not add another glowing grid.
            let line = Rgba::new(0, 0, 0, 90);
            let mut y = 3;
            while y < h - 3 {
                c.fill_rect(1, y, w - 2, 1, line);
                y += 2;
            }
        }

        let grid = Rgba::from_hex(&t.lit, 0.10);
        for k in 1..8 {
            c.fill_rect(1 + (w - 2) * k / 8, 2, 1, h - 4, grid);
        }
        for k in 1..4 {
            c.fill_rect(1, 2 + (h - 4) * k / 4, w - 2, 1, grid);
        }
        c.fill_rect(1, h / 2, w - 2, 1, Rgba::from_hex(&t.lit, 0.20));

        // Trail then trace. Each is bloomed on a disposable clone - never the
        // persisted buffer itself, or the halo would compound frame over frame
        // (see `stroke_into`) - then composited over the already-opaque panel
        // via `draw_over`; blooming in place on `c` would hide the halo
        // entirely once it sits under the opaque panel fill drawn above.
        if let Some(trail) = self.trail.as_ref() {
            let mut glow = trail.clone();
            glow.bloom((t.bloom * 0.8) as i32, 0.9);
            c.draw_over(&glow);
        }
        if let Some(trace) = self.trace.as_ref() {
            let mut glow = trace.clone();
            glow.bloom(t.bloom as i32, 0.9);
            c.draw_over(&glow);
        }

        // The panel above is only inset 1-2px from the canvas edge, but
        // `Canvas::bloom` only clips at the canvas boundary, not the panel's,
        // so the halo just composited via `draw_over` can spread past that
        // thin margin onto the bare transparent/acrylic background outside
        // the rounded "screen" - exactly the bug already fixed once for
        // `segmented.rs` (see its own step 7 and `fix-bloom-containment-report.md`).
        // Must run after the trail/trace composite and before the edge bezel
        // below, mirroring segmented.rs's fix.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);

        // RGB misregistration, for a Pantone colourway; 0 on every other one. Applied to the
        // composited frame rather than to the persisted trace buffer, and that distinction matters
        // here more than in the other families: the buffer is re-read and decayed every frame, so
        // shifting its channels in place would drag the plates a further `aberration` pixels apart
        // on every frame until the trace was three separate coloured traces. See the same class of
        // compounding bug in `stroke_into`'s note about blooming the persisted buffer.
        if t.aberration.is_finite() && t.aberration != 0.0 {
            c.chromatic_aberration(t.aberration.round() as i32);
        }

        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::bands::NUM_BANDS;
    use crate::render::golden::canvas_to_ascii;
    use crate::themes::builtin;

    fn wave(amp: f32) -> FrameData {
        let mut d = FrameData::default();
        for i in 0..256 {
            d.waveform[i] = amp * ((i as f32 / 256.0) * std::f32::consts::TAU * 2.0).sin();
        }
        d.levels = [0.0; NUM_BANDS];
        d
    }

    /// Music-like content whose phase WALKS between frames, which is what makes the trigger do any
    /// work at all. A fixed-phase tone stands still on screen with or without a trigger, so a test
    /// built on one cannot tell a locked sweep from a free-running one. Shared with
    /// `dump_scope_frames`, which is where it started life.
    ///
    /// `noise` is off for the trigger tests and on for the visual dump, and the difference is not
    /// cosmetic. The trigger takes the FIRST rising zero crossing in its search window, and the noise
    /// term manufactures extra crossings whose ordering shifts as the phase walks - so which crossing
    /// wins jumps between frames and the sweep does not lock at all. Measured: with noise, the trace
    /// slid a median 60px a frame with the flourish OFF, which left the test unable to tell a locked
    /// sweep from a free-running one. It is the right content for judging a smear by eye and the wrong
    /// content for asserting lock.
    fn music(t: f32, amp: f32, noise: bool) -> f32 {
        let mut acc = (t * std::f32::consts::TAU * 2.0).sin();
        acc += 0.45 * (t * std::f32::consts::TAU * 5.0).sin();
        acc += 0.22 * (t * std::f32::consts::TAU * 11.0).sin();
        let n = if noise {
            // deterministic pseudo-noise; a random number generator has no place in a test
            ((t * 7919.0).sin() * 43758.55) % 1.0
        } else {
            0.0
        };
        (acc / 1.67 + n * 0.12) * amp
    }

    /// The y of the brightest pixel in each column: where the live trace sits.
    ///
    /// Brightest rather than any-lit because the persistence leaves several older traces on screen at
    /// lower alpha, and a threshold scan would pick up whichever of them happened to be highest.
    fn trace_profile(c: &Canvas) -> Vec<i32> {
        (5..(c.width() - 5))
            .map(|x| {
                (0..c.height())
                    .max_by(|a, b| lum(c.get(x, *a)).partial_cmp(&lum(c.get(x, *b))).unwrap())
                    .unwrap_or(0)
            })
            .collect()
    }

    /// How far the trace slid horizontally between two frames, in pixels.
    ///
    /// The lag that best aligns the two profiles. A locked sweep holds a periodic wave still, so the
    /// best alignment is zero; a free-running one slides a few pixels a frame.
    ///
    /// Whole-frame luminance difference was tried first and is nearly useless here - it measured
    /// 0.0299 unlocked against 0.0239 locked, a factor of 1.24, because most of it is the phosphor
    /// decaying and the non-periodic part of the content changing, neither of which the flourish
    /// touches. The search is wide (+/-80px) because the raw audio's own phase walk is ~7.9 samples a
    /// frame, about 11px, ON TOP of the drift - it is precisely that walk the trigger normally cancels.
    fn slide_px(a: &Canvas, b: &Canvas) -> i32 {
        let (pa, pb) = (trace_profile(a), trace_profile(b));
        let n = pa.len() as i32;
        let mut best = (f32::MAX, 0i32);
        for lag in -80..=80 {
            let (mut acc, mut count) = (0.0f32, 0.0f32);
            for x in 0..n {
                let j = x + lag;
                if j < 0 || j >= n {
                    continue;
                }
                acc += (pa[x as usize] - pb[j as usize]).abs() as f32;
                count += 1.0;
            }
            // Require most of the width to overlap, or a large lag wins on a handful of columns.
            if count < n as f32 * 0.6 {
                continue;
            }
            let mean = acc / count;
            if mean < best.0 {
                best = (mean, lag);
            }
        }
        best.1
    }

    /// A theme for the flourish tests, at a chosen strength.
    fn scope_theme(strength: f32) -> Theme {
        let mut t = builtin::all()
            .into_iter()
            .find(|t| t.family == "scope" && t.dual.is_none())
            .expect("no single-layer scope colourway");
        t.flourish = strength;
        t
    }

    /// One frame of phase-walking tone (no noise - see `music`) at the given band levels.
    fn scope_frame(frame: usize, levels: [f32; NUM_BANDS]) -> FrameData {
        let mut d = FrameData {
            dt_ms: 16.7,
            levels,
            ..FrameData::default()
        };
        d.peaks = levels;
        for i in 0..256 {
            d.waveform[i] = music(i as f32 / 256.0 + frame as f32 * 0.031, 0.35, false);
        }
        d
    }

    #[test]
    fn the_flourish_loses_trigger_lock() {
        // The effect IS this family's documented worst bug, deliberately re-entered for 1400ms, so it
        // is asserted with that bug's own metric rather than a new one: frame-to-frame difference on
        // phase-walking content. Locked, consecutive frames barely differ. Free-running, the trace
        // slides several pixels a frame and they differ a great deal.
        let seq = crate::dsp::flourish::firing_sequence(NUM_BANDS);
        let run = |strength: f32| -> i32 {
            let t = scope_theme(strength);
            let mut sc = Scope::default();
            let mut c = Canvas::new(190, 60);
            let mut frame = 0usize;
            // Settle the peak follower and fill the trigger's own history window before the hit.
            for _ in 0..40 {
                sc.draw(&mut c, &t, &scope_frame(frame, [0.10; NUM_BANDS]));
                frame += 1;
            }
            for row in &seq {
                let mut levels = [0.0f32; NUM_BANDS];
                for (i, v) in levels.iter_mut().enumerate() {
                    *v = row.get(i).copied().unwrap_or(0.0);
                }
                sc.draw(&mut c, &t, &scope_frame(frame, levels));
                frame += 1;
            }
            // Measure over the frames just after the hit, where the envelope is near full. Both arms
            // see byte-identical audio; only `flourish` differs. Median of the per-frame slides, so
            // one frame in which the argmax profile latched onto a decaying older trace cannot decide
            // the result.
            let mut slides = Vec::new();
            for _ in 0..6 {
                let before = c.clone();
                sc.draw(&mut c, &t, &scope_frame(frame, [0.10; NUM_BANDS]));
                frame += 1;
                slides.push(slide_px(&before, &c).abs());
            }
            slides.sort_unstable();
            slides[slides.len() / 2]
        };

        let locked = run(0.0);
        let unlocked = run(crate::themes::DEFAULT_FLOURISH);
        assert!(
            locked <= 1,
            "the sweep was not locked with the flourish off: it slid {locked}px a frame, so this test \
             cannot tell lock from loss of it"
        );
        assert!(
            unlocked >= 4,
            "the sweep never came unstuck: it slid {unlocked}px a frame against {locked}px locked"
        );
    }

    #[test]
    fn losing_lock_never_flattens_the_trace() {
        // Guards the bound on the drift offset. Unbounded, it walks past sample 255, `stroke_into`
        // pins every column there, and the trace becomes a flat horizontal line - no panic, just a
        // dead display for as long as the flourish lasts. Re-triggered repeatedly, which is how the
        // offset gets that far: `drift` only resets once the envelope expires.
        //
        // `fade = 1.0` kills the persistence, and that is what makes this test able to fail at all.
        // The first version left persistence on and scanned for any pixel above a luminance
        // threshold, so it was reading the older traces still decaying in the buffer - it passed with
        // the bound REMOVED ENTIRELY, which is precisely the failure it was written to catch. With no
        // persistence the profile is the live trace and nothing else.
        let mut t = scope_theme(crate::themes::DEFAULT_FLOURISH);
        t.fade = 1.0;
        t.dual = None;
        let seq = crate::dsp::flourish::firing_sequence(NUM_BANDS);
        let quiet = vec![0.10f32; NUM_BANDS];
        let mut sc = Scope::default();
        let mut c = Canvas::new(190, 60);
        let mut worst = i32::MAX;
        let mut frame = 0usize;
        for pass in 0..6 {
            for row in seq.iter().chain(std::iter::repeat(&quiet).take(20)) {
                let mut levels = [0.0f32; NUM_BANDS];
                for (i, v) in levels.iter_mut().enumerate() {
                    *v = row.get(i).copied().unwrap_or(0.0);
                }
                sc.draw(&mut c, &t, &scope_frame(frame, levels));
                frame += 1;
                if pass == 0 {
                    continue; // let the peak follower settle and the persistence fill in first
                }
                // Vertical spread of the live trace across the whole width. A trace pinned to one
                // sample is flat, so its spread collapses; a sliding one keeps the wave's shape.
                let prof = trace_profile(&c);
                let (lo, hi) = prof.iter().fold((i32::MAX, i32::MIN), |(l, h), y| (l.min(*y), h.max(*y)));
                if lo <= hi {
                    worst = worst.min(hi - lo);
                }
            }
        }
        assert!(
            worst >= 8,
            "the trace flattened while lock was lost: {worst}px of vertical spread, which is what an \
             unbounded drift offset looks like once it walks past the end of the sample buffer"
        );
    }

    /// The panel is fully opaque (`panel_alpha: 1.0`), so `.a` is 255 at every
    /// in-bounds pixel regardless of what is drawn on top of the panel -
    /// asserting on `.a` alone is vacuous here. Luminance is what actually
    /// distinguishes a lit trace/halo pixel from bare dark panel.
    fn lum(p: Rgba) -> f32 {
        (0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32) * (p.a as f32 / 255.0)
    }

    #[test]
    fn draws_a_graticule_even_with_no_signal() {
        let mut c = Canvas::new(190, 60);
        Scope::default().draw(&mut c, &builtin::p1_green(), &wave(0.0));
        let ascii = canvas_to_ascii(&c);
        assert!(ascii.contains('.') || ascii.contains(':'), "graticule should be faintly visible");
    }

    #[test]
    fn a_flat_signal_traces_the_centre_line() {
        let mut c = Canvas::new(190, 60);
        Scope::default().draw(&mut c, &builtin::p1_green(), &wave(0.0));
        let mid = lum(c.get(95, 30));
        let top = lum(c.get(95, 8));
        assert!(mid > top, "flat trace must sit on the centre line (mid lum {mid}, top lum {top})");
    }

    #[test]
    fn a_large_signal_reaches_away_from_the_centre() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p1_green(), &wave(1.0));
        // Somewhere in the upper third must be lit by the excursion.
        let lit_high = (0..190).any(|x| lum(c.get(x, 12)) > 100.0);
        assert!(lit_high, "full-scale wave should reach the upper third");
    }

    #[test]
    fn persistence_accumulates_across_frames() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);

        // Frame 1: signal high. Frame 2: flat. The old trace must still be faintly there.
        s.draw(&mut c, &builtin::p1_green(), &wave(1.0));
        let lit_after_one: f32 = (0..190).map(|x| lum(c.get(x, 12))).sum();

        s.draw(&mut c, &builtin::p1_green(), &wave(0.0));
        let lit_after_two: f32 = (0..190).map(|x| lum(c.get(x, 12))).sum();

        // The dark panel and its graticule contribute non-zero luminance at row
        // 12 even with no signal ever drawn (the panel isn't pure black, and a
        // vertical grid line crosses every row). So ">0" against that baseline
        // alone would pass even if persistence were totally broken - it has to
        // be compared against what a scope with NO history at all reads here.
        let mut fresh = Canvas::new(190, 60);
        Scope::default().draw(&mut fresh, &builtin::p1_green(), &wave(0.0));
        let baseline: f32 = (0..190).map(|x| lum(fresh.get(x, 12))).sum();

        assert!(
            lit_after_two > baseline,
            "the previous trace must persist above the bare-panel baseline ({lit_after_two} vs baseline {baseline})"
        );
        assert!(lit_after_two < lit_after_one, "but it must be decaying");
    }

    #[test]
    fn a_high_fade_decays_faster_than_a_low_one() {
        let fast = builtin::p11_blue_violet(); // fade 0.20
        let slow = builtin::scope_amber();     // fade 0.11
        assert!(fast.fade > slow.fade, "test premise: p11 fades faster than amber");

        let residue = |t: &Theme| {
            let mut s = Scope::default();
            let mut c = Canvas::new(190, 60);
            s.draw(&mut c, t, &wave(1.0));
            for _ in 0..8 {
                s.draw(&mut c, t, &wave(0.0));
            }
            (0..190).map(|x| lum(c.get(x, 12))).sum::<f32>()
        };
        assert!(residue(&fast) < residue(&slow), "higher fade must leave less residue");
    }

    #[test]
    fn p7_uses_two_buffers_and_the_others_use_one() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p7_dual(), &wave(0.5));
        assert!(s.trail.is_some(), "P7 must allocate a second buffer for its trail");

        let mut s2 = Scope::default();
        s2.draw(&mut c, &builtin::p1_green(), &wave(0.5));
        assert!(s2.trail.is_none(), "single-layer phosphors must not allocate a trail");
    }

    #[test]
    fn bloom_halo_does_not_leak_outside_the_panel_bezel() {
        // The panel is `rounded_rect(1, 2, w-2, h-4, 4, ...)`, so its row range
        // is 2..=57 on a 60px-tall canvas; rows 0-1 and 58-59 sit outside it,
        // on the bare transparent background. A high bloom radius (p7-dual's
        // 8) composited with a full-amplitude wave without a clip-back spreads
        // a visible halo into those rows - the same containment bug fixed once
        // already for `segmented.rs`. Assert those rows stay fully transparent.
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p7_dual(), &wave(1.0));
        for y in [0, 1, 58, 59] {
            for x in 0..190 {
                let p = c.get(x, y);
                assert_eq!(
                    p.a, 0,
                    "row {y} col {x} must stay blank outside the panel bezel, got {p:?}"
                );
            }
        }
    }

    #[test]
    fn p7_trail_colour_is_distinct_from_its_trace() {
        // The task requires the trail to be "genuinely distinct" from the
        // trace, not just present as a second buffer. Read the private
        // buffers directly (this test module already does so via
        // `p7_uses_two_buffers_and_the_others_use_one`) and check hue, not
        // just that both are lit.
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p7_dual(), &wave(1.0));

        let trace = s.trace.as_ref().expect("trace buffer must exist");
        let trail = s.trail.as_ref().expect("P7 must allocate a trail buffer");

        let lit = |buf: &Canvas| -> Option<Rgba> {
            (0..buf.height())
                .flat_map(|y| (0..buf.width()).map(move |x| (x, y)))
                .map(|(x, y)| buf.get(x, y))
                .find(|p| p.a > 40)
        };

        let trace_px = lit(trace).expect("trace must have a lit pixel");
        let trail_px = lit(trail).expect("trail must have a lit pixel");

        // trace is #e8f4ff (blue-white: b >= g >= r); trail is #cfe86a
        // (yellow-green: g > b > r-ish, definitely g > b). These are cheap,
        // hue-shape checks rather than exact-value checks, so they survive
        // small colour tweaks while still catching "trail is just the trace
        // colour again" (which would fail the trail's `g > b` check).
        assert!(
            trace_px.b >= trace_px.g && trace_px.g >= trace_px.r,
            "trace pixel should read blue-white, got {trace_px:?}"
        );
        assert!(
            trail_px.g > trail_px.b,
            "trail pixel should read yellow-green (g > b), got {trail_px:?}"
        );
        assert_ne!(
            (trace_px.r, trace_px.g, trace_px.b),
            (trail_px.r, trail_px.g, trail_px.b),
            "trail must be a genuinely different colour from the trace"
        );
    }

    #[test]
    fn resizing_the_canvas_does_not_panic() {
        // The widget rect changes width; buffers must be reallocated, not indexed stale.
        let mut s = Scope::default();
        let mut a = Canvas::new(190, 60);
        s.draw(&mut a, &builtin::p1_green(), &wave(0.5));
        let mut b = Canvas::new(120, 60);
        s.draw(&mut b, &builtin::p1_green(), &wave(0.5));
        assert_eq!(b.bits().len(), 120 * 60);
    }

    #[test]
    fn golden_p1_green() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p1_green(), &wave(0.7));
        let expected = include_str!("../../tests/golden/p1-green.txt");
        // `matches_golden` normalises line endings - a bare `assert_eq!` on the
        // string broke every golden test once already on a branch merge where
        // git checked the committed file out as CRLF while `canvas_to_ascii`
        // emits LF, with byte-identical content otherwise.
        assert!(
            crate::render::golden::matches_golden(&c, expected),
            "golden mismatch - if this change is intended, overwrite \
             tests/golden/p1-green.txt and eyeball the diff:\n{}",
            canvas_to_ascii(&c)
        );
    }

    #[test]
    #[ignore]
    fn regenerate_golden() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p1_green(), &wave(0.7));
        std::fs::write("tests/golden/p1-green.txt", canvas_to_ascii(&c)).unwrap();
    }

    /// Dumps every scope colourway to raw RGBA for visual inspection. Not a golden:
    /// "is it a smear" is a question you have to answer with your eyes.
    ///
    /// Run: cargo test --release dump_scope_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_scope_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();

        let mut n = 0usize;
        for (label, amp) in [("quiet", 0.05f32), ("normal", 0.35), ("loud", 0.85)] {
            for t in builtin::all().into_iter().filter(|t| t.family == "scope") {
                let mut sc = Scope::default();
                let mut c = Canvas::new(190, 60);
                // 90 frames = 1.5s, long enough for the peak follower to settle and for
                // any excessive persistence to have filled the screen.
                for f in 0..90 {
                    let mut d = FrameData::default();
                    for i in 0..256 {
                        let phase = i as f32 / 256.0 + f as f32 * 0.031;
                        d.waveform[i] = music(phase, amp, true);
                    }
                    sc.draw(&mut c, &t, &d);
                }
                let mut out = Vec::with_capacity(190 * 60 * 4);
                for y in 0..60 {
                    for x in 0..190 {
                        let px = c.get(x, y);
                        // un-premultiply onto the dark taskbar the overlay really sits on
                        let a = px.a as f32 / 255.0;
                        let bg = 22.0;
                        for ch in [px.r, px.g, px.b] {
                            out.push((ch as f32 + bg * (1.0 - a)).min(255.0) as u8);
                        }
                        out.push(255);
                    }
                }
                std::fs::write(dir.join(format!("scope-{}-{}.rgba", t.id, label)), &out).unwrap();
                n += 1;
            }
        }
        println!("wrote {} rgba dumps to {}", n, dir.display());
    }
}
