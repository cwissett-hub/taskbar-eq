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
    /// Shuffle to any colourway in any family.
    pub random_theme: String,
    /// Shuffle within the family already showing.
    pub random_colourway: String,
    /// Fire the current family's flourish right now, without waiting for an exceptional hit.
    pub flourish: String,
    /// Turn flourishes on or off, persisted across restarts.
    pub flourish_toggle: String,
    /// Identify the current song with Shazam and show it in the banner.
    pub identify_song: String,
    /// Show the current track name in the banner, the same way a track change does.
    pub show_now_playing: String,
}

impl Hotkeys {
    /// The chord text for slot `i`, in the same order as `hotkeys::Slot::ALL`.
    ///
    /// **Indexed accessors rather than a `match` at each call site.** The capture-dialog handler in
    /// `main` used `match i { 0 => .., 3 => .., _ => random_colourway }`, and a catch-all arm over an
    /// index is a trap: adding a sixth slot silently aliased it onto the fifth field, so binding the
    /// new action would have overwritten an existing one and the bug would look like "my shuffle key
    /// changed by itself". Returning `None` past the end cannot alias.
    pub fn slot(&self, i: usize) -> Option<&str> {
        Some(match i {
            0 => &self.play_pause,
            1 => &self.next_track,
            2 => &self.prev_track,
            3 => &self.random_theme,
            4 => &self.random_colourway,
            5 => &self.flourish,
            6 => &self.flourish_toggle,
            7 => &self.identify_song,
            8 => &self.show_now_playing,
            _ => return None,
        })
    }

