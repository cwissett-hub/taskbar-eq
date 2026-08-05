use super::{Texture, Theme, TubeParams, VaporParams, Zone};
use crate::dsp::ballistics::Ballistics;
use serde::Deserialize;
use std::fmt;
use std::path::Path;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub enum ThemeError {
    Toml(String),
    UnsupportedSchema(u32),
    MissingField(&'static str),
    BadFamily(String),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::Toml(e) => write!(f, "not valid TOML: {e}"),
            ThemeError::UnsupportedSchema(v) => write!(
                f,
                "schema = {v} is not supported (this build understands schema = {SCHEMA_VERSION})"
            ),
            ThemeError::MissingField(k) => write!(f, "missing required field `{k}`"),
            ThemeError::BadFamily(fam) => {
                // Lists the renderer's families rather than a hardcoded three, so the message
                // cannot go stale the way the gate itself did.
                write!(
                    f,
                    "unknown family `{fam}` (expected one of: {})",
                    crate::render::KNOWN_FAMILIES.join(", ")
                )
            }
        }
    }
}

#[derive(Deserialize)]
struct RawZone {
    upto: f32,
    lit: String,
    hot: String,
}

#[derive(Deserialize, Default)]
struct RawColour {
    lit: Option<String>,
    hot: Option<String>,
    panel: Option<String>,
    panel_alpha: Option<f32>,
    edge: Option<String>,
    edge_alpha: Option<f32>,
}

#[derive(Deserialize, Default)]
struct RawLook {
    ghost: Option<f32>,
    bloom: Option<f32>,
    /// Halo brightness. NOT interchangeable with `bloom`, which is the radius: a theme
    /// file that raises `bloom` expecting more glow gets LESS, because a wider blur
    /// kernel spreads the same energy thinner. This is the brightness knob.
    glow_strength: Option<f32>,
    /// Dim halo confined to the display's edge ring, as a multiple of glow_strength.
    edge_glow: Option<f32>,
    fade: Option<f32>,
    texture: Option<String>,
    /// Display gain on the audio level before it reaches the meter. Raise it if a
    /// meter feels dead: the raw values are much smaller than they look (typical music
    /// is an RMS of 0.02-0.12), which is why the VU needle and the scope trace both
    /// barely moved before this existed.
    sensitivity: Option<f32>,
}

#[derive(Deserialize, Default)]
struct RawBallistics {
    attack: Option<f32>,
    decay: Option<f32>,
    peak_fall: Option<f32>,
}

/// The `[vaporwave]` table. Every field optional, so a theme file overrides only what it
/// cares about; names match the design spec's schema exactly and must not be renamed
/// without bumping `schema`.
#[derive(Deserialize, Default)]
struct RawVapor {
    horizon: Option<f32>,
    amp: Option<f32>,
    lines: Option<i32>,
    verts: Option<i32>,
    scroll: Option<f32>,
    persp: Option<f32>,
    spread: Option<f32>,
    glow: Option<f32>,
    smoothing: Option<f32>,
    sun: Option<f32>,
    slots: Option<i32>,
    slot_bias: Option<f32>,
    slot_top: Option<f32>,
    halo: Option<f32>,
    warmth: Option<f32>,
    bolt_sens: Option<f32>,
    bolt_bright: Option<f32>,
    sky_flash: Option<f32>,
    grid_flash: Option<f32>,
    bolt_decay: Option<f32>,
    occlusion: Option<bool>,
    crisp: Option<bool>,
    sun_rim: Option<bool>,
    sky_top: Option<String>,
    sky_horizon: Option<String>,
    ground: Option<String>,
    sun_crown: Option<String>,
    sun_upper: Option<String>,
    sun_lower: Option<String>,
    sun_base: Option<String>,
}

