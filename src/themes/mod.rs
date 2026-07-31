pub mod builtin;

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
            panel_alpha: 0.55,
            edge: "#96e1ff".into(),
            edge_alpha: 0.13,
            ghost: 0.11,
            bloom: 9.0,
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
}
