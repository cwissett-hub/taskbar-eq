// VERIFIED-WORKING REFERENCE - not compiled as part of this crate.
//
// This is the spike that proved the layered-overlay approach on real pixels
// before Task 5 was implemented. It ran on the target machine on 2026-07-30 and
// self-reported:
//
//     widget: X=1425 Y=1140 W=190 H=60  name="Widgets 19C Mostly cloudy"
//     drew 26 bars, 5px wide, 2px gap
//     BitBlt ok=true  GetDIBits lines=60
//     distinct colours=300  ice-blue pixels=3120
//     VERDICT: OVERLAY IS COMPOSITING - 3120 ice-blue pixels over the widget
//
// 26 bars x 5px wide x 24px tall = 3120 exactly, so every drawn pixel survived
// compositing with no blend loss. That validates, together: per-monitor-v2 DPI
// awareness, UIA widget-rect discovery, premultiplied-BGRA packing, and
// UpdateLayeredWindow over the wallpaper-tinted acrylic taskbar.
//
// Three things this spike established that cost real time to discover:
//
//  1. AC_SRC_ALPHA, AC_SRC_OVER and BLENDFUNCTION are in Graphics::Gdi, NOT in
//     UI::WindowsAndMessaging. The wrong path fails with E0432.
//  2. CoInitializeEx returns HRESULT, not Result. `.ok()` is required before any
//     `if let Err(..)`.
//  3. DPI_AWARENESS_CONTEXT wraps *mut c_void - an opaque pseudo-handle. It
//     cannot be hex-formatted or meaningfully compared, so awareness can only be
//     checked via AreDpiAwarenessContextsEqual (which treats per-monitor v1 and
//     v2 as equal).
//
// Also note the widget rect was observed at X=1385, then 1416, then 1425 across
// one afternoon as the weather text changed. Never cache it.
//
// Self-sampling matters: an earlier attempt sampled from a separate PowerShell
// process and produced 12 byte-identical readings, because Add-Type startup
// outlasted the overlay's hold window. Verify from inside the process that drew.

// Spike 2: prove the layered overlay actually composites over the Widgets
// button, on real pixels, while the session is unlocked. This de-risks Task 5
// of the plan before the implementation lands.
use anyhow::{anyhow, Result};
use windows::core::{w, BSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject,
    GetDC, GetDIBits, ReleaseDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO,
    BITMAPINFOHEADER, BLENDFUNCTION, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, SRCCOPY,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, FindWindowW, PeekMessageW, RegisterClassW,
    SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow, HWND_TOPMOST, MSG, PM_REMOVE,
    SWP_NOACTIVATE, SWP_NOSIZE, SW_SHOWNA, ULW_ALPHA, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

fn find_widget_rect() -> Result<(i32, i32, i32, i32, String)> {
    let tray: HWND = unsafe { FindWindowW(w!("Shell_TrayWnd"), None)? };
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let root = unsafe { automation.ElementFromHandle(tray)? };
    let cond = unsafe { automation.CreateTrueCondition()? };
    let all = unsafe { root.FindAll(TreeScope_Descendants, &cond)? };
    for i in 0..unsafe { all.Length()? } {
        let el = unsafe { all.GetElement(i)? };
        let name: BSTR = unsafe { el.CurrentName().unwrap_or_default() };
        let name = name.to_string();
        if name.starts_with("Widgets") {
            let r = unsafe { el.CurrentBoundingRectangle()? };
            return Ok((r.left, r.top, r.right - r.left, r.bottom - r.top, name));
        }
    }
    Err(anyhow!("Widgets button not found"))
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(h, m, w, l) }
}

/// Premultiplied 0xAARRGGBB (== BGRA in DIB memory order).
fn pack(r: u8, g: u8, b: u8, a: u8) -> u32 {
    let af = a as u32;
    let pm = |v: u8| ((v as u32 * af + 127) / 255) & 0xff;
    (af << 24) | (pm(r) << 16) | (pm(g) << 8) | pm(b)
}