fn vapor_from(raw: Option<RawVapor>, d: VaporParams) -> VaporParams {
    let Some(r) = raw else { return d };
    VaporParams {
        horizon: r.horizon.unwrap_or(d.horizon),
        amp: r.amp.unwrap_or(d.amp),
        lines: r.lines.unwrap_or(d.lines),
        verts: r.verts.unwrap_or(d.verts),
        scroll: r.scroll.unwrap_or(d.scroll),
        persp: r.persp.unwrap_or(d.persp),
        spread: r.spread.unwrap_or(d.spread),
        glow: r.glow.unwrap_or(d.glow),
        smoothing: r.smoothing.unwrap_or(d.smoothing),
        sun: r.sun.unwrap_or(d.sun),
        slots: r.slots.unwrap_or(d.slots),
        slot_bias: r.slot_bias.unwrap_or(d.slot_bias),
        slot_top: r.slot_top.unwrap_or(d.slot_top),
        halo: r.halo.unwrap_or(d.halo),
        warmth: r.warmth.unwrap_or(d.warmth),
        bolt_sens: r.bolt_sens.unwrap_or(d.bolt_sens),
        bolt_bright: r.bolt_bright.unwrap_or(d.bolt_bright),
        sky_flash: r.sky_flash.unwrap_or(d.sky_flash),
        grid_flash: r.grid_flash.unwrap_or(d.grid_flash),
        bolt_decay: r.bolt_decay.unwrap_or(d.bolt_decay),
        occlusion: r.occlusion.unwrap_or(d.occlusion),
        crisp: r.crisp.unwrap_or(d.crisp),
        sun_rim: r.sun_rim.unwrap_or(d.sun_rim),
        sky_top: r.sky_top.unwrap_or(d.sky_top),
        sky_horizon: r.sky_horizon.unwrap_or(d.sky_horizon),
        ground: r.ground.unwrap_or(d.ground),
        sun_crown: r.sun_crown.unwrap_or(d.sun_crown),
        sun_upper: r.sun_upper.unwrap_or(d.sun_upper),
        sun_lower: r.sun_lower.unwrap_or(d.sun_lower),
        sun_base: r.sun_base.unwrap_or(d.sun_base),
    }
}

/// The `[tube]` table. Material colours for the valve-row family; all optional.
#[derive(Deserialize, Default)]
struct RawTube {
    chassis_top: Option<String>,
    chassis_bottom: Option<String>,
    internals: Option<String>,
    socket: Option<String>,
    collar: Option<String>,
    glass: Option<String>,
}

fn tube_from(raw: Option<RawTube>, d: TubeParams) -> TubeParams {
    let Some(r) = raw else { return d };
    TubeParams {
        chassis_top: r.chassis_top.unwrap_or(d.chassis_top),
        chassis_bottom: r.chassis_bottom.unwrap_or(d.chassis_bottom),
        internals: r.internals.unwrap_or(d.internals),
        socket: r.socket.unwrap_or(d.socket),
        collar: r.collar.unwrap_or(d.collar),
        glass: r.glass.unwrap_or(d.glass),
    }
}

#[derive(Deserialize, Default)]
struct RawDual {
    trail: Option<String>,
    fade: Option<f32>,
}

/// Unknown keys are permitted at every level - this is what makes a theme file
/// written for a later build still load here.
#[derive(Deserialize)]
struct RawTheme {
    schema: Option<u32>,
    id: Option<String>,
    name: Option<String>,
    family: Option<String>,
    #[serde(default)]
    colour: RawColour,
    #[serde(default)]
    look: RawLook,
    #[serde(default)]
    ballistics: RawBallistics,
    #[serde(default)]
    dual: Option<RawDual>,
    vaporwave: Option<RawVapor>,
    tube: Option<RawTube>,
    #[serde(default)]
    zone: Vec<RawZone>,
}

fn texture_from(name: Option<String>, fallback: Texture) -> Texture {
    match name.as_deref() {
        Some("glass") => Texture::Glass,
        Some("scanlines") => Texture::Scanlines,
        Some("haze") => Texture::Haze,
        Some("filament") => Texture::Filament,
        Some("grille") => Texture::Grille,
        Some("none") => Texture::None_,
        // An unrecognised texture must not sink the whole theme.
        Some(_) => Texture::None_,
        None => fallback,
    }
}

