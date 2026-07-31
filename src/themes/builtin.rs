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
        // Must OCCLUDE the widget's own content, not merely tint it. At 0.55 the
        // white weather text composited to ~45% of 255 and stayed plainly legible
        // through the panel. The design brief chose "EQ replaces the weather while
        // playing", not a translucent wash, so the panel has to actually hide it.
        panel_alpha: 0.96,
        edge: "#96e1ff".into(),
        edge_alpha: 0.13,
        ghost: 0.11,
        // Generous on purpose - this is a phosphor display and the glow is the
        // point. Tuned by eye against the real taskbar.
        bloom: 16.0,
        texture: Texture::Glass,
        ..Theme::default()
    }
}
