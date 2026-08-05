use super::{Texture, Theme, Zone};
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
                write!(f, "unknown family `{fam}` (expected segmented, scope or vu)")
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
    if !matches!(family.as_str(), "segmented" | "scope" | "vu") {
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
            dl.trail.map(|t| (t, dl.fade.unwrap_or(0.055)))
        }),
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
