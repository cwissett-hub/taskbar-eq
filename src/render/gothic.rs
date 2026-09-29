//! A bespoke pixel-BLACKLETTER font, for the `sesh` family and nothing else.
//!
//! The shared `Canvas::text_3x5` is a clean, legible 3x5 grotesque - exactly the wrong voice here.
//! The `sesh` family's whole aesthetic IS one word (`SESH`, `BONES`, `TEAMSESH`, ...) set in the
//! gothic/old-english lettering of the TeamSESH / Bones tapes, centred on the taskbar. A generic
//! pixel font makes it read as a status label; the blackletter makes it read as the logo. So this is
//! the one place in the family that earns a hand-drawn font.
//!
//! The blackletter tells, drawn deliberately into every glyph at 9 and 13 rows:
//! - heavy vertical stems 2 px wide (the test asserts every letter has an adjacent-bit pair),
//! - diamond/lozenge terminals - a 1 px point offset above the head and below the foot of a stem,
//! - broken bowls on `O`/`S`/`B` - a straight diagonal where a round face would curve,
//! - hairline 1 px horizontals for the arms of `E`/`T`/`L`/`Z`.
//!
//! Only the 13 letters these five words need, plus space: `S E H B O N T A M L W Y Z`.
//!
//! Row encoding: each `u16` is one row, **bit 0 = the leftmost column** (so `1 << col` tests a
//! column). The `// ....##` picture beside every literal is the authoritative, rendered-orientation
//! view; because bit 0 is the LEFT end, the binary literal itself reads left-to-right as the MIRROR
//! of that picture. Trust the comment. The glyph data was authored as ASCII pictures and the
//! literals generated from them, so the two cannot drift.
//!
//! Nothing in the crate calls this yet: the `sesh` family (the next task) is its only consumer, so
//! the module-level `dead_code` allow keeps the CI `-D warnings` gate green until that wiring lands.
//! It follows the same pattern as the not-yet-read `FrameData` fields in `render/mod.rs`.
#![allow(dead_code)]

use crate::render::canvas::{Canvas, Rgba};

/// The only characters with a glyph. Anything else yields `None` from [`glyph`].
pub const GOTHIC_CHARS: &str = "SEHBONTAMLWYZ ";

/// Two hand-drawn sizes: `Small` is 9 rows (glyphs 5-6 columns, space 3), `Large` is 13 rows
/// (glyphs 7-8 columns, space 4). The `sesh` family picks the largest that fits the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GothicSize {
    Small,
    Large,
}

