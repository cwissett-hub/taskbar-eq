use super::{Texture, Theme};

// `main` selects `vfd_ice()` directly - there is no theme-picker yet to
// enumerate the registry, so nothing calls `all()` in production. It exists
// for that future picker and for tests that want to exercise every builtin.
#[allow(dead_code)]
pub fn all() -> Vec<Theme> {
    vec![vfd_ice()]
}

pub fn vfd_ice() -> Theme {
    Theme {
        id: "vfd-ice".into(),
        name: "VFD Ice".into(),
        family: "segmented".into(),
        lit: "#8fe4ff".into(),
        hot: "#e4f8ff".into(),
        panel: "#040a0e".into(),
        panel_alpha: 0.55,
        edge: "#96e1ff".into(),
        edge_alpha: 0.13,
        ghost: 0.11,
        bloom: 9.0,
        texture: Texture::Glass,
        ..Theme::default()
    }
}
