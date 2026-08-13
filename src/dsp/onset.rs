//! Onset detection, in one place, calibrated against committed recordings of real music.
//!
//! **Why this module exists.** Three families had grown their own onset detector. The vaporwave grid
//! and the fluid tank had independently written the SAME one - spectral flux against a slow-following
//! average, with a refractory period - differing only in a ratio and in whether the refractory was
//! counted in frames or milliseconds. The radar has a genuinely different one, measuring low-band
//! energy rather than broadband change.
//!
//! Two copies of one algorithm is two thresholds to get wrong, and this project has already shipped
//! that mistake twice in ways a user noticed:
//!
//! - the vaporwave lightning trigger could not fire on real music **at all** - the largest bass rise
//!   in the fixture was 0.140 against a threshold of 0.157;
//! - the radar warning receiver's launch flash could not fire either, through four successive
//!   attempts, each of which passed its synthetic tests.
//!
//! Both were absolute thresholds calibrated against something other than real audio. So this module
//! carries the algorithms AND the fixture harness that measures them, and
//! `every_detector_fires_at_a_musical_rate_on_every_fixture` is the guard that a detector which
//! cannot fire never ships again.
//!
//! # The two detectors
//!
//! - [`Flux`] - **spectral flux**, the sum of positive change across every band, thresholded against
//!   a slow-following average of itself. The standard onset measure. Broadband: it fires on snares,
//!   stabs and chord changes as well as kicks. Used by the vaporwave lightning and the fluid tank's
//!   droplets.
//! - [`BassRise`] - the **peak of the lowest bands** against a slew-limited average of that peak.
//!   Narrow and kick-specific, and it reports HOW FAR above the average the rise went rather than
//!   only that it happened, because the radar's warning receiver needs the magnitude to decide
//!   whether a hit is exceptional. Used by the radar family.
//!
//! Both take a threshold from the caller rather than owning one, so a per-colourway knob stays a
//! per-colourway knob and this module has no opinion about how busy any particular display should be.

/// How fast the flux average follows, per millisecond.
///
/// 0.02 per nominal 16.7ms frame, expressed per-ms so a slow frame moves it by the right amount.
/// Slow enough to represent "this track", fast enough to follow an arrangement change within a
/// couple of seconds.
const FLUX_AVG_PER_MS: f32 = 0.02 / 16.7;

/// Spectral-flux onset detector: broadband positive change against its own recent average.
#[derive(Default)]
pub struct Flux {
    prev: Vec<f32>,
    avg: f32,
    since_ms: f32,
    last: f32,
}

