//! The trigger every family's flourish hangs off: a rare, exceptional hit.
//!
//! Asked for as "a special feature for all the themes like the lightning in the vaporwave... something
//! special and unique for each theme to do on big hits", and then, crucially: "keep it fairly rare,
//! just for big hits, but allow it to be tunable per theme so I can tune later to my taste".
//!
//! **"Rare" and "big" are the whole problem, and this project has got them wrong twice.** Both times
//! the mistake was the same shape - an absolute threshold in the units of whatever the detector
//! happened to emit:
//!
//! - the vaporwave lightning could not fire on real music at all;
//! - the radar's launch flash could not fire either, across four attempts, each passing its synthetic
//!   tests.
//!
//! The radar's fix is the one reused here, because it was measured across four tracks captured from a
//! real listening session and it held: **judge a hit against the median of recent hits, not against a
//! constant.** A median rather than a mean, because the two things that poison a mean here are exactly
//! the two things that occur - a detector's startup transient, and a genuinely big hit, which is
//! precisely the event that must not be allowed to redefine what ordinary means.
//!
//! So one onset detector finds candidate hits, and a hit is a FLOURISH when it is a multiple of the
//! median candidate. That multiple is what `Theme::flourish` sets, and because everything is relative
//! the same setting means the same thing on a compressed pop master and on a dynamic drum-and-bass
//! track.
//!
//! # What a family does with it
//!
//! Nothing, unless it wants to. `Trigger::update` returns true on the rare frame and false otherwise;
//! each family decides what that looks like. The point of putting the decision here is that "rare"
//! and "big" are then defined once, and one fixture measurement covers every family at once - rather
//! than nine families each shipping their own unfireable threshold.

use super::onset::Flux;

/// Ratio and refractory used to find CANDIDATE hits.
///
/// Deliberately permissive - this is not the flourish threshold, it is the net that catches events
/// worth ranking. 2.0 with a 180ms refractory yields a few per second on real music, which is enough
/// population for a median to mean something within a couple of seconds of a track starting.
const CANDIDATE_RATIO: f32 = 2.0;
const CANDIDATE_REFRACTORY_MS: f32 = 180.0;

/// Candidate magnitudes kept, and how many are needed before a flourish may fire.
///
/// 16 is about eight seconds of music at the candidate rate: long enough to describe "what this track
/// is like", short enough to follow a change of track. `MIN_SAMPLES` stops the very first hits of a
/// session - which include the detector's own startup - from being ranked against almost nothing.
const WINDOW: usize = 16;
const MIN_SAMPLES: usize = 8;

/// Multiple of the median candidate that counts as a flourish, at `flourish` = 1 and at `flourish`
/// approaching 0.
///
/// The floor is above 1.0, which makes "never on a steady groove" structural rather than tuned: a
/// metronomic kick has a ratio of 1.0 against the median by definition, so a floor above 1.0 cannot
/// fire on one at any setting. The radar's launch knob learned this the hard way - its first mapping
/// multiplied one ratio by the knob and so fired on literally every beat at the loose end.
const RATIO_AT_FULL: f32 = 1.30;
const RATIO_AT_ZERO: f32 = 2.60;

/// Hard floor on the gap between flourishes, whatever the material.
///
/// A flourish is a whole-display event - a self-test sweep, a trigger loss, a needle slamming to OVER.
/// Two of them inside a second read as a malfunction rather than as punctuation, and no ratio can
/// guarantee a gap on its own because a track can simply contain two enormous hits in quick
/// succession. This is the backstop that makes "rare" true rather than likely.
const MIN_GAP_MS: f32 = 2500.0;

/// Fires on a rare, exceptional hit. One per family instance.
#[derive(Default)]
pub struct Trigger {
    onset: Flux,
    recent: [f32; WINDOW],
    n: usize,
    head: usize,
    since_ms: f32,
}

