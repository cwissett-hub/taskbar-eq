//! One shared 3x5 bitmap LABEL font: the whole alphabet, digits, and the punctuation a meter's
//! chrome needs, drawn at 4px cell pitch.
//!
//! # Why this exists next to `canvas::glyph_3x5`
//!
//! `Canvas::glyph_3x5` is deliberately a PARTIAL font: it serves the radar/RWR threat designators,
//! where a glyph that could be misread is worse than one not offered, so it omits every letter that
//! is ambiguous at 3 px (`S` reads as `5`, `T` as `I`, `N` has no unambiguous 3-wide form). That is
//! the right call there and it stays as it is.
//!
//! A LABEL, though, is read in context - `PLAY`, `REC`, `TRACKING`, `SYSTEM`, `MALFUNCTION` - where
//! the surrounding letters disambiguate, so a full alphabet reads fine and the missing letters are
//! what actually hurt (the `sesh` stamp rendered `PLAY`/`REC` as `PLA`/`R` through the partial font).
//! So this font completes the alphabet, taking the conventional pixel-font form for the ambiguous
//! letters:
//! - `S` uses the standard S and is a pixel twin of `5` in isolation;
//! - `T` uses the standard T and differs from `I` only in the foot;
//! - `N` uses a filled-diagonal form that reads as N in a word but as a blob alone.
//!
//! Three families grew a private copy of a 3x5 font (this one seeded from `sesh`'s stamp glyphs).
//! `vsghost` and `vswings` still carry their own private phrase fonts; those are out of scope here and
//! left to migrate to this module later.
//!
//! Row format matches `canvas::glyph_3x5`: five rows of three bits, **bit 2 leftmost** (so
//! `row & (0b100 >> dx)` tests column `dx`). An unsupported character advances the cursor and draws
//! nothing, exactly as the shared font does, so a stray character degrades to a gap.

use crate::render::canvas::{Canvas, Rgba};

/// One glyph, five rows of three bits (bit 2 leftmost), or `None` for an unsupported character.
///
/// `'>'` renders as a solid right-pointing triangle - the PLAY marker.
pub fn glyph(ch: char) -> Option<[u8; 5]> {
    // Upper-cased first: this is a single-case 3x5 font (lowercase is not distinguishable at 3px),
    // so a terminal command typed in lowercase renders in the same capitals as the sesh/night
    // labels. A no-op for the all-caps callers that predate this.
    Some(match ch.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b110, 0b100, 0b110, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        // The arch form: the old near-solid N ([101,111,111,111,101]) differed from M by one pixel, so
        // "DANGER TO MANIFOLD" read as "DAMGER TO MAMIFOLD". An arch cannot be confused with M or H.
        'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b111, 0b101, 0b111, 0b100, 0b100],
        'Q' => [0b111, 0b101, 0b101, 0b111, 0b001],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        // S: the conventional S, a pixel twin of `5` in isolation (see the module note).
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        // T: the conventional T, differs from `I` only in the foot (see the module note).
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        // The terminal family's meter blocks and the punctuation its command/label/trace strings
        // need: a solid bar cell, an outline peak marker, and `' _ ~ # % =` (`: . - /` already above).
        '▮' => [0b111, 0b111, 0b111, 0b111, 0b111],
        '▯' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '\'' => [0b010, 0b010, 0b000, 0b000, 0b000],
        '_' => [0b000, 0b000, 0b000, 0b000, 0b111],
        '~' => [0b000, 0b110, 0b011, 0b000, 0b000],
        '#' => [0b101, 0b111, 0b101, 0b111, 0b101],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '=' => [0b000, 0b111, 0b000, 0b111, 0b000],
        // Added for the term family's dodgy .mp3 suffixes (`_[320kbps]_[LEGIT]`,
        // `_(slowed+reverb)`): a plus and a pair of square brackets.
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '[' => [0b011, 0b010, 0b010, 0b010, 0b011],
        ']' => [0b110, 0b010, 0b010, 0b010, 0b110],
        // Added for the bling family's glitter phrases (`~*UR MINE*~`, `$$$`): an asterisk and a
        // dollar (an S with the bar through it). `~` was already here for the term family.
        '*' => [0b000, 0b101, 0b010, 0b101, 0b000],
        '$' => [0b011, 0b110, 0b010, 0b011, 0b110],
        // The PLAY marker: a solid right-pointing triangle.
        '>' => [0b100, 0b110, 0b111, 0b110, 0b100],
        ' ' => [0, 0, 0, 0, 0],
        _ => return None,
    })
}

