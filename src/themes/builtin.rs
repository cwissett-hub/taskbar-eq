use super::{Texture, Theme, TubeParams, VaporParams, Zone};
use crate::dsp::ballistics::Ballistics;

pub fn all() -> Vec<Theme> {
    vec![
        vfd_ice(),
        matrix_green(),
        neon_pink(),
        vac_tube_orange(),
        classic_three_colour(),
        hifi_white(),
        chrome(),
        rgb_wave(),
        p1_green(),
        p7_dual(),
        p11_blue_violet(),
        scope_amber(),
        scope_white(),
        mw2_trace(),
        scope_red(),
        scope_azure(),
        scope_magenta(),
        tek_teal(),
        p39_yellow_green(),
        scope_rgb_wave(),
        vu_cream(),
        vu_amber(),
        vu_ice(),
        vu_green(),
        vu_red(),
        vu_cyan(),
        vu_hot_pink(),
        vu_lime(),
        vu_rgb_wave(),
        vapor_sunset(),
        vapor_miami(),
        vapor_outrun(),
        vapor_toxic(),
        vapor_mono(),
        vapor_tokyo(),
        vapor_sunrise(),
        vapor_noir(),
        tube_soviet(),
        tube_steel(),
        tube_mercury(),
        tube_bakelite(),
        tube_nixie_green(),
        tube_copper(),
        tube_red_plate(),
    ]
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
        panel_alpha: 1.0,
        edge: "#96e1ff".into(),
        edge_alpha: 0.13,
        ghost: 0.11,
        // Radius, not intensity - see Theme::default. Measured against the live
        // taskbar: at 16 the halos of adjacent bars merged into one wash sitting
        // behind the segments; at 4 each segment keeps its own halo.
        bloom: 4.0,
        glow_strength: 0.35,
        edge_glow: 4.0,
        texture: Texture::Glass,
        ..Theme::default()
    }
}

pub fn matrix_green() -> Theme {
    Theme {
        id: "matrix-green".into(),
        name: "Matrix Green".into(),
        lit: "#35ff6e".into(),
        hot: "#ccffdb".into(),
        panel: "#000903".into(),
        panel_alpha: 1.0,
        edge: "#3cff78".into(),
        edge_alpha: 0.14,
        ghost: 0.17,
        // Tightened from 8 for the same reason as neon-pink - measured ratio 8.16,
        // i.e. barely any visible halo.
        bloom: 5.0,
        texture: Texture::Scanlines,
        ballistics: crate::dsp::ballistics::Ballistics {
            attack: 0.55,
            decay: 0.13,
            peak_fall: 0.0070,
        },
        ..vfd_ice()
    }
}

pub fn neon_pink() -> Theme {
    Theme {
        id: "neon-pink".into(),
        name: "Neon Pink".into(),
        lit: "#ff4fb0".into(),
        hot: "#ffd9ee".into(),
        panel: "#0d020b".into(),
        panel_alpha: 1.0,
        edge: "#ff4fb0".into(),
        edge_alpha: 0.22,
        ghost: 0.09,
        // 14 measured as the FAINTEST halo of the five, not the strongest: a wider
        // radius spreads the same energy across a much bigger kernel, so per-pixel
        // intensity drops. 6 keeps neon's soft character without losing the halo.
        bloom: 6.0,
        texture: Texture::Haze,
        ballistics: crate::dsp::ballistics::Ballistics {
            attack: 0.55,
            decay: 0.11,
            peak_fall: 0.0050,
        },
        ..vfd_ice()
    }
}

pub fn vac_tube_orange() -> Theme {
    Theme {
        id: "vac-tube-orange".into(),
        name: "Vac Tube Orange".into(),
        lit: "#ff9a2e".into(),
        hot: "#ffe9c9".into(),
        panel: "#0f0602".into(),
        panel_alpha: 1.0,
        edge: "#ff9632".into(),
        edge_alpha: 0.16,
        ghost: 0.13,
        bloom: 12.0,
        texture: Texture::Filament,
        // Slowest peak fall of the five - heat dissipating rather than snapping back.
        ballistics: crate::dsp::ballistics::Ballistics {
            attack: 0.50,
            decay: 0.09,
            peak_fall: 0.0035,
        },
        ..vfd_ice()
    }
}

pub fn classic_three_colour() -> Theme {
    Theme {
        id: "classic-three-colour".into(),
        name: "Classic Three-Colour".into(),
        lit: "#3ddc5a".into(),
        hot: "#b6ffc6".into(),
        panel: "#060708".into(),
        panel_alpha: 1.0,
        edge: "#c8d2d7".into(),
        edge_alpha: 0.12,
        ghost: 0.13,
        bloom: 7.0,
        texture: Texture::Grille,
        zones: vec![
            Zone { upto: 0.58, lit: "#3ddc5a".into(), hot: "#b6ffc6".into() },
            Zone { upto: 0.84, lit: "#ffc21f".into(), hot: "#fff0b8".into() },
            Zone { upto: 1.01, lit: "#ff3b30".into(), hot: "#ffc2bd".into() },
        ],
        ballistics: crate::dsp::ballistics::Ballistics {
            attack: 0.55,
            decay: 0.11,
            peak_fall: 0.0045,
        },
        ..vfd_ice()
    }
}

