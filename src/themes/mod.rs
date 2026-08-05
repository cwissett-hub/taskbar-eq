pub mod builtin;
pub mod schema;
pub mod watch;

use crate::dsp::ballistics::Ballistics;

// Only `builtin::vfd_ice` exists so far, and it uses `Glass`. The other
// panel-texture variants are exercised by the remaining four segmented
// colourways from the reference mockup (Matrix Green/scanlines, Neon
// Pink/haze, Vac Tube Orange/filament, Classic Three-Colour/grille) - not
// yet ported into `builtin::all`.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Texture {
    Glass,
    Scanlines,
    Haze,
    Filament,
    Grille,
    None_,
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub upto: f32,
    pub lit: String,
    pub hot: String,
}

#[derive(Debug, Clone)]
pub struct Theme {
    // `id`/`name` identify a theme for a future theme-picker UI; nothing
    // reads them yet since only one theme (`vfd_ice`) is ever selected.
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub name: String,
    pub family: String,
    pub lit: String,
    pub hot: String,
    pub panel: String,
    pub panel_alpha: f32,
    pub edge: String,
    pub edge_alpha: f32,
    pub ghost: f32,
    pub bloom: f32,
    /// Halo intensity, independent of `bloom` (which is the radius). Kept separate
    /// because the two are not interchangeable: the radius must stay small relative
    /// to the bar pitch or adjacent halos merge at ANY strength, so brightness has
    /// to come from here.
    pub glow_strength: f32,
    /// Strength of the dim halo confined to the display's edge ring, as a fraction
    /// of `glow_strength`.
    pub edge_glow: f32,
    /// Display gain applied to the audio level before it is mapped to the meter.
    ///
    /// Exists because the raw values are far smaller than they look: typical music sits
    /// at an RMS of 0.02-0.12, so feeding that straight to a needle put it at 2-12% of
    /// arc travel and the meter barely moved. Each family applies this on top of its own
    /// sane default mapping, so 1.0 is already usable and this is the knob to reach for
    /// if a meter feels dead.
    pub sensitivity: f32,
    // Cross-fade duration for switching themes at runtime (Task 11+); the
    // segmented renderer draws every frame from scratch and has no
    // transition state to feed it yet.
    #[allow(dead_code)]
    pub fade: f32,
    pub texture: Texture,
    pub ballistics: Ballistics,
    pub zones: Vec<Zone>,
    /// (trail colour, trail fade) - scope family only, models a dual-layer phosphor.
    #[allow(dead_code)]
    pub dual: Option<(String, f32)>,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            id: "unnamed".into(),
            name: "Unnamed".into(),
            family: "segmented".into(),
            lit: "#8fe4ff".into(),
            hot: "#e4f8ff".into(),
            panel: "#040a0e".into(),
            // FULLY opaque. Not a preference: the panel must occlude the Widgets
            // button's own icon and text. 0.55 left the weather plainly legible; even
            // 0.96 transmitted 4% of it, which is invisible against a lit bar but
            // clearly visible against the dark segment gaps and the dormant grid.
            panel_alpha: 1.0,
            edge: "#96e1ff".into(),
            edge_alpha: 0.13,
            ghost: 0.11,
            // Radius, NOT intensity. Must stay small relative to the bar pitch or
            // adjacent halos merge into one wash behind the segments.
            bloom: 4.0,
            glow_strength: 0.35,
            // 0.30 was not merely too dim, it was NEGATIVE - the ring measured darker
            // than the panel it sat on. 4.0 puts it ~60 luminance above.
            edge_glow: 4.0,
            sensitivity: 1.0,
            fade: 0.30,
            texture: Texture::Glass,
            ballistics: Ballistics::default(),
            zones: Vec::new(),
            dual: None,
        }
    }
}

impl Theme {
    /// Colour of the segment at `frac` up the bar, honouring zones if present.
    pub fn lit_at(&self, frac: f32) -> &str {
        for z in &self.zones {
            if frac <= z.upto {
                return &z.lit;
            }
        }
        self.zones.last().map(|z| z.lit.as_str()).unwrap_or(&self.lit)
    }

    pub fn hot_at(&self, frac: f32) -> &str {
        for z in &self.zones {
            if frac <= z.upto {
                return &z.hot;
            }
        }
        self.zones.last().map(|z| z.hot.as_str()).unwrap_or(&self.hot)
    }

    /// The printed overload arc. Red on every dial except the red one, where it
    /// goes white because red-on-red is illegible.
    pub fn overload_hex(&self) -> &str {
        if self.id == "vu-red" {
            "#ffffff"
        } else {
            "#ff5a46"
        }
    }
}

