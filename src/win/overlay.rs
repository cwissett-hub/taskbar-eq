use crate::geom::Rect;
use crate::render::canvas::Canvas;
use anyhow::{anyhow, Result};
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, SIZE, WPARAM};
// NOTE: AC_SRC_ALPHA, AC_SRC_OVER and BLENDFUNCTION live in Graphics::Gdi, NOT
// in UI::WindowsAndMessaging. Verified against the compiler - importing them from
// WindowsAndMessaging fails with E0432.
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, AC_SRC_ALPHA,
    AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, PeekMessageW, RegisterClassW, SetWindowPos,
    ShowWindow, TranslateMessage, UpdateLayeredWindow, HWND_TOPMOST, MSG, PM_REMOVE,
    SWP_NOACTIVATE, SWP_NOSIZE, SW_HIDE, SW_SHOWNA, ULW_ALPHA, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

pub struct Overlay {
    hwnd: HWND,
}

impl Overlay {
    pub fn new() -> Result<Self> {
        unsafe {
            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                lpszClassName: w!("TaskbarEqOverlay"),
                ..Default::default()
            };
            // RegisterClassW returns an ATOM (u16), not a bool - 0 means failure.
            // The brief compared it directly against an integer literal, which
            // does typecheck (ATOM derefs to u16) but is worth calling out since
            // it is easy to instead compare against `false`/a bool and get E0308.
            if RegisterClassW(&class) == 0 {
                return Err(anyhow!("RegisterClassW failed"));
            }
            // WS_EX_TOOLWINDOW keeps it out of Alt-Tab; WS_EX_NOACTIVATE stops it
            // stealing focus from whatever you are actually working in.
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("TaskbarEqOverlay"),
                w!("Taskbar EQ"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )?;
            Ok(Overlay { hwnd })
        }
    }

    pub fn show(&self, rect: Rect, canvas: &Canvas) -> Result<()> {
        unsafe {
            let screen_dc = HDC::default();
            let mem_dc = CreateCompatibleDC(Some(screen_dc));

            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: canvas.width(),
                    // Negative height = top-down rows, matching our buffer order.
                    biHeight: -canvas.height(),
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
                &bi,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            )?;
            let old = SelectObject(mem_dc, dib.into());

            std::ptr::copy_nonoverlapping(
                canvas.bits().as_ptr(),
                bits as *mut u32,
                canvas.bits().len(),
            );

            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                rect.x,
                rect.y,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE,
            );
            let _ = ShowWindow(self.hwnd, SW_SHOWNA);

            let pos = POINT { x: rect.x, y: rect.y };
            let src = POINT { x: 0, y: 0 };
            let size = SIZE { cx: canvas.width(), cy: canvas.height() };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            // UpdateLayeredWindow takes these by pointer but never writes back
            // through them - clippy correctly flags `&mut` here as unneeded.
            let r = UpdateLayeredWindow(
                self.hwnd,
                Some(screen_dc),
                Some(&pos),
                Some(&size),
                Some(mem_dc),
                Some(&src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );

            SelectObject(mem_dc, old);
            let _ = DeleteObject(dib.into());
            let _ = DeleteDC(mem_dc);
            r?;
            Ok(())
        }
    }

    pub fn hide(&self) -> Result<()> {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
        Ok(())
    }

    /// Non-blocking pump. The overlay has no UI of its own yet, but a window
    /// that never pumps messages is considered hung by the shell.
    pub fn pump_messages(&self) {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Rgba;

    #[test]
    fn new_creates_a_window_then_show_and_hide_succeed() {
        let overlay = Overlay::new().expect("window creation should succeed");
        let mut canvas = Canvas::new(10, 10);
        canvas.fill_rect(0, 0, 10, 10, Rgba::new(255, 255, 255, 255));
        overlay
            .show(Rect { x: 0, y: 0, w: 10, h: 10 }, &canvas)
            .expect("show should succeed on a real desktop session");
        overlay.hide().expect("hide should succeed");
    }
}
