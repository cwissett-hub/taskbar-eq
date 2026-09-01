use super::{OrbitParams, 
    ChromaParams, FluidParams, PantoneParams, RadarParams, Texture, Theme, TubeParams, VaporParams,
    Zone,
};
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
        flame_sodium(),
        flame_propane(),
        flame_copper(),
        flame_strontium(),
        flame_potassium(),
        flame_plasma(),
        flame_rainbow(),
        dolphin_sony_amber(),
        dolphin_jvc_ice(),
        dolphin_pioneer_green(),
        dolphin_xplod_orange(),
        dolphin_kenwood_red(),
        mesh_wmp_cyan(),
        mesh_winamp_green(),
        mesh_amber_stack(),
        mesh_magenta_deck(),
        mesh_white_stack(),
        pipes_win95_teal(),
        pipes_brass(),
        pipes_neon_magenta(),
        pipes_copper(),
        pipes_chrome(),
        orbit_chrome(),
        orbit_sodium(),
        orbit_plasma(),
        orbit_ion(),
        orbit_lime(),
        orbit_rainbow(),
        orbit_solo(),
        orbit_swarm(),
        blossom_dusk(),
        blossom_night(),
        blossom_lantern(),
        blossom_plum(),
        blossom_gold(),
        nixie_orange(),
        nixie_ice(),
        nixie_neon_green(),
        nixie_magenta(),
        nixie_aged(),
        waterfall_heat(),
        waterfall_ice(),
        waterfall_viridis(),
        waterfall_mono(),
        waterfall_inferno(),
        reel_studio_grey(),
        reel_wood_console(),
        reel_black_chrome(),
        reel_olive_military(),
        reel_cream_domestic(),
        reel_neon_miami(),
        reel_acid_lime(),
        reel_pantone_press(),
        patch_classic(),
        patch_buchla(),
        patch_noir(),
        patch_rainbow(),
        patch_uv(),
        radar_p1(),
        radar_amber(),
        radar_ice(),
        radar_alert(),
        radar_mono(),
        radar_nato(),
        radar_ru(),
        radar_cn(),
        fluid_deep(),
        fluid_mercury(),
        fluid_oil(),
        fluid_coolant(),
        fluid_ink(),
        fluid_pantone(),
        pantone_spectrum(),
        pantone_process(),
        pantone_barcode(),
        pantone_misregister(),
        pantone_halftone(),
        vfd_pantone(),
        scope_pantone(),
        vu_pantone(),
        chroma_spectrum(),
        chroma_cmyk(),
        chroma_barcode(),
        chroma_misreg(),
        chroma_halftone(),
        chroma_riso(),
        chroma_duotone(),
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

// ===================== Nixie tubes =====================
/// Shared skeleton for the nixie row.
///
/// `ghost` earns its keep here rather than being switched off as the valve row does: in a nixie the
/// nine UNLIT cathodes are visible wire, and they are what tells you how high the struck digit is
/// sitting. At 0.0 the family degenerates to a single floating digit with no scale behind it.
fn nixie_base() -> Theme {
    Theme {
        family: "nixie".into(),
        // No panel texture: the tubes already carry their own interior, rails and rim, and a
        // scanline or grille pattern over a 3px glyph competes with the digit for the same pixels.
        texture: Texture::None_,
        ghost: 0.13,
        // Tight, for the same reason as the valve row: the envelopes are 9px apart, and a wide
        // blur welds the row into one bar. Brightness comes from glow_strength, not radius.
        bloom: 4.0,
        glow_strength: 0.60,
        ..Theme::default()
    }
}

/// The reference: IN-12 neon orange in clear glass on a dark chassis.
pub fn nixie_orange() -> Theme {
    Theme {
        id: "nixie-orange".into(),
        name: "Nixie orange".into(),
        lit: "#ff7a1a".into(),
        hot: "#ffd9a8".into(),
        panel: "#0d0a06".into(),
        panel_alpha: 1.0,
        edge: "#8a5a28".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#3a3128".into(),
            chassis_bottom: "#14110c".into(),
            internals: "#080604".into(),
            socket: "#241a10".into(),
            collar: "#8a6a2a".into(),
            glass: "#e8dcc8".into(),
            ..TubeParams::default()
        },
        ..nixie_base()
    }
}

/// Cold blue-white - the argon-filled tubes, not the neon ones.
pub fn nixie_ice() -> Theme {
    Theme {
        id: "nixie-ice".into(),
        name: "Nixie ice".into(),
        lit: "#a8dcff".into(),
        hot: "#f0faff".into(),
        panel: "#05090f".into(),
        panel_alpha: 1.0,
        edge: "#4a7ea8".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#2a3440".into(),
            chassis_bottom: "#0c1016".into(),
            internals: "#04070a".into(),
            socket: "#141a20".into(),
            collar: "#8d9aa8".into(),
            glass: "#dcecf8".into(),
            ..TubeParams::default()
        },
        ..nixie_base()
    }
}

/// Green, for a row that matches the Matrix VFD.
pub fn nixie_neon_green() -> Theme {
    Theme {
        id: "nixie-green".into(),
        name: "Nixie green".into(),
        lit: "#5cff9a".into(),
        hot: "#e0ffec".into(),
        panel: "#050e08".into(),
        panel_alpha: 1.0,
        edge: "#3f8a58".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#26362a".into(),
            chassis_bottom: "#0a120c".into(),
            internals: "#040806".into(),
            socket: "#141c16".into(),
            collar: "#7f9a84".into(),
            glass: "#d8ece0".into(),
            ..TubeParams::default()
        },
        ..nixie_base()
    }
}

/// Magenta. No real nixie ever glowed this colour; it is here because the row reads well against a
/// violet chassis and the family should not be five shades of orange.
pub fn nixie_magenta() -> Theme {
    Theme {
        id: "nixie-magenta".into(),
        name: "Nixie magenta".into(),
        lit: "#ff5ce0".into(),
        hot: "#ffd4f6".into(),
        panel: "#0c0510".into(),
        panel_alpha: 1.0,
        edge: "#8a3f9a".into(),
        edge_alpha: 0.24,
        tube: TubeParams {
            chassis_top: "#382a42".into(),
            chassis_bottom: "#120c18".into(),
            internals: "#080410".into(),
            socket: "#1e1424".into(),
            collar: "#9a7fa8".into(),
            glass: "#ecdcf4".into(),
            ..TubeParams::default()
        },
        ..nixie_base()
    }
}

/// Sixty years in a drawer: a browned envelope, sputtered glass and a tired cathode.
///
/// The dim look is spent on the ENVELOPE rather than on `lit`, which stays bright enough to clear
/// 3:1 against its own panel - an aged tube is one you read through cloudy glass, not one you
/// cannot read. `glass` is a dirty grey-brown instead of near-white so the rim reads as sputtered
/// deposit, `glow_strength` is down because a tired cathode's discharge does not reach the walls,
/// and `ghost` is UP because on an old tube the unlit cathodes have gone visibly cloudy.
pub fn nixie_aged() -> Theme {
    Theme {
        id: "nixie-aged".into(),
        name: "Nixie aged".into(),
        lit: "#d98a3a".into(),
        hot: "#f2c68a".into(),
        panel: "#0a0908".into(),
        panel_alpha: 1.0,
        edge: "#6a5540".into(),
        edge_alpha: 0.18,
        ghost: 0.18,
        bloom: 5.0,
        glow_strength: 0.34,
        tube: TubeParams {
            chassis_top: "#2e2a22".into(),
            chassis_bottom: "#100e0a".into(),
            internals: "#0a0806".into(),
            socket: "#1e1810".into(),
            collar: "#6e5a34".into(),
            glass: "#8f8676".into(),
            ..TubeParams::default()
        },
        ..nixie_base()
    }
}

// ===================== Spectrogram =====================
fn waterfall_base() -> Theme {
    Theme {
        family: "waterfall".into(),
        texture: Texture::None_,
        ghost: 0.0,
        // Tight, and tighter than any other family's. The bloom kernel is separable and blurs
        // along the TIME axis as well as the frequency axis, so a wide radius smears three
        // seconds of history into a wash - a snare stops being a 1px column and becomes a
        // gradient. At 2 it only softens the boundary between a lit cell and the dark panel
        // beside it, which is where a spectrogram wants its glow.
        bloom: 2.0,
        glow_strength: 0.45,
        // Snappier than the segmented defaults every family inherits. A spectrogram's whole
        // subject is WHEN something happened, and a 143ms decay smears the onset across nine
        // columns of the plot, which is 9px at the reference width.
        ballistics: Ballistics { attack: 0.85, decay: 0.26, peak_fall: 0.006 },
        ..Theme::default()
    }
}

/// Classic heat: black through red and yellow to white.
///
/// `zones` are the ramp's authored stops - see `render::waterfall::ramp_stops`. There is no black
/// stop because there does not need to be one: the ramp's bottom stop is synthesised fully
/// transparent, so the near-black panel IS the black end. That is also what lets every authored
/// stop stay bright enough to clear the project's 3:1 contrast rule against its own panel, which a
/// real dark-red stop could not.
pub fn waterfall_heat() -> Theme {
    Theme {
        id: "waterfall-heat".into(),
        name: "Heat".into(),
        // lit/hot are the fallback ramp for an external theme that declares no zones, so they are
        // the mid and top of this ramp rather than an unrelated accent pair.
        lit: "#ffb02a".into(),
        hot: "#fff4de".into(),
        panel: "#0a0503".into(),
        panel_alpha: 1.0,
        edge: "#d4622a".into(),
        edge_alpha: 0.20,
        zones: vec![
            Zone { upto: 0.30, lit: "#ff3b0f".into(), hot: "#ff6a2a".into() },
            Zone { upto: 0.64, lit: "#ffb02a".into(), hot: "#ffd06a".into() },
            Zone { upto: 1.01, lit: "#fff4de".into(), hot: "#ffffff".into() },
        ],
        ..waterfall_base()
    }
}

/// Ice: black through blue and cyan to white.
pub fn waterfall_ice() -> Theme {
    Theme {
        id: "waterfall-ice".into(),
        name: "Ice".into(),
        lit: "#35e8ff".into(),
        hot: "#eafcff".into(),
        panel: "#02060c".into(),
        panel_alpha: 1.0,
        edge: "#3f8fd8".into(),
        edge_alpha: 0.20,
        zones: vec![
            Zone { upto: 0.30, lit: "#2f6cff".into(), hot: "#5f8fff".into() },
            Zone { upto: 0.64, lit: "#35e8ff".into(), hot: "#8ff2ff".into() },
            Zone { upto: 1.01, lit: "#eafcff".into(), hot: "#ffffff".into() },
        ],
        ..waterfall_base()
    }
}

/// Viridis, the perceptually-uniform default of every scientific spectrogram.
///
/// The real viridis starts at #440154, a dark purple with a contrast of 1.4:1 against a near-black
/// panel - it would be invisible AND would fail the project's contrast rule. The ramp therefore
/// starts at viridis' teal (#31688e, 3.3:1) and lets the transparent floor supply the dark end,
/// which is the same thing the eye sees on a scientific plot anyway: the deep purple there is
/// doing the job of "background".
pub fn waterfall_viridis() -> Theme {
    Theme {
        id: "waterfall-viridis".into(),
        name: "Viridis".into(),
        lit: "#35b779".into(),
        hot: "#fde725".into(),
        panel: "#04080a".into(),
        panel_alpha: 1.0,
        edge: "#3f8f7a".into(),
        edge_alpha: 0.20,
        zones: vec![
            Zone { upto: 0.30, lit: "#31688e".into(), hot: "#3f86b4".into() },
            Zone { upto: 0.62, lit: "#35b779".into(), hot: "#6fd89a".into() },
            Zone { upto: 1.01, lit: "#fde725".into(), hot: "#ffffff".into() },
        ],
        ..waterfall_base()
    }
}

/// Monochrome, for the sonagraph look - and the one colourway where the ramp cannot hide a
/// mistake, since every step differs only in luminance.
pub fn waterfall_mono() -> Theme {
    Theme {
        id: "waterfall-mono".into(),
        name: "Monochrome".into(),
        lit: "#c2cad2".into(),
        hot: "#ffffff".into(),
        panel: "#060708".into(),
        panel_alpha: 1.0,
        edge: "#8a949e".into(),
        edge_alpha: 0.20,
        zones: vec![
            Zone { upto: 0.32, lit: "#7f8a94".into(), hot: "#9aa4ae".into() },
            Zone { upto: 0.66, lit: "#c2cad2".into(), hot: "#dde2e8".into() },
            Zone { upto: 1.01, lit: "#ffffff".into(), hot: "#ffffff".into() },
        ],
        ..waterfall_base()
    }
}

/// Inferno: purple through orange to pale yellow.
///
/// Same compromise as viridis at the dark end - inferno's #420a68 and #932667 are both under
/// 3:1 against this panel (2.5:1 for the magenta), so the ramp opens at a brighter magenta and
/// the transparent floor carries the near-black.
pub fn waterfall_inferno() -> Theme {
    Theme {
        id: "waterfall-inferno".into(),
        name: "Inferno".into(),
        lit: "#f2701e".into(),
        hot: "#fcffa4".into(),
        panel: "#08040a".into(),
        panel_alpha: 1.0,
        edge: "#a8407e".into(),
        edge_alpha: 0.20,
        zones: vec![
            Zone { upto: 0.28, lit: "#b5307a".into(), hot: "#d0559a".into() },
            Zone { upto: 0.60, lit: "#f2701e".into(), hot: "#ffa04a".into() },
            Zone { upto: 1.01, lit: "#fcffa4".into(), hot: "#ffffff".into() },
        ],
        ..waterfall_base()
    }
}