    pub fn slot_mut(&mut self, i: usize) -> Option<&mut String> {
        Some(match i {
            0 => &mut self.play_pause,
            1 => &mut self.next_track,
            2 => &mut self.prev_track,
            3 => &mut self.random_theme,
            4 => &mut self.random_colourway,
            5 => &mut self.flourish,
            6 => &mut self.flourish_toggle,
            7 => &mut self.identify_song,
            8 => &mut self.show_now_playing,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub theme: String,
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
    /// Show the track name for a couple of seconds when the track changes.
    pub show_track_name: bool,
    /// Whether family flourishes happen at all.
    ///
    /// A global on/off, separate from each colourway's `flourish` RATE - so turning them off does not
    /// discard the per-colourway tuning, and turning them back on restores it exactly. Persisted so a
    /// toggle survives a restart, because a setting that silently resets is worse than no setting.
    pub flourishes: bool,
    /// The most recently selected themes, newest first, including the current one.
    ///
    /// Exists because the menu is 163 colourways across 25 families, and getting BACK to one you liked is
    /// the common case rather than discovering a new one. The tray menu lists these above the families.
    ///
    /// An array of strings rather than a table, so it can sit here among the scalars without disturbing
    /// the emit order `hotkeys` depends on.
    pub recents: Vec<String>,
    /// Declared LAST because `toml` emits tables after root scalars; keeping struct order and file
    /// order the same is what lets the existing round-trip test keep proving serialisation works.
    pub hotkeys: Hotkeys,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "vfd-ice".into(),
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
            show_track_name: true,
            flourishes: true,
            recents: Vec::new(),
            hotkeys: Hotkeys::default(),
        }
    }
}

impl Config {
    pub fn dir() -> PathBuf {
        // A test-only override so the suite never reads or writes the developer's
        // real %APPDATA%\taskbar-eq. Empty is treated as unset, so exporting it
        // blank cannot accidentally point the config at the current directory.
        if let Ok(d) = std::env::var("TASKBAR_EQ_CONFIG_DIR") {
            if !d.is_empty() {
                return PathBuf::from(d);
            }
        }
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
                // The exe is a GUI-subsystem binary, so `eprintln!` goes nowhere;
                // the app logger is the only channel a support request can read.
                crate::log::write(&format!("config: {e}; using defaults"));
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    /// How many recent themes are remembered.
    ///
    /// Six is about a screen of menu at this row height. Past that a "recent" list stops being a
    /// shortcut and becomes a second, worse copy of the theme list.
    pub const RECENTS_MAX: usize = 6;

    /// Records a theme as the current one AND as the most recent.
    ///
    /// One function rather than an assignment at each call site, because there are four sites - the menu,
    /// the menu's shuffle, the shuffle hotkey and the startup resolve - and a recents list that three of
    /// the four forgot to update would be worse than no list at all: it would look like it was working.
    pub fn note_theme(&mut self, id: &str) {
        self.theme = id.to_string();
        self.recents.retain(|r| r != id);
        self.recents.insert(0, id.to_string());
        self.recents.truncate(Self::RECENTS_MAX);
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(Self::dir())?;
        let tmp = Self::dir().join("config.toml.tmp");
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        // Rename is atomic on NTFS, so a crash mid-write can only lose the .tmp, never the config.
        std::fs::rename(&tmp, Self::path())?;
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

/// The one lock that serialises every test - in ANY module - that reads or
/// writes `Config::dir()`/`Config::path()` or the `TASKBAR_EQ_CONFIG_DIR`
/// override. The override is process-global and `cargo test` runs the binary's
/// tests in parallel, so a test that touches the config directory without
/// holding this can race a `config::tests` temp-dir closure and land its files
/// in another test's directory. `pub(crate)` so `themes::tests` (which reads
/// `Config::dir()`) shares the one discipline rather than inventing its own.
#[cfg(test)]
pub(crate) static CONFIG_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    // Config lives at a fixed path derived from the environment, and several
    // tests below need to read and write it without touching the developer's
    // real %APPDATA%\taskbar-eq\config.toml. `Config::dir` honours the
    // `TASKBAR_EQ_CONFIG_DIR` override, and this helper points it at a fresh,
    // per-test temp dir for the duration of the closure.
    //
    // Environment variables are PROCESS-GLOBAL, and `cargo test` runs the
    // tests in one binary in parallel by default, so every test that reads or
    // writes the override - or that reads `Config::path()`/`Config::dir()` and
    // would be confused by an override another test set - must hold
    // `CONFIG_TEST_LOCK` for as long as it cares about the value.
    fn with_temp_dir<T>(name: &str, f: impl FnOnce(&std::path::Path) -> T) -> T {
        let _g = CONFIG_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("taskbar-eq-cfg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("TASKBAR_EQ_CONFIG_DIR", &dir);
        let r = f(&dir);
        std::env::remove_var("TASKBAR_EQ_CONFIG_DIR");
        let _ = std::fs::remove_dir_all(&dir);
        r
    }

    #[test]
    fn dir_honours_the_test_override() {
        with_temp_dir("dir", |d| assert_eq!(Config::dir(), d));
    }

    #[test]
    fn save_is_atomic_and_leaves_no_tmp_behind() {
        with_temp_dir("atomic", |d| {
            let mut c = Config::default();
            c.width = 123;
            c.save().unwrap();
            assert!(d.join("config.toml").exists());
            assert!(!d.join("config.toml.tmp").exists());
            assert_eq!(Config::load().width, 123);
        });
    }

    #[test]
    fn a_stale_tmp_does_not_shadow_the_real_file() {
        with_temp_dir("stale", |d| {
            let mut c = Config::default();
            c.width = 321;
            c.save().unwrap();
            std::fs::write(d.join("config.toml.tmp"), "width = 1\n").unwrap();
            assert_eq!(Config::load().width, 321);
            c.save().unwrap();
            assert!(!d.join("config.toml.tmp").exists());
        });
    }

    #[test]
    fn an_unknown_media_backend_does_not_lose_the_rest_of_the_config() {
        with_temp_dir("backend", |d| {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join("config.toml"), "theme = \"vu-cream\"\nwidth = 300\nmedia_backend = \"nonsense\"\n").unwrap();
            let c = Config::load();
            assert_eq!(c.theme, "vu-cream");
            assert_eq!(c.width, 300);
            assert_eq!(c.media_backend, crate::win::media::Backend::Session);
        });
    }

    #[test]
    fn a_v020_config_without_the_new_keys_loads_intact() {
        with_temp_dir("v020", |d| {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join("config.toml"), "theme = \"tube-soviet\"\nwidth = 380\n[hotkeys]\nplay_pause = \"Win+Ctrl+Space\"\n").unwrap();
            let c = Config::load();
            assert_eq!(c.theme, "tube-soviet");
            assert_eq!(c.hotkeys.play_pause, "Win+Ctrl+Space");
            assert_eq!(c.hotkeys.identify_song, "");
        });
    }

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
        // A non-default value on a field that IS read, so the round trip proves something. This used
        // `brightness`, which was removed - it was parsed and saved and read by nothing, so the test was
        // round-tripping a field whose value could never matter.
        c.hide_ms = 6000;
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
    fn a_v0_3_2_config_without_the_show_now_playing_key_still_loads() {
        // A real [hotkeys] table as v0.3.2 would have written it - seven keys, no
        // `show_now_playing`, because that slot did not exist yet. `#[serde(default)]` on
        // `Hotkeys` is what makes this load instead of falling back to `Config::default()` and
        // silently discarding the user's theme, width and every other setting alongside it.
        let s = r#"
theme = "vfd-ice"
threshold_dbfs = -55.0
reveal_ms = 400
hide_ms = 4500
fade_ms = 450
width = 380
autostart = false
media_backend = "session"
show_track_name = true
flourishes = true
recents = []

[hotkeys]
play_pause = "Win+Ctrl+Space"
next_track = "Win+Ctrl+Period"
prev_track = "Win+Ctrl+Comma"
random_theme = ""
random_colourway = ""
flourish = ""
flourish_toggle = ""
identify_song = "Win+Ctrl+I"
"#;
        let c: Config = toml::from_str(s).expect("an old config missing one key must still parse");
        assert_eq!(c.hotkeys.play_pause, "Win+Ctrl+Space");
        assert_eq!(c.hotkeys.identify_song, "Win+Ctrl+I");
        assert_eq!(
            c.hotkeys.show_now_playing, "",
            "the new slot must default to unbound, not alias another field"
        );
        assert_eq!(c.hotkeys.slot(crate::win::hotkeys::SLOTS - 1), Some(""));
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
        with_temp_dir("corrupt", |d| {
            std::fs::create_dir_all(d).expect("dir should be creatable");
            std::fs::write(d.join("config.toml"), "this is not toml {{{")
                .expect("writing garbage should succeed");
            assert_eq!(
                Config::load(),
                Config::default(),
                "load() must fall back to defaults instead of panicking on a corrupt file"
            );
        });
    }

    /// Same requirement, missing-file case: confirm load() returns defaults
    /// rather than propagating the I/O error when nothing is on disk.
    #[test]
    fn load_falls_back_to_defaults_on_a_missing_file() {
        with_temp_dir("missing", |d| {
            assert!(!d.join("config.toml").exists(), "precondition: file must actually be absent");
            assert_eq!(
                Config::load(),
                Config::default(),
                "load() must fall back to defaults instead of panicking on a missing file"
            );
        });
    }

    #[test]
    fn gate_config_is_derived_from_the_file() {
        let mut c = Config::default();
        c.reveal_ms = 900;
        assert_eq!(c.gate_config().reveal_ms, 900);
    }

    #[test]
    fn config_lives_under_appdata() {
        // Reads Config::path(), so it must hold the lock and clear any override
        // a parallel test left set, or it could observe a temp dir instead.
        let _g = CONFIG_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("TASKBAR_EQ_CONFIG_DIR");
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
        with_temp_dir("roundtrip", |d| {
            let mut c = Config::default();
            c.theme = "round-trip-test".into();
            c.save().expect("save() should be able to create the config dir");

            assert!(d.join("config.toml").exists(), "save() must leave a real, readable config.toml behind");
            assert_eq!(Config::load(), c, "load() must read back exactly what save() wrote");
        });
    }
}

#[cfg(test)]
mod slot_tests {
    use super::*;

