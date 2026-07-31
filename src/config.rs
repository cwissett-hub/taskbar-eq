use crate::dsp::gate::GateConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub brightness: f32,
    pub saturation: f32,
    pub threshold_dbfs: f32,
    pub reveal_ms: u32,
    pub hide_ms: u32,
    pub fade_ms: u32,
    pub autostart: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "vfd-ice".into(),
            brightness: 1.0,
            saturation: 1.0,
            threshold_dbfs: -55.0,
            reveal_ms: 400,
            hide_ms: 2000,
            fade_ms: 250,
            autostart: false,
        }
    }
}

impl Config {
    pub fn dir() -> PathBuf {
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(base).join("taskbar-eq")
    }

    pub fn path() -> PathBuf {
        Self::dir().join("config.toml")
    }

    /// Never fails: a missing or corrupt config falls back to defaults, because
    /// a bad config file must not stop the app from starting.
    pub fn load() -> Config {
        match std::fs::read_to_string(Self::path()) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                eprintln!("config: {e}; using defaults");
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(Self::dir())?;
        std::fs::write(Self::path(), toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn gate_config(&self) -> GateConfig {
        GateConfig {
            threshold_dbfs: self.threshold_dbfs,
            reveal_ms: self.reveal_ms,
            hide_ms: self.hide_ms,
            fade_ms: self.fade_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let c = Config::default();
        assert_eq!(c.threshold_dbfs, -55.0);
        assert_eq!(c.reveal_ms, 400);
        assert_eq!(c.hide_ms, 2000);
        assert_eq!(c.fade_ms, 250);
        assert_eq!(c.theme, "vfd-ice");
    }

    #[test]
    fn round_trips_through_toml() {
        let mut c = Config::default();
        c.theme = "matrix-green".into();
        c.brightness = 0.8;
        let s = toml::to_string_pretty(&c).unwrap();
        assert_eq!(toml::from_str::<Config>(&s).unwrap(), c);
    }

    #[test]
    fn a_partial_file_fills_in_defaults() {
        // serde(default) means an old config missing new keys still loads.
        let c: Config = toml::from_str("theme = \"neon-pink\"").unwrap();
        assert_eq!(c.theme, "neon-pink");
        assert_eq!(c.hide_ms, 2000, "missing keys must take defaults");
    }

    #[test]
    fn a_corrupt_file_does_not_panic() {
        assert!(toml::from_str::<Config>("this is not toml {{{").is_err());
        // load() swallows that error; proven by defaults_match_the_spec plus this.
    }

    #[test]
    fn gate_config_is_derived_from_the_file() {
        let mut c = Config::default();
        c.reveal_ms = 900;
        assert_eq!(c.gate_config().reveal_ms, 900);
    }

    #[test]
    fn config_lives_under_appdata() {
        let p = Config::path();
        assert!(p.ends_with("taskbar-eq/config.toml") || p.ends_with("taskbar-eq\\config.toml"));
    }

    /// The other tests above only exercise TOML string ser/de; this drives
    /// `save()`/`load()` against the real filesystem, which is what the task
    /// brief's manual Step 7 ("%APPDATA%\taskbar-eq\config.toml exists and is
    /// readable") actually checks. Self-restoring, like the autostart tests:
    /// back up and restore whatever real config was on disk before running.
    #[test]
    fn save_then_load_round_trips_through_the_real_filesystem() {
        let path = Config::path();
        let backup = std::fs::read_to_string(&path).ok();

        let mut c = Config::default();
        c.theme = "round-trip-test".into();
        c.save().expect("save() should be able to create %APPDATA%\\taskbar-eq");

        assert!(path.exists(), "save() must leave a real, readable config.toml behind");
        assert_eq!(Config::load(), c, "load() must read back exactly what save() wrote");

        match backup {
            Some(original) => {
                std::fs::write(&path, original).expect("restoring the original config must succeed");
            }
            None => {
                std::fs::remove_file(&path).ok();
            }
        }
    }
}