// ===== LARGE (13 rows) =====
const S_L: ([u16; 13], i32) = (
    [
        0b1111100, // ..#####
        0b1000110, // .##...#
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000110, // .##....
        0b0011100, // ..###..
        0b0110000, // ....##.
        0b1100000, // .....##
        0b1100001, // #....##
        0b1100011, // ##...##
        0b0110110, // .##.##.
        0b0011100, // ..###..
        0b0000000, // .......
    ],
    7,
);
const E_L: ([u16; 13], i32) = (
    [
        0b0000010, // .#.....
        0b0011111, // #####..
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0011111, // #####..
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0111111, // ######.
        0b0000001, // #......
    ],
    7,
);
const H_L: ([u16; 13], i32) = (
    [
        0b0100010, // .#...#.
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1111111, // #######
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b0100001, // #....#.
    ],
    7,
);
const B_L: ([u16; 13], i32) = (
    [
        0b0000010, // .#.....
        0b0011111, // #####..
        0b0110011, // ##..##.
        0b0110011, // ##..##.
        0b0011011, // ##.##..
        0b0001111, // ####...
        0b0011011, // ##.##..
        0b0110011, // ##..##.
        0b0110011, // ##..##.
        0b0110011, // ##..##.
        0b0110011, // ##..##.
        0b0011111, // #####..
        0b0000001, // #......
    ],
    7,
);
const O_L: ([u16; 13], i32) = (
    [
        0b0011100, // ..###..
        0b0110110, // .##.##.
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b0110110, // .##.##.
        0b0011100, // ..###..
    ],
    7,
);
const N_L: ([u16; 13], i32) = (
    [
        0b0100010, // .#...#.
        0b1100011, // ##...##
        0b1100111, // ###..##
        0b1100111, // ###..##
        0b1101011, // ##.#.##
        0b1101011, // ##.#.##
        0b1101011, // ##.#.##
        0b1110011, // ##..###
        0b1110011, // ##..###
        0b1110011, // ##..###
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b0100001, // #....#.
    ],
    7,
);
const T_L: ([u16; 13], i32) = (
    [
        0b1111111, // #######
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0001000, // ...#...
    ],
    7,
);
const A_L: ([u16; 13], i32) = (
    [
        0b0001000, // ...#...
        0b0011100, // ..###..
        0b0011100, // ..###..
        0b0110110, // .##.##.
        0b0110110, // .##.##.
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1111111, // #######
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b1100011, // ##...##
        0b0100001, // #....#.
    ],
    7,
);
const M_L: ([u16; 13], i32) = (
    [
        0b01000010, // .#....#.
        0b11100111, // ###..###
        0b10111111, // ######.#
        0b11011011, // ##.##.##
        0b11011011, // ##.##.##
        0b11000011, // ##....##
        0b11000011, // ##....##
        0b11000011, // ##....##
        0b11000011, // ##....##
        0b11000011, // ##....##
        0b11000011, // ##....##
        0b11000011, // ##....##
        0b10000001, // #......#
    ],
    8,
);
const L_L: ([u16; 13], i32) = (
    [
        0b0000010, // .#.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0111111, // ######.
        0b1000001, // #.....#
    ],
    7,
);
const W_L: ([u16; 13], i32) = (
    [
        0b01000001, // #.....#.
        0b01100011, // ##...##.
        0b01100011, // ##...##.
        0b01100011, // ##...##.
        0b01100011, // ##...##.
        0b01101011, // ##.#.##.
        0b01101011, // ##.#.##.
        0b01101011, // ##.#.##.
        0b01110111, // ###.###.
        0b01101011, // ##.#.##.
        0b00110110, // .##.##..
        0b00110110, // .##.##..
        0b00010100, // ..#.#...
    ],
    8,
);
const Y_L: ([u16; 13], i32) = (
    [
        0b0100010, // .#...#.
        0b1100011, // ##...##
        0b0110110, // .##.##.
        0b0110110, // .##.##.
        0b0011100, // ..###..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0001000, // ...#...
    ],
    7,
);
const Z_L: ([u16; 13], i32) = (
    [
        0b0111111, // ######.
        0b0110000, // ....##.
        0b0011000, // ...##..
        0b0011000, // ...##..
        0b0001100, // ..##...
        0b0001100, // ..##...
        0b0000110, // .##....
        0b0000110, // .##....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0000011, // ##.....
        0b0111111, // ######.
        0b1000000, // ......#
    ],
    7,
);
const SP_L: ([u16; 13], i32) = ([0; 13], 4);

