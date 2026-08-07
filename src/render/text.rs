//! Real text, rendered once by GDI into a coverage mask the `Canvas` can tint.
//!
//! WHY NOT THE EXISTING FONT. `canvas::text_3x5` has fifteen glyphs and the digits - enough for "L",
//! "R" and a dB figure, which is all it was built for. A track title is arbitrary Unicode, so it needs
//! a real typeface.
//!
//! WHY A MASK RATHER THAN COLOURED PIXELS. The whole point of the banner is that it costs no per-theme
//! tuning: it takes its colour from whichever of the 88 colourways is active. So GDI renders white on
//! black, the luminance of the result IS the coverage, and the caller multiplies that by the theme's
//! own `lit`/`hot`. One render, any palette.
//!
//! Rendered ONCE per banner and kept, not once per frame. A banner is on screen for a couple of
//! seconds at ~60fps, so re-rasterising it every tick would be ~150 pointless GDI round trips.

use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW,
    SelectObject, SetBkMode, SetTextColor, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY,
    DIB_RGB_COLORS, DT_CALCRECT, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, FW_SEMIBOLD, TRANSPARENT,
};

/// An antialiased glyph run: `cov[y * w + x]` is 0..255 coverage.
pub struct TextMask {
    pub w: i32,
    pub h: i32,
    pub cov: Vec<u8>,
}

impl TextMask {
    pub fn coverage_at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return 0;
        }
        self.cov[(y * self.w + x) as usize]
    }
}