// ===================== Reel-to-reel =====================
/// The reel-to-reel family reuses `TubeParams` rather than introducing a table of its own, and
/// the mapping is close enough to be worth stating once here instead of in five colourways:
///
/// | field | on a tape deck |
/// |---|---|
/// | `chassis_top` / `chassis_bottom` | the deck plate, lit from above |
/// | `internals` | the tape itself, and the head stack - the darkest thing on the deck |
/// | `socket` | the reel flange (the visible disc) |
/// | `collar` | rim, hub, screws and the head's top face - chrome or brass |
/// | `glass` | the sheen along the top edge of the tape, and the head gap |
///
/// `internals` must stay clearly DARKER than `chassis_bottom`: the tape is read as a dark curve
/// against the plate, and the plate's gradient is at its darkest exactly where the tape sags to
/// at full level. A theme that gets this wrong loses the family's position cue, not just some
/// contrast.
fn reel_base() -> Theme {
    Theme {
        family: "reel".into(),
        // The plate has its own gradient; a segmented-style overlay texture on top of it just
        // muddies the reels.
        texture: Texture::None_,
        // Dormant strip bars. Without them the record-level meter reads as switched off between
        // notes, which on a deck is the one thing that should never look off.
        ghost: 0.12,
        // Tight, for the same reason as the valve row: the spokes are 3-6px wide and a wide blur
        // welds the three arms into a bright disc, which is precisely the invisible-rotation
        // failure the spokes exist to avoid.
        bloom: 4.0,
        glow_strength: 0.55,
        ..Theme::default()
    }
}

/// The reference: a grey broadcast machine, chrome rims, cold blue-white meters.
pub fn reel_studio_grey() -> Theme {
    Theme {
        id: "reel-studio-grey".into(),
        name: "Studio grey".into(),
        lit: "#b9d8ff".into(),
        hot: "#eaf4ff".into(),
        panel: "#22252a".into(),
        panel_alpha: 1.0,
        edge: "#7d8894".into(),
        edge_alpha: 0.20,
        tube: TubeParams {
            chassis_top: "#4a505c".into(),
            chassis_bottom: "#191c22".into(),
            internals: "#0a0b0e".into(),
            socket: "#2e333c".into(),
            collar: "#aab4c2".into(),
            glass: "#dfe8f2".into(),
            ..TubeParams::default()
        },
        ..reel_base()
    }
}

/// Walnut end cheeks and brass - the domestic hi-fi console.
pub fn reel_wood_console() -> Theme {
    Theme {
        id: "reel-wood-console".into(),
        name: "Warm wood console".into(),
        lit: "#ffd08a".into(),
        hot: "#fff2d8".into(),
        panel: "#2a1d12".into(),
        panel_alpha: 1.0,
        edge: "#9a7040".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#5a3d24".into(),
            chassis_bottom: "#1b1108".into(),
            internals: "#0d0906".into(),
            socket: "#3a2716".into(),
            collar: "#c79a4a".into(),
            glass: "#f0dcb8".into(),
            ..TubeParams::default()
        },
        ..reel_base()
    }
}

/// Near-black plate, bright chrome, white meters - the mastering deck.
pub fn reel_black_chrome() -> Theme {
    Theme {
        id: "reel-black-chrome".into(),
        name: "Black and chrome".into(),
        lit: "#e8f2ff".into(),
        hot: "#ffffff".into(),
        panel: "#0e1013".into(),
        panel_alpha: 1.0,
        edge: "#98a6b6".into(),
        edge_alpha: 0.20,
        tube: TubeParams {
            chassis_top: "#33383f".into(),
            // Not black. The plate's gradient bottoms out where the tape sags to, and at
            // #0a0c0f the tape and the plate were within 4 luminance of each other there - the
            // sag was invisible at exactly the levels it matters most.
            chassis_bottom: "#141821".into(),
            internals: "#06070b".into(),
            socket: "#1c2027".into(),
            collar: "#c2ccd8".into(),
            glass: "#ffffff".into(),
            ..TubeParams::default()
        },
        ..reel_base()
    }
}

/// Olive drab and brass - the field recorder.
pub fn reel_olive_military() -> Theme {
    Theme {
        id: "reel-olive-military".into(),
        name: "Olive military".into(),
        lit: "#d4e8a8".into(),
        hot: "#f2ffd8".into(),
        panel: "#232a1c".into(),
        panel_alpha: 1.0,
        edge: "#6f7a52".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#47513a".into(),
            chassis_bottom: "#171c11".into(),
            internals: "#090b07".into(),
            socket: "#333c26".into(),
            collar: "#8a6a2a".into(),
            glass: "#cfe0d8".into(),
            ..TubeParams::default()
        },
        ..reel_base()
    }
}

/// Cream plastic reels and gold trim - the 1960s domestic machine. The only colourway where the
/// FLANGE is the lightest thing on the deck, which is why the wound tape pack has to stay dark:
/// the spokes are read against the pack, not against the flange.
pub fn reel_cream_domestic() -> Theme {
    Theme {
        id: "reel-cream-domestic".into(),
        name: "Cream domestic".into(),
        lit: "#ffe2aa".into(),
        hot: "#fff6e0".into(),
        panel: "#1b1610".into(),
        panel_alpha: 1.0,
        edge: "#b89a68".into(),
        edge_alpha: 0.20,
        tube: TubeParams {
            chassis_top: "#3e372c".into(),
            chassis_bottom: "#14110c".into(),
            internals: "#0b0806".into(),
            socket: "#d8cbb0".into(),
            collar: "#b8a888".into(),
            glass: "#fff6e0".into(),
            ..TubeParams::default()
        },
        ..reel_base()
    }
}

// The three below were asked for as "brighter and more in keeping with our other colours, think
// pantone and neon". The five above are all authentic machines - broadcast grey, walnut, black
// chrome, olive drab, cream plastic - which is a faithful palette and, reported from use, a boring
// one. These borrow the language the vaporwave, pantone and flame families already speak instead.
//
// The constraint that shapes all three: the SPOKES are read against the wound tape pack, not against
// the flange (see `reel_cream_domestic`). So a bright flange needs `internals` kept near-black or the
// rotation stops being visible - which is the one failure this family cannot afford, since a reel
// that does not visibly turn is just a circle.

/// Hot magenta deck, electric cyan flanges - the Miami machine.
pub fn reel_neon_miami() -> Theme {
    Theme {
        id: "reel-neon-miami".into(),
        name: "Neon Miami".into(),
        lit: "#ff54d6".into(),
        hot: "#ffd8f4".into(),
        panel: "#150a20".into(),
        panel_alpha: 1.0,
        edge: "#b03cc8".into(),
        edge_alpha: 0.28,
        tube: TubeParams {
            chassis_top: "#4a1060".into(),
            chassis_bottom: "#17061f".into(),
            internals: "#10050f".into(),
            socket: "#2de0f0".into(),
            collar: "#7cf3ff".into(),
            glass: "#ffe9fb".into(),
            ..TubeParams::default()
        },
        ..reel_base()
    }
}

/// Acid lime flanges against charcoal, magenta trim.
pub fn reel_acid_lime() -> Theme {
    Theme {
        id: "reel-acid-lime".into(),
        name: "Acid lime".into(),
        lit: "#c6ff3d".into(),
        hot: "#f2ffd0".into(),
        panel: "#101408".into(),
        panel_alpha: 1.0,
        edge: "#7fbf2a".into(),
        edge_alpha: 0.26,
        tube: TubeParams {
            chassis_top: "#29331a".into(),
            chassis_bottom: "#0c1005".into(),
            internals: "#070a04".into(),
            socket: "#b8ff36".into(),
            collar: "#ff5fa8".into(),
            glass: "#f4ffe0".into(),
            ..TubeParams::default()
        },
        ..reel_base()
    }
}

/// Flat process inks - yellow flanges, warm red trim - the press-room deck.
///
/// The one of the three that is not neon: Pantone's brightness comes from FLAT saturated ink on a
/// near-black plate rather than from a glow, so the chassis stays deliberately neutral and lets the
/// two inks carry it.
pub fn reel_pantone_press() -> Theme {
    Theme {
        id: "reel-pantone-press".into(),
        name: "Pantone press".into(),
        lit: "#ffd400".into(),
        hot: "#fff8d0".into(),
        panel: "#0e1013".into(),
        panel_alpha: 1.0,
        edge: "#d8b400".into(),
        edge_alpha: 0.24,
        tube: TubeParams {
            chassis_top: "#23262b".into(),
            chassis_bottom: "#0b0d10".into(),
            internals: "#08090b".into(),
            socket: "#ffcf00".into(),
            collar: "#ff3b5c".into(),
            glass: "#fffbe8".into(),
            ..TubeParams::default()
        },
        ..reel_base()
    }
}

// ===================== Patchbay =====================
fn patchbay_base() -> Theme {
    Theme {
        family: "patchbay".into(),
        texture: Texture::None_,
        ghost: 0.0,
        // Tight. The only thing that blooms is a 1px core down each cable, and a wider blur
        // welds neighbouring cables into one band - at which point the SAG, which is the whole
        // cue this family rests on, cannot be read at all.
        bloom: 4.0,
        glow_strength: 0.50,
        ..Theme::default()
    }
}

/// The reference: black anodised panel, nickel hardware, primary-coloured cables.
///
/// The cables are coloured through `zones`, not by `lit` alone. A patchbay with one cable
/// colour reads as a harp; the whole point of the hardware is that you can tell one patch from
/// another at a glance, and `lit_at`/`hot_at` already exist to index a colour by position, so
/// this needs nothing added to the theme schema. One stop per cable at the reference width, so
/// every cable in the row is a different colour - the earlier version cycled only three and read
/// as a repeat.
pub fn patch_classic() -> Theme {
    Theme {
        id: "patch-classic".into(),
        name: "Classic black".into(),
        lit: "#ff4a3d".into(),
        hot: "#ffcdc7".into(),
        panel: "#0e0f11".into(),
        panel_alpha: 1.0,
        edge: "#9aa3ad".into(),
        edge_alpha: 0.20,
        tube: TubeParams {
            chassis_top: "#2a2c30".into(),
            chassis_bottom: "#0a0b0d".into(),
            internals: "#050607".into(),
            socket: "#08090a".into(),
            collar: "#b9bfc7".into(),
            glass: "#e6ecf2".into(),
        },
        // One stop per cable, so every cable is a different colour and the colourway is
        // identifiable by its cables and not only by its panel. `lit_at` maps a cable's
        // position along the row onto these, which is why they are zones rather than a new
        // theme field.
        zones: vec![
            Zone { upto: 0.200, lit: "#ff4a3d".into(), hot: "#ff4a3d".into() },
            Zone { upto: 0.400, lit: "#ffd22e".into(), hot: "#ffd22e".into() },
            Zone { upto: 0.600, lit: "#4a9cff".into(), hot: "#4a9cff".into() },
            Zone { upto: 0.800, lit: "#e8e4dc".into(), hot: "#e8e4dc".into() },
            Zone { upto: 1.000, lit: "#8d94a0".into(), hot: "#8d94a0".into() },
        ],
        ..patchbay_base()
    }
}

/// Buchla-ish: an aged ivory panel with deep saturated cables.
///
/// The one deliberate inversion in the set, and it is forced by the contrast floor rather than
/// chosen: on a cream panel a cable has to be DARK to clear 3:1, so real Buchla's bright banana
/// leads are not available here. The consequence to know about is that the brightening cue runs
/// backwards on this colourway - an idle cable is near-black and therefore at its most contrasty,
/// and driving it lightens the body while the `hot` core appears as a highlight down its middle.
/// The sag is unaffected, which is why this is acceptable: position is the primary cue and
/// brightness was only ever the confirmation.
pub fn patch_buchla() -> Theme {
    Theme {
        id: "patch-buchla".into(),
        name: "Buchla cream".into(),
        lit: "#7d2b1c".into(),
        hot: "#d4643a".into(),
        panel: "#c0b49c".into(),
        panel_alpha: 1.0,
        edge: "#6a5f4a".into(),
        edge_alpha: 0.38,
        tube: TubeParams {
            chassis_top: "#d6cbb4".into(),
            chassis_bottom: "#a89a80".into(),
            internals: "#3a3226".into(),
            socket: "#4a4132".into(),
            collar: "#8f8570".into(),
            glass: "#fffaf0".into(),
        },
        // One stop per cable, so every cable is a different colour and the colourway is
        // identifiable by its cables and not only by its panel. `lit_at` maps a cable's
        // position along the row onto these, which is why they are zones rather than a new
        // theme field.
        zones: vec![
            Zone { upto: 0.200, lit: "#7d2b1c".into(), hot: "#7d2b1c".into() },
            Zone { upto: 0.400, lit: "#1f4f52".into(), hot: "#1f4f52".into() },
            Zone { upto: 0.600, lit: "#3f4a1e".into(), hot: "#3f4a1e".into() },
            Zone { upto: 0.800, lit: "#2a2f6b".into(), hot: "#2a2f6b".into() },
            Zone { upto: 1.000, lit: "#5a2350".into(), hot: "#5a2350".into() },
        ],
        ..patchbay_base()
    }
}

