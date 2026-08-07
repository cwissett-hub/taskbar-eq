//! The tray icon, drawn at runtime rather than embedded as a `.ico`.
//!
//! WHY NOT A RESOURCE FILE. Embedding an icon the usual way needs a `.rc`, a resource compiler and a
//! `build.rs` - this project has none of those - or a crate like `embed-resource`/`winres`. Neither is
//! in the local cargo registry cache (checked), so adding one would mean a crates.io fetch on a
//! machine whose proxy already blocks `gh`, in order to ship a 16x16 picture. Drawing it costs a few
//! dozen lines, no build step, and keeps the single-portable-exe promise intact.
//!
//! Two things fall out of doing it this way rather than being extra work:
//!
//! - It is DPI-correct by construction. The size comes from `SM_CXSMICON`, so a 125% or 150% desktop
//!   gets a glyph drawn at its real pixel size instead of a 16x16 resource stretched up. That matters
//!   here: the standing rule in this project is that GUI work must be DPI-aware because the dev
//!   machine runs at 125% and stretch-blur is invisible in review.
//! - It can follow the Windows light/dark setting, which a single embedded icon cannot. A tray icon
//!   sits directly on the taskbar, so one fixed colour is legible on one theme and mud on the other.
//!
//! ALPHA IS DELIBERATELY BINARY - every pixel is either fully opaque or fully transparent. A 32bpp
//! `CreateIconIndirect` bitmap and this app's `Canvas` disagree about whether alpha is premultiplied,
//! and at 0 or 255 the two are identical, so the question does not arise. It also keeps the glyph
//! crisp at 16px, where antialiasing turns a 2px bar into a smudge.

use crate::render::canvas::{Canvas, Rgba};
use windows::Win32::Foundation::TRUE;
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};

/// Relative bar heights, left to right. An equaliser reading rather than a flat ramp, because a ramp
/// at this size reads as a signal-strength or wifi glyph.
const BARS: [f32; 5] = [0.45, 0.80, 1.0, 0.55, 0.90];

/// Draws the glyph. Pure and returns a `Canvas`, so the shape is testable without any Win32 call.
pub fn glyph(size: i32, dark_taskbar: bool) -> Canvas {
    let mut c = Canvas::new(size.max(8), size.max(8));
    let s = size.max(8);
    // A margin, then bars with single-pixel gaps. Computed from `s` so the glyph is drawn at the real
    // size rather than scaled from a fixed one.
    let margin = (s / 12).max(1);
    let inner = (s - margin * 2).max(4);
    let n = BARS.len() as i32;
    // Bars are TWO units wide and gaps ONE, rather than both one unit. Reviewed at 16px with equal
    // widths and the bars came out a single pixel each, which reads as faint hatching rather than as
    // an equaliser - and 16px is the most common case, since it is what SM_CXSMICON gives at 100%.
    // Two units on a 14-unit budget (5 bars + 4 gaps) makes them 2px at 16px and 4px at 32px.
    let unit = (inner / (n * 2 + (n - 1))).max(1);
    let bar_w = unit * 2;
    let gap = unit;
    let used = bar_w * n + gap * (n - 1);
    let x0 = margin + (inner - used) / 2;
    let base = s - margin;

    // On a dark taskbar the glyph has to be light, and on a light one dark. This is the reason the
    // icon is drawn instead of embedded: a single fixed colour is legible on one theme and mud on the
    // other, and the tray sits directly on the taskbar with no panel of its own to sit against.
    let col = if dark_taskbar {
        Rgba::from_hex("#7fe8ff", 1.0)
    } else {
        Rgba::from_hex("#0b4a66", 1.0)
    };

    for (i, frac) in BARS.iter().enumerate() {
        let h = ((inner as f32) * frac).round().max(1.0) as i32;
        let x = x0 + i as i32 * (bar_w + gap);
        c.fill_rect(x, base - h, bar_w, h, col);
    }
    c
}