/// Neutral white, the modern hi-fi look rather than a phosphor.
///
/// `Texture::None_` is the point of it: every other segmented colourway wears a texture, and a
/// perfectly clean panel is what separates a current amplifier from a vintage one.
pub fn hifi_white() -> Theme {
    Theme {
        id: "hifi-white".into(),
        name: "Hi-fi white".into(),
        lit: "#f2f6fa".into(),
        hot: "#ffffff".into(),
        panel: "#0a0c0e".into(),
        panel_alpha: 1.0,
        edge: "#c8d4e0".into(),
        edge_alpha: 0.16,
        texture: Texture::None_,
        bloom: 3.0,
        ghost: 0.08,
        ..vfd_ice()
    }
}

/// Brushed chrome: cool grey segments with a bright specular top, no colour cast at all.
pub fn chrome() -> Theme {
    Theme {
        id: "chrome".into(),
        name: "Chrome".into(),
        lit: "#c8d8e8".into(),
        hot: "#f8fcff".into(),
        panel: "#101418".into(),
        panel_alpha: 1.0,
        edge: "#8fa4b8".into(),
        edge_alpha: 0.20,
        texture: Texture::Grille,
        // Distinct from classic-three-colour, which is the other Grille theme, by bloom as well as
        // colour - the distinctness guard compares (texture, bloom, lit, fade).
        bloom: 2.0,
        ghost: 0.14,
        ..vfd_ice()
    }
}

/// RGB wave: the gaming-keyboard rainbow, hue sweeping across the bars and drifting over time.
///
/// `lit` and `hot` are still set, and are not decoration - they are what a build with `rainbow = 0`
/// would draw, and they are what the contrast test measures. The live colour comes from
/// `themes::rainbow_hsv`.
pub fn rgb_wave() -> Theme {
    Theme {
        id: "rgb-wave".into(),
        name: "RGB wave".into(),
        lit: "#7dd8ff".into(),
        hot: "#f0fbff".into(),
        panel: "#08090c".into(),
        panel_alpha: 1.0,
        edge: "#8fa4b8".into(),
        edge_alpha: 0.18,
        texture: Texture::None_,
        bloom: 5.0,
        ghost: 0.10,
        // A slow drift. Fast enough to be obviously alive, slow enough that the hue at any one bar
        // is stable long enough to read the spectrum by - a quick cycle turns the display into a
        // strobe and destroys the frequency legend the spread gives you for free.
        rainbow: 0.07,
        rainbow_spread: 0.85,
        ..vfd_ice()
    }
}

fn scope_base() -> Theme {
    Theme {
        family: "scope".into(),
        texture: Texture::None_,
        ghost: 0.0,
        bloom: 6.0,
        ..Theme::default()
    }
}

pub fn p1_green() -> Theme {
    Theme {
        id: "p1-green".into(),
        name: "P1 green".into(),
        lit: "#5cff9a".into(),
        hot: "#ccffdd".into(),
        panel: "#020805".into(),
        panel_alpha: 1.0,
        edge: "#78ffb4".into(),
        edge_alpha: 0.14,
        // 0.14 left 255ms of history on screen. Tolerable when the trace only
        // wobbled a few pixels; a smear once auto-ranging made every frame a
        // full-height sweep.
        fade: 0.22,
        // All five phosphors share this bloom deliberately: it is the tightest of
        // the set and the only one that read as a glowing trace rather than a washed
        // halo, so it is the reference the others were brought down to.
        //
        // They previously ran 5-9, spread apart to satisfy a guard named in a comment
        // here - `each_colourway_has_a_distinct_texture_or_bloom` - that does not
        // exist. The real guard is `colourways_are_visually_distinct_within_their_family`
        // and its signature is (texture, bloom, lit, fade); `lit` already differs for
        // every phosphor, so bloom never had to vary at all. Four themes were
        // disfigured to satisfy a constraint that was never there.
        bloom: 5.0,
        ..scope_base()
    }
}

pub fn p7_dual() -> Theme {
    Theme {
        id: "p7-dual".into(),
        name: "P7 dual-layer".into(),
        lit: "#e8f4ff".into(),
        hot: "#ffffff".into(),
        panel: "#03060c".into(),
        panel_alpha: 1.0,
        edge: "#aad7ff".into(),
        edge_alpha: 0.15,
        fade: 0.34,
        // The real P7 is two phosphor layers: a blue-white flash over a slow
        // yellow-green tail. The trail still fades slower than the trace, but 0.055
        // held it for 680ms - 41 frames - and once auto-ranging made each of those
        // frames a full-height sweep, 41 of them overlaid filled the screen solid.
        // This is why P7 was the worst offender of the five: its afterglow ran 3-6x
        // longer than any other phosphor here.
        dual: Some(("#cfe86a".into(), 0.20)),
        // was 8.0 - see the bloom note in `p1_green`
        bloom: 5.0,
        ..scope_base()
    }
}

