//! Choosing a colourway at random.
//!
//! Pure and seeded, so the behaviour that actually matters - never picking what is already showing,
//! and staying inside the current family when asked - is testable without a window, a keyboard or a
//! clock. The caller supplies the seed.

use super::Theme;

/// Which kind of shuffle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomKind {
    /// Any colourway from any family, so the family changes too.
    AnyTheme,
    /// A different colourway inside the family already showing.
    SameFamily,
}

/// Mixes a seed into a well-distributed 64-bit value: the SplitMix64 finaliser.
///
/// **This replaced a single xorshift round, which was measurably biased.** Reported as "the random
/// buttons would seem to favour certain colourways", and the measurement is in `bias_probe`:
///
/// ```text
///   one xorshift round, walking 20,000 presses inside one family:  chi2/df 24.93
///   splitmix64 finaliser, same walk:                               chi2/df ~1
/// ```
///
/// Two things combined to cause it, and neither is visible on its own:
///
/// 1. **The seed's low bits are dead.** The caller seeds from `SystemTime::now().as_nanos()`, and the
///    Windows system clock is quantised to 100ns - measured, `nanos & 3` is 0 on every single sample,
///    and `nanos & 7` only ever takes two of its eight values.
/// 2. **One xorshift round barely touches the low bits.** With `x ^= x<<13; x ^= x>>7; x ^= x<<17`,
///    output bit 0 is just input bit 0 xor bit 7 - the shifts left cannot affect it at all. So the low
///    bits of the result are a near-trivial function of the low bits of the seed.
///
/// `% n` reads exactly those low bits. With 92 eligible the modulus is not a power of two so it pulls
/// in the higher, better-mixed bits and the bias hides; inside a family of nine, `% 8` reads ONLY the
/// low three bits and the bias is stark. That is why the whole-list shuffle looked fine while the
/// within-family one did not.
///
/// SplitMix64's finaliser avalanches every input bit across all 64 output bits, which fixes small and
/// power-of-two moduli together. Still no dependency, still exactly reproducible from a seed, so every
/// test below stays deterministic.
fn mix(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Picks an index into `themes`, or `None` when there is nothing eligible to switch to.
///
/// EXCLUDES WHAT IS ALREADY SHOWING. That is the whole difference between a feature and an apparent
/// no-op: with 88 colourways a shuffle that can return the current one looks broken roughly one press
/// in 88, and inside a family of five it looks broken one press in five.
///
/// The one exception is a family with a single colourway, where there is no other choice. `None` is
/// returned rather than the current index, so the caller can say nothing happened instead of
/// pretending something did.
pub fn pick(themes: &[Theme], current_id: &str, kind: RandomKind, seed: u64) -> Option<usize> {
    if themes.is_empty() {
        return None;
    }
    let current_family = themes.iter().find(|t| t.id == current_id).map(|t| t.family.clone());

    let eligible: Vec<usize> = themes
        .iter()
        .enumerate()
        .filter(|(_, t)| t.id != current_id)
        .filter(|(_, t)| match (kind, &current_family) {
            (RandomKind::SameFamily, Some(f)) => &t.family == f,
            // No current family known - the selected id is not in the list, which happens after a
            // theme file is deleted - so "same family" has nothing to be the same as. Falling back to
            // the whole list beats doing nothing.
            (RandomKind::SameFamily, None) => true,
            (RandomKind::AnyTheme, _) => true,
        })
        .map(|(i, _)| i)
        .collect();

    if eligible.is_empty() {
        return None;
    }

    // ANY-THEME PICKS A FAMILY FIRST, then a colourway inside it.
    //
    // Uniform over colourways is NOT uniform over looks, and the difference is large enough to be the
    // other half of "favour certain colourways or theme". Measured over 20,000 presses: the
    // oscilloscope family took 14.2% of them because it has thirteen colourways, against 5.1% for
    // nixie with five - a 2.6x difference in how often a look appears. Somebody pressing "any theme"
    // wants to see the thirteen looks, not a lottery weighted by how many variants each happens to
    // have.
    //
    // Within-family shuffle is untouched by this: there is only one family involved, and its job is to
    // walk that family's colourways evenly.
    if kind == RandomKind::AnyTheme {
        let mut families: Vec<&str> = Vec::new();
        for i in &eligible {
            let f = themes[*i].family.as_str();
            if !families.contains(&f) {
                families.push(f);
            }
        }
        if families.len() > 1 {
            let fam = families[(mix(seed) % families.len() as u64) as usize];
            let within: Vec<usize> =
                eligible.iter().copied().filter(|i| themes[*i].family == fam).collect();
            if !within.is_empty() {
                // A SECOND, independent draw. Reusing `mix(seed)` for both would tie the colourway to
                // the family - with thirteen families and up to thirteen colourways, some pairs would
                // never occur at all.
                return Some(within[(mix(seed ^ 0xA5A5_5A5A_C3C3_3C3C) % within.len() as u64) as usize]);
            }
        }
    }
    Some(eligible[(mix(seed) % eligible.len() as u64) as usize])
}

#[cfg(test)]
mod bias_probe {
    use super::*;

    /// Is the shuffle actually uniform when seeded the way the app seeds it?
    ///
    /// Run: cargo test --release probe_shuffle_bias -- --ignored --nocapture
    ///
    /// Reported as "the random buttons would seem to favour certain colourways or theme". Two suspects,
    /// and this measures both: the SEED (the app uses `SystemTime::now().as_nanos()`, and the Windows
    /// system clock is quantised to ~100ns, so the low bits carry far less entropy than the number's
    /// size suggests) and the MIXING (one xorshift round barely touches the low bits - output bit 0 is
    /// just input bit 0 xor bit 7 - and `% n` reads exactly those bits).
    #[test]
    #[ignore]
    fn probe_shuffle_bias() {
        // 1. What the real clock actually gives us in its low bits.
        let mut lows = [0usize; 8];
        let mut mod4 = [0usize; 4];
        for _ in 0..2000 {
            let n = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1);
            lows[(n & 7) as usize] += 1;
            mod4[(n & 3) as usize] += 1;
            std::hint::black_box(n);
        }
        println!("clock nanos & 7  : {lows:?}");
        println!("clock nanos & 3  : {mod4:?}   <- all in one bucket means 100ns quantisation");

        // 2. The distribution the user actually sees, seeded exactly as the app seeds it.
        let themes = crate::themes::builtin::all();
        let current = "radar-p1";
        for kind in [RandomKind::AnyTheme, RandomKind::SameFamily] {
            let mut hist = std::collections::BTreeMap::new();
            let draws = 4000;
            for _ in 0..draws {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0x51ed_2701);
                if let Some(i) = pick(&themes, current, kind, seed) {
                    *hist.entry(i).or_insert(0usize) += 1;
                }
            }
            let n = hist.len();
            let expect = draws as f64 / n as f64;
            let chi2: f64 = hist.values().map(|c| {
                let d = *c as f64 - expect;
                d * d / expect
            }).sum();
            let mut counts: Vec<usize> = hist.values().copied().collect();
            counts.sort_unstable();
            println!(
                "{kind:?}: {n} distinct of {} eligible, min {} max {} (expected {:.0} each), chi2/df {:.1}",
                themes.iter().filter(|t| t.id != current).count(),
                counts[0],
                counts[counts.len() - 1],
                expect,
                chi2 / (n as f64 - 1.0)
            );
        }

        // 3. THE REAL SEQUENCE. The app updates `current` to whatever was picked, so successive presses
        // are a walk, not independent draws from a fixed starting point - and a walk can be biased in
        // ways independent draws are not.
        for kind in [RandomKind::AnyTheme, RandomKind::SameFamily] {
            let mut cur = themes[0].id.clone();
            let mut hist = std::collections::BTreeMap::new();
            let mut fam = std::collections::BTreeMap::new();
            let presses = 20_000;
            for _ in 0..presses {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0x51ed_2701);
                if let Some(i) = pick(&themes, &cur, kind, seed) {
                    *hist.entry(themes[i].id.clone()).or_insert(0usize) += 1;
                    *fam.entry(themes[i].family.clone()).or_insert(0usize) += 1;
                    cur = themes[i].id.clone();
                }
            }
            let n = hist.len();
            let expect = presses as f64 / n as f64;
            let chi2: f64 = hist.values().map(|c| {
                let d = *c as f64 - expect;
                d * d / expect
            }).sum();
            let mut by: Vec<(&String, &usize)> = hist.iter().collect();
            by.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
            // For AnyTheme the per-COLOURWAY figure is expected to be high and is not a defect:
            // drawing a family first makes a colourway in a five-strong family likelier than one in a
            // thirteen-strong family. The number that matters there is the per-family one below.
            let what = if kind == RandomKind::AnyTheme {
                "chi2/df per colourway (EXPECTED high - see per-family)"
            } else {
                "chi2/df per colourway"
            };
            println!(
                "{kind:?} WALK over {presses} presses: {n} distinct, {what} {:.2}",
                chi2 / (n as f64 - 1.0)
            );
            println!("  most:  {:?}", &by[..5.min(by.len())]);
            println!("  least: {:?}", &by[by.len().saturating_sub(5)..]);
            if kind == RandomKind::AnyTheme {
                let mut fams: Vec<(&String, &usize)> = fam.iter().collect();
                fams.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
                let fexpect = presses as f64 / fams.len() as f64;
                let fchi2: f64 = fams
                    .iter()
                    .map(|(_, c)| {
                        let d = **c as f64 - fexpect;
                        d * d / fexpect
                    })
                    .sum();
                println!(
                    "  by family: chi2/df {:.2} (THIS is the one that matters), target {:.2}% each",
                    fchi2 / (fams.len() as f64 - 1.0),
                    100.0 / fams.len() as f64
                );
                for (f, c) in &fams {
                    let size = themes.iter().filter(|t| &t.family == *f).count();
                    println!(
                        "    {f:11} {c:5} presses, {size:2} colourways, {:.2}%",
                        **c as f64 / presses as f64 * 100.0
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn all() -> Vec<Theme> {
        builtin::all()
    }

    /// Seeds shaped like the ones the app really gets.
    ///
    /// **Multiples of 100, because that is what the Windows system clock produces.** Measured on the
    /// real clock, `SystemTime::now().as_nanos() & 3` is zero on every sample. A test that swept
    /// arbitrary consecutive seeds would have passed against the old single-round xorshift and missed
    /// the bias entirely - the quantisation is half the bug, and a fixture has to contain it.
    fn clocklike(i: u64) -> u64 {
        1_700_000_000_000_000_000u64.wrapping_add(i.wrapping_mul(100))
    }

    /// chi-squared per degree of freedom over a histogram. 1.0 is a perfect fit.
    fn chi2_per_df(counts: &[usize]) -> f64 {
        let n = counts.len() as f64;
        let total: usize = counts.iter().sum();
        let expect = total as f64 / n;
        let chi2: f64 = counts.iter().map(|c| {
            let d = *c as f64 - expect;
            d * d / expect
        }).sum();
        chi2 / (n - 1.0)
    }

    #[test]
    fn a_within_family_shuffle_is_uniform_even_with_a_quantised_clock() {
        // Reported as the random buttons favouring certain colourways. Measured before the fix, walking
        // one family for 20,000 presses gave chi2/df 24.93 - the two least-visited colourways were
        // seven sigma light. After it, 1.19.
        //
        // A within-family shuffle is the hard case and the reason the bug hid: nine colourways means
        // eight eligible, and `% 8` reads ONLY the low three bits - exactly the bits a 100ns-quantised
        // clock and one xorshift round leave unmixed. The whole-list shuffle uses a modulus that is not
        // a power of two, pulls in better-mixed higher bits, and looked fine throughout.
        let themes = all();
        let mut cur = themes[0].id.clone();
        let family = themes[0].family.clone();
        let size = themes.iter().filter(|t| t.family == family).count();
        assert!(size >= 5, "this test needs a family with several colourways, got {size}");

        let mut hist = std::collections::BTreeMap::new();
        let presses = 8000u64;
        for i in 0..presses {
            let Some(k) = pick(&themes, &cur, RandomKind::SameFamily, clocklike(i)) else {
                panic!("a family of {size} should always have somewhere to go");
            };
            assert_eq!(themes[k].family, family, "a same-family shuffle left the family");
            *hist.entry(themes[k].id.clone()).or_insert(0usize) += 1;
            cur = themes[k].id.clone();
        }
        assert_eq!(hist.len(), size, "the walk did not reach every colourway in the family");
        let counts: Vec<usize> = hist.values().copied().collect();
        let x = chi2_per_df(&counts);
        assert!(
            x < 3.0,
            "within-family shuffle is biased: chi2/df {x:.2} over {presses} presses, counts {counts:?}"
        );
    }

    #[test]
    fn any_theme_is_uniform_over_families_not_over_colourways() {
        // The other half of the report - "favour certain theme". Uniform over colourways is not uniform
        // over LOOKS: measured, the thirteen-colourway oscilloscope family took 14.2% of presses against
        // 5.1% for five-colourway nixie, a 2.6x difference in how often a look appeared. Somebody
        // pressing "any theme" wants the looks evenly, so a family is drawn first.
        //
        // The consequence is deliberate and asserted here too: a colourway in a small family IS more
        // likely than one in a large family, so a per-colourway uniformity test would now fail on
        // purpose.
        let themes = all();
        let mut cur = themes[0].id.clone();
        let mut fams = std::collections::BTreeMap::new();
        let presses = 13_000u64;
        for i in 0..presses {
            let Some(k) = pick(&themes, &cur, RandomKind::AnyTheme, clocklike(i)) else {
                panic!("the whole list should always have somewhere to go");
            };
            *fams.entry(themes[k].family.clone()).or_insert(0usize) += 1;
            cur = themes[k].id.clone();
        }
        let all_families: std::collections::BTreeSet<&String> =
            themes.iter().map(|t| &t.family).collect();
        assert_eq!(
            fams.len(),
            all_families.len(),
            "some family was never reached: {:?}",
            all_families.iter().filter(|f| !fams.contains_key(**f)).collect::<Vec<_>>()
        );
        let counts: Vec<usize> = fams.values().copied().collect();
        let x = chi2_per_df(&counts);
        assert!(
            x < 3.0,
            "families are not evenly reached: chi2/df {x:.2}, {fams:?}"
        );
        // And within one family, the colourways ARE still even - drawing a family first must not make
        // one of its members a favourite.
        let biggest = all_families
            .iter()
            .max_by_key(|f| themes.iter().filter(|t| &t.family == **f).count())
            .unwrap();
        let mut within = std::collections::BTreeMap::new();
        for i in 0..presses {
            if let Some(k) = pick(&themes, "no-such-id", RandomKind::AnyTheme, clocklike(i)) {
                if &themes[k].family == *biggest {
                    *within.entry(themes[k].id.clone()).or_insert(0usize) += 1;
                }
            }
        }
        let wcounts: Vec<usize> = within.values().copied().collect();
        assert!(wcounts.len() > 3, "not enough samples inside {biggest}");
        let wx = chi2_per_df(&wcounts);
        assert!(wx < 4.0, "inside {biggest} the colourways are uneven: chi2/df {wx:.2} {within:?}");
    }

    #[test]
    fn a_shuffle_never_returns_what_is_already_showing() {
        // The failure this prevents is a press that appears to do nothing. Swept over many seeds
        // rather than one, because a single seed proves nothing about a random function.
        let themes = all();
        for kind in [RandomKind::AnyTheme, RandomKind::SameFamily] {
            for t in &themes {
                for seed in 1..40u64 {
                    if let Some(i) = pick(&themes, &t.id, kind, seed * 7919) {
                        assert_ne!(
                            themes[i].id, t.id,
                            "{kind:?} returned the current colourway {} at seed {seed}",
                            t.id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn same_family_stays_inside_the_family_and_any_theme_can_leave_it() {
        let themes = all();
        let start = themes.iter().find(|t| t.family == "fluid").expect("a fluid colourway");

        // SameFamily must never leave.
        for seed in 1..200u64 {
            if let Some(i) = pick(&themes, &start.id, RandomKind::SameFamily, seed * 104_729) {
                assert_eq!(
                    themes[i].family, start.family,
                    "SameFamily jumped to {} at seed {seed}",
                    themes[i].family
                );
            }
        }

        // AnyTheme must actually be capable of leaving, or the two modes are the same feature. With
        // 13 families it should escape within a handful of seeds.
        let escaped = (1..200u64).any(|seed| {
            pick(&themes, &start.id, RandomKind::AnyTheme, seed * 104_729)
                .map(|i| themes[i].family != start.family)
                .unwrap_or(false)
        });
        assert!(escaped, "AnyTheme never left the starting family, so it is not a different mode");
    }

    #[test]
    fn a_shuffle_reaches_every_other_colourway_in_a_family_rather_than_a_favourite_few() {
        // A picker that returns one or two values would satisfy "not the current one" and still be
        // useless. This checks coverage.
        let themes = all();
        let start = themes.iter().find(|t| t.family == "fluid").expect("a fluid colourway");
        let family_size = themes.iter().filter(|t| t.family == start.family).count();
        let mut seen = std::collections::HashSet::new();
        for seed in 1..400u64 {
            if let Some(i) = pick(&themes, &start.id, RandomKind::SameFamily, seed * 2_654_435_761) {
                seen.insert(themes[i].id.clone());
            }
        }
        assert_eq!(
            seen.len(),
            family_size - 1,
            "only reached {} of the {} other colourways in {}: {seen:?}",
            seen.len(),
            family_size - 1,
            start.family
        );
    }

    #[test]
    fn a_family_with_one_colourway_reports_that_nothing_can_change() {
        // None rather than the current index, so the caller can stay quiet instead of pretending.
        let only = vec![builtin::fluid_deep()];
        assert_eq!(pick(&only, "fluid-deep", RandomKind::SameFamily, 42), None);
        assert_eq!(pick(&only, "fluid-deep", RandomKind::AnyTheme, 42), None);
        // But a second one in the same family is reachable.
        let two = vec![builtin::fluid_deep(), builtin::fluid_mercury()];
        assert_eq!(pick(&two, "fluid-deep", RandomKind::SameFamily, 42), Some(1));
    }

    #[test]
    fn an_unknown_current_id_still_picks_something() {
        // Happens for real: a theme file is deleted while its id is still the saved selection. Doing
        // nothing would make the shuffle look broken exactly when the user is trying to get away from
        // a broken selection.
        let themes = all();
        for kind in [RandomKind::AnyTheme, RandomKind::SameFamily] {
            assert!(
                pick(&themes, "no-such-colourway", kind, 12345).is_some(),
                "{kind:?} gave up on an unknown current id"
            );
        }
    }

    #[test]
    fn a_zero_seed_is_still_random_rather_than_stuck() {
        // xorshift is a fixed point at zero: seeded with 0 it returns 0 for ever, so every press
        // would pick the same colourway. The caller seeds from the clock, which can plausibly be 0.
        let themes = all();
        let a = pick(&themes, "fluid-deep", RandomKind::AnyTheme, 0);
        assert!(a.is_some());
        // And it must not coincidentally be the first eligible entry, which is what a broken
        // generator returns.
        let distinct: std::collections::HashSet<usize> =
            (0..8u64).filter_map(|k| pick(&themes, "fluid-deep", RandomKind::AnyTheme, k)).collect();
        assert!(distinct.len() > 1, "seeds 0..8 all gave the same answer: {distinct:?}");
    }

    #[test]
    fn the_empty_list_is_handled_rather_than_panicking() {
        assert_eq!(pick(&[], "anything", RandomKind::AnyTheme, 1), None);
        assert_eq!(pick(&[], "anything", RandomKind::SameFamily, 1), None);
    }
}