/// All-black panel, white cables. The austere one.
pub fn patch_noir() -> Theme {
    Theme {
        id: "patch-noir".into(),
        name: "Noir".into(),
        lit: "#f2f5f8".into(),
        hot: "#ffffff".into(),
        panel: "#08080a".into(),
        panel_alpha: 1.0,
        edge: "#7e838a".into(),
        edge_alpha: 0.18,
        // No zones: one cable colour is the point here, so the sag is carrying the whole
        // reading. Also the case that proves `lit_at` degrades to `lit` with no special case.
        tube: TubeParams {
            chassis_top: "#1c1c20".into(),
            chassis_bottom: "#050506".into(),
            internals: "#000000".into(),
            socket: "#040405".into(),
            collar: "#9ba0a6".into(),
            glass: "#f4f7fa".into(),
        },
        // One stop per cable, so every cable is a different colour and the colourway is
        // identifiable by its cables and not only by its panel. `lit_at` maps a cable's
        // position along the row onto these, which is why they are zones rather than a new
        // theme field.
        zones: vec![
            Zone { upto: 0.200, lit: "#ffffff".into(), hot: "#ffffff".into() },
            Zone { upto: 0.400, lit: "#cfd6de".into(), hot: "#cfd6de".into() },
            Zone { upto: 0.600, lit: "#9aa3ad".into(), hot: "#9aa3ad".into() },
            Zone { upto: 0.800, lit: "#e6ecf2".into(), hot: "#e6ecf2".into() },
            Zone { upto: 1.000, lit: "#b6bec8".into(), hot: "#b6bec8".into() },
        ],
        ..patchbay_base()
    }
}

/// Rainbow cables on graphite grey - the "which one is which" panel.
pub fn patch_rainbow() -> Theme {
    Theme {
        id: "patch-rainbow".into(),
        name: "Rainbow on grey".into(),
        lit: "#ffc21f".into(),
        hot: "#fff2c0".into(),
        // Graphite rather than mid-grey. A genuinely mid grey cannot clear 3:1 against
        // saturated cables in either direction (measured: #33343a puts a red cable at 2.7:1),
        // so "grey" here means dark grey with the brushed gradient doing the rest.
        // Darkened from #2a2b30 (luminance 43). Cables are dimmed to SHEATH_IDLE when no signal
        // flows, and against a MID grey the red and the violet landed at luminance 47 and 55 -
        // within 4 and 12 of the panel, i.e. invisible on an idle patchbay, which is the one thing
        // this family must never look like. Measured: a panel below luminance 22 clears all five by
        // at least 26. It is still visibly grey rather than black.
        panel: "#14151a".into(),
        panel_alpha: 1.0,
        edge: "#b0b6bf".into(),
        edge_alpha: 0.22,
        tube: TubeParams {
            chassis_top: "#4a4c54".into(),
            chassis_bottom: "#1d1e22".into(),
            internals: "#111216".into(),
            socket: "#16171a".into(),
            collar: "#c4cad2".into(),
            glass: "#eef2f6".into(),
        },
        // One stop per cable, so every cable is a different colour and the colourway is
        // identifiable by its cables and not only by its panel. `lit_at` maps a cable's
        // position along the row onto these, which is why they are zones rather than a new
        // theme field.
        zones: vec![
            // Brightened from #ff4a3d. Dimmed to SHEATH_IDLE at silence it measured 70.6 against
            // the brushed jack strip at 48.3 - a separation of 22 where the visibility floor is 25,
            // so an idle cable faded into the panel. Note the strip's brightness comes from the
            // jack row, NOT from `panel`: darkening the panel was the obvious fix and moved the
            // measurement not at all, which is what the test proved.
            //
            // Latent until now - the old zone boundaries meant this red was never selected for any
            // cable, so giving every cable its own colour is what surfaced it.
            Zone { upto: 0.200, lit: "#ff6f5f".into(), hot: "#ff6f5f".into() },
            Zone { upto: 0.400, lit: "#ffc21f".into(), hot: "#ffc21f".into() },
            Zone { upto: 0.600, lit: "#3ddc5a".into(), hot: "#3ddc5a".into() },
            Zone { upto: 0.800, lit: "#35c8ff".into(), hot: "#35c8ff".into() },
            Zone { upto: 1.000, lit: "#b06aff".into(), hot: "#b06aff".into() },
        ],
        ..patchbay_base()
    }
}

/// Blacklight: a violet-black panel with UV-reactive cables.
///
/// The only colourway with the glow turned up. Everything else in the set keeps `glow_strength`
/// at 0.50 so the cables stay separable, but a UV look is the one case where the halo IS the
/// look, so the trade is taken knowingly: the bloom radius stays at the family's 4.0, since
/// widening that (rather than brightening it) is what merges neighbouring cables.
pub fn patch_uv() -> Theme {
    Theme {
        id: "patch-uv".into(),
        name: "Blacklight".into(),
        lit: "#ff3df0".into(),
        hot: "#ffd6fb".into(),
        panel: "#0a0618".into(),
        panel_alpha: 1.0,
        edge: "#8f5fd0".into(),
        edge_alpha: 0.26,
        glow_strength: 0.72,
        tube: TubeParams {
            chassis_top: "#241a3a".into(),
            chassis_bottom: "#070410".into(),
            internals: "#050208".into(),
            socket: "#0d0720".into(),
            collar: "#7a5fd0".into(),
            glass: "#e0c8ff".into(),
        },
        // One stop per cable, so every cable is a different colour and the colourway is
        // identifiable by its cables and not only by its panel. `lit_at` maps a cable's
        // position along the row onto these, which is why they are zones rather than a new
        // theme field.
        zones: vec![
            Zone { upto: 0.200, lit: "#ff3df0".into(), hot: "#ff3df0".into() },
            Zone { upto: 0.400, lit: "#3dfff0".into(), hot: "#3dfff0".into() },
            Zone { upto: 0.600, lit: "#c8ff3d".into(), hot: "#c8ff3d".into() },
            Zone { upto: 0.800, lit: "#9a5cff".into(), hot: "#9a5cff".into() },
            Zone { upto: 1.000, lit: "#ff5c9a".into(), hot: "#ff5c9a".into() },
        ],
        ..patchbay_base()
    }
}


// ===================== Radar =====================
fn radar_base() -> Theme {
    Theme {
        family: "radar".into(),
        texture: Texture::None_,
        ghost: 0.0,
        // Tight, and dimmer than the valve row's. The face carries up to 32 blips plus a
        // beam, and a wide halo on that many small marks welds neighbouring bearings into one
        // arc - which destroys the only thing the display is for.
        bloom: 4.0,
        glow_strength: 0.42,
        // Repurposed as phosphor persistence by the radar family (as `scope` already does for
        // its trace), so it is the knob for how much history the wake shows. 0.30 puts the
        // oldest return at ~12% brightness after one sweep.
        fade: 0.30,
        ..Theme::default()
    }
}

/// The reference: P1 phosphor, the green every real PPI used.
pub fn radar_p1() -> Theme {
    Theme {
        id: "radar-p1".into(),
        name: "P1 green".into(),
        lit: "#3dff7a".into(),
        hot: "#d8ffe6".into(),
        panel: "#020a05".into(),
        panel_alpha: 1.0,
        edge: "#3f8a58".into(),
        edge_alpha: 0.20,
        ..radar_base()
    }
}

/// Amber, the later CRT stock - warmer and easier to read for long watches.
pub fn radar_amber() -> Theme {
    Theme {
        id: "radar-amber".into(),
        name: "Amber".into(),
        lit: "#ffb43c".into(),
        hot: "#ffe8c4".into(),
        panel: "#0f0902".into(),
        panel_alpha: 1.0,
        edge: "#a8762a".into(),
        edge_alpha: 0.20,
        // A longer wake than P1: amber tubes were the slow ones, and it also gives a second
        // colourway with visibly more history rather than just a different hue.
        fade: 0.46,
        ..radar_base()
    }
}

pub fn radar_ice() -> Theme {
    Theme {
        id: "radar-ice".into(),
        name: "Ice blue".into(),
        lit: "#7fdcff".into(),
        hot: "#e4f8ff".into(),
        panel: "#020b12".into(),
        panel_alpha: 1.0,
        edge: "#3f83a8".into(),
        edge_alpha: 0.20,
        ..radar_base()
    }
}

/// Red alert. Red is the darkest usable accent here, so the halo is pushed up to compensate -
/// at the shared 0.42 the blips read as maroon smudges against the panel rather than as
/// contacts.
pub fn radar_alert() -> Theme {
    Theme {
        id: "radar-alert".into(),
        name: "Red alert".into(),
        lit: "#ff4436".into(),
        hot: "#ffcac4".into(),
        panel: "#0d0303".into(),
        panel_alpha: 1.0,
        edge: "#a83028".into(),
        edge_alpha: 0.22,
        glow_strength: 0.55,
        // Short: an alert display should look urgent, and less history means the beam is the
        // dominant feature.
        fade: 0.20,
        ..radar_base()
    }
}

pub fn radar_mono() -> Theme {
    Theme {
        id: "radar-mono".into(),
        name: "Monochrome".into(),
        lit: "#dbe6f2".into(),
        hot: "#ffffff".into(),
        panel: "#05070b".into(),
        panel_alpha: 1.0,
        edge: "#8fa4b8".into(),
        edge_alpha: 0.18,
        ..radar_base()
    }
}


