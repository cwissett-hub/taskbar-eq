use crate::dsp::gate::GateConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Key bindings for the Spotify transport controls.
///
/// Strings, and empty by default. Nothing is bound until the user asks for it: `RegisterHotKey` is
/// exclusive and first-come, so a default binding would seize keys machine-wide from the moment this
/// ornament starts - and it can autostart, so it would usually win that race at logon. The first bug
/// report would be "the media keys broke my YouTube", with no reason to connect it to a taskbar
/// visualiser.
///
/// The inner `serde(default)` is load-bearing, not cosmetic: without it a `[hotkeys]` table that is
/// missing one key fails the WHOLE document, `Config::load` falls back to `Config::default()`, and
/// the user silently loses their theme, width and every timing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Hotkeys {
    pub play_pause: String,
    pub next_track: String,
    pub prev_track: String,
}

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
    /// Display width in physical pixels, measured leftward from the Widgets button's right edge.
    ///
    /// Customisable because the amount of dead taskbar available depends entirely on how many
    /// apps are pinned and open, which is per-machine and changes minute to minute. This is a
    /// REQUEST, not a guarantee: `placement::widened` clamps it to whatever clearance actually
    /// exists, because the overlay receives its own clicks and so cannot be allowed to cover a
    /// pinned button.
    pub width: i32,
    pub autostart: bool,
    /// Which mechanism sends transport commands. See `win::media::Backend`.
    pub media_backend: crate::win::media::Backend,
    /// Declared LAST because `toml` emits tables after root scalars; keeping struct order and file
    /// order the same is what lets the existing round-trip test keep proving serialisation works.
    pub hotkeys: Hotkeys,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "vfd-ice".into(),
            brightness: 1.0,
            saturation: 1.0,
            threshold_dbfs: -55.0,
            reveal_ms: 400,
            hide_ms: 4500,
            fade_ms: 450,
            // Roughly double the ~190px the Widgets button occupies. On the development
            // machine there are 352px of empty taskbar between the last pinned app and the
            // widget, so this fits with room to spare and shrinks automatically when it does
            // not.
            width: 380,
            autostart: false,
            media_backend: crate::win::media::Backend::default(),
            hotkeys: Hotkeys::default(),
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
    use std::sync::Mutex;

    // Every test below that touches the real Config::path() file (the
    // round-trip test plus the two load()-against-a-real-file tests added
    // for the corrupt/missing-file finding) shares that one file on disk.
    // A plain `cargo test` runs tests in the same binary in parallel by
    // default, so without serialising them one test's write/restore can
    // race another's - the exact failure mode already fixed for the
    // registry in win::autostart::tests. Lock for the duration of any test
    // that reads or writes Config::path().
    static CONFIG_FILE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn defaults_match_the_spec() {
        let c = Config::default();
        assert_eq!(c.threshold_dbfs, -55.0);
        assert_eq!(c.reveal_ms, 400);
        assert_eq!(c.hide_ms, 4500);
        assert_eq!(c.fade_ms, 450);
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
        assert_eq!(c.hide_ms, 4500, "missing keys must take defaults");
    }

    #[test]
    fn a_corrupt_file_does_not_panic() {
        // This only pins down that the `toml` crate itself returns `Err` for
        // garbage input - a property of that crate, not of this codebase's
        // error handling. It does NOT exercise Config::load() at all; see
        // `load_falls_back_to_defaults_on_a_real_corrupt_file` below for a
        // test that actually calls load() against a corrupt file on disk.
        assert!(toml::from_str::<Config>("this is not toml {{{").is_err());
    }

    /// Drives the actual requirement ("a bad config must not stop the app
    /// starting") through the real function against the real path: writes
    /// garbage bytes to Config::path(), calls Config::load(), and asserts it
    /// returns Config::default() without panicking. Self-restoring like
    /// `save_then_load_round_trips_through_the_real_filesystem`.
    #[test]
    fn load_falls_back_to_defaults_on_a_real_corrupt_file() {
        let _guard = CONFIG_FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = Config::path();
        let backup = std::fs::read_to_string(&path).ok();

        std::fs::create_dir_all(Config::dir()).expect("dir should be creatable");
        std::fs::write(&path, "this is not toml {{{").expect("writing garbage should succeed");

        assert_eq!(
            Config::load(),
            Config::default(),
            "load() must fall back to defaults instead of panicking on a corrupt file"
        );

        match backup {
            Some(original) => {
                std::fs::write(&path, original).expect("restoring the original config must succeed");
            }
            None => {
                std::fs::remove_file(&path).ok();
            }
        }
    }

    /// Same requirement, missing-file case: delete whatever is at
    /// Config::path() and confirm load() still returns defaults rather than
    /// propagating the I/O error.
    #[test]
    fn load_falls_back_to_defaults_on_a_missing_file() {
        let _guard = CONFIG_FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = Config::path();
        let backup = std::fs::read_to_string(&path).ok();

        std::fs::remove_file(&path).ok();
        assert!(!path.exists(), "precondition: file must actually be gone");

        assert_eq!(
            Config::load(),
            Config::default(),
            "load() must fall back to defaults instead of panicking on a missing file"
        );

        if let Some(original) = backup {
            std::fs::create_dir_all(Config::dir()).expect("dir should be creatable");
            std::fs::write(&path, original).expect("restoring the original config must succeed");
        }
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
        let _guard = CONFIG_FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