/// The glyph on its own dark rounded badge, for the EXE's file icon.
///
/// `cfg(test)` because it is an ASSET GENERATOR, not runtime code: `dump_icon` writes its output, and
/// `assets/taskbar-eq.ico` is built from that and committed, then embedded by `build.rs`. Nothing in
/// the running app calls it, and pretending otherwise with an `allow(dead_code)` would hide that.
///
/// The tray glyph is transparent and inverts with the taskbar theme, which is right for the tray and
/// wrong for a file icon: Explorer, Alt-Tab and a pinned taskbar button all draw it against
/// backgrounds this app does not choose and cannot query at build time. A self-contained badge reads
/// on any of them.
#[cfg(test)]
pub fn badge(size: i32) -> Canvas {
    let s = size.max(16);
    let mut c = Canvas::new(s, s);
    // Rounded dark plate, then the bars on top in the light colour.
    let r = (s / 6).max(2);
    c.rounded_rect(0, 0, s, s, r, Rgba::from_hex("#12181f", 1.0));
    let inner = glyph(s, true);
    c.draw_over(&inner);
    c
}

/// Builds an `HICON` from the glyph. `None` if any GDI call fails, which the caller treats as
/// "fall back to the stock icon" rather than as a reason not to start.
pub fn create(size: i32, dark_taskbar: bool) -> Option<HICON> {
    let s = size.max(8);
    let c = glyph(s, dark_taskbar);
    unsafe {
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: s,
                // NEGATIVE height means a top-down DIB, which matches the canvas's row order. A
                // positive height here silently renders the glyph upside down.
                biHeight: -s,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(dib.into());
            return None;
        }
        let px = bits as *mut u8;
        for y in 0..s {
            for x in 0..s {
                let p = c.get(x, y);
                let o = ((y * s + x) * 4) as isize;
                // BGRA, which is what a 32bpp BI_RGB DIB wants.
                *px.offset(o) = p.b;
                *px.offset(o + 1) = p.g;
                *px.offset(o + 2) = p.r;
                *px.offset(o + 3) = p.a;
            }
        }
        // A mask is required even for a 32bpp icon, where the alpha channel is what actually gets
        // used. An all-zero monochrome bitmap is the conventional filler.
        let mask = CreateBitmap(s, s, 1, 1, None);
        let info = ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: dib,
        };
        let icon = CreateIconIndirect(&info).ok();
        // CreateIconIndirect COPIES both bitmaps, so these are ours to free - leaking them would leak
        // a DIB per launch, and this is called once but the tray icon is rebuilt when the theme
        // changes.
        let _ = DeleteObject(mask.into());
        let _ = DeleteObject(dib.into());
        icon
    }
}