/// Rasterises `text` at `px` pixels tall. `None` if GDI refuses or the string is empty.
pub fn render(text: &str, px: i32) -> Option<TextMask> {
    let text = text.trim();
    if text.is_empty() || px < 4 {
        return None;
    }
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return None;
        }
        let font = CreateFontW(
            -px,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            Default::default(),
            Default::default(),
            Default::default(),
            // ClearType, for the same reason the capture window asks for it: default quality does not
            // guarantee it, and at these sizes the difference is the difference between crisp and
            // smeared.
            CLEARTYPE_QUALITY,
            Default::default(),
            windows::core::w!("Segoe UI"),
        );
        let old_font = SelectObject(dc, font.into());

        // Measure first. DT_CALCRECT fills the rect instead of drawing, which is the only reliable way
        // to size the bitmap - guessing from character count is wrong for any proportional face.
        let mut rc = windows::Win32::Foundation::RECT { left: 0, top: 0, right: 0, bottom: 0 };
        DrawTextW(dc, &mut wide, &mut rc, DT_CALCRECT | DT_SINGLELINE | DT_LEFT | DT_NOPREFIX);
        // A pixel of slack each side: ClearType can touch the cell beyond the measured advance.
        let (w, h) = ((rc.right - rc.left + 2).max(1), (rc.bottom - rc.top + 2).max(1));

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down, matching the canvas
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok();
        let Some(dib) = dib else {
            SelectObject(dc, old_font);
            let _ = DeleteObject(font.into());
            let _ = DeleteDC(dc);
            return None;
        };
        let old_bmp = SelectObject(dc, dib.into());

        // The DIB starts zeroed, i.e. black, which is exactly the background this needs: white text on
        // black means the luminance of each pixel IS its coverage.
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, windows::Win32::Foundation::COLORREF(0x00FF_FFFF));
        let mut draw_rc = windows::Win32::Foundation::RECT { left: 1, top: 1, right: w, bottom: h };
        DrawTextW(dc, &mut wide, &mut draw_rc, DT_SINGLELINE | DT_LEFT | DT_NOPREFIX);

        let px_ptr = bits as *const u8;
        let mut cov = vec![0u8; (w * h) as usize];
        for i in 0..(w * h) as isize {
            let b = *px_ptr.offset(i * 4);
            let g = *px_ptr.offset(i * 4 + 1);
            let r = *px_ptr.offset(i * 4 + 2);
            // Max rather than a luminance weighting: ClearType puts DIFFERENT coverage in each channel
            // for subpixel positioning, and weighting them would make the text lighter or heavier
            // depending on which side of a pixel a stem landed. Max is the honest greyscale collapse.
            cov[i as usize] = r.max(g).max(b);
        }

        SelectObject(dc, old_bmp);
        SelectObject(dc, old_font);
        let _ = DeleteObject(dib.into());
        let _ = DeleteObject(font.into());
        let _ = DeleteDC(dc);
        Some(TextMask { w, h, cov })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How wide real titles are at each candidate size, against the room a 190px and a 380px panel
    /// actually have. Run: cargo test --release probe_text_widths -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_text_widths() {
        // pad is 3 each side in the banner, interior is w-2.
        let avail = |w: i32| w - 2 - 6;
        println!("room: 190px panel = {}px, 380px panel = {}px", avail(190), avail(380));
        let titles = [
            "Hot Dog - Limp Bizkit",
            "Encore Une Fois - Original Edit",
            "Free - DJ Hirohito Hitmix",
            "All I Ever Wanted - Basshunter",
        ];
        for px in [13, 15, 17, 19, 21, 24] {
            let frac = px as f32 / 56.0;
            let mut widths = Vec::new();
            for t in titles {
                widths.push(render(t, px).map(|m| m.w).unwrap_or(0));
            }
            let worst = *widths.iter().max().unwrap();
            println!(
                "  {px}px (fraction {frac:.2})  widths {widths:?}  worst {worst}px                   fits190={} fits380={}",
                worst <= avail(190),
                worst <= avail(380)
            );
        }
    }

    #[test]
    fn text_rasterises_with_actual_ink_in_it() {
        let m = render("Hot Dog", 14).expect("GDI should rasterise on a desktop session");
        assert!(m.w > 10 && m.h >= 14, "implausible size {}x{}", m.w, m.h);
        let ink = m.cov.iter().filter(|c| **c > 40).count();
        assert!(ink > 20, "only {ink} inked pixels - the text did not draw");
        // And it is not a solid block, which is what a filled rect or an inverted mask would give.
        assert!(ink * 2 < m.cov.len(), "{ink} of {} pixels inked", m.cov.len());
    }

    #[test]
    fn width_tracks_the_string_so_long_titles_can_be_detected_as_too_wide() {
        // Load-bearing: the banner decides whether to scroll by comparing this width to the panel.
        let short = render("Hi", 14).expect("short");
        let long = render("Encore Une Fois - Original Edit", 14).expect("long");
        assert!(
            long.w > short.w * 3,
            "{}px for 31 characters against {}px for 2 - the measurement is not real",
            long.w,
            short.w
        );
    }

    #[test]
    fn height_scales_with_the_requested_size() {
        let small = render("Ag", 10).expect("small");
        let big = render("Ag", 24).expect("big");
        assert!(big.h > small.h, "24px was not taller than 10px: {} vs {}", big.h, small.h);
        assert!(big.w > small.w, "24px was not wider than 10px: {} vs {}", big.w, small.w);
    }

    #[test]
    fn unicode_and_awkward_input_do_not_panic() {
        // Real track titles contain all of this.
        for s in ["Björk - Jóga", "菅野よう子", "café", "a — b", "100%", "  padded  "] {
            let _ = render(s, 14);
        }
        // And the degenerate cases return None rather than a zero-sized mask.
        assert!(render("", 14).is_none());
        assert!(render("   ", 14).is_none());
        assert!(render("x", 1).is_none());
    }

    #[test]
    fn coverage_reads_are_bounds_checked() {
        let m = render("x", 14).expect("x");
        assert_eq!(m.coverage_at(-1, 0), 0);
        assert_eq!(m.coverage_at(0, -1), 0);
        assert_eq!(m.coverage_at(m.w, 0), 0);
        assert_eq!(m.coverage_at(0, m.h), 0);
    }
}