/// After a hot reload, decide which theme should now be selected. If `wanted`
/// still exists in `themes`, keep it - a hot reload of the theme currently on
/// screen must not silently switch anything. Otherwise fall back to the first
/// available theme rather than leaving the app pointing at an id that no
/// longer exists, which happens when the user deletes the file for the theme
/// that was selected.
pub fn reconcile_reload(themes: &[Theme], wanted: &str) -> Theme {
    themes
        .iter()
        .find(|t| t.id == wanted)
        .or_else(|| themes.first())
        .cloned()
        .unwrap_or_default()
}

/// Built-ins first, then `%APPDATA%\taskbar-eq\themes\*.toml`. An external theme
/// sharing a built-in `id` replaces it; a new `id` is appended.
pub fn registry() -> (Vec<Theme>, Vec<String>) {
    let mut themes = builtin::all();
    let dir = crate::config::Config::dir().join("themes");
    let (external, warnings) = schema::load_dir(&dir);
    for ext in external {
        match themes.iter().position(|t| t.id == ext.id) {
            Some(i) => themes[i] = ext,
            None => themes.push(ext),
        }
    }
    (themes, warnings)
}

#[cfg(test)]
mod reconcile_reload_tests {
    use super::*;

    fn theme(id: &str) -> Theme {
        Theme { id: id.into(), name: id.into(), ..Theme::default() }
    }

    #[test]
    fn keeps_the_current_theme_when_it_still_exists() {
        // `wanted` is deliberately NOT the first entry, so a stub that always
        // returns `themes.first()` cannot pass this by accident.
        let themes = vec![theme("a"), theme("b"), theme("c")];
        let picked = reconcile_reload(&themes, "b");
        assert_eq!(picked.id, "b", "the surviving theme must be kept, not swapped");
    }

    #[test]
    fn falls_back_to_the_first_theme_when_the_selected_one_is_gone() {
        let themes = vec![theme("a"), theme("b")];
        let picked = reconcile_reload(&themes, "deleted-by-the-user");
        assert_eq!(picked.id, "a", "a vanished theme must fall back rather than pointing at nothing");
    }

    #[test]
    fn an_empty_registry_falls_back_to_the_default_theme_rather_than_panicking() {
        // Defensive only: `registry()` always includes the built-ins, so this
        // is never hit in practice, but reconcile_reload must not assume it.
        let picked = reconcile_reload(&[], "anything");
        assert_eq!(picked.id, Theme::default().id);
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn an_external_theme_overrides_a_builtin_of_the_same_id() {
        let mut themes = builtin::all();
        let (external, _) = schema::load_dir(Path::new("tests/themes"));
        let before = themes.len();
        for ext in external {
            match themes.iter().position(|t| t.id == ext.id) {
                Some(i) => themes[i] = ext,
                None => themes.push(ext),
            }
        }
        let ice = themes.iter().find(|t| t.id == "vfd-ice").expect("vfd-ice present");
        assert_eq!(ice.name, "VFD Ice (mine)", "override should replace the built-in");
        assert_eq!(ice.lit, "#00ffff");
        assert!(themes.iter().any(|t| t.id == "my-purple"), "new ids are appended");
        assert!(themes.len() > before, "new themes increase the count");
        assert_eq!(
            themes.iter().filter(|t| t.id == "vfd-ice").count(),
            1,
            "override must replace, not duplicate"
        );
    }

    /// The test above drives the merge logic directly against `tests/themes`,
    /// bypassing `registry()`'s own `Config::dir().join("themes")` lookup entirely.
    /// This one exercises `registry()` itself against the real, environment-derived
    /// directory - self-restoring, like `config::tests`' real-filesystem cases.
    #[test]
    fn registry_reads_the_real_appdata_themes_directory() {
        let dir = crate::config::Config::dir().join("themes");
        std::fs::create_dir_all(&dir).expect("themes dir should be creatable");
        let marker = dir.join("__registry_test_marker.toml");
        std::fs::write(
            &marker,
            "schema = 1\nid = \"__registry-test-marker\"\nname = \"Marker\"\nfamily = \"segmented\"",
        )
        .expect("writing the marker file should succeed");

        let (themes, warnings) = registry();

        std::fs::remove_file(&marker).ok();

        assert!(warnings.is_empty(), "a valid marker file should not warn: {warnings:?}");
        assert!(
            themes.iter().any(|t| t.id == "__registry-test-marker"),
            "registry() must pick up a real file from %APPDATA%\\taskbar-eq\\themes"
        );
    }
}