impl Trigger {
    /// Advances the trigger and reports whether this frame is a flourish.
    ///
    /// `strength` is `Theme::flourish`: 0 disables it entirely, and larger values fire more often by
    /// lowering the multiple of the median a hit has to reach. Values above 1 are clamped - the
    /// interesting range is all below it, and letting it run away would defeat `MIN_GAP_MS`.
    pub fn update(&mut self, levels: &[f32], dt_ms: f32, strength: f32) -> bool {
        let dt = if dt_ms.is_finite() { dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        self.since_ms += dt;
        if !self.since_ms.is_finite() {
            self.since_ms = MIN_GAP_MS;
        }
        // Off means off, and it means off BEFORE any state is advanced - a colourway with the flourish
        // disabled should cost nothing and accumulate nothing, so that turning it on mid-session
        // starts from a clean window rather than from a stale one.
        let strength = if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 0.0 };
        if strength <= 0.0 {
            return false;
        }
        if !self.onset.update(levels, dt, CANDIDATE_RATIO, CANDIDATE_REFRACTORY_MS) {
            return false;
        }
        let mag = self.onset.flux();
        if !mag.is_finite() || mag <= 0.0 {
            return false;
        }
        // Recorded before the comparison, so the window always includes this hit. A median cannot be
        // skewed by the single sample being tested, which is what lets it be read immediately.
        self.recent[self.head] = mag;
        self.head = (self.head + 1) % WINDOW;
        self.n = (self.n + 1).min(WINDOW);
        if self.n < MIN_SAMPLES {
            return false;
        }
        let need = RATIO_AT_ZERO + (RATIO_AT_FULL - RATIO_AT_ZERO) * strength;
        if mag >= self.median() * need && self.since_ms >= MIN_GAP_MS {
            self.since_ms = 0.0;
            return true;
        }
        false
    }

    /// Median of the recent candidate magnitudes - what "an ordinary big hit on this track" means.
    fn median(&self) -> f32 {
        let n = self.n.min(WINDOW);
        if n == 0 {
            return f32::MAX;
        }
        let mut buf = [0.0f32; WINDOW];
        buf[..n].copy_from_slice(&self.recent[..n]);
        let s = &mut buf[..n];
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) * 0.5 };
        if mid.is_finite() && mid > 0.0 {
            mid
        } else {
            f32::MAX
        }
    }
}

/// A one-shot envelope for a flourish: full on the frame it fires, decaying to nothing.
///
/// Here rather than in each family because nine families needing the same three lines is nine chances
/// to leave out the NaN guard or to decay per FRAME instead of per millisecond - the mistake this
/// project has already made in the vaporwave refractory and in the fluid tank's droplets. A family
/// keeps only its own decay time, which is a real per-family judgement: a valve's ionisation lingers,
/// a VFD self-test snaps.
#[derive(Default)]
pub struct Envelope {
    level: f32,
}