impl Flux {
    /// Advances the detector by one frame and reports whether this frame is an onset.
    ///
    /// `ratio` is how many times the recent average the flux must reach; `refractory_ms` the minimum
    /// gap between reported onsets. Both come from the caller - see the module note.
    ///
    /// **The refractory is in MILLISECONDS, not frames.** The vaporwave copy counted frames, which
    /// means its gap stretched and shrank with load: at a 33ms frame its 12-frame refractory was
    /// 400ms rather than the 200 it was calibrated as. Every other timing in this project was moved
    /// off frame counts for the same reason.
    pub fn update(&mut self, levels: &[f32], dt_ms: f32, ratio: f32, refractory_ms: f32) -> bool {
        let dt = if dt_ms.is_finite() { dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        if self.prev.len() != levels.len() {
            self.prev = levels.to_vec();
            // A resize is not an onset: every band would read as a full-scale rise from zero.
            self.since_ms = 0.0;
            return false;
        }
        // Positive change only. A note ENDING is not an onset, and counting decays doubles the event
        // rate - which reads as a display firing at twice the music's tempo.
        let mut flux = 0.0f32;
        for (i, &v) in levels.iter().enumerate() {
            if v.is_finite() {
                flux += (v - self.prev[i]).max(0.0);
                self.prev[i] = v;
            }
        }
        if !flux.is_finite() {
            flux = 0.0;
        }
        self.last = flux;
        self.avg += (flux - self.avg) * (FLUX_AVG_PER_MS * dt).clamp(0.0, 1.0);
        if !self.avg.is_finite() {
            self.avg = 0.0;
        }
        self.since_ms += dt;
        if !self.since_ms.is_finite() {
            self.since_ms = refractory_ms.max(0.0);
        }
        let ratio = if ratio.is_finite() { ratio.max(0.0) } else { 3.0 };
        let gap = if refractory_ms.is_finite() { refractory_ms.max(0.0) } else { 200.0 };
        if flux > self.avg * ratio && self.since_ms > gap {
            self.since_ms = 0.0;
            true
        } else {
            false
        }
    }

    /// This frame's flux.
    ///
    /// Read by `dsp::flourish`, which ranks a hit's MAGNITUDE against recent hits rather than only
    /// asking whether an onset happened.
    pub fn flux(&self) -> f32 {
        self.last
    }
}

/// Low-band onset detector, reporting the MAGNITUDE of the rise.
///
/// Separate from [`Flux`] rather than a mode of it, because it answers a different question. Flux
/// asks "did the spectrum change"; this asks "did the bass hit harder than it has been". A kick under
/// a busy arrangement barely moves broadband flux and moves this a lot.
#[derive(Default)]
pub struct BassRise {
    avg: f32,
    peak: f32,
}

impl BassRise {
    /// Advances the detector and returns how far the low band rose above its own average this frame.
    ///
    /// Returns the EXCESS rather than a bool: the caller compares it to its own threshold, and the
    /// radar's warning receiver additionally needs the magnitude to decide whether a hit is
    /// exceptional for the current material. A detector that only said "yes" would have forced that
    /// judgement to be made from a second, independently drifting measurement.
    ///
    /// `bands` is how many of the lowest bands to watch and `ease` how fast the average follows, per
    /// frame. Captured BEFORE the average is updated, so a hit is measured against where the level
    /// was, not against where it has just been dragged to.
    pub fn update(&mut self, levels: &[f32], bands: usize, ease: f32) -> f32 {
        let hi = bands.min(levels.len());
        let mut peak = 0.0f32;
        for v in &levels[..hi] {
            // is_finite FIRST: `f32::clamp` returns NaN unchanged, so one poisoned band would
            // otherwise settle into `avg` and stay there for the life of the process.
            if v.is_finite() {
                peak = peak.max(*v);
            }
        }
        self.peak = peak.clamp(0.0, 1.0);
        if !self.avg.is_finite() {
            self.avg = 0.0;
        }
        let excess = self.peak - self.avg;
        let ease = if ease.is_finite() { ease.clamp(0.0, 1.0) } else { 0.22 };
        self.avg += (self.peak - self.avg) * ease;
        excess
    }

    /// The current low-band peak, for callers that display it as a level.
    #[cfg(test)]
    pub fn peak(&self) -> f32 {
        self.peak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The render loop's interval, which is how often a family actually asks a detector anything.
    const DT: f32 = 16.7;

    /// Milliseconds between fixture ROWS, which is not the same thing at all.
    ///
    /// The fixtures are one row per DSP frame, and a DSP frame is `HOP` samples: 512 at 48kHz is
    /// **10.67ms**, or 93.75 rows a second. The test used to drive them at `DT` and compute their duration
    /// from `DT`, which was wrong twice over:
    ///
    /// - it claimed each fixture was **1.57x longer** than it is, so every rate it printed and asserted
    ///   was 1.57x too LOW. A detector firing at 2.5/s was reported as 1.6/s.
    /// - it fed the detector every row as though the render loop saw all of them. It does not: the loop
    ///   runs at ~60fps against a 93.75/s capture, so it sees roughly two rows in every three. Feeding all
    ///   of them hands the detector more chances to fire than it will ever get, which is the wrong
    ///   direction for a guard whose whole purpose is catching a threshold that CANNOT fire.
    ///
    /// 48kHz is assumed because that is what the capture device on the machine these were recorded on
    /// runs at. A different rate would change the durations but not the shape of the argument.
    const FIXTURE_ROW_MS: f32 = 1000.0 * crate::dsp::bands::HOP as f32 / 48_000.0;
    const N: usize = 64;

    /// Every band at one level.
    fn flat(v: f32) -> Vec<f32> {
        vec![v; N]
    }

    /// The three committed recordings, with the character of each.
    ///
    /// Named rather than anonymous because which one a measurement came from is the whole point: the
    /// launch-flash bug survived four attempts precisely because one unrepresentative capture was
    /// treated as "real music".
    fn fixtures() -> Vec<(&'static str, Vec<Vec<f32>>)> {
        let parse = |csv: &str| -> Vec<Vec<f32>> {
            csv.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.split(',').filter_map(|v| v.parse::<f32>().ok()).collect())
                .collect()
        };
        vec![
            ("steady groove", parse(include_str!("../../tests/fixtures/real-music-bands.csv"))),
            ("dnb, dynamic", parse(include_str!("../../tests/fixtures/real-music-dynamic.csv"))),
            ("flat-mastered", parse(include_str!("../../tests/fixtures/real-music-flat.csv"))),
        ]
    }

