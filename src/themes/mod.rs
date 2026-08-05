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

/// Human-readable name for a family, for the theme menu's submenu titles.
///
/// Falls back to a title-cased version of the raw family id rather than skipping or
/// panicking on an unknown one, so a family added by a TOML file - or a new built-in whose
/// label nobody remembered to add here - still appears in the menu with a readable name.
/// The standing requirement is that themes stay expandable; a lookup that drops unknown
/// families would quietly break that.
pub fn family_label(family: &str) -> String {
    match family {
        "segmented" => "Segmented VFD".into(),
        "scope" => "Oscilloscope".into(),
        "vu" => "VU dials".into(),
        "vapor" => "Vaporwave grid".into(),
        "tube" => "Valve row".into(),
        other => {
            let mut c = other.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => "Other".into(),
            }
        }
    }
}

/// Material colours for the vacuum-tube row family.
///
/// Separate from the `lit`/`hot`/`panel` trio because a valve is made of several materials
/// that are not variations of one accent colour: glass, bakelite, brass and the dark metal
/// of the plate all have to be independently settable or every colourway ends up looking
/// like the same tube under a different gel.
#[derive(Debug, Clone)]
pub struct TubeParams {
    /// Top of the chassis gradient - the lit edge of the metal.
    pub chassis_top: String,
    pub chassis_bottom: String,
    /// Plate and grid metal, silhouetted against the glow. Dark on purpose.
    pub internals: String,
    /// Bakelite socket the envelope sits in.
    pub socket: String,
    /// Brass collar and pins.
    pub collar: String,
    /// Specular highlight on the envelope.
    pub glass: String,
}

impl Default for TubeParams {
    fn default() -> Self {
        TubeParams {
            chassis_top: "#3c4436".into(),
            chassis_bottom: "#161a12".into(),
            internals: "#0b0d08".into(),
            socket: "#241a10".into(),
            collar: "#8a6a2a".into(),
            glass: "#cfe0d8".into(),
        }
    }
}

/// Scene parameters for the vaporwave grid family.
///
/// Every default here was tuned by the user in a live browser tuner, not chosen by me - see
/// `docs/superpowers/specs/2026-07-31-vaporwave-grid-family-design.md` §1 for the raw
/// output and §7 for which of my own guesses it overrode. The tuner reported percentages
/// and hundredths; these are the same numbers as fractions, so `amp = 101` there is 1.01
/// here.
///
/// These names are a published schema the moment a theme file sets one, so they match the
/// spec exactly and must not be renamed without bumping `schema`.
#[derive(Debug, Clone)]
pub struct VaporParams {
    /// Horizon height as a fraction of panel height.
    pub horizon: f32,
    /// Displacement scale for the terrain.
    pub amp: f32,
    /// Receding horizontal grid lines.
    pub lines: i32,
    /// Converging vertical lines.
    pub verts: i32,
    /// Scroll speed.
    pub scroll: f32,
    /// Depth-spacing exponent. Higher bunches lines toward the horizon.
    pub persp: f32,
    /// Width spread of the near edge.
    pub spread: f32,
    /// Peak-glow brightness.
    pub glow: f32,
    /// Spectral smoothing, 0..1. Higher gives rolling hills over spikes.
    pub smoothing: f32,
    /// Sun radius, as a fraction of the base radius.
    pub sun: f32,
    /// Horizontal slots cut in the sun.
    pub slots: i32,
    /// Slot widening toward the horizon. The user chose ZERO - uniform 1px slots.
    pub slot_bias: f32,
    /// How far down the sun the first slot sits.
    pub slot_top: f32,
    /// Halo strength.
    pub halo: f32,
    /// Gradient warmth; higher is pinker at the horizon.
    pub warmth: f32,
    /// Rise in bass needed to fire a bolt.
    pub bolt_sens: f32,
    pub bolt_bright: f32,
    pub sky_flash: f32,
    pub grid_flash: f32,
    pub bolt_decay: f32,
    /// Hidden-line removal. Off looks like overlapping spaghetti; see the family docs.
    pub occlusion: bool,
    pub crisp: bool,
    pub sun_rim: bool,
    /// True: the grid recedes, and the newest audio lands on the NEAREST lines.
    ///
    /// This is the difference between a terrain that reacts and one that does not, and the two
    /// properties are inseparable. Displacement scales with depth, so the nearest lines move most -
    /// and a line only keeps the spectrum from when it was born. With the grid flowing toward the
    /// viewer, new audio is born at the horizon, where it is rendered at the SMALLEST displacement,
    /// and it then needs a full scroll cycle - about 1.3s - to reach the front where it would be
    /// biggest. Both penalties at once, which reads as a calm grid that lags the music.
    ///
    /// Receding puts the newest spectrum on the nearest, largest lines immediately. The cost is the
    /// direction of travel: set this false for the classic flying-forward look, and accept that the
    /// front of the grid shows what the music did a second ago.
    pub recede: bool,
    pub sky_top: String,
    pub sky_horizon: String,
    pub ground: String,
    pub sun_crown: String,
    pub sun_upper: String,
    pub sun_lower: String,
    pub sun_base: String,
}