pub fn parse(src: &str) -> Result<Theme, ThemeError> {
    let raw: RawTheme = toml::from_str(src).map_err(|e| ThemeError::Toml(e.to_string()))?;

    match raw.schema {
        Some(v) if v == SCHEMA_VERSION => {}
        Some(v) => return Err(ThemeError::UnsupportedSchema(v)),
        None => return Err(ThemeError::MissingField("schema")),
    }

    let id = raw.id.ok_or(ThemeError::MissingField("id"))?;
    let name = raw.name.ok_or(ThemeError::MissingField("name"))?;
    let family = raw.family.ok_or(ThemeError::MissingField("family"))?;
    // Gated on the renderer's own list rather than a hand-written one. This was
    // `matches!(family.as_str(), "segmented" | "scope" | "vu")`, which silently made the two
    // newest families impossible to author: `family = "tube"` and `family = "vapor"` were both
    // REJECTED, so the entire `[tube]` and `[vaporwave]` tables this file already parses were
    // unreachable, and `sensitivity` could not be set for them no matter what the renderer did.
    // The README documented all five families and both tables as supported the whole time.
    //
    // The failure mode was worse than inert. To load at all a file must declare a legal family,
    // so the obvious workaround - copying a built-in tube theme and setting family =
    // "segmented" to make it parse - matches a built-in `id` and REPLACES it, leaving a menu
    // entry with the valve name that draws a segmented bar meter.
    //
    // Third hardcoded list in this project to go stale (after the family test's own array and a
    // dump harness's colourway count), hence pointing at KNOWN_FAMILIES instead of restating it.
    if !crate::render::KNOWN_FAMILIES.contains(&family.as_str()) {
        return Err(ThemeError::BadFamily(family));
    }

    let d = Theme::default();
    let db = Ballistics::default();

    Ok(Theme {
        id,
        name,
        family,
        lit: raw.colour.lit.unwrap_or(d.lit),
        hot: raw.colour.hot.unwrap_or(d.hot),
        panel: raw.colour.panel.unwrap_or(d.panel),
        panel_alpha: raw.colour.panel_alpha.unwrap_or(d.panel_alpha),
        edge: raw.colour.edge.unwrap_or(d.edge),
        edge_alpha: raw.colour.edge_alpha.unwrap_or(d.edge_alpha),
        ghost: raw.look.ghost.unwrap_or(d.ghost),
        bloom: raw.look.bloom.unwrap_or(d.bloom),
        glow_strength: raw.look.glow_strength.unwrap_or(d.glow_strength),
        edge_glow: raw.look.edge_glow.unwrap_or(d.edge_glow),
        sensitivity: raw.look.sensitivity.unwrap_or(d.sensitivity),
        fade: raw.look.fade.unwrap_or(d.fade),
        texture: texture_from(raw.look.texture, d.texture),
        ballistics: Ballistics {
            attack: raw.ballistics.attack.unwrap_or(db.attack),
            decay: raw.ballistics.decay.unwrap_or(db.decay),
            peak_fall: raw.ballistics.peak_fall.unwrap_or(db.peak_fall),
        },
        zones: raw
            .zone
            .into_iter()
            .map(|z| Zone { upto: z.upto, lit: z.lit, hot: z.hot })
            .collect(),
        dual: raw.dual.and_then(|dl| {
            dl.trail.map(|t| (t, dl.fade.unwrap_or(0.20)))
        }),
        vapor: vapor_from(raw.vaporwave, d.vapor),
        tube: tube_from(raw.tube, d.tube),
    })
}