/// The tray icon at the system's small-icon size, following the current light/dark setting.
pub fn tray() -> Option<HICON> {
    let s = unsafe { GetSystemMetrics(SM_CXSMICON) };
    // GetSystemMetrics returns 0 on failure rather than an error.
    let s = if s >= 8 { s } else { 16 };
    create(s, crate::win::darkmode::windows_prefers_dark())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit_pixels(c: &Canvas, size: i32) -> usize {
        let mut n = 0;
        for y in 0..size {
            for x in 0..size {
                if c.get(x, y).a > 0 {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn the_glyph_is_drawn_at_every_size_the_system_can_ask_for() {
        // SM_CXSMICON is 16 at 100%, 20 at 125%, 24 at 150%, and larger on a 4K desktop. All of them
        // must produce a glyph with actual bars, not an empty square and not a panic.
        for s in [8, 16, 20, 24, 32, 48, 64] {
            let c = glyph(s, true);
            let lit = lit_pixels(&c, s);
            assert!(lit > 0, "size {s} drew nothing");
            // Loose bounds on purpose - the assertion is that it is a GLYPH, i.e. neither blank nor a
            // solid block. Five bars with gaps can only cover part of the square.
            let total = (s * s) as usize;
            assert!(
                lit * 8 > total && lit * 2 < total,
                "size {s} covered {lit} of {total} pixels, which is not a bar glyph"
            );
        }
    }

    #[test]
    fn the_glyph_inverts_with_the_taskbar_theme() {
        // The whole reason this is drawn rather than embedded. If both themes produced the same
        // colour, the icon would be illegible on one of them and there would be no point.
        let (dark, light) = (glyph(16, true), glyph(16, false));
        let lum = |c: &Canvas| -> f32 {
            let mut best = 0.0f32;
            for y in 0..16 {
                for x in 0..16 {
                    let p = c.get(x, y);
                    if p.a > 0 {
                        best = best.max(
                            0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32,
                        );
                    }
                }
            }
            best
        };
        let (ld, ll) = (lum(&dark), lum(&light));
        assert!(
            ld > ll + 80.0,
            "the dark-taskbar glyph ({ld:.0}) must be far lighter than the light-taskbar one ({ll:.0})"
        );
        // And the two must occupy the same pixels - only the colour changes, not the shape.
        assert_eq!(lit_pixels(&dark, 16), lit_pixels(&light, 16));
    }

    #[test]
    fn the_bars_have_different_heights_so_it_reads_as_a_meter() {
        // A flat row of equal bars reads as a barcode or a signal-strength glyph; the point is that
        // it is recognisably an equaliser. Measured as the number of distinct bar top rows.
        let s = 32;
        let c = glyph(s, true);
        let mut tops = Vec::new();
        for x in 0..s {
            let top = (0..s).find(|y| c.get(x, *y).a > 0);
            if let Some(t) = top {
                tops.push(t);
            }
        }
        tops.sort_unstable();
        tops.dedup();
        assert!(tops.len() >= 4, "only {} distinct bar heights: {tops:?}", tops.len());
    }

    #[test]
    fn every_pixel_is_fully_opaque_or_fully_transparent() {
        // Load-bearing: it is what makes the premultiplied-versus-straight alpha question moot when
        // the canvas is copied into a 32bpp DIB, and it keeps a 2px bar crisp at 16px.
        let s = 20;
        let c = glyph(s, false);
        for y in 0..s {
            for x in 0..s {
                let a = c.get(x, y).a;
                assert!(a == 0 || a == 255, "pixel ({x},{y}) has partial alpha {a}");
            }
        }
    }

    /// Dumps the glyph at the sizes Windows asks for, so it can be judged by eye.
    /// Run: cargo test --release dump_icon -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_icon() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        for s in [16, 20, 24, 32, 48, 64, 128, 256] {
            for dark in [true, false] {
                let c = if dark { glyph(s, dark) } else { glyph(s, dark) };
                let mut out = Vec::with_capacity((s * s * 4) as usize);
                for y in 0..s {
                    for x in 0..s {
                        let p = c.get(x, y);
                        out.extend_from_slice(&[p.r, p.g, p.b, p.a]);
                    }
                }
                let tag = if dark { "dark" } else { "light" };
                std::fs::write(dir.join(format!("icon-{s}-{tag}.rgba")), &out).unwrap();
            }
            {
                let c = badge(s);
                let mut out = Vec::with_capacity((s * s * 4) as usize);
                for y in 0..s {
                    for x in 0..s {
                        let p = c.get(x, y);
                        out.extend_from_slice(&[p.r, p.g, p.b, p.a]);
                    }
                }
                std::fs::write(dir.join(format!("badge-{s}.rgba")), &out).unwrap();
            }
        }
        println!("wrote icon dumps to {}", dir.display());
    }

    /// Builds a real icon. Ignored: it needs a desktop session for GDI.
    /// Run: cargo test --release live_icon -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_icon_creation_succeeds() {
        let s = unsafe { GetSystemMetrics(SM_CXSMICON) };
        println!("SM_CXSMICON = {s}");
        for dark in [true, false] {
            let icon = create(if s >= 8 { s } else { 16 }, dark);
            println!("  dark={dark} -> {:?}", icon.map(|h| h.0 as usize));
            assert!(icon.is_some(), "icon creation failed for dark={dark}");
        }
    }
}
