use super::{Texture, Theme, Zone};

pub fn all() -> Vec<Theme> {
    vec![
        vfd_ice(),
        matrix_green(),
        neon_pink(),
        vac_tube_orange(),
        classic_three_colour(),
        p1_green(),
        p7_dual(),
        p11_blue_violet(),
        scope_amber(),
        scope_white(),
        mw2_trace(),
        scope_red(),
        scope_azure(),
        scope_magenta(),
        vu_cream(),
        vu_amber(),
        vu_ice(),
        vu_green(),
        vu_red(),
        vu_cyan(),
        vu_hot_pink(),
        vu_lime(),
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
    fn every_family_ships_and_no_theme_is_orphaned() {
        // Deliberately a floor, not an exact count. This asserted `len() == 15` and
        // exactly 5 per family, which made adding a colourway a test failure - directly
        // against the standing requirement that themes stay expandable. What it is
        // actually worth guarding is that no family silently loses its colourways and
        // that no theme carries a family name the renderer cannot dispatch on.
        let all = all();
        const FAMILIES: [&str; 3] = ["segmented", "scope", "vu"];
        let mut counted = 0;
        for fam in FAMILIES {
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