// ===== SMALL (9 rows) =====
const S_S: ([u16; 9], i32) = (
    [
        0b11110, // .####
        0b10011, // ##..#
        0b00011, // ##...
        0b01110, // .###.
        0b11000, // ...##
        0b11001, // #..##
        0b01110, // .###.
        0b00011, // ##...
        0b00000, // .....
    ],
    5,
);
const E_S: ([u16; 9], i32) = (
    [
        0b00010, // .#...
        0b01111, // ####.
        0b00011, // ##...
        0b00011, // ##...
        0b00111, // ###..
        0b00011, // ##...
        0b00011, // ##...
        0b01111, // ####.
        0b00001, // #....
    ],
    5,
);
const H_S: ([u16; 9], i32) = (
    [
        0b01010, // .#.#.
        0b11011, // ##.##
        0b11011, // ##.##
        0b11011, // ##.##
        0b11111, // #####
        0b11011, // ##.##
        0b11011, // ##.##
        0b11011, // ##.##
        0b01001, // #..#.
    ],
    5,
);
const B_S: ([u16; 9], i32) = (
    [
        0b00010, // .#...
        0b00111, // ###..
        0b01011, // ##.#.
        0b01011, // ##.#.
        0b00111, // ###..
        0b01011, // ##.#.
        0b01011, // ##.#.
        0b00111, // ###..
        0b00001, // #....
    ],
    5,
);
const O_S: ([u16; 9], i32) = (
    [
        0b01110, // .###.
        0b11011, // ##.##
        0b11011, // ##.##
        0b11011, // ##.##
        0b11011, // ##.##
        0b11011, // ##.##
        0b11011, // ##.##
        0b01110, // .###.
        0b00000, // .....
    ],
    5,
);
const N_S: ([u16; 9], i32) = (
    [
        0b01010, // .#.#.
        0b11011, // ##.##
        0b10111, // ###.#
        0b10111, // ###.#
        0b11011, // ##.##
        0b11011, // ##.##
        0b11011, // ##.##
        0b11011, // ##.##
        0b01001, // #..#.
    ],
    5,
);
const T_S: ([u16; 9], i32) = (
    [
        0b11111, // #####
        0b00100, // ..#..
        0b00100, // ..#..
        0b00100, // ..#..
        0b00100, // ..#..
        0b00100, // ..#..
        0b00100, // ..#..
        0b00100, // ..#..
        0b00100, // ..#..
    ],
    5,
);
const A_S: ([u16; 9], i32) = (
    [
        0b00100, // ..#..
        0b01110, // .###.
        0b01010, // .#.#.
        0b11011, // ##.##
        0b11111, // #####
        0b11011, // ##.##
        0b11011, // ##.##
        0b11011, // ##.##
        0b10001, // #...#
    ],
    5,
);
const M_S: ([u16; 9], i32) = (
    [
        0b010010, // .#..#.
        0b110111, // ###.##
        0b111111, // ######
        0b111011, // ##.###
        0b100011, // ##...#
        0b100011, // ##...#
        0b100011, // ##...#
        0b100011, // ##...#
        0b100001, // #....#
    ],
    6,
);
const L_S: ([u16; 9], i32) = (
    [
        0b00010, // .#...
        0b00011, // ##...
        0b00011, // ##...
        0b00011, // ##...
        0b00011, // ##...
        0b00011, // ##...
        0b00011, // ##...
        0b01111, // ####.
        0b10001, // #...#
    ],
    5,
);
const W_S: ([u16; 9], i32) = (
    [
        0b100001, // #....#
        0b110011, // ##..##
        0b110011, // ##..##
        0b110011, // ##..##
        0b111011, // ##.###
        0b110111, // ###.##
        0b011110, // .####.
        0b011110, // .####.
        0b001100, // ..##..
    ],
    6,
);
const Y_S: ([u16; 9], i32) = (
    [
        0b01010, // .#.#.
        0b11011, // ##.##
        0b01110, // .###.
        0b00100, // ..#..
        0b00100, // ..#..
        0b00100, // ..#..
        0b00100, // ..#..
        0b00100, // ..#..
        0b00100, // ..#..
    ],
    5,
);
const Z_S: ([u16; 9], i32) = (
    [
        0b01111, // ####.
        0b01000, // ...#.
        0b00100, // ..#..
        0b00100, // ..#..
        0b00010, // .#...
        0b00010, // .#...
        0b00001, // #....
        0b01111, // ####.
        0b10000, // ....#
    ],
    5,
);
const SP_S: ([u16; 9], i32) = ([0; 9], 3);

