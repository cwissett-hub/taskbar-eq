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

/// A small xorshift, seeded by the caller.
///
/// Deliberately not a dependency. The requirement is "pick a different one", not statistical quality,
/// and a seeded generator written here is what lets every test below be exact rather than flaky.
fn next(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
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
    let mut s = if seed == 0 { 0x9e37_79b9_7f4a_7c15 } else { seed };
    Some(eligible[(next(&mut s) % eligible.len() as u64) as usize])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn all() -> Vec<Theme> {
        builtin::all()
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