    #[test]
    fn flux_fires_on_a_step_and_not_on_a_steady_level() {
        let mut f = Flux::default();
        // Prime, then hold. The first call only seeds `prev`.
        for _ in 0..200 {
            assert!(!f.update(&flat(0.3), DT, 3.0, 200.0), "a held level is not an onset");
        }
        assert!(f.update(&flat(0.9), DT, 3.0, 200.0), "a step across every band must be an onset");
    }

    #[test]
    fn the_refractory_is_wall_clock_and_not_a_frame_count() {
        // THE reason this module exists rather than two copies of one algorithm. The vaporwave version
        // counted 12 frames, so under load its 200ms gap silently became 400ms or more.
        //
        // Measured as the GAP between the first two onsets, in milliseconds. The first version of this
        // test counted onsets over a fixed wall-clock window at two cadences and expected the totals
        // to match; they did not, and the code was right - a coarse frame adapts the running average
        // by proportionally more each step, so the totals differ for a reason that has nothing to do
        // with the refractory. Counting is the wrong measurement; the gap is the thing being claimed.
        let first_gap = |dt: f32| -> f32 {
            let mut f = Flux::default();
            // Settle the average on a quiet floor, so the threshold is not what limits the rate.
            for _ in 0..(4000.0 / dt) as usize {
                f.update(&flat(0.05), dt, 3.0, 200.0);
            }
            // Then drive an impulse as often as the signal allows and time the first two onsets.
            let mut t = 0.0f32;
            let mut fires: Vec<f32> = Vec::new();
            for i in 0..(4000.0 / dt) as usize {
                t += dt;
                if f.update(&flat(if i % 2 == 0 { 0.9 } else { 0.05 }), dt, 3.0, 200.0) {
                    fires.push(t);
                    if fires.len() == 2 {
                        break;
                    }
                }
            }
            assert_eq!(fires.len(), 2, "only {} onsets at dt={dt}ms", fires.len());
            fires[1] - fires[0]
        };
        for dt in [16.7f32, 33.3, 100.0] {
            let gap = first_gap(dt);
            assert!(
                gap > 200.0,
                "at dt={dt}ms two onsets were {gap:.0}ms apart, inside the 200ms refractory"
            );
            // Within a few frames of the configured gap. A 12-FRAME refractory would give 200ms at
            // 16.7ms/frame and 1200ms at 100ms/frame, which is far outside this bound - that is the
            // discriminating power.
            assert!(
                gap <= 200.0 + 3.0 * dt,
                "at dt={dt}ms the gap was {gap:.0}ms, not the configured 200ms - the refractory is being counted in frames"
            );
        }
    }

    #[test]
    fn flux_adapts_so_a_busy_passage_does_not_fire_continuously() {
        // The property that makes an absolute threshold unnecessary, and the one whose absence made
        // the old lightning trigger unreachable.
        let mut f = Flux::default();
        let mut fired = 0;
        // Alternating hard, which is as busy as this signal gets.
        for i in 0..600 {
            if f.update(&flat(if i % 2 == 0 { 0.1 } else { 0.9 }), DT, 3.0, 200.0) {
                fired += 1;
            }
        }
        // 600 frames is 10 seconds. Firing on most frames would be a strobe.
        assert!(fired < 60, "{fired} onsets in 10s of a continuous alternation is a strobe");
        assert!(fired > 0, "it must still fire at all");
    }

    #[test]
    fn a_resize_is_not_an_onset() {
        // Every band reads as a full-scale rise from zero when the band count changes, which would
        // fire a bolt on every window resize.
        let mut f = Flux::default();
        for _ in 0..50 {
            f.update(&flat(0.4), DT, 3.0, 200.0);
        }
        assert!(!f.update(&vec![0.9; N * 2], DT, 3.0, 200.0), "a band-count change fired an onset");
    }

    #[test]
    fn bass_rise_reports_the_magnitude_and_settles_on_a_sustained_level() {
        let mut b = BassRise::default();
        // A sustained bass line must stop reporting a rise - otherwise a display keyed to this fires
        // continuously and the event becomes a permanent fixture.
        let mut last = 0.0;
        for _ in 0..80 {
            last = b.update(&flat(0.6), 8, 0.22);
        }
        assert!(last.abs() < 0.01, "a sustained level still reports a rise of {last:.3}");
        // And a kick on top of it reports roughly how much bigger it was.
        let mut kick = flat(0.6);
        for v in &mut kick[..8] {
            *v = 0.9;
        }
        let excess = b.update(&kick, 8, 0.22);
        assert!(
            (excess - 0.3).abs() < 0.05,
            "a 0.3 jump should report about 0.3, reported {excess:.3}"
        );
    }