/// The rows (bit 0 = leftmost column) and column width of `ch`, or `None` if it has no glyph.
pub fn glyph(ch: char, size: GothicSize) -> Option<(&'static [u16], i32)> {
    let g = match (size, ch) {
        (GothicSize::Large, 'S') => (&S_L.0[..], S_L.1),
        (GothicSize::Large, 'E') => (&E_L.0[..], E_L.1),
        (GothicSize::Large, 'H') => (&H_L.0[..], H_L.1),
        (GothicSize::Large, 'B') => (&B_L.0[..], B_L.1),
        (GothicSize::Large, 'O') => (&O_L.0[..], O_L.1),
        (GothicSize::Large, 'N') => (&N_L.0[..], N_L.1),
        (GothicSize::Large, 'T') => (&T_L.0[..], T_L.1),
        (GothicSize::Large, 'A') => (&A_L.0[..], A_L.1),
        (GothicSize::Large, 'M') => (&M_L.0[..], M_L.1),
        (GothicSize::Large, 'L') => (&L_L.0[..], L_L.1),
        (GothicSize::Large, 'W') => (&W_L.0[..], W_L.1),
        (GothicSize::Large, 'Y') => (&Y_L.0[..], Y_L.1),
        (GothicSize::Large, 'Z') => (&Z_L.0[..], Z_L.1),
        (GothicSize::Large, ' ') => (&SP_L.0[..], SP_L.1),
        (GothicSize::Small, 'S') => (&S_S.0[..], S_S.1),
        (GothicSize::Small, 'E') => (&E_S.0[..], E_S.1),
        (GothicSize::Small, 'H') => (&H_S.0[..], H_S.1),
        (GothicSize::Small, 'B') => (&B_S.0[..], B_S.1),
        (GothicSize::Small, 'O') => (&O_S.0[..], O_S.1),
        (GothicSize::Small, 'N') => (&N_S.0[..], N_S.1),
        (GothicSize::Small, 'T') => (&T_S.0[..], T_S.1),
        (GothicSize::Small, 'A') => (&A_S.0[..], A_S.1),
        (GothicSize::Small, 'M') => (&M_S.0[..], M_S.1),
        (GothicSize::Small, 'L') => (&L_S.0[..], L_S.1),
        (GothicSize::Small, 'W') => (&W_S.0[..], W_S.1),
        (GothicSize::Small, 'Y') => (&Y_S.0[..], Y_S.1),
        (GothicSize::Small, 'Z') => (&Z_S.0[..], Z_S.1),
        (GothicSize::Small, ' ') => (&SP_S.0[..], SP_S.1),
        _ => return None,
    };
    Some(g)
}

/// Width `text` would occupy: the sum of glyph widths plus 1 px between adjacent glyphs (no leading
/// or trailing gap). The space glyph counts as an ordinary glyph at its own width.
pub fn text_width(text: &str, size: GothicSize) -> i32 {
    let mut total = 0;
    let mut first = true;
    for ch in text.chars() {
        if let Some((_, w)) = glyph(ch, size) {
            if !first {
                total += 1;
            }
            total += w;
            first = false;
        }
    }
    total
}

/// The longest prefix of `text`, cut on glyph boundaries, whose [`text_width`] is `<= max_w`.
pub fn truncate_to_width(text: &str, size: GothicSize, max_w: i32) -> &str {
    let mut end = 0;
    let mut acc = 0;
    let mut first = true;
    for (bi, ch) in text.char_indices() {
        match glyph(ch, size) {
            Some((_, w)) => {
                let add = if first { w } else { w + 1 };
                if acc + add > max_w {
                    break;
                }
                acc += add;
                first = false;
                end = bi + ch.len_utf8();
            }
            // An unsupported char costs nothing in `text_width`, so keep the prefix consistent by
            // carrying it for free rather than stopping short.
            None => end = bi + ch.len_utf8(),
        }
    }
    &text[..end]
}