    #[test]
    fn every_hotkey_slot_has_its_own_field_and_none_alias() {
        // THE guard on the trap this accessor replaced. `main` used
        // `match i { 0 => .., 3 => .., _ => random_colourway }`, so a sixth slot would have written
        // its chord into the fifth field - binding the new key would silently overwrite the shuffle
        // key, and the report would be "my shuffle key changed by itself".
        //
        // Writing a distinct value through every slot and reading them all back is the only check that
        // catches aliasing, because an aliased pair still round-trips individually.
        let mut h = Hotkeys::default();
        for i in 0..crate::win::hotkeys::SLOTS {
            let field = h.slot_mut(i).unwrap_or_else(|| panic!("slot {i} has no field"));
            *field = format!("KEY{i}");
        }
        for i in 0..crate::win::hotkeys::SLOTS {
            assert_eq!(
                h.slot(i),
                Some(format!("KEY{i}").as_str()),
                "slot {i} does not read back what was written - two slots share a field"
            );
        }
        // And nothing past the end, so an out-of-range menu index is a no-op rather than a write to
        // whichever field happened to be last.
        assert_eq!(h.slot(crate::win::hotkeys::SLOTS), None);
        assert!(h.slot_mut(crate::win::hotkeys::SLOTS).is_none());
        assert_eq!(h.slot(999), None);
    }
}