    #[test]
    fn bass_rise_only_watches_the_bands_it_is_told_to() {
        let mut b = BassRise::default();
        for _ in 0..60 {
            b.update(&flat(0.2), 8, 0.22);
        }
        // Energy well above the window must not register at all.
        let mut treble = flat(0.2);
        for v in &mut treble[40..] {
            *v = 0.95;
        }
        let excess = b.update(&treble, 8, 0.22);
        assert!(excess.abs() < 0.01, "a treble-only event moved the bass detector by {excess:.3}");
    }

    #[test]
    fn neither_detector_can_be_poisoned_by_nan_or_infinity() {
        // f32::clamp propagates NaN, which this project has been bitten by twice. A poisoned average
        // never recovers, so the display loses the feature for the life of the process.
        let mut f = Flux::default();
        let mut b = BassRise::default();
        for _ in 0..20 {
            f.update(&flat(0.4), DT, 3.0, 200.0);
            b.update(&flat(0.4), 8, 0.22);
        }
        let mut bad = flat(0.4);
        bad[0] = f32::NAN;
        bad[1] = f32::INFINITY;
        bad[2] = f32::NEG_INFINITY;
        for dt in [f32::NAN, f32::INFINITY, 0.0, -5.0] {
            f.update(&bad, dt, f32::NAN, f32::NAN);
            b.update(&bad, 8, f32::NAN);
        }
        // Recovery is the real assertion: clean frames afterwards must still produce events.
        let mut recovered = false;
        for i in 0..400 {
            if f.update(&flat(if i % 30 == 0 { 0.9 } else { 0.2 }), DT, 3.0, 200.0) {
                recovered = true;
            }
        }
        assert!(recovered, "the flux detector never fired again after a NaN");
        let mut kick = flat(0.2);
        for v in &mut kick[..8] {
            *v = 0.8;
        }
        for _ in 0..40 {
            b.update(&flat(0.2), 8, 0.22);
        }
        assert!(b.update(&kick, 8, 0.22) > 0.3, "the bass detector never recovered after a NaN");
        assert!(b.peak().is_finite());
    }

    #[test]
    fn every_detector_fires_at_a_musical_rate_on_every_fixture() {
        // THE guard this module exists for. Both of this project's user-visible onset bugs were a
        // threshold that could not fire on real audio while passing every synthetic test, and both
        // took a live report to find. Three recordings of genuinely different material, both
        // detectors, at the shipped settings.
        //
        // The band is deliberately wide. The claim is not that any exact rate is right - that is a
        // per-colourway judgement - but that a detector fires somewhere between "noticeably" and
        // "not a strobe" on every kind of music, which is precisely what a zero-firing or
        // every-frame threshold fails.
        for (name, frames) in fixtures() {
            assert!(frames.len() > 500, "{name}: fixture looks truncated, {} frames", frames.len());
            // The recording's REAL duration, from the row interval - not from the render interval.
            let secs = frames.len() as f32 * FIXTURE_ROW_MS / 1000.0;

            let mut f = Flux::default();
            let mut flux_hits = 0u32;
            let mut b = BassRise::default();
            let mut bass_hits = 0u32;
            // Driven the way production drives it: one update per render frame, reading whichever row is
            // current at that moment. At 60fps against a 93.75/s capture that means about two rows in
            // every three are never seen by a detector at all, which is exactly the behaviour a guard
            // against "cannot fire" has to reproduce.
            let total_ms = frames.len() as f32 * FIXTURE_ROW_MS;
            let mut t = 0.0f32;
            while t < total_ms {
                let idx = ((t / FIXTURE_ROW_MS) as usize).min(frames.len() - 1);
                let row = &frames[idx];
                // 2.8 and 200ms are the vaporwave lightning's shipped settings; 0.055 is the radar's.
                if f.update(row, DT, 2.8, 200.0) {
                    flux_hits += 1;
                }
                if b.update(row, 8, 0.22) > 0.055 {
                    bass_hits += 1;
                }
                t += DT;
            }
            let (fr, br) = (flux_hits as f32 / secs, bass_hits as f32 / secs);
            println!("{name:16} flux {fr:.2}/s bass rise {br:.2}/s ({secs:.1}s)");
            assert!(
                (0.3..=6.0).contains(&fr),
                "{name}: the flux detector fires {fr:.2}/s, which is not a musical rate"
            );
            assert!(
                (0.3..=12.0).contains(&br),
                "{name}: the bass detector fires {br:.2}/s, which is not a musical rate"
            );
        }
    }
}