pub fn p11_blue_violet() -> Theme {
    Theme {
        id: "p11-blue-violet".into(),
        name: "P11 blue-violet".into(),
        lit: "#9db4ff".into(),
        hot: "#dde5ff".into(),
        panel: "#03040c".into(),
        panel_alpha: 1.0,
        edge: "#96afff".into(),
        edge_alpha: 0.15,
        fade: 0.26,
        // was 6.0 - see the bloom note in `p1_green`
        bloom: 5.0,
        ..scope_base()
    }
}

pub fn scope_amber() -> Theme {
    Theme {
        id: "scope-amber".into(),
        name: "Amber".into(),
        lit: "#ffc766".into(),
        hot: "#ffe9c9".into(),
        panel: "#0c0602".into(),
        panel_alpha: 1.0,
        edge: "#ffc878".into(),
        edge_alpha: 0.15,
        fade: 0.20,
        // was 7.0 - see the bloom note in `p1_green`
        bloom: 5.0,
        ..scope_base()
    }
}

pub fn scope_white() -> Theme {
    Theme {
        id: "scope-white".into(),
        name: "White-hot".into(),
        lit: "#f6fbff".into(),
        hot: "#ffffff".into(),
        panel: "#040609".into(),
        panel_alpha: 1.0,
        edge: "#dceeff".into(),
        edge_alpha: 0.15,
        fade: 0.24,
        // was 9.0 - see the bloom note in `p1_green`
        bloom: 5.0,
        ..scope_base()
    }
}

/// The green oscilloscope readout from the 2009 Modern Warfare 2 reveal trailer.
///
/// Not a CRT phosphor like the other five, which is why it is tuned against them rather
/// than with them: a military data readout, so the green is acid chartreuse rather than
/// the soft mint of P1, the persistence is short enough to read as instantaneous, and it
/// is the only scope colourway with scanlines.
pub fn mw2_trace() -> Theme {
    Theme {
        id: "mw2-trace".into(),
        name: "MW2 trace".into(),
        lit: "#a8ff2e".into(),
        hot: "#e6ffb8".into(),
        panel: "#040703".into(),
        panel_alpha: 1.0,
        edge: "#5f9a1e".into(),
        edge_alpha: 0.18,
        // Crisp, not smeary - a readout rather than an afterglow.
        fade: 0.28,
        texture: Texture::Scanlines,
        // P1 green's bloom, the reference the whole family was brought down to.
        bloom: 5.0,
        ..scope_base()
    }
}

/// Bright, saturated scope traces, as a counterpoint to the five muted CRT phosphors.
///
/// The phosphor set is faithful but all of it is low-key by nature; these are picked for
/// punch on a dark taskbar instead. They share P1 green's bloom of 5.0 - the reference the
/// family was tuned to - and a short fade, because a saturated colour smears far more
/// visibly than a soft mint one does.
pub fn scope_red() -> Theme {
    Theme {
        id: "scope-red".into(),
        name: "Signal red".into(),
        lit: "#ff4a3d".into(),
        hot: "#ffcdc7".into(),
        panel: "#0a0302".into(),
        panel_alpha: 1.0,
        edge: "#c23a2e".into(),
        edge_alpha: 0.17,
        fade: 0.25,
        bloom: 5.0,
        ..scope_base()
    }
}

pub fn scope_azure() -> Theme {
    Theme {
        id: "scope-azure".into(),
        name: "Electric azure".into(),
        lit: "#35b6ff".into(),
        hot: "#cdeeff".into(),
        panel: "#01060c".into(),
        panel_alpha: 1.0,
        edge: "#2680bd".into(),
        edge_alpha: 0.17,
        fade: 0.23,
        bloom: 5.0,
        ..scope_base()
    }
}

pub fn scope_magenta() -> Theme {
    Theme {
        id: "scope-magenta".into(),
        name: "Hot magenta".into(),
        lit: "#ff45cf".into(),
        hot: "#ffcbf4".into(),
        panel: "#0a0209".into(),
        panel_alpha: 1.0,
        edge: "#bd3399".into(),
        edge_alpha: 0.17,
        fade: 0.27,
        bloom: 5.0,
        ..scope_base()
    }
}

/// The Tektronix look: a teal-green storage phosphor, colder than P1.
pub fn tek_teal() -> Theme {
    Theme {
        id: "tek-teal".into(),
        name: "Tek teal".into(),
        lit: "#22e0c8".into(),
        hot: "#d4fff8".into(),
        panel: "#02100e".into(),
        panel_alpha: 1.0,
        edge: "#189a8a".into(),
        edge_alpha: 0.17,
        fade: 0.24,
        bloom: 5.0,
        ..scope_base()
    }
}

/// P39, the long-persistence yellow-green.
///
/// The slowest fade in the family on purpose - P39's whole character is a tail that hangs. That was
/// unusable before the sweep was triggered, because a long tail with an untriggered trace smeared
/// several different waveforms over each other; with the trace standing still the tail reinforces
/// one shape instead, which is what makes this colourway possible at all.
pub fn p39_yellow_green() -> Theme {
    Theme {
        id: "p39-yellow-green".into(),
        name: "P39 long-tail".into(),
        lit: "#c8ff5a".into(),
        hot: "#f2ffd8".into(),
        panel: "#070c02".into(),
        panel_alpha: 1.0,
        edge: "#8aa838".into(),
        edge_alpha: 0.17,
        fade: 0.14,
        bloom: 5.0,
        ..scope_base()
    }
}

