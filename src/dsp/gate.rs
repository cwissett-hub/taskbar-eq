// This task introduces Gate standalone; a later task wires it into the
// audio-capture -> render pipeline in main. Until then, rustc's binary-crate
// dead-code check flags these items as unused even though the tests below
// exercise them.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy)]
pub struct GateConfig {
    pub threshold_dbfs: f32,
    pub reveal_ms: u32,
    pub hide_ms: u32,
    pub fade_ms: u32,
}

impl Default for GateConfig {
    fn default() -> Self {
        GateConfig { threshold_dbfs: -55.0, reveal_ms: 400, hide_ms: 2000, fade_ms: 250 }
    }
}

pub struct Gate {
    cfg: GateConfig,
    above_ms: u32,
    below_ms: u32,
    shown: bool,
    opacity: f32,
}

impl Gate {
    pub fn new(cfg: GateConfig) -> Self {
        Gate { cfg, above_ms: 0, below_ms: 0, shown: false, opacity: 0.0 }
    }

    pub fn is_visible(&self) -> bool {
        self.opacity > 0.0
    }

    /// `rms` is linear 0.0..=1.0. Returns the opacity to draw at this frame.
    pub fn update(&mut self, rms: f32, dt_ms: u32) -> f32 {
        let dbfs = if rms > 1e-9 { 20.0 * rms.log10() } else { -200.0 };
        let above = dbfs > self.cfg.threshold_dbfs;

        if above {
            self.above_ms = self.above_ms.saturating_add(dt_ms);
            self.below_ms = 0;
        } else {
            self.below_ms = self.below_ms.saturating_add(dt_ms);
            self.above_ms = 0; // a blip must not accumulate across silence
        }

        if !self.shown && self.above_ms >= self.cfg.reveal_ms {
            self.shown = true;
        } else if self.shown && self.below_ms >= self.cfg.hide_ms {
            self.shown = false;
        }

        let step = if self.cfg.fade_ms == 0 {
            1.0
        } else {
            dt_ms as f32 / self.cfg.fade_ms as f32
        };
        let target = if self.shown { 1.0 } else { 0.0 };
        if self.opacity < target {
            self.opacity = (self.opacity + step).min(target);
        } else if self.opacity > target {
            self.opacity = (self.opacity - step).max(target);
        }
        self.opacity = self.opacity.clamp(0.0, 1.0);
        self.opacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: u32 = 16; // ~60fps

    fn loud() -> f32 {
        0.2 // about -14 dBFS
    }
    fn silent() -> f32 {
        0.0
    }

    fn run(g: &mut Gate, rms: f32, ms: u32) -> f32 {
        let mut last = 0.0;
        for _ in 0..(ms / FRAME) {
            last = g.update(rms, FRAME);
        }
        last
    }

    #[test]
    fn starts_hidden() {
        let g = Gate::new(GateConfig::default());
        assert!(!g.is_visible());
    }

    #[test]
    fn does_not_reveal_before_the_delay_elapses() {
        let mut g = Gate::new(GateConfig::default());
        run(&mut g, loud(), 300);
        assert!(!g.is_visible(), "must still be hidden 300ms into a 400ms delay");
    }

    #[test]
    fn reveals_after_sustained_audio() {
        let mut g = Gate::new(GateConfig::default());
        run(&mut g, loud(), 500);
        assert!(g.is_visible());
        let op = run(&mut g, loud(), 400);
        assert!((op - 1.0).abs() < 0.01, "should reach full opacity, got {op}");
    }

    #[test]
    fn a_notification_ding_does_not_reveal_it() {
        // THE requirement: a 200ms blip must never blank the weather.
        let mut g = Gate::new(GateConfig::default());
        run(&mut g, loud(), 200);
        run(&mut g, silent(), 100);
        assert!(!g.is_visible(), "a 200ms ding must be ignored entirely");
    }

    #[test]
    fn several_separated_dings_still_do_not_reveal_it() {
        let mut g = Gate::new(GateConfig::default());
        for _ in 0..5 {
            run(&mut g, loud(), 150);
            run(&mut g, silent(), 600);
        }
        assert!(!g.is_visible(), "the above-threshold timer must reset on silence");
    }

    #[test]
    fn rides_through_the_gap_between_tracks() {
        let mut g = Gate::new(GateConfig::default());
        run(&mut g, loud(), 600);
        assert!(g.is_visible());
        run(&mut g, silent(), 1500); // typical inter-track gap
        assert!(g.is_visible(), "1.5s of silence must not hide it (2s threshold)");
    }

    #[test]
    fn hides_after_sustained_silence() {
        let mut g = Gate::new(GateConfig::default());
        run(&mut g, loud(), 600);
        run(&mut g, silent(), 2100);
        run(&mut g, silent(), 300); // allow the fade to finish
        assert!(!g.is_visible(), "should be fully hidden after 2s + fade");
    }

    #[test]
    fn quiet_passages_below_threshold_are_treated_as_silence() {
        let mut g = Gate::new(GateConfig::default());
        // -60 dBFS, below the -55 threshold
        let very_quiet = 10f32.powf(-60.0 / 20.0);
        run(&mut g, very_quiet, 1000);
        assert!(!g.is_visible());
    }

    #[test]
    fn opacity_is_always_in_range() {
        let mut g = Gate::new(GateConfig::default());
        for i in 0..500 {
            let op = g.update(if i % 60 < 30 { loud() } else { silent() }, FRAME);
            assert!((0.0..=1.0).contains(&op), "opacity {op} out of range");
        }
    }
}