/// Paints `text` in one colour with its top-left at (x, y); the caller-facing [`draw`] layers this.
fn paint(c: &mut Canvas, x: i32, y: i32, text: &str, size: GothicSize, col: Rgba) {
    let mut cx = x;
    let mut first = true;
    for ch in text.chars() {
        if let Some((rows, w)) = glyph(ch, size) {
            if !first {
                cx += 1;
            }
            for (dy, row) in rows.iter().enumerate() {
                for c_i in 0..w {
                    if row & (1 << c_i) != 0 {
                        c.fill_rect(cx + c_i, y + dy as i32, 1, 1, col);
                    }
                }
            }
            cx += w;
            first = false;
        }
    }
}

/// Draws `text` with its top-left at (x, y), returning the width drawn (== [`text_width`]).
///
/// When `outline` is `Some`, the text is first stamped in the outline colour at the eight 1 px
/// neighbour offsets and then the fill is laid on top - so every fill pixel is wrapped in outline
/// and the word stays legible over a busy VHS background. Allocates nothing: it stamps glyph pixels
/// straight onto the canvas with `fill_rect`.
pub fn draw(
    c: &mut Canvas,
    x: i32,
    y: i32,
    text: &str,
    size: GothicSize,
    fill: Rgba,
    outline: Option<Rgba>,
) -> i32 {
    if let Some(oc) = outline {
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                paint(c, x + dx, y + dy, text, size, oc);
            }
        }
    }
    paint(c, x, y, text, size, fill);
    text_width(text, size)
}

#[cfg(test)]
// The `x <= 4 + w` / `y <= 3 + 9` bounds below are the brief's verbatim acceptance tests; clippy
// would rather see `(2..=3 + 9).contains(&y)` but the range form is clearer as a box check here.
#[allow(clippy::manual_range_contains)]
mod tests {
    use super::*;
    use crate::render::canvas::{Canvas, Rgba};

    #[test]
    fn every_supported_char_has_a_glyph_at_both_sizes_and_rows_fit_the_width() {
        for ch in GOTHIC_CHARS.chars() {
            for size in [GothicSize::Small, GothicSize::Large] {
                let (rows, w) = glyph(ch, size).unwrap_or_else(|| panic!("{ch:?} {size:?}"));
                let want_rows = match size { GothicSize::Small => 9, GothicSize::Large => 13 };
                assert_eq!(rows.len(), want_rows, "{ch:?} {size:?} row count");
                assert!(w >= 3 && w <= 8, "{ch:?} {size:?} width {w}");
                for (i, r) in rows.iter().enumerate() {
                    assert_eq!(r >> w, 0, "{ch:?} {size:?} row {i} has bits beyond width {w}");
                }
                if ch != ' ' {
                    assert!(rows.iter().any(|r| *r != 0), "{ch:?} {size:?} is blank");
                }
            }
        }
        assert!(glyph('Q', GothicSize::Small).is_none());
    }

    #[test]
    fn blackletter_strokes_are_two_pixels_wide_somewhere_in_every_letter() {
        // The tell of the style: at least one row of every letter has two adjacent set bits.
        for ch in GOTHIC_CHARS.chars().filter(|c| *c != ' ') {
            let (rows, _) = glyph(ch, GothicSize::Large).unwrap();
            assert!(rows.iter().any(|r| (r & (r >> 1)) != 0), "{ch:?} has no heavy stroke");
        }
    }

    #[test]
    fn width_is_glyphs_plus_gaps() {
        let s = text_width("SESH", GothicSize::Small);
        let sum: i32 = "SESH".chars().map(|c| glyph(c, GothicSize::Small).unwrap().1).sum();
        assert_eq!(s, sum + 3);
        assert_eq!(text_width("", GothicSize::Small), 0);
    }