/// Width `text` occupies at 4px cell pitch, no trailing gap. Counts every character (supported or
/// not), the same as the cursor advance in [`draw`] and as `Canvas::text_3x5_width`.
pub fn width(text: &str) -> i32 {
    (text.chars().count() as i32 * 4 - 1).max(0)
}

/// Draws `text` with its top-left at `(x, y)`, 4px cell pitch, returning the width drawn. An
/// unsupported character advances the cursor and draws nothing.
pub fn draw(c: &mut Canvas, x: i32, y: i32, text: &str, col: Rgba) -> i32 {
    let mut cx = x;
    for ch in text.chars() {
        if let Some(rows) = glyph(ch) {
            for (dy, row) in rows.iter().enumerate() {
                for dx in 0..3 {
                    if row & (0b100 >> dx) != 0 {
                        c.fill_rect(cx + dx, y + dy as i32, 1, 1, col);
                    }
                }
            }
        }
        cx += 4;
    }
    (cx - x - 1).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::{Canvas, Rgba};

    #[test]
    fn every_letter_and_digit_has_a_glyph_within_three_bits() {
        for ch in ('A'..='Z').chain('0'..='9') {
            let rows = glyph(ch).unwrap_or_else(|| panic!("no glyph for {ch:?}"));
            for (i, r) in rows.iter().enumerate() {
                assert_eq!(r >> 3, 0, "{ch:?} row {i} has bits beyond 3 columns");
            }
            assert!(rows.iter().any(|r| *r != 0), "{ch:?} is blank");
        }
        assert!(glyph('@').is_none(), "an unsupported char must yield None");
    }

    #[test]
    fn width_is_four_px_pitch_without_a_trailing_gap() {
        assert_eq!(width("PLAY"), 15);
        assert_eq!(width(""), 0);
        assert_eq!(width("A"), 3);
    }

    #[test]
    fn draw_paints_inside_its_box_and_returns_the_width() {
        let mut c = Canvas::new(60, 12);
        let col = Rgba::new(255, 255, 255, 255);
        let w = draw(&mut c, 2, 3, "PLAY", col);
        assert_eq!(w, 15);
        let mut lit = 0;
        for y in 0..12 {
            for x in 0..60 {
                if c.get(x, y).a > 8 {
                    assert!((2..2 + 15).contains(&x) && (3..3 + 5).contains(&y), "paint at {x},{y} outside the box");
                    lit += 1;
                }
            }
        }
        assert!(lit > 20, "PLAY drew almost nothing: {lit}");
    }

    #[test]
    fn the_labels_the_families_need_all_have_glyphs() {
        // The sesh stamp and the strings the night family will need.
        let labels = [
            "PLAY", "REC", "TRACKING", "00:00:00", "RAM", "HP", "NET", "SYSTEM", "MALFUNCTION",
            "RELIC 2.0 ERR",
            // The term family's block glyphs and its status/comment labels.
            "▮▯>", "~/music", "UTF-8  LF", "EXIT 101", "# bpm ~ 142",
            // The term family's dodgy .mp3 filename suffixes (see `term::SUFFIXES`) - the reason
            // `+`, `[`, `]` were added above.
            "_(not_a_virus)", "_(official_audio)_(real)", "_FINAL_v2_FINAL", "(1)",
            "_[320kbps]_[LEGIT]", "_(free_download)", "_-_Copy", "_(slowed+reverb)",
            "_(100%_no_virus)", "_(radio_edit)_(extended)",
            // The bling family's glitter phrases (see `bling::PHRASES`) - the reason `*` and `$`
            // were added above.
            "BLING BLING", "ICED OUT", "4 REAL", "XOXO", "~*UR MINE*~", "$$$", "HOTTIE", "LUV U 4EVA",
        ];
        for label in labels {
            for ch in label.chars() {
                assert!(glyph(ch).is_some(), "no glyph for {ch:?} in {label:?}");
            }
        }
        assert!(glyph('>').is_some(), "the PLAY marker has no glyph");
    }
}