pub fn load_dir(dir: &Path) -> (Vec<Theme>, Vec<String>) {
    let mut themes = Vec::new();
    let mut warnings = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        // No themes directory is the normal case, not a problem.
        Err(_) => return (themes, warnings),
    };

    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "toml").unwrap_or(false))
        .collect();
    paths.sort(); // deterministic load order

    for path in paths {
        let label = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        match std::fs::read_to_string(&path) {
            Ok(src) => match parse(&src) {
                Ok(t) => themes.push(t),
                Err(e) => warnings.push(format!("{label}: {e}")),
            },
            Err(e) => warnings.push(format!("{label}: unreadable ({e})")),
        }
    }
    (themes, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_valid_theme() {
        let t = parse(include_str!("../../tests/themes/valid.toml")).expect("should parse");
        assert_eq!(t.id, "my-purple");
        assert_eq!(t.name, "My Purple");
        assert_eq!(t.family, "segmented");
        assert_eq!(t.lit, "#c07fff");
        assert_eq!(t.panel_alpha, 0.62);
        assert_eq!(t.texture, Texture::Haze);
        assert_eq!(t.ballistics.attack, 0.55);
    }

    #[test]
    fn an_unknown_key_is_ignored_and_the_theme_still_loads() {
        // Forward compatibility: a theme written for a later build must not break.
        let t = parse(include_str!("../../tests/themes/unknown-key.toml"))
            .expect("unknown keys must not fail the parse");
        assert_eq!(t.id, "unknown-key-theme");
        assert_eq!(t.texture, Texture::Haze);
    }

    #[test]
    fn malformed_toml_is_an_error_not_a_panic() {
        let e = parse(include_str!("../../tests/themes/malformed.toml"));
        assert!(matches!(e, Err(ThemeError::Toml(_))), "got {e:?}");
    }

    #[test]
    fn a_future_schema_is_rejected_with_a_clear_message() {
        match parse(include_str!("../../tests/themes/future-schema.toml")) {
            Err(ThemeError::UnsupportedSchema(v)) => {
                assert_eq!(v, 99);
                let msg = ThemeError::UnsupportedSchema(99).to_string();
                assert!(msg.contains("99") && msg.contains("1"), "message was: {msg}");
            }
            other => panic!("expected UnsupportedSchema, got {other:?}"),
        }
    }

    #[test]
    fn missing_required_fields_are_named_in_the_error() {
        match parse("schema = 1\nname = \"No Id\"\nfamily = \"segmented\"") {
            Err(ThemeError::MissingField(k)) => assert_eq!(k, "id"),
            other => panic!("expected MissingField(id), got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_family_is_rejected() {
        let src = "schema = 1\nid = \"x\"\nname = \"X\"\nfamily = \"hologram\"";
        match parse(src) {
            Err(ThemeError::BadFamily(f)) => assert_eq!(f, "hologram"),
            other => panic!("expected BadFamily, got {other:?}"),
        }
    }

    #[test]
    fn omitted_keys_take_documented_defaults() {
        let src = "schema = 1\nid = \"bare\"\nname = \"Bare\"\nfamily = \"segmented\"";
        let t = parse(src).expect("a minimal file is valid");
        let d = Theme::default();
        assert_eq!(t.bloom, d.bloom);
        assert_eq!(t.ghost, d.ghost);
        assert_eq!(t.panel_alpha, d.panel_alpha);
    }

    #[test]
    fn zones_are_parsed_in_order() {
        let t = parse(include_str!("../../tests/themes/zoned.toml")).expect("should parse");
        assert_eq!(t.zones.len(), 2);
        assert_eq!(t.lit_at(0.3), "#40e060");
        assert_eq!(t.lit_at(0.9), "#ff4030");
    }

    #[test]
    fn every_texture_name_round_trips() {
        for (name, want) in [
            ("glass", Texture::Glass),
            ("scanlines", Texture::Scanlines),
            ("haze", Texture::Haze),
            ("filament", Texture::Filament),
            ("grille", Texture::Grille),
            ("none", Texture::None_),
        ] {
            let src = format!(
                "schema = 1\nid = \"t\"\nname = \"T\"\nfamily = \"segmented\"\n\n[look]\ntexture = \"{name}\""
            );
            assert_eq!(parse(&src).unwrap().texture, want, "texture {name}");
        }
    }

    #[test]
    fn an_unknown_texture_falls_back_rather_than_failing() {
        let src = "schema = 1\nid = \"t\"\nname = \"T\"\nfamily = \"segmented\"\n\n[look]\ntexture = \"marble\"";
        let t = parse(src).expect("unknown texture should not fail the whole theme");
        assert_eq!(t.texture, Texture::None_);
    }

    #[test]
    fn load_dir_skips_bad_files_and_reports_them() {
        let dir = Path::new("tests/themes");
        let (themes, warnings) = load_dir(dir);
        let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"my-purple"), "good files must load");
        assert!(!ids.contains(&"from-the-future"), "future schema must be skipped");
        assert_eq!(warnings.len(), 2, "expected warnings for malformed + future, got {warnings:?}");
        assert!(
            warnings.iter().any(|w| w.contains("malformed")),
            "warning should name the offending file: {warnings:?}"
        );
    }

    #[test]
    fn load_dir_on_a_missing_directory_is_empty_not_an_error() {
        let (themes, warnings) = load_dir(Path::new("tests/does-not-exist"));
        assert!(themes.is_empty());
        assert!(warnings.is_empty(), "a missing themes dir is normal, not a warning");
    }
}

