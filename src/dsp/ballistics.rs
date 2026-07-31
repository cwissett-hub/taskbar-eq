// Task 7 introduces this type standalone; a later task wires Smoother into
// the audio-capture -> render pipeline in main. Until then, rustc's
// binary-crate dead-code check flags these items as unused even though the
// tests below exercise them.
#![allow(dead_code)]

use crate::dsp::bands::NUM_BANDS;

#[derive(Debug, Clone, Copy)]
pub struct Ballistics {
    pub attack: f32,
    pub decay: f32,
    pub peak_fall: f32,
}

impl Default for Ballistics {
    fn default() -> Self {
        // Matches the VFD Ice colourway in the spec.
        Ballistics { attack: 0.55, decay: 0.11, peak_fall: 0.0055 }
    }
}

pub struct Smoother {
    b: Ballistics,
    levels: [f32; NUM_BANDS],
    peaks: [f32; NUM_BANDS],
}

impl Smoother {
    pub fn new(b: Ballistics) -> Self {
        Smoother { b, levels: [0.0; NUM_BANDS], peaks: [0.0; NUM_BANDS] }
    }
    pub fn set_ballistics(&mut self, b: Ballistics) {
        self.b = b;
    }
    pub fn levels(&self) -> &[f32; NUM_BANDS] {
        &self.levels
    }
    pub fn peaks(&self) -> &[f32; NUM_BANDS] {
        &self.peaks
    }
    pub fn update(&mut self, target: &[f32; NUM_BANDS]) {
        for i in 0..NUM_BANDS {
            // f32::clamp does not sanitise NaN (NaN < min and NaN > max are both
            // false, so clamp returns NaN unchanged). A single non-finite sample
            // would otherwise poison `levels[i]` permanently, since NaN + x = NaN
            // and clamp(NaN) = NaN on every subsequent frame.
            let t = if target[i].is_finite() { target[i] } else { 0.0 }.clamp(0.0, 1.0);
            // Asymmetric one-pole: snap up, ease down.
            let rate = if t > self.levels[i] { self.b.attack } else { self.b.decay };
            self.levels[i] = (self.levels[i] + (t - self.levels[i]) * rate).clamp(0.0, 1.0);
            self.peaks[i] = (self.peaks[i] - self.b.peak_fall).max(self.levels[i]).clamp(0.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(v: f32) -> [f32; NUM_BANDS] {
        [v; NUM_BANDS]
    }

    #[test]
    fn starts_at_zero() {
        let s = Smoother::new(Ballistics::default());
        assert!(s.levels().iter().all(|&v| v == 0.0));
        assert!(s.peaks().iter().all(|&v| v == 0.0));
    }

    #[test]
    fn rises_faster_than_it_falls() {
        let b = Ballistics::default();
        let mut up = Smoother::new(b);
        up.update(&flat(1.0));
        let after_one_rise = up.levels()[0];

        let mut down = Smoother::new(b);
        for _ in 0..40 {
            down.update(&flat(1.0));
        }
        let peak_level = down.levels()[0];
        down.update(&flat(0.0));
        let dropped = peak_level - down.levels()[0];

        assert!(
            after_one_rise > dropped,
            "attack ({after_one_rise}) must outpace decay ({dropped}) or the meter feels dead"
        );
    }

    #[test]
    fn converges_toward_a_held_target() {
        let mut s = Smoother::new(Ballistics::default());
        for _ in 0..80 {
            s.update(&flat(0.7));
        }
        assert!((s.levels()[0] - 0.7).abs() < 0.01, "got {}", s.levels()[0]);
    }

    #[test]
    fn never_overshoots_or_goes_negative() {
        let mut s = Smoother::new(Ballistics::default());
        for i in 0..200 {
            s.update(&flat(if i % 2 == 0 { 1.0 } else { 0.0 }));
            assert!(s.levels().iter().all(|&v| (0.0..=1.0).contains(&v)));
            assert!(s.peaks().iter().all(|&v| (0.0..=1.0).contains(&v)));
        }
    }

    #[test]
    fn peaks_hold_above_levels_then_sink_slowly() {
        let mut s = Smoother::new(Ballistics::default());
        for _ in 0..60 {
            s.update(&flat(1.0));
        }
        let held = s.peaks()[0];
        assert!(held > 0.9);

        s.update(&flat(0.0));
        assert!(s.peaks()[0] > s.levels()[0], "peak must lag the level down");

        // peak_fall of 0.0055 per frame is ~0.33 per second at 60fps
        let before = s.peaks()[0];
        for _ in 0..10 {
            s.update(&flat(0.0));
        }
        let fell = before - s.peaks()[0];
        assert!(fell > 0.0 && fell < 0.15, "peak fell {fell}, expected a slow sink");
    }

    #[test]
    fn peak_jumps_immediately_to_a_new_high() {
        let mut s = Smoother::new(Ballistics::default());
        for _ in 0..60 {
            s.update(&flat(0.3));
        }
        let low = s.peaks()[0];
        for _ in 0..60 {
            s.update(&flat(0.9));
        }
        assert!(s.peaks()[0] > low + 0.4, "peak must track a new maximum");
    }

    #[test]
    fn a_single_nan_sample_does_not_poison_the_level_forever() {
        let mut s = Smoother::new(Ballistics::default());
        s.update(&flat(0.5));
        s.update(&flat(f32::NAN));
        assert!(
            s.levels().iter().all(|v| v.is_finite()),
            "a NaN target must not leave levels non-finite, got {:?}",
            s.levels()
        );
        assert!(
            s.peaks().iter().all(|v| v.is_finite()),
            "a NaN target must not leave peaks non-finite, got {:?}",
            s.peaks()
        );

        // Confirm the poisoning doesn't linger: subsequent normal samples must
        // still converge, not stay NaN forever.
        for _ in 0..80 {
            s.update(&flat(0.7));
        }
        assert!((s.levels()[0] - 0.7).abs() < 0.01, "got {}", s.levels()[0]);
    }
}