/// NATO/US receiver, completing the set with RU and CN: whose hardware you are looking at decides
/// what the scope can say.
///
/// The threat library here is the proper NATO one, and it is the mirror image of `radar_ru`'s. A
/// Western receiver annotates Soviet and Russian systems, and those are known by their NATO REPORTING
/// numbers - the SA-series - where a Russian receiver annotating Western hardware uses US designation
/// numbers. So a `6` here is an SA-6 Gainful and a `10` an SA-10 Grumble, while a `104` on the RU scope
/// is a MIM-104 Patriot. Same display, opposite libraries.
///
/// Deliberately weighted to threats rather than to friendly emitters: `M` for MiG-family airborne
/// intercept radars, `A` for gun-laying radar, `U` for an emitter the library cannot identify - which
/// is the entry a real receiver shows more often than anyone would like. `H` and `P` are absent, unlike
/// the family default: HAWK and Patriot are own-side systems, and putting them on a NATO threat scope
/// would be showing yourself to yourself.
///
/// The palette is the yellow-green of an ALR-67-era monochrome display rather than the pure phosphor
/// green of `radar_p1`, so the three national variants read as three cockpits.
pub fn radar_nato() -> Theme {
    Theme {
        id: "radar-nato".into(),
        name: "NATO (US)".into(),
        lit: "#a8e848".into(),
        hot: "#eeffd0".into(),
        panel: "#060a02".into(),
        panel_alpha: 1.0,
        edge: "#7f9a38".into(),
        edge_alpha: 0.20,
        radar: RadarParams {
            codes: [
                "6", "8", "10", "11", "13", "15", "17", "19", "20", "22", "2", "3", "M", "A", "U",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            ..RadarParams::default()
        },
        ..radar_base()
    }
}

/// Soviet/Russian cockpit, looking OUTWARD at Western hardware.
///
/// **What is authentic here is the threat list, not the instrument.** A real SPO-15 "Beryoza" is not a
/// round coded scope at all - it is a lamp panel: a ring of sector lamps for bearing, a bar for
/// relative range, and a row of class lamps. Reproducing that would be a different element rather than
/// a recolour, and this family's scope geometry is deliberately shared with the other colourways. So
/// what changes is the palette and, more usefully, WHO the emitters are.
///
/// A Russian receiver annotates Western systems, and Western systems are known by their US designation
/// numbers rather than by the NATO reporting numbers a US receiver uses for Soviet ones. Hence 23 for
/// the MIM-23 HAWK, 104 for the MIM-104 Patriot, 120 for the AIM-120, 9 for the AIM-9, 7 for the AIM-7,
/// and the airframe numbers 15/16/18. That is the one substantive difference between this table and
/// `radar_p1`'s: the SA-series numerals make no sense on a receiver operated by the people who field
/// them.
///
/// This is a plausible operator-perspective set, not a transcription of any real aircraft's threat
/// library - which is exactly why `codes` is a TOML list. Substitute your own.
///
/// The palette is the distinctive pale aquamarine of Soviet instrument lighting rather than the P1
/// green or the ice blue, so it reads as a different cockpit and not a hue shift of an existing one.
pub fn radar_ru() -> Theme {
    Theme {
        id: "radar-ru".into(),
        name: "Beryoza (RU)".into(),
        lit: "#5fe0c0".into(),
        hot: "#ddfff4".into(),
        panel: "#02100c".into(),
        panel_alpha: 1.0,
        edge: "#3c8a78".into(),
        edge_alpha: 0.20,
        radar: RadarParams {
            codes: ["23", "104", "120", "16", "15", "9", "7", "18", "P", "H", "F", "A"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ..RadarParams::default()
        },
        ..radar_base()
    }
}

/// PLAAF-style receiver, also looking outward at Western and allied hardware.
///
/// Same honesty as `radar_ru`: Chinese RWR symbology is not meaningfully documented in public, so
/// nothing here claims to be a transcription. What it is is a coherent second operator perspective -
/// a shorter list, weighted toward the airframes and the naval systems a receiver in that theatre
/// would see most, so it repeats sooner and reads as a narrower threat picture than the RU table.
///
/// The palette is a colder blue-white than `radar_ice`, closer to a modern LCD multi-function display
/// than to a phosphor tube, which is also the right period cue for the airframes in the list.
pub fn radar_cn() -> Theme {
    Theme {
        id: "radar-cn".into(),
        name: "PLAAF (CN)".into(),
        lit: "#7fb4ff".into(),
        hot: "#e8f2ff".into(),
        panel: "#03070f".into(),
        panel_alpha: 1.0,
        edge: "#4a6fa8".into(),
        edge_alpha: 0.20,
        // Shorter than the RU list on purpose - see the note above.
        radar: RadarParams {
            codes: ["16", "15", "35", "18", "120", "104", "3", "2", "F", "P"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ..RadarParams::default()
        },
        ..radar_base()
    }
}

// ===================== Pantone =====================
/// Shared base for the Pantone family.
///
/// The panel is near-black and that is arithmetic, not taste. Every lit colour must clear 3:1
/// against its own panel; full chroma across a continuous wheel cannot clear it against ANY single
/// panel colour (blue needs a panel at luminance >= 0.317, yellow needs <= 0.276), and sweeping
/// every grey from black to white the best possible panel is pure BLACK at 2.44:1 worst-hue. A
/// lighter panel is a legitimate answer to a contrast rule in general - it is not an answer to this
/// one. #07070a measures 0.0022, which is dark enough that the three process inks clear 6.41:1 at
/// FULL chroma. See `themes::RAINBOW_SAT` and `themes::quantise_hue` for the whole table.
///
/// `bloom` is tight and `glow_strength` low for a reason particular to this look: a wide halo
/// averages the 4px halftone lattice into a flat wash, and the moire - which only exists because
/// that lattice beats against the 9px bar pitch - disappears with it. This is print, not phosphor.
fn pantone_base() -> Theme {
    Theme {
        family: "pantone".into(),
        panel: "#07070a".into(),
        panel_alpha: 1.0,
        edge: "#ffffff".into(),
        edge_alpha: 0.16,
        texture: Texture::None_,
        ghost: 0.0,
        bloom: 3.0,
        glow_strength: 0.26,
        edge_glow: 0.0,
        // A slow drift. The hue must be stable long enough at any one column to read the spectrum
        // by; a fast cycle turns the panel into a strobe and destroys the frequency legend the
        // spread gives for free. Same reasoning as `rgb_wave`.
        rainbow: 0.05,
        // A FULL turn across the width, unlike rgb-wave's 0.85: the whole point of this family is
        // edge-to-edge full-spectrum, so the wheel must close rather than stop short of itself.
        rainbow_spread: 1.0,
        aberration: 2.0,
        ..Theme::default()
    }
}

/// The continuous wheel, and the one colourway here that CANNOT run at full chroma.
///
/// Held to `RAINBOW_SAT`'s 0.68 because a continuous wheel contains hue 240, and full-chroma blue
/// reaches only 2.44:1 against pure black - the best panel that exists. That is the honest cost of
/// keeping the gradient smooth, and it is why the other four colourways quantise instead.
pub fn pantone_spectrum() -> Theme {
    Theme {
        id: "pantone-spectrum".into(),
        name: "Full spectrum".into(),
        lit: "#ff3fa8".into(),
        hot: "#ffffff".into(),
        inks: 0,
        ink_morph: 0.35,
        pantone: PantoneParams { barcode: 0.16, halftone: 0.55, glitch: 4.0, split: true },
        ..pantone_base()
    }
}

/// Three-ink process separation, at FULL chroma.
///
/// `inks = 3` snaps the wheel to hues 60/180/300 - yellow, cyan and magenta, the chromatic process
/// set - which measures 6.41:1 against this panel at saturation 1.0. The key plate is the barcode
/// band and the screen, which are drawn achromatic. This is the colourway that proves the point:
/// maximum chroma is available here precisely because an ink set is not a spectrum.
pub fn pantone_process() -> Theme {
    Theme {
        id: "pantone-process".into(),
        name: "Process CMYK".into(),
        lit: "#00e0ff".into(),
        hot: "#ffffff".into(),
        ink_chroma: 1.0,
        inks: 3,
        ink_morph: 0.35,
        pantone: PantoneParams { barcode: 0.20, halftone: 0.75, glitch: 3.0, split: true },
        ..pantone_base()
    }
}

/// Barcode-dominant, and deliberately achromatic.
///
/// `rainbow = 0` is what makes it monochrome, and it goes through the identical code path as the
/// others: `render::tint` falls back to the fixed `lit` hex when a colourway is not a rainbow one,
/// so this needed no second colour path in the family. Nearly half the panel is given to the stripe
/// band, which is also where misregistration is most legible - a mis-set plate shows at a hard
/// black-and-white edge far more than inside a coloured field, which is why this one carries the
/// widest fringe of the set bar `pantone-misregister`.
pub fn pantone_barcode() -> Theme {
    Theme {
        id: "pantone-barcode".into(),
        name: "Barcode".into(),
        lit: "#f0f0f0".into(),
        hot: "#ffffff".into(),
        rainbow: 0.0,
        aberration: 3.0,
        pantone: PantoneParams { barcode: 0.45, halftone: 0.20, glitch: 2.0, split: false },
        ..pantone_base()
    }
}

/// Heavy misregistration: a page whose plates are badly out, on a six-ink separation.
///
/// `inks = 6` measures 3.22:1 at full chroma, which passes but with little margin, so `chroma` is
/// pulled to 0.92 - desaturating toward WHITE raises luminance, which is the safe direction here,
/// and it measures 3.47:1 as shipped. The glitch
/// slice is at its widest here and the halo tightest, because a 6px plate offset over a soft halo
/// just reads as a colour wash rather than as two displaced plates.
pub fn pantone_misregister() -> Theme {
    Theme {
        id: "pantone-misregister".into(),
        name: "Misregistration".into(),
        lit: "#ff2d55".into(),
        hot: "#ffffff".into(),
        ink_chroma: 0.92,
        inks: 6,
        ink_morph: 0.35,
        bloom: 2.0,
        aberration: 6.0,
        pantone: PantoneParams { barcode: 0.10, halftone: 0.35, glitch: 8.0, split: true },
        ..pantone_base()
    }
}

/// Halftone-dominant duotone, at full chroma.
///
/// `inks = 2` is a two-ink press run, which is what a duotone is: hues 90 and 270, acid chartreuse
/// against violet, measuring 3.22:1 at saturation 1.0. The screen is at full strength so it invades
/// the lit bars as well as the dormant field, and the fringe is kept narrow - at 4-6px the plates
/// separate by more than the 4px lattice pitch and the dots stop reading as dots.
pub fn pantone_halftone() -> Theme {
    Theme {
        id: "pantone-halftone".into(),
        name: "Halftone duotone".into(),
        lit: "#c8ff2e".into(),
        hot: "#ffffff".into(),
        ink_chroma: 1.0,
        inks: 2,
        ink_morph: 0.35,
        aberration: 1.0,
        pantone: PantoneParams { barcode: 0.08, halftone: 1.0, glitch: 1.0, split: true },
        ..pantone_base()
    }
}

/// Pantone on the segmented VFD: process inks at full chroma, with the plates out of register.
///
/// Shares `rgb_wave`'s mechanism entirely - `render::tint`, `rainbow`, `rainbow_spread` - and adds
/// only the two new `[look]` numbers. What separates it from the rainbow colourway visually is that
/// `inks = 3` turns a continuous sweep into three flat ink bands, and `aberration` splits every
/// segment's edge into a red and a blue one. A segmented grid is the ideal carrier for that: 25 bars
/// of hard vertical edges is 50 places for a mis-set plate to show.
pub fn vfd_pantone() -> Theme {
    Theme {
        id: "vfd-pantone".into(),
        name: "Pantone".into(),
        lit: "#00e0ff".into(),
        hot: "#ffffff".into(),
        panel: "#07070a".into(),
        panel_alpha: 1.0,
        edge: "#ffffff".into(),
        edge_alpha: 0.18,
        ghost: 0.10,
        bloom: 4.0,
        texture: Texture::Grille,
        rainbow: 0.05,
        rainbow_spread: 1.0,
        ink_chroma: 1.0,
        inks: 3,
        ink_morph: 0.35,
        aberration: 2.0,
        ..vfd_ice()
    }
}

/// Pantone on the oscilloscope: a three-ink trace whose plates have come apart.
///
/// A 1px trace is the most legible carrier for misregistration in the whole set - a hairline
/// literally splits into a red one and a blue one either side of the green - so this runs the widest
/// fringe of the three. The fade is short for the same reason `scope_red` is: a saturated trace
/// smears far more visibly than a soft phosphor, and three of them smear three times as much.
pub fn scope_pantone() -> Theme {
    Theme {
        id: "scope-pantone".into(),
        name: "Pantone".into(),
        lit: "#ff2fd0".into(),
        hot: "#ffffff".into(),
        panel: "#07070a".into(),
        panel_alpha: 1.0,
        edge: "#ffffff".into(),
        edge_alpha: 0.18,
        fade: 0.30,
        bloom: 4.0,
        rainbow: 0.06,
        rainbow_spread: 1.0,
        ink_chroma: 1.0,
        inks: 3,
        ink_morph: 0.35,
        aberration: 3.0,
        ..scope_base()
    }
}

/// Pantone on the VU dials: printed dial faces with the plates out.
///
/// The needle and the arc are both hairlines, so they carry the fringe well; the overload arc and
/// the overload needle still stay red, exactly as they do under the rainbow, because that colour
/// means something and a hue that happened to land on red anyway would make it unreadable.
pub fn vu_pantone() -> Theme {
    Theme {
        id: "vu-pantone".into(),
        name: "Pantone".into(),
        lit: "#ffe11f".into(),
        hot: "#ffffff".into(),
        panel: "#07070a".into(),
        panel_alpha: 1.0,
        edge: "#ffffff".into(),
        edge_alpha: 0.18,
        bloom: 4.0,
        rainbow: 0.05,
        rainbow_spread: 1.0,
        ink_chroma: 1.0,
        inks: 3,
        ink_morph: 0.35,
        aberration: 2.0,
        ..vu_base()
    }
}

// ===================== Chroma field =====================
// RETIRED: `CHROMA_BLUE_FLOOR`, the 2.30:1 contrast opt-in this family used to need.
//
// It existed for exactly one reason: the stripe ramp was an HSV sweep at saturation 1.0 and value
// 1.0, whose worst hue is pure blue, and pure blue on a near-black panel measures 2.358:1 whatever
// panel colour it is given. The three ramp colourways declared 2.30 and paid for it with the 1px
// keyline around every stripe.
//
// The perceptual ramp removes the CAUSE rather than the symptom. Holding OKLab lightness constant
// means luminance no longer depends on hue, so the worst case stops being a dark blue. Measured by
// `render::chroma::tests::probe_chroma_inks`: `chroma-spectrum`'s worst ink is now #0854ff at
// 3.59:1, and misregistration and halftone reach 5.53:1 - all clear of the project's 3:1 rule with
// room to spare, at the full chroma the gamut allows.
//
// So the whole family is back on the standard floor and no colourway in this project lowers it any
// more, which `no_colourway_lowers_the_contrast_floor_any_more` now asserts.

fn chroma_base() -> Theme {
    Theme {
        family: "chroma".into(),
        texture: Texture::None_,
        ghost: 0.0,
        // A LIGHTBOX glow: the field is bloomed on its own layer, so the halo lands behind the
        // print and escapes into the panel margin rather than softening any edge. See the note at
        // the top of `render::chroma`. 0 on a colourway gives the flat print back.
        //
        // 7px at 0.80, up from 5 at 0.55 on the first eyeball pass, where the bleed was present but
        // too faint to read as backlighting at all - the margin is only 4px, so the halo has to be
        // wider than the gap it is escaping into or all of it lands under the print that made it.
        bloom: 7.0,
        glow_strength: 0.80,
        edge_glow: 0.0,
        hot: "#ffffff".into(),
        panel_alpha: 1.0,
        ..Theme::default()
    }
}

/// The reference: the full visible spectrum, red at the bass end, at maximum chroma.
pub fn chroma_spectrum() -> Theme {
    Theme {
        id: "chroma-spectrum".into(),
        name: "Full spectrum".into(),
        // The WORST-CONTRAST ink this colourway actually prints, declared here deliberately so the
        // ordinary contrast test measures the real worst case rather than a flattering average. Read
        // off `probe_chroma_inks`, not remembered: it measures 3.59:1 against the panel below.
        lit: "#0854ff".into(),
        panel: "#05060a".into(),
        edge: "#8f8fa0".into(),
        edge_alpha: 0.16,
        chroma: ChromaParams {
            // Chosen by eye from a six-way sheet: deep and print-like, the golds reading as gold
            // rather than olive. See `ChromaParams::lightness` and `lightness_tilt`.
            lightness: 0.62,
            lightness_tilt: 0.35,
            ..ChromaParams::default()
        },
        ..chroma_base()
    }
}

/// Process colour only - cyan, magenta, yellow - with black as the key plate.
///
/// Which is how a press actually works: K is not a stripe colour, it is the keylines and the
/// halftone screen. That also means this colourway needs no contrast opt-in at all, since the
/// darkest of the three process inks is magenta at 6.5:1 against its panel.
pub fn chroma_cmyk() -> Theme {
    Theme {
        id: "chroma-cmyk".into(),
        name: "CMYK".into(),
        lit: "#ff00ff".into(),
        panel: "#04060a".into(),
        edge: "#7f9fa8".into(),
        edge_alpha: 0.16,
        chroma: ChromaParams {
            inks: vec!["#00ffff".into(), "#ff00ff".into(), "#ffff00".into()],
            // Ordered, not scrambled: a press lays its plates in sequence, and the repeating
            // C-M-Y cycle against varying widths is what makes it read as process colour
            // rather than as a random palette.
            halftone: 0.55,
            // An ODD pitch, deliberately. On the 45-degree lattice u and v always share their
            // parity, so an even pitch couples them: at pitch 4 only three coverages are
            // reachable at all (12.5%, 87.5%, solid), which is a screen that jumps rather than
            // ramps. Odd pitches decouple and give the full set of steps.
            halftone_pitch: 3,
            ..ChromaParams::default()
        },
        ..chroma_base()
    }
}

/// Chroma withheld almost entirely - the stripe works that are black, white and grey, with a
/// single hot accent.
///
/// The accent tracks the LOUDEST group, so the one coloured stripe is a position cue for where
/// the energy is rather than decoration. Its hues are kept warm (red through orange, 5.0:1 at
/// worst) so this colourway keeps the full 3:1 floor.
pub fn chroma_barcode() -> Theme {
    Theme {
        id: "chroma-barcode".into(),
        name: "Barcode".into(),
        lit: "#f2f2f2".into(),
        panel: "#07070a".into(),
        edge: "#9a9a9a".into(),
        edge_alpha: 0.18,
        chroma: ChromaParams {
            // Denser than the reference field: a barcode is a fine rule pattern, and with no
            // hue to carry the spectrum the width variation has to do it alone. 12px gives 15
            // stripes at 190px.
            stripe_px: 12.0,
            inks: vec![
                "#f2f2f2".into(),
                "#0a0a0c".into(),
                "#f2f2f2".into(),
                "#8f8f96".into(),
                "#0a0a0c".into(),
            ],
            scramble: true,
            accent: true,
            hue_offset: 0.02,
            hue_span: 0.10,
            // Lighter than the reference: at 12px stripes a 2px shift is a sixth of a stripe.
            shift_r: 1,
            shift_b: -1,
            halftone: 0.25,
            ..ChromaParams::default()
        },
        ..chroma_base()
    }
}

/// Misregistration dominant: the plates pulled 3px apart, as badly out as this size allows.
///
/// Wider stripes to pay for it - 20px gives 9 at the reference - because a 3px fringe either
/// side of an 18px stripe leaves very little pure core, and the fringe has to read as a
/// printing error rather than as the colour scheme.
pub fn chroma_misreg() -> Theme {
    Theme {
        id: "chroma-misreg".into(),
        name: "Misregistration".into(),
        // Measured worst ink: 5.53:1 against the panel below.
        lit: "#0088eb".into(),
        panel: "#06050a".into(),
        edge: "#a08f9a".into(),
        edge_alpha: 0.16,
        chroma: ChromaParams {
            // Brighter and flatter than the reference, because the SUBJECT here is the fringe: a
            // lighter ink leaves more room for the displaced red and blue plates to show against it,
            // and less hue-to-hue lightness variation stops the ramp competing with the fringe.
            lightness: 0.74,
            lightness_tilt: 0.20,
            stripe_px: 20.0,
            shift_r: 3,
            shift_b: -3,
            halftone: 0.30,
            glitch_px: 12,
            ..ChromaParams::default()
        },
        ..chroma_base()
    }
}

/// Halftone dominant: the screen over the whole field, coarse, at full strength.
///
/// Pitch 5 rather than 3 because a screen covering everything needs a legible dot - and the
/// coarser lattice quantises the tone ramp into five steps instead of three, so the ramp
/// reads as a ramp.
pub fn chroma_halftone() -> Theme {
    Theme {
        id: "chroma-halftone".into(),
        name: "Halftone".into(),
        // Measured worst ink: 5.55:1 against the panel below.
        lit: "#0088eb".into(),
        panel: "#050508".into(),
        edge: "#9a9aa8".into(),
        edge_alpha: 0.16,
        chroma: ChromaParams {
            // Lightest of the ramps. A screen covering the whole field removes ink everywhere, so a
            // darker ramp would put dots on colours already close to the panel and the tone steps
            // would stop reading as tone.
            lightness: 0.80,
            lightness_tilt: 0.25,
            stripe_px: 16.0,
            halftone: 1.0,
            halftone_pitch: 5,
            halftone_strength: 1.0,
            // Light, so the dots fringe without the stripes themselves smearing.
            shift_r: 1,
            shift_b: -1,
            ..ChromaParams::default()
        },
        ..chroma_base()
    }
}

/// Risograph: the real fluorescent spot inks, on a warm paper-ish key.
///
/// A Riso has no process ramp at all - it prints from a small drum of premixed inks, one pass each,
/// and its palette is famous precisely because those inks are outside CMYK's gamut. So this is an
/// `inks` colourway rather than a hue ramp, and the six hexes are the actual Riso spot colours:
/// Fluorescent Pink, Blue, Yellow, Green, Orange and Bright Red.
///
/// Scrambled rather than cycled, unlike CMYK. A press lays plates in sequence; a Riso operator
/// changes drums between passes and the order is whatever the job wanted, so a repeating six-cycle
/// would read as a pattern the process does not have.
///
/// The key is a warm near-black rather than pure ink, because Riso black is a soft, slightly brown
/// drum rather than a dense process K - and the keylines are what keep full-chroma stripes legible,
/// so this is the one place the family's warmth can show without costing contrast.
pub fn chroma_riso() -> Theme {
    Theme {
        id: "chroma-riso".into(),
        name: "Risograph".into(),
        // The worst-contrast ink of the six against the panel below - measured, not guessed.
        lit: "#0078bf".into(),
        panel: "#0a0806".into(),
        edge: "#a8927f".into(),
        edge_alpha: 0.18,
        chroma: ChromaParams {
            inks: vec![
                "#ff48b0".into(), // Fluorescent Pink
                "#0078bf".into(), // Blue
                "#ffe800".into(), // Yellow
                "#00a95c".into(), // Green
                "#ff6c2f".into(), // Orange
                "#f15060".into(), // Bright Red
            ],
            scramble: true,
            ink: "#140f0a".into(),
            // Wider stripes than the reference: six spot inks in a narrow field repeat too often to
            // read as separate passes.
            stripe_px: 22.0,
            // A Riso's registration is genuinely poor - the paper is fed once per drum - so the plates
            // sitting a pixel apart is characteristic rather than a defect.
            shift_r: 1,
            shift_b: -1,
            halftone: 0.40,
            halftone_pitch: 3,
            ..ChromaParams::default()
        },
        ..chroma_base()
    }
}

/// Two inks and nothing else: a deep teal and a warm bone, with the accent picking out the energy.
///
/// The counterweight to the rest of the family. Everything else here is about how much colour a print
/// can carry; this is about how little. Two inks cannot encode a spectrum, so the WIDTHS do all the
/// work and the single accented stripe says where the energy is - which is the same trick the barcode
/// colourway uses, at half the palette.
///
/// The accent's hue is pinned narrow and warm so it stays legible against both inks, and because a
/// duotone with a rainbow accent would not be a duotone.
pub fn chroma_duotone() -> Theme {
    Theme {
        id: "chroma-duotone".into(),
        name: "Duotone".into(),
        lit: "#e8dcc8".into(),
        panel: "#04080a".into(),
        edge: "#7f9a96".into(),
        edge_alpha: 0.16,
        chroma: ChromaParams {
            inks: vec![
                "#0f6b6b".into(), // deep teal
                "#e8dcc8".into(), // warm bone
                "#0f6b6b".into(),
                "#a8bfb8".into(), // the teal knocked back, so the pattern has three weights
            ],
            scramble: true,
            accent: true,
            // Warm and narrow - see the note above.
            hue_offset: 0.04,
            hue_span: 0.06,
            lightness: 0.66,
            lightness_tilt: 0.0,
            stripe_px: 14.0,
            halftone: 0.45,
            halftone_pitch: 3,
            shift_r: 1,
            shift_b: -1,
            ..ChromaParams::default()
        },
        ..chroma_base()
    }
}

// ===================== Fluid =====================
fn fluid_base() -> Theme {
    Theme {
        family: "fluid".into(),
        texture: Texture::None_,
        ghost: 0.0,
        // Modest radius. The light in this scene is a 1px meniscus and 1px droplets, and a wide
        // blur turns the surface line into a band that swallows the crests it is meant to pick out.
        bloom: 3.0,
        glow_strength: 0.45,
        ..Theme::default()
    }
}

/// The reference: a deep tank of water, cyan meniscus, caustics under the crests.
pub fn fluid_deep() -> Theme {
    Theme {
        id: "fluid-deep".into(),
        name: "Deep water".into(),
        lit: "#6fd8ff".into(),
        hot: "#eaffff".into(),
        panel: "#050b12".into(),
        panel_alpha: 1.0,
        edge: "#3f7ea8".into(),
        edge_alpha: 0.20,
        fluid: FluidParams::default(),
        ..fluid_base()
    }
}

/// Mercury. Heavy, almost lossless, and OPAQUE - so it rings in a standing lattice, it will not
/// heap as high as water, and no light reaches below the surface at all.
pub fn fluid_mercury() -> Theme {
    Theme {
        id: "fluid-mercury".into(),
        name: "Mercury".into(),
        lit: "#e8eef5".into(),
        hot: "#ffffff".into(),
        panel: "#14161a".into(),
        panel_alpha: 1.0,
        edge: "#8a93a0".into(),
        edge_alpha: 0.26,
        // A brighter, tighter halo: a liquid metal's highlight is specular, not diffuse.
        bloom: 4.0,
        glow_strength: 0.60,
        fluid: FluidParams {
            surface: 0.58,
            body_top: "#9aa7b4".into(),
            body_deep: "#2b3138".into(),
            cone: "#3a4048".into(),
            cone_dark: "#14171b".into(),
            // 0.9992 per sub-step: a wave arrives at the far wall at 0.92 of its amplitude and
            // comes back, so successive reflections overlap into a standing pattern that persists
            // for seconds after the sound stops. This is the colourway where the interference is
            // the subject rather than a detail.
            damping: 0.9992,
            // Slow: heavy liquid, long swell. Also keeps the standing lattice coarse enough to
            // resolve at a 190px width.
            wave_speed: 0.65,
            // Mercury's surface tension is enormous, so it does not pile up - the amplitude is
            // deliberately lower than water's even though it rings far longer.
            surface_gain: 13.0,
            cone_travel: 0.13,
            coupling: 0.30,
            droplets: 9,
            // Beads, not spray: fewer, faster, higher.
            droplet_v: 195.0,
            // Opaque metal - nothing focuses below the surface.
            caustics: false,
            underglow: 0.0,
            sheen: 0.55,
            ..FluidParams::default()
        },
        ..fluid_base()
    }
}

/// An oil slick: a SHALLOW film, fast fine ripples, and a colour that shifts with the angle of the
/// surface rather than staying one hue.
pub fn fluid_oil() -> Theme {
    Theme {
        id: "fluid-oil".into(),
        name: "Oil slick".into(),
        lit: "#ffb0e6".into(),
        hot: "#fff0ff".into(),
        panel: "#0a0810".into(),
        panel_alpha: 1.0,
        edge: "#7a5aa0".into(),
        edge_alpha: 0.22,
        fluid: FluidParams {
            // A film, not a tank: the surface sits low in the panel, which leaves the top two
            // thirds as headroom for spray and makes the whole scene read as a puddle seen from
            // the side rather than as an aquarium.
            surface: 0.62,
            body_top: "#3d2a6b".into(),
            body_deep: "#0d0718".into(),
            film: "#5cffd0".into(),
            cone: "#241a38".into(),
            cone_dark: "#0b0714".into(),
            damping: 0.997,
            // Fast, but NOT fine - that half of the claim was wrong and the measurement caught it.
            // `wave_speed` scales the number of fixed sub-steps taken per frame, so it is a TIME
            // scale: it makes a wave cross the tank sooner, and if anything lengthens the wavelength,
            // because the source oscillates just as slowly while its output travels further. Measured
            // chop (mean column-to-column step in the drawn edge) is 0.11 here against deep water's
            // 0.25 - this colourway is SMOOTHER than the reference, not shimmerier. Wavelength here
            // is set by the mouth width and the rate the cone moves, neither of which this touches.
            wave_speed: 1.70,
            // Measured: at coupling 0.16 the film's fine structure never reached a whole pixel -
            // chop came out at 0.10 against mercury's 0.52, so the colourway documented as the
            // FINEST rippling measured second-flattest of the five. The wavelength was right and
            // the amplitude was starved: the cone barely coupled into the liquid, so the shimmer
            // was there in the field and quantised away on the way to the screen. Raising the two
            // together keeps the short wavelength and gives it enough travel to survive rounding.
            surface_gain: 20.0,
            cone_travel: 0.20,
            coupling: 0.34,
            // Light spray, and lots of it.
            droplets: 17,
            droplet_v: 140.0,
            caustics: true,
            underglow: 0.45,
            // The signature: the meniscus is mixed toward `film` by the local slope, so a crest's
            // rising flank and its falling flank are different colours.
            iridescence: 0.85,
            ..FluidParams::default()
        },
        ..fluid_base()
    }
}

/// Glowing coolant. The liquid is the light source, so the body itself is bloomed rather than
/// merely being brightly coloured.
pub fn fluid_coolant() -> Theme {
    Theme {
        id: "fluid-coolant".into(),
        name: "Glowing coolant".into(),
        lit: "#6cffb0".into(),
        hot: "#e8fff4".into(),
        panel: "#04120c".into(),
        panel_alpha: 1.0,
        edge: "#3f9e70".into(),
        edge_alpha: 0.24,
        // Wide and strong, because here the halo comes off a body of liquid rather than off a 1px
        // line - this is the one colourway where a big bloom is the point.
        bloom: 6.0,
        glow_strength: 0.70,
        fluid: FluidParams {
            surface: 0.64,
            body_top: "#1fbf7a".into(),
            body_deep: "#03170f".into(),
            cone: "#183028".into(),
            cone_dark: "#04100a".into(),
            damping: 0.9985,
            wave_speed: 1.30,
            // The violent one. A pumped coolant loop is agitated, not calm, so this heaps far
            // higher than water does: measured at 190x60 over the fixture it runs 15.9px of median
            // peak-to-trough relief against water's 7.3px, which is what stops it reading as the
            // reference colourway in green. (`fluid`'s own colourway test asserts that gap.)
            surface_gain: 22.0,
            cone_travel: 0.20,
            coupling: 0.26,
            droplets: 15,
            droplet_v: 180.0,
            caustics: true,
            emissive: 0.75,
            ..FluidParams::default()
        },
        ..fluid_base()
    }
}

/// Dark ink. So viscous that the waves die before they leave the cone, which inverts the whole
/// composition: what you read is the two cones working, not the pattern in the middle.
pub fn fluid_ink() -> Theme {
    Theme {
        id: "fluid-ink".into(),
        name: "Dark ink".into(),
        lit: "#b9b4d8".into(),
        hot: "#e6e2f5".into(),
        panel: "#0b0a0e".into(),
        panel_alpha: 1.0,
        edge: "#4a4560".into(),
        edge_alpha: 0.18,
        bloom: 2.0,
        glow_strength: 0.30,
        fluid: FluidParams {
            surface: 0.58,
            // Measured: at #232030 over #08070c the body read 1.21:1 against its own panel, i.e.
            // not visible - "so viscous the waves die" had become "so dark there is no liquid".
            // These give 2.08:1, which is enough to see a body of ink without making it a bright
            // colourway; the character is meant to come from the stillness, not from the darkness.
            body_top: "#403a5c".into(),
            body_deep: "#1c1930".into(),
            cone: "#161420".into(),
            cone_dark: "#07060a".into(),
            // 0.945 per sub-step is 0.80 per nominal frame: a wave is down to a fifth within four
            // frames and never reaches the middle at all. That is the whole character - the two
            // mounds heave locally and the tank between them stays glassy.
            damping: 0.9955,
            wave_speed: 0.55,
            // 5.0 left the two mounds a 4px heave over 8 seconds. The waves still die before the
            // middle - that is `damping`, and it is untouched - but the drivers' own liquid has to
            // move enough to read as liquid rather than as a line.
            surface_gain: 23.0,
            // The cone gets the travel the liquid does not: with the surface nearly static, cone
            // POSITION is the only thing left carrying the channel, so it is given the most.
            cone_travel: 0.26,
            // Thick liquid clings to the diaphragm and follows it closely.
            coupling: 0.50,
            // Nothing this viscous throws a droplet.
            droplets: 0,
            caustics: false,
            underglow: 0.0,
            sheen: 0.12,
            ..FluidParams::default()
        },
        ..fluid_base()
    }
}

/// Fluid in process inks: the tank becomes a slowly cycling DUOTONE, quantised onto the subtractive
/// process set, with the plates misregistered.
///
/// The other Pantone colourways sit on families whose colour all flows through `tint`, so they
/// inherit the ink machinery for free. Fluid's body did not - it read its hex directly - which is
/// why the body is now tinted at two positions (see the note in `fluid.rs`). The result is the one
/// thing a Pantone colourway has to have here: the LIQUID is the ink, not just the highlight on it.
///
/// Kept on a dark panel deliberately. A light stock is the more obvious choice for a print
/// reference, and it was measured and rejected: against a light panel the yellow ink lands at
/// 1.00:1 and the plate simply disappears, where against this panel the worst ink is comfortable.
pub fn fluid_pantone() -> Theme {
    Theme {
        id: "fluid-pantone".into(),
        name: "Pantone".into(),
        lit: "#00e0ff".into(),
        hot: "#ffffff".into(),
        panel: "#07070a".into(),
        panel_alpha: 1.0,
        edge: "#ffffff".into(),
        edge_alpha: 0.18,
        bloom: 3.0,
        rainbow: 0.05,
        rainbow_spread: 1.0,
        ink_chroma: 1.0,
        inks: 3,
        ink_morph: 0.35,
        aberration: 2.0,
        fluid: FluidParams {
            surface: 0.56,
            // Fallbacks only: with `rainbow` on, both ends come from the ink set. They still matter
            // for the TOML case where someone overrides `rainbow` back to 0.
            body_top: "#00c8ff".into(),
            body_deep: "#101038".into(),
            cone: "#1a1a24".into(),
            cone_dark: "#08080c".into(),
            damping: 0.9975,
            wave_speed: 1.15,
            surface_gain: 13.5,
            cone_travel: 0.24,
            coupling: 0.30,
            // Ink spatter, and a fast one - it is the closest thing here to a printing accident.
            droplets: 16,
            droplet_v: 155.0,
            // Off: caustics are a light-through-water effect and read as noise once the body is a
            // flat ink. The hard specular horizon replaces them, which is the printed-edge look.
            caustics: false,
            underglow: 1.0,
            sheen: 0.55,
            ..FluidParams::default()
        },
        ..fluid_base()
    }
}


// ---------------------------------------------------------------------------------------------------
// Flame organ
// ---------------------------------------------------------------------------------------------------

/// Shared settings for the flame organ. See `render::flame`.
///
/// `bloom` is deliberately modest and is NOT what makes this family glow: the renderer builds its own
/// two-stage spill from the heat field, because a flame is a volume of light and the colourway's bloom is
/// tuned for point sources like a cathode or a lamp. What `bloom` still controls here is the hot core.
fn flame_base() -> Theme {
    Theme {
        family: "flame".into(),
        texture: Texture::None_,
        ghost: 0.0,
        bloom: 3.0,
        glow_strength: 0.55,
        // A flame has mass. Snapping to the band makes the plumes flicker as data rather than burn, and
        // the diffusion field already adds its own liveliness on top of whatever arrives.
        ballistics: Ballistics { attack: 0.55, decay: 0.14, peak_fall: 0.006 },
        ..Theme::default()
    }
}

/// The ramps are PALE ALL THE WAY DOWN, which is the family's one hard rule.
///
/// `render::flame` gets faintness from alpha - the body is translucent, 0.34 rising to 0.80 with heat -
/// so a ramp does not need, and must not have, a dark cool end. An early sodium ramp started at #7a1500
/// and the flames read as heavy and solid instead of ghostly; dimness and insubstantiality are not the
/// same property. Every stop here is a colour a flame of that chemistry actually emits, at full
/// luminance, and the alpha does the rest.
///
/// Sodium: the ordinary warm gas flame, and the one the renderer was tuned against.
pub fn flame_sodium() -> Theme {
    Theme {
        id: "flame-sodium".into(),
        name: "Sodium".into(),
        lit: "#ff7a00".into(),
        hot: "#ffe6a8".into(),
        panel: "#0b0705".into(),
        panel_alpha: 1.0,
        edge: "#c9722f".into(),
        edge_alpha: 0.18,
        zones: vec![
            Zone { upto: 0.30, lit: "#ff4a00".into(), hot: "#ff4a00".into() },
            Zone { upto: 0.55, lit: "#ff7a00".into(), hot: "#ff7a00".into() },
            Zone { upto: 0.78, lit: "#ffab1f".into(), hot: "#ffab1f".into() },
            Zone { upto: 1.01, lit: "#ffe6a8".into(), hot: "#ffe6a8".into() },
        ],
        ..flame_base()
    }
}

/// Propane: what a gas flame looks like when it is burning CORRECTLY - blue, with a white-hot core.
///
/// The most physically honest of the set and the least like anything else in the project.
pub fn flame_propane() -> Theme {
    Theme {
        id: "flame-propane".into(),
        name: "Propane".into(),
        lit: "#1f8cff".into(),
        hot: "#d8f2ff".into(),
        panel: "#04070b".into(),
        panel_alpha: 1.0,
        edge: "#2f7ec9".into(),
        edge_alpha: 0.18,
        zones: vec![
            Zone { upto: 0.30, lit: "#0a5cff".into(), hot: "#0a5cff".into() },
            Zone { upto: 0.55, lit: "#1f8cff".into(), hot: "#1f8cff".into() },
            Zone { upto: 0.78, lit: "#4fc2ff".into(), hot: "#4fc2ff".into() },
            Zone { upto: 1.01, lit: "#d8f2ff".into(), hot: "#d8f2ff".into() },
        ],
        ..flame_base()
    }
}

/// Copper: the flame test for copper salts, a green nothing else in this project goes near.
pub fn flame_copper() -> Theme {
    Theme {
        id: "flame-copper".into(),
        name: "Copper".into(),
        lit: "#1fe07a".into(),
        hot: "#d4ffe2".into(),
        panel: "#030805".into(),
        panel_alpha: 1.0,
        edge: "#2fa05e".into(),
        edge_alpha: 0.18,
        zones: vec![
            Zone { upto: 0.30, lit: "#00c24a".into(), hot: "#00c24a".into() },
            Zone { upto: 0.55, lit: "#1fe07a".into(), hot: "#1fe07a".into() },
            Zone { upto: 0.78, lit: "#5cf5a8".into(), hot: "#5cf5a8".into() },
            Zone { upto: 1.01, lit: "#d4ffe2".into(), hot: "#d4ffe2".into() },
        ],
        ..flame_base()
    }
}

/// Strontium: the crimson of a distress flare.
///
/// The hardest of the seven to keep ghostly, because crimson wants to be dark. Held light by starting at
/// a bright rose rather than a blood red - see the note on `flame_sodium` for what a dark cool end did.
pub fn flame_strontium() -> Theme {
    Theme {
        id: "flame-strontium".into(),
        name: "Strontium".into(),
        lit: "#ff3d6e".into(),
        hot: "#ffd8e2".into(),
        panel: "#0a0406".into(),
        panel_alpha: 1.0,
        edge: "#c9384f".into(),
        edge_alpha: 0.18,
        zones: vec![
            Zone { upto: 0.30, lit: "#ff0a4a".into(), hot: "#ff0a4a".into() },
            Zone { upto: 0.55, lit: "#ff3d6e".into(), hot: "#ff3d6e".into() },
            Zone { upto: 0.78, lit: "#ff7a9c".into(), hot: "#ff7a9c".into() },
            Zone { upto: 1.01, lit: "#ffd8e2".into(), hot: "#ffd8e2".into() },
        ],
        ..flame_base()
    }
}

/// Potassium: pale lilac, the most delicate of the set.
pub fn flame_potassium() -> Theme {
    Theme {
        id: "flame-potassium".into(),
        name: "Potassium".into(),
        lit: "#a34fff".into(),
        hot: "#ecdcff".into(),
        panel: "#06040b".into(),
        panel_alpha: 1.0,
        edge: "#7a4fc9".into(),
        edge_alpha: 0.18,
        zones: vec![
            Zone { upto: 0.30, lit: "#7a1fff".into(), hot: "#7a1fff".into() },
            Zone { upto: 0.55, lit: "#a34fff".into(), hot: "#a34fff".into() },
            Zone { upto: 0.78, lit: "#c98cff".into(), hot: "#c98cff".into() },
            Zone { upto: 1.01, lit: "#ecdcff".into(), hot: "#ecdcff".into() },
        ],
        ..flame_base()
    }
}

/// Plasma: the one entry that is not a chemistry - violet through cyan to white.
pub fn flame_plasma() -> Theme {
    Theme {
        id: "flame-plasma".into(),
        name: "Plasma".into(),
        lit: "#7a3dff".into(),
        hot: "#e8f8ff".into(),
        panel: "#05060c".into(),
        panel_alpha: 1.0,
        edge: "#6a3fd0".into(),
        edge_alpha: 0.20,
        zones: vec![
            Zone { upto: 0.30, lit: "#c400ff".into(), hot: "#c400ff".into() },
            Zone { upto: 0.55, lit: "#7a3dff".into(), hot: "#7a3dff".into() },
            Zone { upto: 0.78, lit: "#2fb8ff".into(), hot: "#2fb8ff".into() },
            Zone { upto: 1.01, lit: "#e8f8ff".into(), hot: "#e8f8ff".into() },
        ],
        ..flame_base()
    }
}

/// Rainbow flame: hue sweeps across the manifold, so each burner is a different colour.
///
/// The `rainbow` machinery suits this family better than most: it colours by position along the row, and
/// here the row is a rank of physically separate burners rather than one continuous display, so a hue per
/// burner reads as deliberate rather than as a gradient laid over something.
///
/// `zones` are still authored, because the ramp is what carries HEAT - the rainbow re-tints it per column
/// on top. Left as the pale neutral set so the hue is the colourway's own and not a tint of orange.
pub fn flame_rainbow() -> Theme {
    Theme {
        id: "flame-rainbow".into(),
        name: "Rainbow flame".into(),
        lit: "#ffffff".into(),
        hot: "#ffffff".into(),
        panel: "#07070a".into(),
        panel_alpha: 1.0,
        edge: "#8a8a9a".into(),
        edge_alpha: 0.18,
        rainbow: 1.0,
        zones: vec![
            Zone { upto: 0.30, lit: "#b0b0b8".into(), hot: "#b0b0b8".into() },
            Zone { upto: 0.55, lit: "#d8d8dd".into(), hot: "#d8d8dd".into() },
            Zone { upto: 0.78, lit: "#f0f0f4".into(), hot: "#f0f0f4".into() },
            Zone { upto: 1.01, lit: "#ffffff".into(), hot: "#ffffff".into() },
        ],
        ..flame_base()
    }
}

// ===================== Dolphin LCD =====================
//
// The 1990s aftermarket head unit. Every colourway is ONE hue at three brightness levels - lit dot,
// peak-hold cap, unlit well - because that is what a single-backlight dot-matrix LCD is. No gradients,
// no second accent.
//
// FIELD MAPPING, in the reel model (no params struct of its own):
//   lit                 the lit dot
//   hot                 the leap and the splash - the only place a fourth level appears
//   ghost               the unlit dark-well alpha, as `segmented` uses it for dormant bars
//   panel               the LCD substrate
//   ballistics.peak_fall  how fast the peak caps fall
//   bloom/glow_strength   backlight halo, kept LOW: a transmissive LCD barely has one
//
// The well alpha is the interesting number. Composited it must be VISIBLE as a lattice yet clearly
// read as OFF, and those need different instruments: measured it lands +9.1 to +10.9 dL* above the
// panel (4x the ~2.3 dL* floor of visibility `tube.rs:58` established) while sitting at 1.20-1.25:1
// contrast against it - far below the 3.0:1 this project calls lit.
fn dolphin_base() -> Theme {
    Theme {
        family: "dolphin".into(),
        // The family draws its OWN lattice; a Glass or Scanlines overlay would double-screen it.
        texture: Texture::None_,
        panel_alpha: 1.0,
        // Tight, and low. A backlight glows; it does not flare. A wide bloom also welds adjacent dots
        // into a bar and destroys the one thing that makes this a dot-matrix display.
        bloom: 2.0,
        glow_strength: 0.22,
        edge_glow: 1.0,
        ballistics: Ballistics { attack: 0.55, decay: 0.11, peak_fall: 0.0055 },
        // `flourish` is inherited from Theme::default(), which IS DEFAULT_FLOURISH - no colourway in
        // this file sets it explicitly, and a copy here could drift from the calibrated value.
        ..Theme::default()
    }
}

/// Sony amber. The reference, and the one with the most room for a three-level ladder.
///
/// Chosen over a hotter Xplod orange on arithmetic rather than taste: a hotter orange has less L*
/// headroom, so the ladder is squeezed from both ends. At CAP_ALPHA 0.60 the cap of `#ff5a1e` measures
/// 2.92:1 against its panel and FAILS the 3.0 floor outright, and raising the alpha to rescue it
/// collapses the other end - the cap starts merging into the lit dot.
/// Measured ladder, dL*: panel 2.40 -> well 11.52 -> cap 47.05 -> lit 73.42 -> hot 88.44.
pub fn dolphin_sony_amber() -> Theme {
    Theme {
        id: "dolphin-sony-amber".into(),
        name: "Sony amber".into(),
        lit: "#ff9e1f".into(),
        hot: "#ffd8a2".into(),
        panel: "#0d0803".into(),
        edge: "#ff9e1f".into(),
        edge_alpha: 0.13,
        ghost: 0.13,
        ..dolphin_base()
    }
}

/// JVC ice blue. Deliberately more chromatic than `vfd-ice` so it is a new colour, not a third copy.
///
/// OKLab chroma 0.126 against vfd-ice's 0.090. Measured: lit 10.62:1 against panel, steps of
/// 10.88 / 36.05 / 27.32 / 20.06 dL* up the ladder.
pub fn dolphin_jvc_ice() -> Theme {
    Theme {
        id: "dolphin-jvc-ice".into(),
        name: "JVC ice blue".into(),
        lit: "#5cc8ff".into(),
        hot: "#e8f8ff".into(),
        panel: "#04090f".into(),
        edge: "#5cc8ff".into(),
        edge_alpha: 0.14,
        ghost: 0.14,
        ..dolphin_base()
    }
}

/// Pioneer green. A yellow-green, distinct from `matrix-green` and `nixie-green`.
///
/// Chroma 0.214, the most saturated of the three. Measured: lit 12.77:1, a 4.3x margin on the floor.
pub fn dolphin_pioneer_green() -> Theme {
    Theme {
        id: "dolphin-pioneer-green".into(),
        name: "Pioneer green".into(),
        lit: "#8fe63c".into(),
        hot: "#e4ffb4".into(),
        panel: "#040c07".into(),
        edge: "#8fe63c".into(),
        edge_alpha: 0.12,
        ghost: 0.12,
        ..dolphin_base()
    }
}

/// Xplod orange - the hotter Sony, viable as its own colourway.
///
/// Measured at CAP_ALPHA 0.60: lit 7.66:1 against panel, cap 3.34:1, lit/cap 2.29:1. It clears the
/// floor with less margin than amber, which is exactly why amber is the reference and this is the
/// alternative rather than the other way round.
pub fn dolphin_xplod_orange() -> Theme {
    Theme {
        id: "dolphin-xplod-orange".into(),
        name: "Xplod orange".into(),
        lit: "#ff7a1e".into(),
        hot: "#ffcaa0".into(),
        panel: "#0f0702".into(),
        edge: "#ff7a1e".into(),
        edge_alpha: 0.13,
        ghost: 0.13,
        ..dolphin_base()
    }
}

/// Kenwood red. The hardest hue to make work here, and the reason is structural.
///
/// Red carries little luminance (0.2126 of the WCAG weighting against green's 0.7152), so a red that
/// looks saturated is dark, and a red bright enough to clear the floor has to be pushed toward salmon.
/// This is the same wall the rainbow hit with blue at 2.31:1.
pub fn dolphin_kenwood_red() -> Theme {
    Theme {
        id: "dolphin-kenwood-red".into(),
        name: "Kenwood red".into(),
        lit: "#ff6b57".into(),
        hot: "#ffc4bb".into(),
        panel: "#0e0403".into(),
        edge: "#ff6b57".into(),
        edge_alpha: 0.13,
        ghost: 0.13,
        ..dolphin_base()
    }
}

// ===================== 3D spectrum =====================
//
// Depth is TIME here, so brightness carries DEPTH and height carries LEVEL. That split is why these
// colourways need a `hot` that is clearly separable from `lit`: the near row goes hot during the
// flourish, and if the two are close the surge is invisible.
fn mesh_base() -> Theme {
    Theme {
        family: "mesh".into(),
        // The bars ARE the texture; an overlay pattern on top of five staggered rows is noise.
        texture: Texture::None_,
        panel_alpha: 1.0,
        // Tight. A wide bloom bleeds the depth rows into each other and destroys the occlusion that
        // makes the stack read as depth at all.
        bloom: 2.0,
        glow_strength: 0.26,
        edge_glow: 1.0,
        ballistics: Ballistics { attack: 0.60, decay: 0.16, peak_fall: 0.006 },
        ..Theme::default()
    }
}

/// The WMP look: cyan bars receding into near-black.
pub fn mesh_wmp_cyan() -> Theme {
    Theme {
        id: "mesh-wmp-cyan".into(),
        name: "WMP cyan".into(),
        lit: "#3fd6ff".into(),
        hot: "#dff6ff".into(),
        panel: "#05090d".into(),
        edge: "#3fd6ff".into(),
        edge_alpha: 0.16,
        ghost: 0.10,
        ..mesh_base()
    }
}

/// Winamp green - the classic spectrum colour, on the classic near-black.
pub fn mesh_winamp_green() -> Theme {
    Theme {
        id: "mesh-winamp-green".into(),
        name: "Winamp green".into(),
        lit: "#6ee85a".into(),
        hot: "#e2ffd8".into(),
        panel: "#040a05".into(),
        edge: "#6ee85a".into(),
        edge_alpha: 0.15,
        ghost: 0.10,
        ..mesh_base()
    }
}

/// Amber, to sit beside the head-unit family.
pub fn mesh_amber_stack() -> Theme {
    Theme {
        id: "mesh-amber-stack".into(),
        name: "Amber stack".into(),
        lit: "#ffab33".into(),
        hot: "#ffe6bf".into(),
        panel: "#0c0702".into(),
        edge: "#ffab33".into(),
        edge_alpha: 0.15,
        ghost: 0.10,
        ..mesh_base()
    }
}

/// Magenta, the vaporwave end of the lineage.
pub fn mesh_magenta_deck() -> Theme {
    Theme {
        id: "mesh-magenta-deck".into(),
        name: "Magenta deck".into(),
        lit: "#ff5fd0".into(),
        hot: "#ffd9f4".into(),
        panel: "#0c0410".into(),
        edge: "#ff5fd0".into(),
        edge_alpha: 0.16,
        ghost: 0.10,
        ..mesh_base()
    }
}

/// Cold white, for when the colour should be the music and not the meter.
pub fn mesh_white_stack() -> Theme {
    Theme {
        id: "mesh-white-stack".into(),
        name: "White stack".into(),
        lit: "#dfe9f5".into(),
        hot: "#ffffff".into(),
        panel: "#07080a".into(),
        edge: "#dfe9f5".into(),
        edge_alpha: 0.14,
        ghost: 0.10,
        ..mesh_base()
    }
}

/// Every ball its own hue, taken from its position around the ring.
///
/// Because position IS the frequency slice a ball reads, the hue is a frequency legend rather than
/// decoration: the bass balls are always at one end of the spectrum and the treble at the other. Uses the
/// shared rainbow machinery, so `RAINBOW_SAT` applies - a MEASURED ceiling of 0.68, because at full
/// saturation pure blue manages only 2.31:1 against a near-black panel and fails this project's 3:1 rule
/// at every brightness.
pub fn orbit_rainbow() -> Theme {
    Theme {
        id: "orbit-rainbow".into(),
        name: "Rainbow".into(),
        lit: "#ffffff".into(),
        hot: "#ffffff".into(),
        panel: "#05060a".into(),
        edge: "#8899aa".into(),
        edge_alpha: 0.14,
        ghost: 0.10,
        rainbow: 1.0,
        // A full turn of hue around the ring, so each ball has its own colour and the sequence is a
        // frequency legend rather than a cycling wash. `rainbow_spread` is documented as exactly this:
        // 0 makes the whole display shift together, ~0.8 makes hue vary by POSITION.
        rainbow_spread: 1.0,
        ..orbit_base()
    }
}

/// ONE ball on a long elliptical path - the original ask, kept as its own colourway.
///
/// A different thing to watch rather than a smaller version of the ring. With nothing to compare itself
/// against, its depth is carried entirely by its own size and the tilt of its path, so the near/far taper
/// does more work here than anywhere else in the family - which is why it takes a 1.55x ball.
///
/// That multiplier is the whole reason `OrbitParams::scale` exists. An earlier version of this comment
/// claimed the bigger ball while the code did nothing of the kind, which is worse than either having it
/// or not having it: a doc describing behaviour the code lacks is a trap for whoever reads it next.
pub fn orbit_solo() -> Theme {
    Theme {
        id: "orbit-solo".into(),
        name: "Solo".into(),
        lit: "#7fe3ff".into(),
        hot: "#f0fbff".into(),
        panel: "#04080d".into(),
        edge: "#7fe3ff".into(),
        edge_alpha: 0.15,
        ghost: 0.10,
        orbit: OrbitParams { balls: 1, scale: 1.55, reactive: false },
        ..orbit_base()
    }
}

/// The ring GROWS with the music: one ball in the quiet, up to twelve when the track opens up.
///
/// The count is the meter here, which is a mapping nothing else in the family has - and it is a legible
/// one, because a count is read at a glance where a brightness is not. Each ball fades in by growing from
/// nothing rather than appearing, since a ball popping into existence is the same discontinuity that got
/// both flourishes reworked.
pub fn orbit_swarm() -> Theme {
    Theme {
        id: "orbit-swarm".into(),
        name: "Swarm".into(),
        lit: "#ffffff".into(),
        hot: "#ffffff".into(),
        panel: "#05060a".into(),
        edge: "#8899aa".into(),
        edge_alpha: 0.14,
        ghost: 0.10,
        rainbow: 1.0,
        rainbow_spread: 1.0,
        orbit: OrbitParams { balls: 12, scale: 1.0, reactive: true },
        ..orbit_base()
    }
}

// ===================== Cherry blossom =====================
//
// FIELD MAPPING, borrowing TubeParams for the branch the way the reel family borrows it for a chassis -
// no params struct of its own, because two colours is not a schema:
//   lit                 the petal
//   hot                 a petal catching the light, and the lit edge of every petal
//   ghost               how dark the furthest petals go (floored at 0.45 - a petal that dim is a smudge)
//   tube.chassis_top    the lit top of the branch
//   tube.chassis_bottom the branch in shadow
//   tube.socket         the sky at the top of the panel
//   tube.collar         the sky toward the horizon
//   tube.glass          the moon
//
// Every colourway is a DUSK: a pale petal needs a dark sky to clear the 3:1 rule, and pale-pink-on-white
// is the one cherry blossom picture this panel cannot draw. A near-black sky with lantern-pale petals is
// both legal and, as it turns out, the more evocative of the two.
fn blossom_base() -> Theme {
    Theme {
        family: "blossom".into(),
        texture: Texture::None_,
        panel_alpha: 1.0,
        // Soft, and the one family where that is right: a petal catching light HAS a halo, and the marks
        // are 3px so there is nothing fine for a bloom to weld together.
        bloom: 4.0,
        glow_strength: 0.34,
        edge_glow: 1.0,
        ballistics: Ballistics { attack: 0.50, decay: 0.10, peak_fall: 0.005 },
        ..Theme::default()
    }
}

/// Dusk: the reference. Pale pink against deep indigo, brown branch.
pub fn blossom_dusk() -> Theme {
    Theme {
        id: "blossom-dusk".into(),
        name: "Dusk".into(),
        lit: "#ffb7d2".into(),
        hot: "#fff0f6".into(),
        panel: "#0a0710".into(),
        edge: "#ffb7d2".into(),
        edge_alpha: 0.13,
        ghost: 0.52,
        tube: TubeParams {
            chassis_top: "#6b5240".into(),
            chassis_bottom: "#2e2218".into(),
            // The sky and the moon: this family borrows the three unused TubeParams slots rather
            // than growing a params struct for what is three hex values.
            socket: "#120b22".into(),
            collar: "#2a1430".into(),
            glass: "#f3ddc8".into(),
            ..TubeParams::default()
        },
        ..blossom_base()
    }
}

/// Night: cooler, almost white petals under moonlight.
pub fn blossom_night() -> Theme {
    Theme {
        id: "blossom-night".into(),
        name: "Moonlit".into(),
        lit: "#dfe4ff".into(),
        hot: "#ffffff".into(),
        panel: "#05060d".into(),
        edge: "#dfe4ff".into(),
        edge_alpha: 0.12,
        ghost: 0.50,
        tube: TubeParams {
            chassis_top: "#4a4a5e".into(),
            chassis_bottom: "#1c1c26".into(),
            // The sky and the moon: this family borrows the three unused TubeParams slots rather
            // than growing a params struct for what is three hex values.
            socket: "#04050e".into(),
            collar: "#101a30".into(),
            glass: "#eef2ff".into(),
            ..TubeParams::default()
        },
        ..blossom_base()
    }
}

/// Lantern: warm, festival-lit - the deepest pink of the three.
pub fn blossom_lantern() -> Theme {
    Theme {
        id: "blossom-lantern".into(),
        name: "Lantern".into(),
        lit: "#ff9ec0".into(),
        hot: "#ffe6d8".into(),
        panel: "#100608".into(),
        edge: "#ff9ec0".into(),
        edge_alpha: 0.15,
        ghost: 0.55,
        tube: TubeParams {
            chassis_top: "#7a4f38".into(),
            chassis_bottom: "#331d14".into(),
            // The sky and the moon: this family borrows the three unused TubeParams slots rather
            // than growing a params struct for what is three hex values.
            socket: "#180608".into(),
            collar: "#3a1010".into(),
            glass: "#ffcf9a".into(),
            ..TubeParams::default()
        },
        ..blossom_base()
    }
}

/// White plum: the paler cousin, and the coldest branch.
pub fn blossom_plum() -> Theme {
    Theme {
        id: "blossom-plum".into(),
        name: "White plum".into(),
        lit: "#f6f2ff".into(),
        hot: "#ffffff".into(),
        panel: "#07070c".into(),
        edge: "#f6f2ff".into(),
        edge_alpha: 0.12,
        ghost: 0.48,
        tube: TubeParams {
            chassis_top: "#565064".into(),
            chassis_bottom: "#211f28".into(),
            // The sky and the moon: this family borrows the three unused TubeParams slots rather
            // than growing a params struct for what is three hex values.
            socket: "#06060e".into(),
            collar: "#141426".into(),
            glass: "#ffffff".into(),
            ..TubeParams::default()
        },
        ..blossom_base()
    }
}

/// Sakura gold: pink petals, gilded branch - the most ornamental of the five.
pub fn blossom_gold() -> Theme {
    Theme {
        id: "blossom-gold".into(),
        name: "Sakura gold".into(),
        lit: "#ffc2d6".into(),
        hot: "#fff6dc".into(),
        panel: "#0b0709".into(),
        edge: "#ffc2d6".into(),
        edge_alpha: 0.14,
        ghost: 0.52,
        tube: TubeParams {
            chassis_top: "#a8843c".into(),
            chassis_bottom: "#3d2f16".into(),
            // The sky and the moon: this family borrows the three unused TubeParams slots rather
            // than growing a params struct for what is three hex values.
            socket: "#100a10".into(),
            collar: "#2c1a18".into(),
            glass: "#ffe9b0".into(),
            ..TubeParams::default()
        },
        ..blossom_base()
    }
}

// ===================== 3D Pipes =====================
//
// The screensaver, driven. Brightness carries which AXIS a segment runs along - the three-tone shading
// that makes an isometric solid read as solid - so these colourways need a `lit` with enough range
// beneath it that a 62% and a 34% mix toward the panel are still clearly different from each other and
// from the panel. A very dark `lit` collapses all three tones together and the pipe reads flat.
fn pipes_base() -> Theme {
    Theme {
        family: "pipes".into(),
        texture: Texture::None_,
        panel_alpha: 1.0,
        // Tight: a wide bloom fills the gaps between pipe runs and the lattice stops reading as depth.
        bloom: 3.0,
        glow_strength: 0.40,
        edge_glow: 1.0,
        ballistics: Ballistics { attack: 0.55, decay: 0.14, peak_fall: 0.006 },
        ..Theme::default()
    }
}

/// The 1995 default: chrome-teal pipes on near-black, the OpenGL screensaver look.
pub fn pipes_win95_teal() -> Theme {
    Theme {
        id: "pipes-win95-teal".into(),
        name: "Win95 teal".into(),
        lit: "#5fe0d8".into(),
        hot: "#e0fffb".into(),
        panel: "#040b0c".into(),
        edge: "#5fe0d8".into(),
        edge_alpha: 0.15,
        ghost: 0.10,
        ..pipes_base()
    }
}

/// Brass plumbing, the warm end.
pub fn pipes_brass() -> Theme {
    Theme {
        id: "pipes-brass".into(),
        name: "Brass".into(),
        lit: "#ffbe4d".into(),
        hot: "#fff0cf".into(),
        panel: "#0d0803".into(),
        edge: "#ffbe4d".into(),
        edge_alpha: 0.15,
        ghost: 0.10,
        ..pipes_base()
    }
}

/// Hot magenta, for the vaporwave end of the same lineage.
pub fn pipes_neon_magenta() -> Theme {
    Theme {
        id: "pipes-neon-magenta".into(),
        name: "Neon magenta".into(),
        lit: "#ff6ad5".into(),
        hot: "#ffdaf4".into(),
        panel: "#0c0410".into(),
        edge: "#ff6ad5".into(),
        edge_alpha: 0.16,
        ghost: 0.10,
        ..pipes_base()
    }
}

/// Copper, and the darkest panel of the five - the tone separation has the most room here.
pub fn pipes_copper() -> Theme {
    Theme {
        id: "pipes-copper".into(),
        name: "Copper".into(),
        lit: "#ff9a6b".into(),
        hot: "#ffe0d0".into(),
        panel: "#0a0503".into(),
        edge: "#ff9a6b".into(),
        edge_alpha: 0.15,
        ghost: 0.10,
        ..pipes_base()
    }
}

/// Chrome: no hue at all, so the shape does the work.
pub fn pipes_chrome() -> Theme {
    Theme {
        id: "pipes-chrome".into(),
        name: "Chrome".into(),
        lit: "#dfe7f0".into(),
        hot: "#ffffff".into(),
        panel: "#070809".into(),
        edge: "#dfe7f0".into(),
        edge_alpha: 0.14,
        ghost: 0.10,
        ..pipes_base()
    }
}

// ===================== Orbit =====================
//
// Spheres circling in 3D, pulsing on size rather than brightness. Depth is cued three ways - size,
// shading and occlusion - so these colourways need a `hot` well clear of `lit` (the lit cap is what
// makes a disc read as a sphere rather than a hole) and a `panel` dark enough that dimming toward it
// reads as distance.
fn orbit_base() -> Theme {
    Theme {
        family: "orbit".into(),
        texture: Texture::None_,
        panel_alpha: 1.0,
        // Modest. A wide bloom merges overlapping balls and destroys the occlusion that carries depth.
        bloom: 4.0,
        glow_strength: 0.46,
        edge_glow: 1.0,
        ballistics: Ballistics { attack: 0.60, decay: 0.14, peak_fall: 0.006 },
        ..Theme::default()
    }
}

/// Chrome: no hue, so the shading and the occlusion do all the work.
pub fn orbit_chrome() -> Theme {
    Theme {
        id: "orbit-chrome".into(),
        name: "Chrome".into(),
        lit: "#c9d6e4".into(),
        hot: "#ffffff".into(),
        panel: "#06070a".into(),
        edge: "#c9d6e4".into(),
        edge_alpha: 0.14,
        ghost: 0.10,
        ..orbit_base()
    }
}

/// Sodium: the streetlamp orange, warm and heavy.
pub fn orbit_sodium() -> Theme {
    Theme {
        id: "orbit-sodium".into(),
        name: "Sodium".into(),
        lit: "#ff9d3c".into(),
        hot: "#fff0d4".into(),
        panel: "#0c0602".into(),
        edge: "#ff9d3c".into(),
        edge_alpha: 0.15,
        ghost: 0.10,
        ..orbit_base()
    }
}

/// Plasma: hot magenta into white, the vaporwave end.
pub fn orbit_plasma() -> Theme {
    Theme {
        id: "orbit-plasma".into(),
        name: "Plasma".into(),
        lit: "#ff5fd0".into(),
        hot: "#ffe4f8".into(),
        panel: "#0b0410".into(),
        edge: "#ff5fd0".into(),
        edge_alpha: 0.16,
        ghost: 0.10,
        ..orbit_base()
    }
}

/// Ion: cold cyan, the cleanest read of the five.
pub fn orbit_ion() -> Theme {
    Theme {
        id: "orbit-ion".into(),
        name: "Ion".into(),
        lit: "#5fd8ff".into(),
        hot: "#e6faff".into(),
        panel: "#03090e".into(),
        edge: "#5fd8ff".into(),
        edge_alpha: 0.15,
        ghost: 0.10,
        ..orbit_base()
    }
}

/// Lime: the most saturated, and the one with the most room under `lit` for the depth shading.
pub fn orbit_lime() -> Theme {
    Theme {
        id: "orbit-lime".into(),
        name: "Lime".into(),
        lit: "#a6f03c".into(),
        hot: "#f0ffd0".into(),
        panel: "#040a04".into(),
        edge: "#a6f03c".into(),
        edge_alpha: 0.14,
        ghost: 0.10,
        ..orbit_base()
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
    fn the_flame_ramps_keep_real_chroma() {
        // TRANSLUCENCY AND PALLOR ARE DIFFERENT THINGS, and confusing them cost this family its colour
        // once already. `render::flame` takes faintness from ALPHA - the body is translucent throughout -
        // so the ramp does not need to be pale, and making it pale as well washed the flames out. The
        // feedback was "the colours could be more saturated", and the fix was to put the chroma back
        // without touching a single alpha.
        //
        // Measured in OKLab, so this is perceptual chroma rather than an sRGB ratio. A mid orange sits
        // near 0.11 and the most saturated colours sRGB can express reach about 0.32.
        use crate::render::canvas::Rgba;
        for t in all().into_iter().filter(|t| t.family == "flame") {
            // Rainbow is exempt BY DESIGN: its hue comes from the per-column rainbow tint, so a chromatic
            // ramp underneath would fight it. Named explicitly rather than skipped by a chroma test that
            // silently tolerates grey.
            if t.id == "flame-rainbow" {
                continue;
            }
            assert_eq!(t.zones.len(), 4, "{}: expected four ramp stops", t.id);
            // The hottest stop is allowed to be pale - a real flame does go white at its base, and forcing
            // chroma there would make the core look tinted rather than hot.
            for (i, z) in t.zones.iter().enumerate().take(3) {
                let c = Rgba::oklab_chroma_of(Rgba::from_hex(&z.lit, 1.0));
                assert!(
                    c > 0.085,
                    "{} stop {i} ({}) has chroma {c:.3}, which is washed out. Faintness in this family                      comes from alpha, not from pale colour - see render::flame's body alpha",
                    t.id,
                    z.lit
                );
            }
            // And the coolest stop must be the most saturated of the three, because that is what a flame
            // does: chroma falls away as it approaches white-hot.
            let cool = Rgba::oklab_chroma_of(Rgba::from_hex(&t.zones[0].lit, 1.0));
            let warm = Rgba::oklab_chroma_of(Rgba::from_hex(&t.zones[2].lit, 1.0));
            assert!(
                cool > warm,
                "{}: chroma should fall toward the hot end, got {cool:.3} at the cool stop and                  {warm:.3} nearer the core",
                t.id
            );
        }
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
                worst.0 >= t.contrast_floor,
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
    fn no_colourway_lowers_the_contrast_floor_any_more() {
        // The note where CHROMA_BLUE_FLOOR used to be claimed a guard called
        // `only_the_recorded_colourways_lower_the_contrast_floor` kept the 2.30:1 opt-in from
        // spreading. That test never existed - the doc comment described an intention. This is it,
        // and now that the perceptual ramp has removed the need for any opt-in it can assert
        // something stronger than a permitted list: that the list is EMPTY.
        let lowered: Vec<(String, f32)> = all()
            .into_iter()
            .filter(|t| t.contrast_floor < 3.0)
            .map(|t| (t.id.clone(), t.contrast_floor))
            .collect();
        assert!(
            lowered.is_empty(),
            "no colourway should need a contrast opt-in any more, but these declare one: {lowered:?}"
        );
    }

    #[test]
    fn every_lit_colour_clears_three_to_one_against_its_own_panel() {
        // The spec's hard requirement, computed rather than eyeballed.
        //
        // Measured against each colourway's DECLARED floor, which is now 3.0 for every one of them:
        // the chroma field used to declare 2.30 and no longer needs to. The floor stays a
        // per-colourway declaration rather than a constant precisely so this test still bites on
        // every colourway, and so the next family that genuinely cannot meet the rule has to record
        // the fact rather than quietly relaxing it for everyone.
        for t in all() {
            let ratio = contrast(&t.lit, &t.panel);
            assert!(
                ratio >= t.contrast_floor,
                "{}: lit {} vs panel {} = {ratio:.2}:1, below its declared floor of {:.2}",
                t.id,
                t.lit,
                t.panel,
                t.contrast_floor
            );
            for z in &t.zones {
                let zr = contrast(&z.lit, &t.panel);
                assert!(zr >= t.contrast_floor, "{} zone {}: {zr:.2}:1", t.id, z.lit);
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
    fn within_the_segmented_family_only_the_classic_theme_uses_zones() {
        // Narrowed from "only the classic theme uses zones", which was true when `zones` existed
        // solely to express green/amber/red headroom on a bar meter. It is now also the natural way
        // to declare a multi-stop COLOUR RAMP, which the spectrogram and the LED ladder both need -
        // so the old premise is false by design rather than by accident.
        //
        // What is still worth guarding is the original intent: inside the segmented family, zones
        // change what a bar MEANS, so a second segmented colourway acquiring them by a stray
        // `..classic_three_colour()` would silently turn a plain meter into a headroom meter.
        for t in all().iter().filter(|t| t.family == "segmented") {
            let expect_zones = t.id == "classic-three-colour";
            assert_eq!(
                !t.zones.is_empty(),
                expect_zones,
                "{} is a segmented theme; only classic-three-colour should carry zones",
                t.id
            );
        }
        // And any family that does use them as a ramp must have them in ascending order reaching
        // the top - already asserted by `zones_ascend_and_reach_the_top`, which covers every family.
        let themes = all();
        let rampers: std::collections::BTreeSet<&str> = themes
            .iter()
            .filter(|t| !t.zones.is_empty() && t.family != "segmented")
            .map(|t| t.family.as_str())
            .collect();
        assert!(
            !rampers.is_empty(),
            "if no family uses zones as a ramp any more, fold this test back into the strict version"
        );
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