    #[test]
    fn truncates_to_whole_glyphs() {
        let full = text_width("SESHOLLOWATERBOYZ", GothicSize::Small);
        let cut = truncate_to_width("SESHOLLOWATERBOYZ", GothicSize::Small, full / 2);
        assert!(!cut.is_empty() && cut.len() < 17);
        assert!(text_width(cut, GothicSize::Small) <= full / 2);
        let one_more = &"SESHOLLOWATERBOYZ"[..cut.len() + 1];
        assert!(text_width(one_more, GothicSize::Small) > full / 2);
        assert_eq!(truncate_to_width("SESH", GothicSize::Small, 1), "");
    }

    /// Dumps every letter at both sizes, white on `#202020`, as raw RGBA for the eye test.
    ///
    /// Upscaled 8x (nearest-neighbour) because a 9-13px-tall glyph is unreadable at 1:1 - the point
    /// of the dump is to JUDGE the blackletter shapes, not to measure them. A PowerShell step turns
    /// the `.rgba` into a PNG (see the task report). Fill drawn with a 1px outline in a dim grey so
    /// the diamond terminals read against the panel the way they will on the tape family.
    ///
    /// Run: cargo test --release dump_gothic -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_gothic() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let bg = Rgba::from_hex("#202020", 1.0);
        let white = Rgba::from_hex("#ffffff", 1.0);
        let scale: i32 = 8;
        for (tag, size, rows) in [("small", GothicSize::Small, 9), ("large", GothicSize::Large, 13)] {
            // Two words on two lines: the alphabet, and a real phrase, so joins read in context.
            let lines = ["SEHBONTAMLWYZ", "TEAM SESH BONES"];
            let margin = 4;
            let line_gap = 4;
            let w = margin * 2
                + lines.iter().map(|s| text_width(s, size)).max().unwrap();
            let h = margin * 2 + (rows + line_gap) * lines.len() as i32 - line_gap;
            let mut c = Canvas::new(w, h);
            c.fill_rect(0, 0, w, h, bg);
            let mut y = margin;
            for line in lines {
                draw(&mut c, margin, y, line, size, white, None);
                y += rows + line_gap;
            }
            // Upscale nearest-neighbour into the RGBA buffer.
            let (sw, sh) = (w * scale, h * scale);
            let mut out = Vec::with_capacity((sw * sh * 4) as usize);
            for yy in 0..sh {
                for xx in 0..sw {
                    let px = c.get(xx / scale, yy / scale);
                    out.push(px.r);
                    out.push(px.g);
                    out.push(px.b);
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("gothic-{tag}.rgba")), &out).unwrap();
            println!("wrote gothic-{tag}.rgba {sw}x{sh}");
        }
    }

    #[test]
    fn draw_paints_inside_its_box_and_outline_surrounds_fill() {
        let mut c = Canvas::new(80, 20);
        let fill = Rgba::from_hex("#ffffff", 1.0);
        let out = Rgba::from_hex("#ff0000", 1.0);
        let w = draw(&mut c, 4, 3, "SESH", GothicSize::Small, fill, Some(out));
        assert_eq!(w, text_width("SESH", GothicSize::Small));
        let mut white = 0;
        let mut red = 0;
        for y in 0..20 {
            for x in 0..80 {
                let p = c.get(x, y);
                if p.a > 8 {
                    assert!(x >= 3 && x <= 4 + w && y >= 2 && y <= 3 + 9, "paint at {x},{y} outside the box");
                    if p.r > 200 && p.g > 200 { white += 1 } else if p.r > 200 { red += 1 }
                }
            }
        }
        assert!(white > 40, "fill {white}");
        assert!(red > white / 2, "outline {red} vs fill {white}");
        // Any fill pixel's 4-neighbourhood is fill or outline, never empty.
        for y in 1..19 {
            for x in 1..79 {
                let p = c.get(x, y);
                if p.a > 8 && p.g > 200 {
                    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        assert!(c.get(x + dx, y + dy).a > 8, "fill at {x},{y} has a bare side");
                    }
                }
            }
        }
    }
}