#[cfg(test)]
mod family_gate {
    use super::*;

    #[test]
    fn every_family_the_renderer_knows_can_be_authored_in_toml() {
        // The bug this guards shipped: the gate was a hand-written list of three families, so
        // `family = "tube"` and `family = "vapor"` were rejected outright - both documented in
        // the README as supported, along with the whole [tube] and [vaporwave] tables this file
        // already parses. Driving it from KNOWN_FAMILIES means adding a family cannot leave the
        // parser behind.
        for fam in crate::render::KNOWN_FAMILIES {
            let toml = format!(
                "schema = 1
id = \"probe-{fam}\"
name = \"Probe\"
family = \"{fam}\"
"
            );
            let t = parse(&toml).unwrap_or_else(|e| panic!("family {fam:?} must parse, got: {e}"));
            assert_eq!(t.family, fam);
        }
    }

    #[test]
    fn a_tube_theme_round_trips_its_own_table_and_sensitivity() {
        // Until the gate was fixed none of this was reachable, so a colourway author had no way
        // to tune the valve row at all - which is the whole reason `sensitivity` being unwired in
        // tube.rs mattered so much.
        let toml = r##"
schema = 1
id     = "my-valves"
name   = "My Valves"
family = "tube"
[look]
sensitivity = 1.6
[tube]
glass  = "#ffffff"
collar = "#123456"
"##;
        let t = parse(toml).expect("a tube theme must load");
        assert_eq!(t.family, "tube");
        assert!((t.sensitivity - 1.6).abs() < 1e-6, "sensitivity must round-trip");
        assert_eq!(t.tube.glass, "#ffffff");
        assert_eq!(t.tube.collar, "#123456");
        // Unset keys must keep their defaults rather than being zeroed.
        assert_eq!(t.tube.socket, crate::themes::TubeParams::default().socket);
    }

    #[test]
    fn a_vaporwave_theme_round_trips_its_own_table() {
        let toml = r##"
schema = 1
id     = "my-sunset"
name   = "My Sunset"
family = "vapor"
[vaporwave]
amp    = 1.4
lines  = 14
sky_top = "#010203"
"##;
        let t = parse(toml).expect("a vapor theme must load");
        assert_eq!(t.family, "vapor");
        assert!((t.vapor.amp - 1.4).abs() < 1e-6);
        assert_eq!(t.vapor.lines, 14);
        assert_eq!(t.vapor.sky_top, "#010203");
    }

    #[test]
    fn a_genuinely_unknown_family_is_still_rejected_and_names_the_real_options() {
        let toml = "schema = 1
id = \"x\"
name = \"X\"
family = \"hologram\"
";
        let err = parse(toml).expect_err("an unknown family must still fail");
        let msg = err.to_string();
        assert!(msg.contains("hologram"), "the message must name the offender: {msg}");
        for fam in crate::render::KNOWN_FAMILIES {
            assert!(msg.contains(fam), "the message must list {fam}: {msg}");
        }
    }
}