/// RGB wave on the oscilloscope: the hue sweeps along the trace itself.
pub fn scope_rgb_wave() -> Theme {
    Theme {
        id: "scope-rgb-wave".into(),
        name: "RGB wave".into(),
        lit: "#7dd8ff".into(),
        hot: "#f0fbff".into(),
        panel: "#06070a".into(),
        panel_alpha: 1.0,
        edge: "#8fa4b8".into(),
        edge_alpha: 0.17,
        fade: 0.26,
        bloom: 5.0,
        rainbow: 0.09,
        rainbow_spread: 0.9,
        ..scope_base()
    }
}

fn vu_base() -> Theme {
    Theme {
        family: "vu".into(),
        texture: Texture::Filament,
        ghost: 0.0,
        bloom: 5.0,
        ..Theme::default()
    }
}

pub fn vu_cream() -> Theme {
    Theme {
        id: "vu-cream".into(),
        name: "Warm cream".into(),
        lit: "#ffe2aa".into(),
        hot: "#ffe6b0".into(),
        panel: "#140e06".into(),
        panel_alpha: 1.0,
        edge: "#ffc878".into(),
        edge_alpha: 0.16,
        ..vu_base()
    }
}

pub fn vu_amber() -> Theme {
    Theme {
        id: "vu-amber".into(),
        name: "Amber".into(),
        lit: "#ffbe6e".into(),
        hot: "#ffcf7a".into(),
        panel: "#160b02".into(),
        panel_alpha: 1.0,
        edge: "#ffaf50".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}

pub fn vu_ice() -> Theme {
    Theme {
        id: "vu-ice".into(),
        name: "Ice blue".into(),
        // Deliberately matches the VFD Ice segmented colourway.
        lit: "#bee6ff".into(),
        hot: "#d8f2ff".into(),
        panel: "#040c14".into(),
        panel_alpha: 1.0,
        edge: "#a0dcff".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}

pub fn vu_green() -> Theme {
    Theme {
        id: "vu-green".into(),
        name: "Green".into(),
        // Matches Matrix Green.
        lit: "#b4ffcd".into(),
        hot: "#c8ffd8".into(),
        panel: "#020e06".into(),
        panel_alpha: 1.0,
        edge: "#8cffb4".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}

pub fn vu_red() -> Theme {
    Theme {
        id: "vu-red".into(),
        name: "Red".into(),
        // Closest match to the system accent (#D0000C).
        lit: "#ffaa9b".into(),
        hot: "#ffb3a6".into(),
        panel: "#140302".into(),
        panel_alpha: 1.0,
        edge: "#ff826e".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}

/// Bright dial colourways. The five originals are all warm, low-key vintage panels; these
/// trade authenticity for legibility at 190x60 on a dark taskbar, with a near-black panel
/// so the needle has something to contrast against.
pub fn vu_cyan() -> Theme {
    Theme {
        id: "vu-cyan".into(),
        name: "Neon cyan".into(),
        lit: "#3ff0ff".into(),
        hot: "#e0feff".into(),
        panel: "#01090c".into(),
        panel_alpha: 1.0,
        edge: "#2aa8b8".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}

pub fn vu_hot_pink() -> Theme {
    Theme {
        id: "vu-hot-pink".into(),
        name: "Hot pink".into(),
        lit: "#ff5ad2".into(),
        hot: "#ffd6f5".into(),
        panel: "#0b0209".into(),
        panel_alpha: 1.0,
        edge: "#c23a9a".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}

pub fn vu_lime() -> Theme {
    Theme {
        id: "vu-lime".into(),
        name: "Lime".into(),
        lit: "#a8ff3c".into(),
        hot: "#e8ffc4".into(),
        panel: "#050a02".into(),
        panel_alpha: 1.0,
        edge: "#6f9e24".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}

/// RGB wave on the dials: each dial sits at its own hue, and the whole set drifts.
///
/// Spread is low here on purpose. There are only two to four dials, so a wide spread would put them
/// at wildly unrelated hues and read as a fault rather than a rainbow.
pub fn vu_rgb_wave() -> Theme {
    Theme {
        id: "vu-rgb-wave".into(),
        name: "RGB wave".into(),
        lit: "#7dd8ff".into(),
        hot: "#f0fbff".into(),
        panel: "#06070a".into(),
        panel_alpha: 1.0,
        edge: "#8fa4b8".into(),
        edge_alpha: 0.18,
        rainbow: 0.06,
        rainbow_spread: 0.22,
        ..vu_base()
    }
}

fn vapor_base() -> Theme {
    Theme {
        family: "vapor".into(),
        texture: Texture::None_,
        ghost: 0.0,
        // Snappier than the shared defaults, which every vapor colourway was silently inheriting
        // from the VFD family (attack 0.55, decay 0.11 - about 143ms to fall). Ballistics are
        // applied upstream in dsp::ballistics, so they are per-theme rather than per-family, and a
        // meter tuned for a smooth bar graph is not tuned for a terrain that should spike on a hit.
        ballistics: Ballistics { attack: 0.88, decay: 0.30, peak_fall: 0.006 },
        // Modest: the scene is already a gradient, and a wide blur on a 60px-tall sunset
        // turns the whole panel into fog.
        bloom: 3.0,
        ..Theme::default()
    }
}

/// The tuned reference scene - magenta sunset over a violet grid.
pub fn vapor_sunset() -> Theme {
    Theme {
        id: "vapor-sunset".into(),
        name: "Sunset".into(),
        lit: "#ff53c8".into(),
        hot: "#eafcff".into(),
        panel: "#0a0416".into(),
        panel_alpha: 1.0,
        edge: "#ff5f93".into(),
        edge_alpha: 0.18,
        ..vapor_base()
    }
}

pub fn vapor_miami() -> Theme {
    Theme {
        id: "vapor-miami".into(),
        name: "Miami".into(),
        lit: "#38e8ff".into(),
        hot: "#f2ffff".into(),
        panel: "#04121c".into(),
        panel_alpha: 1.0,
        edge: "#2fb8d8".into(),
        edge_alpha: 0.18,
        vapor: VaporParams {
            sky_top: "#05203a".into(),
            sky_horizon: "#ff9c6e".into(),
            ground: "#04121c".into(),
            sun_crown: "#fffbe0".into(),
            sun_upper: "#ffe07a".into(),
            sun_lower: "#ff8f5a".into(),
            sun_base: "#ff5f7d".into(),
            ..VaporParams::default()
        },
        ..vapor_base()
    }
}

pub fn vapor_outrun() -> Theme {
    Theme {
        id: "vapor-outrun".into(),
        name: "Outrun".into(),
        lit: "#ff2f6d".into(),
        hot: "#fff0f5".into(),
        panel: "#0d0119".into(),
        panel_alpha: 1.0,
        edge: "#ff2f6d".into(),
        edge_alpha: 0.20,
        vapor: VaporParams {
            sky_top: "#12002b".into(),
            sky_horizon: "#7a1a8c".into(),
            ground: "#0d0119".into(),
            sun_crown: "#ffe9f6".into(),
            sun_upper: "#ff9ad5".into(),
            sun_lower: "#ff4d9e".into(),
            sun_base: "#8f1f6b".into(),
            ..VaporParams::default()
        },
        ..vapor_base()
    }
}

/// Toxic green, and the calm one: no lightning, so there is a colourway for anyone who
/// finds the strikes distracting. `bolt_bright = 0` disables them without special-casing.
pub fn vapor_toxic() -> Theme {
    Theme {
        id: "vapor-toxic".into(),
        name: "Toxic".into(),
        lit: "#7dff5a".into(),
        hot: "#eaffe0".into(),
        panel: "#020d05".into(),
        panel_alpha: 1.0,
        edge: "#5fbf3a".into(),
        edge_alpha: 0.18,
        vapor: VaporParams {
            sky_top: "#03140a".into(),
            sky_horizon: "#2f8f4a".into(),
            ground: "#020d05".into(),
            sun_crown: "#f2ffd8".into(),
            sun_upper: "#c8ff7a".into(),
            sun_lower: "#7dd44a".into(),
            sun_base: "#2f8f4a".into(),
            bolt_bright: 0.0,
            sky_flash: 0.0,
            grid_flash: 0.0,
            ..VaporParams::default()
        },
        ..vapor_base()
    }
}

pub fn vapor_mono() -> Theme {
    Theme {
        id: "vapor-mono".into(),
        name: "Monochrome".into(),
        lit: "#d8e4f0".into(),
        hot: "#ffffff".into(),
        panel: "#06080c".into(),
        panel_alpha: 1.0,
        edge: "#8fa4b8".into(),
        edge_alpha: 0.18,
        vapor: VaporParams {
            sky_top: "#080b12".into(),
            sky_horizon: "#5a6a7d".into(),
            ground: "#06080c".into(),
            sun_crown: "#ffffff".into(),
            sun_upper: "#d8e4f0".into(),
            sun_lower: "#8fa4b8".into(),
            sun_base: "#4a5a6d".into(),
            ..VaporParams::default()
        },
        ..vapor_base()
    }
}

/// Tokyo at night: deep indigo sky, neon cyan grid, a violet sun.
pub fn vapor_tokyo() -> Theme {
    Theme {
        id: "vapor-tokyo".into(),
        name: "Tokyo night".into(),
        lit: "#4de2ff".into(),
        hot: "#eafcff".into(),
        panel: "#050a18".into(),
        panel_alpha: 1.0,
        edge: "#2f6f9e".into(),
        edge_alpha: 0.18,
        vapor: VaporParams {
            sky_top: "#04081c".into(),
            sky_horizon: "#6a3fa0".into(),
            ground: "#050a18".into(),
            sun_crown: "#ffe8ff".into(),
            sun_upper: "#ff9ce8".into(),
            sun_lower: "#a05bd8".into(),
            sun_base: "#4a2f8f".into(),
            ..VaporParams::default()
        },
        ..vapor_base()
    }
}

/// Sunrise rather than sunset: a pale warm sky over a dark warm ground.
pub fn vapor_sunrise() -> Theme {
    Theme {
        id: "vapor-sunrise".into(),
        name: "Sunrise".into(),
        lit: "#ff9a4d".into(),
        hot: "#fff2e0".into(),
        panel: "#180d06".into(),
        panel_alpha: 1.0,
        edge: "#c2703a".into(),
        edge_alpha: 0.18,
        vapor: VaporParams {
            sky_top: "#3a1f0e".into(),
            sky_horizon: "#ffcf8a".into(),
            ground: "#180d06".into(),
            sun_crown: "#fffbe8".into(),
            sun_upper: "#ffe6a0".into(),
            sun_lower: "#ffb066".into(),
            sun_base: "#e8703a".into(),
            ..VaporParams::default()
        },
        ..vapor_base()
    }
}

/// Noir: near-black everywhere with a single pink accent.
///
/// Distinct from `vapor_mono`, which is greyscale throughout - this one keeps one saturated colour
/// and desaturates the rest, which reads very differently even though both are described as "dark".
pub fn vapor_noir() -> Theme {
    Theme {
        id: "vapor-noir".into(),
        name: "Noir".into(),
        lit: "#ff2f7a".into(),
        hot: "#ffd0e2".into(),
        panel: "#050406".into(),
        panel_alpha: 1.0,
        edge: "#8f1f4a".into(),
        edge_alpha: 0.18,
        vapor: VaporParams {
            sky_top: "#08070a".into(),
            sky_horizon: "#2e1a24".into(),
            ground: "#050406".into(),
            sun_crown: "#ffe0ec".into(),
            sun_upper: "#ff7aa8".into(),
            sun_lower: "#8f2a52".into(),
            sun_base: "#3a1020".into(),
            // No lightning: the point of noir is restraint.
            bolt_bright: 0.0,
            sky_flash: 0.0,
            grid_flash: 0.0,
            ..VaporParams::default()
        },
        ..vapor_base()
    }
}

fn tube_base() -> Theme {
    Theme {
        family: "tube".into(),
        texture: Texture::Filament,
        ghost: 0.0,
        // Tight. The glow is already a radial gradient inside the glass, and a wide blur
        // spills it across the chassis and welds the row into one bar.
        bloom: 4.0,
        glow_strength: 0.55,
        ..Theme::default()
    }
}

/// The reference: a Soviet lab chassis in military olive, valves running orange.
pub fn tube_soviet() -> Theme {
    Theme {
        id: "tube-soviet".into(),
        name: "Soviet lab".into(),
        lit: "#ff8a2a".into(),
        hot: "#ffd9a0".into(),
        panel: "#20241b".into(),
        panel_alpha: 1.0,
        edge: "#6f7a52".into(),
        edge_alpha: 0.22,
        ..tube_base()
    }
}

/// Cold-war grey steel with white-hot heaters.
pub fn tube_steel() -> Theme {
    Theme {
        id: "tube-steel".into(),
        name: "Grey steel".into(),
        lit: "#ffd08a".into(),
        hot: "#fff6e2".into(),
        panel: "#22242a".into(),
        panel_alpha: 1.0,
        edge: "#6b7280".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#414652".into(),
            chassis_bottom: "#15171c".into(),
            socket: "#1c1e24".into(),
            collar: "#9aa3b0".into(),
            glass: "#dfe8f2".into(),
            ..TubeParams::default()
        },
        ..tube_base()
    }
}

/// Mercury-vapour blue - the rectifier look.
pub fn tube_mercury() -> Theme {
    Theme {
        id: "tube-mercury".into(),
        name: "Mercury vapour".into(),
        lit: "#4fb8ff".into(),
        hot: "#d8f2ff".into(),
        panel: "#141a22".into(),
        panel_alpha: 1.0,
        edge: "#3f7ea8".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#2b3642".into(),
            chassis_bottom: "#0d1218".into(),
            socket: "#141a20".into(),
            collar: "#7d8f9e".into(),
            glass: "#cfe6f5".into(),
            ..TubeParams::default()
        },
        ..tube_base()
    }
}

/// Bakelite and brass, deep amber - the domestic radio set.
pub fn tube_bakelite() -> Theme {
    Theme {
        id: "tube-bakelite".into(),
        name: "Bakelite".into(),
        lit: "#ff6a18".into(),
        hot: "#ffc07a".into(),
        panel: "#2a1a10".into(),
        panel_alpha: 1.0,
        edge: "#8a5a28".into(),
        edge_alpha: 0.24,
        tube: TubeParams {
            chassis_top: "#4a2f1c".into(),
            chassis_bottom: "#180e07".into(),
            socket: "#2e1d10".into(),
            collar: "#b8862e".into(),
            glass: "#e8d8c0".into(),
            ..TubeParams::default()
        },
        ..tube_base()
    }
}

/// Nixie green, for a tube row that matches the matrix VFD.
pub fn tube_nixie_green() -> Theme {
    Theme {
        id: "tube-nixie-green".into(),
        name: "Nixie green".into(),
        lit: "#5cff9a".into(),
        hot: "#dcffe8".into(),
        panel: "#101a12".into(),
        panel_alpha: 1.0,
        edge: "#3f8a58".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#26382a".into(),
            chassis_bottom: "#0a120c".into(),
            socket: "#141c16".into(),
            collar: "#7f9a84".into(),
            glass: "#d4ecdc".into(),
            ..TubeParams::default()
        },
        ..tube_base()
    }
}

/// Copper and brass: a warm gold-glowing valve on an oxidised chassis.
pub fn tube_copper() -> Theme {
    Theme {
        id: "tube-copper".into(),
        name: "Copper".into(),
        lit: "#ffb347".into(),
        hot: "#ffe6b8".into(),
        panel: "#241505".into(),
        panel_alpha: 1.0,
        edge: "#a06a22".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#4a3218".into(),
            chassis_bottom: "#160d04".into(),
            socket: "#2a1a0a".into(),
            collar: "#d9a441".into(),
            glass: "#f0e0c0".into(),
            ..TubeParams::default()
        },
        ..tube_base()
    }
}

/// Red-plating: a valve being driven past its dissipation limit, anode glowing dull red.
///
/// A real fault condition, and the reason it earns a place is that it is the only colourway where
/// the glow colour is close to the plate's own darkness - which makes the climbing-glow cue carry
/// almost all of the reading, rather than brightness.
pub fn tube_red_plate() -> Theme {
    Theme {
        id: "tube-red-plate".into(),
        name: "Red plate".into(),
        lit: "#ff3b25".into(),
        hot: "#ffb9a0".into(),
        panel: "#1a0806".into(),
        panel_alpha: 1.0,
        edge: "#8f2a18".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#38201c".into(),
            chassis_bottom: "#120706".into(),
            socket: "#20100c".into(),
            collar: "#8f6a52".into(),
            glass: "#e8d0c8".into(),
            ..TubeParams::default()
        },
        ..tube_base()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of a #RRGGBB string.
    fn luminance(hex: &str) -> f32 {
        let h = hex.trim_start_matches('#');
        let ch = |i: usize| {
            let v = u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(0) + 0.7152 * ch(2) + 0.0722 * ch(4)
    }

    fn contrast(a: &str, b: &str) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn ships_all_five_segmented_colourways() {
        let ids: Vec<String> = all().iter().map(|t| t.id.clone()).collect();
        for want in ["vfd-ice", "matrix-green", "neon-pink", "vac-tube-orange", "classic-three-colour"] {
            assert!(ids.contains(&want.to_string()), "missing {want}");
        }
    }

    #[test]
    fn the_readme_states_the_real_colourway_and_family_counts() {
        // The README is the remote status dashboard - it is read when the repo cannot be
        // built, so a stale number there is a lie with no way to check it. It has drifted
        // before: it claimed 167 tests when there were 176, caught only because someone
        // happened to run the suite.
        //
        // Deliberately checks the COUNTS and not the prose. Asserting on wording would make
        // every copy edit a test failure, and the failure mode being guarded is a number
        // that no longer matches the code.
        let readme = include_str!("../../README.md");
        let colourways = all().len();
        let families = crate::render::KNOWN_FAMILIES.len();
        assert!(
            readme.contains(&format!("{colourways} colourways")),
            "README does not state the real colourway count ({colourways})"
        );
        assert!(
            readme.contains(&format!("{families} families")),
            "README does not state the real family count ({families})"
        );
        // And every family must be named somewhere in it, so a new one cannot ship unlisted.
        for fam in crate::render::KNOWN_FAMILIES {
            let label = crate::themes::family_label(fam);
            assert!(
                readme.contains(&label),
                "README never mentions the {label:?} family"
            );
        }
    }

    #[test]
    fn every_family_ships_and_no_theme_is_orphaned() {
        // Deliberately a floor, not an exact count. This asserted `len() == 15` and
        // exactly 5 per family, which made adding a colourway a test failure - directly
        // against the standing requirement that themes stay expandable. What it is
        // actually worth guarding is that no family silently loses its colourways and
        // that no theme carries a family name the renderer cannot dispatch on.
        let all = all();
        // Taken from the renderer rather than restated here, so adding a family cannot
        // leave this test asserting against a stale list - which is exactly what happened
        // when the vaporwave family landed.
        let mut counted = 0;
        for fam in crate::render::KNOWN_FAMILIES {
            let n = all.iter().filter(|t| t.family == fam).count();
            assert!(n >= 5, "family {fam} should ship at least 5 colourways, has {n}");
            counted += n;
        }
        assert_eq!(
            counted,
            all.len(),
            "every theme must belong to a known family; {} theme(s) have an unknown one",
            all.len() - counted
        );
    }

    #[test]
    fn every_id_is_unique() {
        let mut ids: Vec<String> = all().iter().map(|t| t.id.clone()).collect();
        let before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate theme ids would break override-by-id");
    }

    #[test]
    fn every_hue_of_a_rainbow_colourway_clears_three_to_one_against_its_panel() {
        // A rainbow colourway's `lit` hex is only what a non-rainbow build would draw, so the
        // ordinary contrast test cannot see the colours actually rendered - they are computed per
        // frame and sweep the whole wheel. This walks all 360 hues.
        //
        // The failure it guards is real and specific: fully saturated BLUE is too dark against a
        // near-black panel. Measured at value 1.0 against #080a0e - saturation 1.0 gives 2.31:1,
        // 0.9 gives 2.48, 0.8 gives 2.88, all failing this project's 3:1 rule; 0.70 is the first
        // that passes, at 3.59. That measurement is what set RAINBOW_SAT, and this test is what
        // stops someone "fixing" the rainbow by turning the saturation back up.
        let rainbows: Vec<Theme> = all().into_iter().filter(|t| t.rainbow > 0.0).collect();
        assert!(rainbows.len() >= 3, "expected a rainbow colourway per supporting family");
        for t in rainbows {
            let mut worst = (f32::MAX, 0u32);
            for deg in 0..360 {
                let (h, s, v) = crate::themes::rainbow_hsv(&t, deg as f32 / 360.0, 0.0, false)
                    .expect("a rainbow colourway must yield a colour");
                let (r, g, b) = hsv_to_rgb(h, s, v);
                let hex = format!("#{r:02x}{g:02x}{b:02x}");
                let c = contrast(&hex, &t.panel);
                if c < worst.0 {
                    worst = (c, deg as u32);
                }
            }
            assert!(
                worst.0 >= 3.0,
                "{}: hue {} only reaches {:.2}:1 against its panel {}",
                t.id,
                worst.1,
                worst.0,
                t.panel
            );
        }
    }

    /// Mirrors `Rgba::from_hsv` so the contrast check above measures the colours actually drawn.
    fn hsv_to_rgb(hue_turns: f32, sat: f32, val: f32) -> (u8, u8, u8) {
        let h = hue_turns.rem_euclid(1.0) * 6.0;
        let c = val * sat;
        let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
        let m = val - c;
        let (r, g, b) = match h as i32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        (
            ((r + m) * 255.0).round() as u8,
            ((g + m) * 255.0).round() as u8,
            ((b + m) * 255.0).round() as u8,
        )
    }

    #[test]
    fn every_lit_colour_clears_three_to_one_against_its_own_panel() {
        // The spec's hard requirement, computed rather than eyeballed.
        for t in all() {
            let ratio = contrast(&t.lit, &t.panel);
            assert!(ratio >= 3.0, "{}: lit {} vs panel {} = {ratio:.2}:1", t.id, t.lit, t.panel);
            for z in &t.zones {
                let zr = contrast(&z.lit, &t.panel);
                assert!(zr >= 3.0, "{} zone {}: {zr:.2}:1", t.id, z.lit);
            }
        }
    }

    #[test]
    fn every_panel_alpha_meets_the_floor() {
        // Below ~0.92 the Widgets button's own weather text shows through the panel.
        // This test asserted 0.55 and so failed to catch four colourways shipping at
        // 0.62-0.70 - the exact user-visible bug that had already been fixed once for
        // vfd-ice. The briefs for those tasks had been generated before that fix.
        for t in all() {
            assert!(t.panel_alpha >= 0.92, "{} has panel_alpha {}", t.id, t.panel_alpha);
        }
    }

    #[test]
    fn attack_always_outpaces_decay() {
        for t in all() {
            assert!(
                t.ballistics.attack > t.ballistics.decay,
                "{}: attack {} must exceed decay {}",
                t.id, t.ballistics.attack, t.ballistics.decay
            );
        }
    }

    #[test]
    fn zones_ascend_and_reach_the_top() {
        for t in all() {
            if t.zones.is_empty() {
                continue;
            }
            for pair in t.zones.windows(2) {
                assert!(pair[1].upto > pair[0].upto, "{}: zones must ascend", t.id);
            }
            assert!(
                t.zones.last().unwrap().upto >= 1.0,
                "{}: final zone must cover the top of the bar",
                t.id
            );
        }
    }

    #[test]
    fn only_the_classic_theme_uses_zones() {
        for t in all() {
            let expect_zones = t.id == "classic-three-colour";
            assert_eq!(!t.zones.is_empty(), expect_zones, "{}", t.id);
        }
    }

    #[test]
    fn classic_zones_are_green_amber_red_in_order() {
        let t = classic_three_colour();
        assert_eq!(t.zones.len(), 3);
        assert_eq!(t.lit_at(0.2), "#3ddc5a", "low = green (headroom)");
        assert_eq!(t.lit_at(0.7), "#ffc21f", "mid = amber (loud)");
        assert_eq!(t.lit_at(0.95), "#ff3b30", "top = red (peaking)");
    }

    #[test]
    fn colourways_are_visually_distinct_within_their_family() {
        // Guards against a theme being an unmodified copy with a new hex.
        // Grouped by family rather than compared across all fifteen: the scope
        // and vu families each share a single (texture, bloom) pair across all
        // five of their own colourways (`Texture::None_`/6.0-ish and
        // `Texture::Filament`/5.0 respectively), so comparing (texture, bloom)
        // globally would flag every scope-vs-scope and vu-vs-vu pair as
        // "identical" even though their `lit`/`fade` genuinely differ.
        for fam in ["segmented", "scope", "vu"] {
            let mut sigs: Vec<String> = all()
                .iter()
                .filter(|t| t.family == fam)
                .map(|t| format!("{:?}/{}/{}/{}", t.texture, t.bloom, t.lit, t.fade))
                .collect();
            let before = sigs.len();
            sigs.sort();
            sigs.dedup();
            assert_eq!(sigs.len(), before, "two {fam} themes are identical");
        }
    }
}