impl Default for VaporParams {
    fn default() -> Self {
        VaporParams {
            horizon: 0.48,
            // amp, lines and persp are the three values re-tuned from the tuner's output.
            //
            // The tuner ran in a browser canvas far taller than 60px, and all three crush
            // at this size. Measured at the tuner's amp=1.01/lines=16/persp=2.07: SEVEN of
            // the sixteen lines collapsed onto just two pixel rows (28 and 29), so the far
            // half of the grid was a solid band rather than a receding grid - and that is
            // also why hidden-line removal had no measurable effect, since lines sharing an
            // integer row cannot occlude one another.
            //
            // persp 1.40 is the highest value at which the STATIC grid keeps one row per
            // line at every scroll phase (2.07 manages 88%). lines 12 keeps the smallest
            // static gap at ~0.9px. amp 0.55 puts the peak displacement near 7.6px - about
            // two to three line gaps - where 1.01 gave 13.9px, half the entire 29px ground.
            //
            // The FORMULAS are left exactly as the spec published them, so `amp = 1.01`
            // still means what the spec says it means; only these defaults are adapted, and
            // they remain overridable per colourway from TOML.
            // 0.55 was chosen when band levels were fed in RAW, and real music only spans
            // about 0.15-0.65 of the 0..1 range - so the nearest ridge moved 1.1px to 4.9px on
            // a 29px ground while the static gap between grid lines is already 0.9-3.2px. On a
            // quiet passage the terrain moved less than one line gap, which is a flat grid.
            // Now that the terrain auto-ranges, the loudest band reliably reaches the top of
            // its displacement range, so this sets how much relief that buys: ~14.5px, half the
            // visible ground. 1.50 was measurably denser without reading as more responsive.
            amp: 1.15,
            // 12 lines made the audio unreadable, and the arithmetic says why: at 12 the near-field
            // gap between lines is 3.4px while the peak displacement is 15.8px, so a ridge crosses
            // about five lines and stops being distinguishable from the grid's own density. The
            // terrain became a hatch that jiggles rather than hills that rise. At 9 the ratio is
            // 3.5x and a ridge reads as a ridge.
            //
            // 9 lines / scroll 2.6 / amp 1.15 is the combination the user picked by eye, from four
            // variants compared live in the running app (kept in docs/reference/theme-variants).
            // Do not change these three without offering that comparison again - it is a taste
            // call on a real trade, not a value that can be derived.
            //
            // This costs resolution in the DEPTH axis only. The 64 log frequency bands across each
            // line are untouched, so the spectrum's fidelity is unchanged - it is the number of
            // historical snapshots on screen that drops.
            lines: 9,
            verts: 18,
            // 1.24 gave a near-field flow of 0.50 px/frame, which is below the rate at which motion
            // reads as motion - the grid crawled. It also meant a line was born only every 6.7
            // frames, so the terrain sampled audio at 8.9Hz and roughly 93% of what was on screen
            // was frozen history at any instant. This is the fourth value from the browser tuner
            // that did not survive 60px, after amp, persp and lines; the tuner's canvas was far
            // larger, so the same phase rate looked far faster there.
            scroll: 2.6,
            persp: 1.40,
            spread: 1.50,
            glow: 0.98,
            // 0.65 was a radius-3 moving average over 64 bands, which is a lot of averaging: it
            // was chosen to stop the ridges reading as spaghetti, but it flattens exactly the peaks
            // that make the terrain look like it is reacting. Lowering it SHARPENS the ridges and
            // raises fidelity at the same time, which is the direction asked for.
            smoothing: 0.40,
            sun: 0.83,
            slots: 6,
            slot_bias: 0.0,
            slot_top: 0.18,
            halo: 0.84,
            warmth: 0.63,
            bolt_sens: 0.55,
            bolt_bright: 0.90,
            sky_flash: 0.35,
            grid_flash: 0.60,
            bolt_decay: 0.55,
            occlusion: true,
            crisp: true,
            sun_rim: true,
            recede: true,
            sky_top: "#1a0b2e".into(),
            sky_horizon: "#ff5f93".into(),
            ground: "#12061f".into(),
            sun_crown: "#fff6d0".into(),
            sun_upper: "#ffd76e".into(),
            sun_lower: "#ff9c4a".into(),
            sun_base: "#ff5f93".into(),
        }
    }
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
    /// Vaporwave-only scene parameters; inert for the other families.
    pub vapor: VaporParams,
    /// Tube-row-only material colours; inert for the other families.
    pub tube: TubeParams,
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
            vapor: VaporParams::default(),
            tube: TubeParams::default(),
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