fn main() -> Result<()> {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)? };
    // CoInitializeEx returns HRESULT, not Result. .ok() maps S_OK/S_FALSE to
    // Ok(()) and only genuine failures to Err.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        eprintln!("CoInitializeEx failed: {e}");
    }

    let (x, y, w, h, name) = find_widget_rect()?;
    println!("widget: X={x} Y={y} W={w} H={h}  name={name:?}");

    // Build a recognisable test pattern: near-black panel + bright ice-blue bars
    // at a known pitch, so pixel sampling can confirm BOTH the panel and the bars.
    let mut px = vec![0u32; (w * h) as usize];
    let panel = pack(4, 10, 14, 140);
    for p in px.iter_mut() {
        *p = panel;
    }
    let bar = pack(0x8f, 0xe4, 0xff, 255);
    let (bar_w, gap, pad_x, pad_y) = (5, 2, 5, 6);
    let mut bx = pad_x;
    let mut bars = 0;
    while bx + bar_w <= w - pad_x {
        for yy in (h / 2)..(h - pad_y) {
            for xx in bx..(bx + bar_w) {
                px[(yy * w + xx) as usize] = bar;
            }
        }
        bx += bar_w + gap;
        bars += 1;
    }
    println!("drew {bars} bars, {bar_w}px wide, {gap}px gap");

    unsafe {
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            lpszClassName: w!("SpikeOverlay"),
            ..Default::default()
        };
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            w!("SpikeOverlay"),
            w!("Spike"),
            WS_POPUP,
            0, 0, 1, 1,
            None, None, None, None,
        )?;

        let screen_dc = HDC::default();
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib: HBITMAP = CreateDIBSection(
            Some(mem_dc),
            &bi as *const _ as *const _,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )?;
        let old = SelectObject(mem_dc, dib.into());
        std::ptr::copy_nonoverlapping(px.as_ptr(), bits as *mut u32, px.len());

        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
        let _ = ShowWindow(hwnd, SW_SHOWNA);

        let mut pos = POINT { x, y };
        let mut src = POINT { x: 0, y: 0 };
        let mut size = SIZE { cx: w, cy: h };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        UpdateLayeredWindow(
            hwnd,
            Some(screen_dc),
            Some(&mut pos),
            Some(&mut size),
            Some(mem_dc),
            Some(&mut src),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        )?;

        println!("overlay up - self-sampling (no cross-process timing race)");

        // Pump for a moment so DWM composites the new layered window.
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(1200) {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // Read back what is actually on screen at the widget rect. This process
        // is per-monitor-v2 aware, so these are true physical pixels.
        let screen = GetDC(None);
        let shot_dc = CreateCompatibleDC(Some(screen));
        let shot = CreateCompatibleBitmap(screen, w, h);
        let old_shot = SelectObject(shot_dc, shot.into());
        let ok = BitBlt(shot_dc, 0, 0, w, h, Some(screen), x, y, SRCCOPY).is_ok();

        let mut buf = vec![0u32; (w * h) as usize];
        let mut bi_read = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let lines = GetDIBits(
            shot_dc,
            shot,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut _),
            &mut bi_read,
            DIB_RGB_COLORS,
        );

        SelectObject(shot_dc, old_shot);
        let _ = DeleteObject(shot.into());
        let _ = DeleteDC(shot_dc);
        ReleaseDC(None, screen);

        println!("BitBlt ok={ok}  GetDIBits lines={lines}");

        let mut ice = 0usize;
        let mut distinct = std::collections::HashSet::new();
        for p in &buf {
            let (r, g, b) = ((p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
            distinct.insert(*p & 0x00ffffff);
            if b > 200 && b.saturating_sub(r) > 60 {
                ice += 1;
            }
        }
        println!("distinct colours={}  ice-blue pixels={ice}", distinct.len());
        if distinct.len() <= 2 {
            println!("VERDICT: INCONCLUSIVE - capture is blank, session is probably locked");
        } else if ice > 400 {
            println!("VERDICT: OVERLAY IS COMPOSITING - {ice} ice-blue pixels over the widget");
        } else {
            println!("VERDICT: OVERLAY NOT VISIBLE - only {ice} ice-blue pixels (expected >400)");
        }

        SelectObject(mem_dc, old);
        let _ = DeleteObject(dib.into());
        let _ = DeleteDC(mem_dc);
    }
    println!("done");
    Ok(())
}