impl Envelope {
    /// Advances the envelope and returns its level, 0..1.
    pub fn update(&mut self, fired: bool, dt_ms: f32, decay_ms: f32) -> f32 {
        let dt = if dt_ms.is_finite() { dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        let decay = if decay_ms.is_finite() { decay_ms.max(1.0) } else { 400.0 };
        if !self.level.is_finite() {
            self.level = 0.0;
        }
        if fired {
            self.level = 1.0;
        } else {
            self.level = (self.level - dt / decay).max(0.0);
        }
        self.level
    }

    /// The current level without advancing, for a family that draws in more than one pass.
    pub fn level(&self) -> f32 {
        if self.level.is_finite() {
            self.level.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// A frame sequence whose LAST frame fires a flourish, for the families' own tests.
///
/// Shared so that nine families agree about what "a flourish happened" looks like, and - more
/// importantly - so that a change to the warm-up requirements here cannot silently turn nine family
/// tests vacuous. `the_shared_firing_sequence_actually_fires` guards the helper itself, because a
/// sequence that quietly stopped firing would make every one of those tests pass while asserting
/// nothing.
///
/// **Measure a frame AFTER the last one, not the last one itself.** The firing frame is full-scale
/// across every band - that is what makes it exceptional - so on most families the display is already
/// saturated by the music there and the flourish changes nothing visible. The first attempt at the
/// VFD's test compared exactly that frame and found the two canvases bit-identical. Draw a few quiet
/// frames afterwards and compare those: the envelope is still high while the music has dropped, which
/// is the only place the effect is separable from the audio.
#[cfg(test)]
pub fn firing_sequence(bands: usize) -> Vec<Vec<f32>> {
    let mut out = Vec::new();
    // An ordinary groove first: enough candidate hits to fill the median window past MIN_SAMPLES, and
    // enough elapsed time to clear MIN_GAP_MS.
    //
    // Deliberately QUIET, and the exceptional hit deliberately NOT full scale. An onset is a big
    // CHANGE, not a big level, so a moderate jump out of a quiet groove is just as exceptional as a
    // jump to full scale - and it leaves headroom above the audio, which several families need to be
    // testable at all. The spectrogram's tear writes a full-scale column; against a full-scale firing
    // frame that is bit-identical to the audio, and its first test reported "the tear changed nothing"
    // while the tear was working perfectly.
    for i in 0..300 {
        out.push(vec![if i % 20 == 0 { 0.22 } else { 0.06 }; bands]);
    }
    // The last groove frame is a quiet one (299 is not a multiple of 20), so this is a 0.39 jump - well
    // clear of the trigger's threshold while leaving the level itself at 0.45.
    //
    // 0.45 rather than higher because several families map levels through a response curve that
    // SATURATES around 0.6: at that point the audio is already at full brightness and a flourish drawn
    // at full scale is indistinguishable from it. The spectrogram's tear test measured 54 lit rows
    // against 54 for exactly that reason. 0.45 maps to about two thirds, leaving real headroom above.
    out.push(vec![0.45; bands]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 16.7;
    const N: usize = 64;

    fn flat(v: f32) -> Vec<f32> {
        vec![v; N]
    }

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

    /// Flourishes fired over one fixture at a given setting.
    fn count(frames: &[Vec<f32>], strength: f32) -> u32 {
        let mut t = Trigger::default();
        let mut n = 0;
        for row in frames {
            if t.update(row, DT, strength) {
                n += 1;
            }
        }
        n
    }

    #[test]
    fn the_envelope_decays_in_wall_clock_time_and_survives_a_poisoned_frame() {
        let mut e = Envelope::default();
        assert_eq!(e.update(true, DT, 400.0), 1.0, "it must be full on the frame it fires");
        // Half the decay time, whatever the frame interval. Two cadences, same elapsed time.
        let after = |dt: f32| -> f32 {
            let mut e = Envelope::default();
            e.update(true, dt, 400.0);
            let steps = (200.0 / dt).round() as usize;
            let mut v = 1.0;
            for _ in 0..steps {
                v = e.update(false, dt, 400.0);
            }
            v
        };
        let (fine, coarse) = (after(16.7), after(50.0));
        assert!(
            (fine - coarse).abs() < 0.1,
            "200ms of decay gave {fine:.2} at 16.7ms frames and {coarse:.2} at 50ms - it is decaying              per frame rather than per millisecond"
        );
        assert!((fine - 0.5).abs() < 0.1, "half the decay time should be about half, got {fine:.2}");
        // Reaches zero and stays there.
        for _ in 0..100 {
            e.update(false, DT, 400.0);
        }
        assert_eq!(e.level(), 0.0);
        // And a poisoned frame cannot leave it stuck.
        e.update(true, f32::NAN, f32::NAN);
        e.update(false, f32::INFINITY, -1.0);
        assert!(e.level().is_finite(), "a poisoned frame left the envelope at {}", e.level());
    }

    #[test]
    fn the_shared_firing_sequence_actually_fires() {
        // The guard on the helper the family tests depend on. If this sequence ever stopped producing
        // a flourish, every family's flourish test would still pass - while asserting nothing at all.
        let seq = firing_sequence(N);
        let mut t = Trigger::default();
        let mut fired_on = None;
        for (i, row) in seq.iter().enumerate() {
            if t.update(row, DT, crate::themes::DEFAULT_FLOURISH) {
                fired_on = Some(i);
            }
        }
        assert_eq!(
            fired_on,
            Some(seq.len() - 1),
            "the sequence must fire on its LAST frame and only there, fired on {fired_on:?}"
        );
    }

    #[test]
    fn zero_is_off_and_costs_nothing() {
        let mut t = Trigger::default();
        for i in 0..2000 {
            assert!(!t.update(&flat(if i % 20 == 0 { 0.95 } else { 0.1 }), DT, 0.0));
        }
        // And no state was accumulated, so enabling it later starts clean rather than from a window
        // full of history the display never used.
        assert_eq!(t.n, 0, "a disabled trigger accumulated {} samples", t.n);
    }

    #[test]
    fn a_metronomic_groove_never_flourishes_at_any_setting() {
        // Structural, not tuned: an identical hit every beat has a ratio of exactly 1.0 against the
        // median, and every setting's threshold is above 1.0. The radar's launch knob shipped a
        // mapping that went BELOW 1.0 at its loose end and fired on every single beat.
        for strength in [0.05f32, 0.25, 0.5, 0.75, 1.0] {
            let mut t = Trigger::default();
            let mut fired = 0;
            // 30 seconds of a perfectly regular kick.
            for i in 0..1800 {
                if t.update(&flat(if i % 30 == 0 { 0.9 } else { 0.2 }), DT, strength) {
                    fired += 1;
                }
            }
            assert_eq!(fired, 0, "a metronomic groove flourished {fired} times at strength {strength}");
        }
    }

    #[test]
    fn an_exceptional_hit_flourishes_and_an_ordinary_one_does_not() {
        let mut t = Trigger::default();
        // Establish what ordinary is on this material.
        for i in 0..900 {
            t.update(&flat(if i % 30 == 0 { 0.5 } else { 0.2 }), DT, 0.5);
        }
        // An ordinary hit, well clear of the minimum gap, must not fire.
        assert!(!t.update(&flat(0.5), DT, 0.5), "an ordinary hit flourished");
        for _ in 0..200 {
            t.update(&flat(0.2), DT, 0.5);
        }
        // A much bigger one must.
        assert!(t.update(&flat(1.0), DT, 0.5), "an exceptional hit did not flourish");
    }

    #[test]
    fn two_exceptional_hits_close_together_yield_only_one_flourish() {
        // The MIN_GAP_MS backstop. No ratio can guarantee a gap on its own, because a track can
        // contain two enormous hits a second apart - and two whole-display events inside a second read
        // as a malfunction rather than as punctuation.
        //
        // Note the shape of this test, because the obvious version does not work AND its failure is
        // the design working: driving a continuous stream of enormous hits produces exactly one
        // flourish, since after the first the median rises to meet them and they stop being
        // exceptional. So the fixture has to be an ordinary groove with two spikes injected into it.
        let mut t = Trigger::default();
        for i in 0..900 {
            t.update(&flat(if i % 30 == 0 { 0.4 } else { 0.1 }), DT, 1.0);
        }
        let mut fired = 0;
        // First spike.
        if t.update(&flat(1.0), DT, 1.0) {
            fired += 1;
        }
        // One second of the ordinary groove - well inside the 2500ms floor.
        for i in 0..60 {
            if t.update(&flat(if i % 30 == 0 { 0.4 } else { 0.1 }), DT, 1.0) {
                fired += 1;
            }
        }
        // Second spike, still inside the floor.
        if t.update(&flat(1.0), DT, 1.0) {
            fired += 1;
        }
        assert_eq!(fired, 1, "two spikes 1s apart produced {fired} flourishes, not one");
    }

    #[test]
    fn the_setting_moves_the_rate_in_the_right_direction_on_real_music() {
        // Monotonic on real material, which is the only place it matters. A knob that does nothing -
        // or the wrong thing - is how the radar's launch threshold went unnoticed for four attempts.
        for (name, frames) in fixtures() {
            let loose = count(&frames, 1.0);
            let strict = count(&frames, 0.1);
            println!("{name:16} strength 1.0 -> {loose} flourishes, 0.1 -> {strict}");
            assert!(
                loose >= strict,
                "{name}: raising the setting from 0.1 to 1.0 reduced the rate, {strict} -> {loose}"
            );
        }
    }

    #[test]
    fn the_default_setting_is_rare_on_every_kind_of_music() {
        // "Fairly rare, just for big hits", measured at the value `Theme::flourish` ships with.
        //
        // Each recording is only ~13 seconds, which is too short to estimate the rate of an event
        // meant to happen every tens of seconds - one occurrence in a clip tells you almost nothing.
        // So each is repeated to about two minutes. That is legitimate for a RATE: the median adapts
        // to the material, so a loop is just more of the same music, and it is the only way to
        // distinguish "one per 13s" from "one per 60s" with the fixtures available.
        let mut seen_any = false;
        for (name, frames) in fixtures() {
            let reps = 9;
            let long: Vec<Vec<f32>> =
                (0..reps).flat_map(|_| frames.iter().cloned()).collect();
            let secs = long.len() as f32 * DT / 1000.0;
            let n = count(&long, crate::themes::DEFAULT_FLOURISH);
            let per_min = n as f32 / secs * 60.0;
            let gap = if n > 0 { secs / n as f32 } else { f32::INFINITY };
            println!(
                "{name:16} {n} flourishes in {secs:.0}s = {per_min:.1}/min, one per {gap:.0}s"
            );
            // Rare: no more often than one every eight seconds on any material.
            assert!(
                gap >= 8.0,
                "{name}: one flourish per {gap:.1}s is not rare for a whole-display event"
            );
            seen_any |= n > 0;
        }
        assert!(
            seen_any,
            "the default produced no flourish on ANY fixture - the exact failure this module exists              to prevent"
        );
    }

    /// The rate curve over whatever `TASKBAR_EQ_FIXTURE` points at, for choosing the default.
    ///
    /// Run: cargo test --release probe_flourish_rate -- --ignored --nocapture
    ///
    /// Exists because looping one 13-second recording is a BAD way to measure the rate of a rare
    /// event: the same one or two exceptional moments recur once per loop, so the count jumps in
    /// steps of the loop count rather than forming a curve. Point this at a long capture of varied
    /// material instead.
    #[test]
    #[ignore]
    fn probe_flourish_rate() {
        let path = std::env::var("TASKBAR_EQ_FIXTURE")
            .expect("set TASKBAR_EQ_FIXTURE to a --levels capture");
        let text = std::fs::read_to_string(&path).expect("read the fixture");
        let frames: Vec<Vec<f32>> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split(',').filter_map(|v| v.parse::<f32>().ok()).collect())
            .collect();
        let secs = frames.len() as f32 * DT / 1000.0;
        println!("{} frames = {secs:.0}s of material", frames.len());
        for st in [0.1f32, 0.2, 0.3, 0.35, 0.4, 0.45, 0.5, 0.6, 0.7, 0.85, 1.0] {
            let n = count(&frames, st);
            let gap = if n > 0 { secs / n as f32 } else { f32::INFINITY };
            println!("  strength {st:.2}: {n:3} flourishes, one per {gap:.0}s");
        }
    }

    #[test]
    fn the_knob_spans_a_useful_range_rather_than_two_settings_that_behave_alike() {
        // A knob whose ends differ by one event over two minutes is not tunable. Measured on the
        // dynamic recording, looped, because that is the material with something to find.
        let (_, frames) = fixtures().into_iter().find(|(n, _)| *n == "dnb, dynamic").unwrap();
        let long: Vec<Vec<f32>> = (0..9).flat_map(|_| frames.iter().cloned()).collect();
        let secs = long.len() as f32 * DT / 1000.0;
        let mut rates = Vec::new();
        for s in [0.1f32, 0.3, 0.35, 0.4, 0.45, 0.5, 0.7, 1.0] {
            let n = count(&long, s);
            rates.push((s, n));
            let gap = if n > 0 { secs / n as f32 } else { f32::INFINITY };
            println!("  strength {s:.2}: {n} in {secs:.0}s = one per {gap:.0}s");
        }
        let loosest = rates.last().unwrap().1;
        let strictest = rates.first().unwrap().1;
        assert!(
            loosest >= strictest * 2 || (loosest >= strictest + 3),
            "the knob barely moves the rate: {strictest} at 0.1 against {loosest} at 1.0"
        );
    }
}
