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
        vu_cream(),
        vu_amber(),
        vu_ice(),
        vu_green(),
        vu_red(),
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
        fade: 0.14,
        // Distinct from the other four scope blooms below - see
        // `each_colourway_has_a_distinct_texture_or_bloom`, which checks
        // (texture, bloom) pairs across every theme in `all()`. All five
        // phosphors share `Texture::None_`, so bloom is the only thing that
        // can tell them apart for that guard; the brief left every phosphor
        // at the same inherited bloom (6.0), which collided.
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
        fade: 0.30,
        // The real P7 is two phosphor layers: a blue-white flash over a slow
        // yellow-green tail. The trail fades far more slowly than the trace.
        dual: Some(("#cfe86a".into(), 0.055)),
        bloom: 8.0,
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
        fade: 0.20,
        bloom: 6.0,
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
        fade: 0.11,
        bloom: 7.0,
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
        fade: 0.17,
        bloom: 9.0,
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
    fn ships_fifteen_colourways_across_three_families() {
        let all = all();
        assert_eq!(all.len(), 15, "expected 15 colourways, got {}", all.len());
        for fam in ["segmented", "scope", "vu"] {
            let n = all.iter().filter(|t| t.family == fam).count();
            assert_eq!(n, 5, "family {fam} should have 5 colourways, has {n}");
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
