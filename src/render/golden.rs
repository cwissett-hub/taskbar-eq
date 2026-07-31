use super::canvas::Canvas;

// This module has no production consumer - `main`'s render loop draws
// straight into a Canvas and hands it to the overlay; nothing renders an
// ASCII map at runtime. It exists for the test suite's golden-file diffs
// (see tests/golden/vfd-ice.txt) and for regenerating them.
#[allow(dead_code)]
const RAMP: &[u8] = b" .:-=+*#%@";

/// Renders a canvas to an ASCII luminance map so golden diffs are readable in
/// review. Alpha-weighted, so transparent areas read as blank.
#[allow(dead_code)]
/// Compares a rendered canvas against a committed golden, tolerant of line endings.
///
/// Belt and braces alongside `.gitattributes`: `canvas_to_ascii` emits LF, but git
/// can hand back CRLF on Windows depending on `core.autocrlf` and how the file was
/// last written - which genuinely broke every golden test during a branch merge, with
/// byte-identical content. A golden failing over invisible whitespace teaches nobody
/// anything.
#[allow(dead_code)]
pub fn matches_golden(c: &Canvas, golden: &str) -> bool {
    fn lf(s: &str) -> String {
        s.replace("\r\n", "\n")
    }
    lf(&canvas_to_ascii(c)) == lf(golden)
}


pub fn canvas_to_ascii(c: &Canvas) -> String {
    let mut s = String::with_capacity(((c.width() + 1) * c.height()) as usize);
    for y in 0..c.height() {
        for x in 0..c.width() {
            let p = c.get(x, y);
            let lum = (0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32)
                * (p.a as f32 / 255.0);
            let i = ((lum / 255.0) * (RAMP.len() - 1) as f32).round() as usize;
            s.push(RAMP[i.min(RAMP.len() - 1)] as char);
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::canvas::{Canvas, Rgba};
    use super::*;

    #[test]
    fn empty_canvas_is_all_blank() {
        let c = Canvas::new(4, 2);
        assert_eq!(canvas_to_ascii(&c), "    \n    \n");
    }

    #[test]
    fn white_is_the_brightest_ramp_character() {
        let mut c = Canvas::new(2, 1);
        c.fill_rect(0, 0, 2, 1, Rgba::new(255, 255, 255, 255));
        assert_eq!(canvas_to_ascii(&c), "@@\n");
    }

    #[test]
    fn transparent_pixels_read_as_blank_regardless_of_colour() {
        let mut c = Canvas::new(2, 1);
        c.fill_rect(0, 0, 2, 1, Rgba::new(255, 255, 255, 0));
        assert_eq!(canvas_to_ascii(&c), "  \n");
    }
}
