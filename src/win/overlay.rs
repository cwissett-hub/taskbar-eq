use crate::geom::Rect;
use crate::render::canvas::Canvas;
use std::os::windows::ffi::OsStrExt as _;
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
use std::sync::atomic::{AtomicU8, Ordering};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_LWIN,
    VK_W,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, PeekMessageW, RegisterClassW, SetWindowPos,
    ShowWindow, TranslateMessage, UpdateLayeredWindow, HWND_TOPMOST, MSG, PM_REMOVE,
    SWP_NOACTIVATE, SWP_NOSIZE, SW_HIDE, SW_SHOWNA, ULW_ALPHA, WNDCLASSW, WS_EX_LAYERED,
    WM_LBUTTONUP, WM_RBUTTONUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

/// What the user did on the overlay itself. The window keeps WS_EX_NOACTIVATE so a
/// click never steals focus from whatever you are working in, but it deliberately
/// does NOT set WS_EX_TRANSPARENT - that would pass every click through and there
/// would be nothing to handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayEvent {
    LeftClick,
    RightClick,
}

// The window procedure is a plain fn with no access to `self`, so the pending click
// lives in a static. One overlay exists per process, so this is not a shared-state
// problem in practice.
static PENDING: AtomicU8 = AtomicU8::new(0);
const P_LEFT: u8 = 1;
const P_RIGHT: u8 = 2;

pub struct Overlay {
    hwnd: HWND,
}

/// Guarantees `DeleteDC` runs exactly once no matter which path `show()`
/// exits through, including an early `?` return from `CreateDIBSection`
/// that happens before the DIB/UpdateLayeredWindow cleanup block below.
/// Without this, a failing `CreateDIBSection` call leaks one GDI HDC per
/// `show()` call - `show()` runs every display tick, so this compounds
/// toward the ~10,000-handle per-process GDI quota.
struct DcGuard(HDC);

impl Drop for DcGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
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
            // Owns DeleteDC(mem_dc) for the rest of this function. Declared
            // before anything fallible below so its Drop still runs when the
            // CreateDIBSection `?` bails out early.
            let _mem_dc_guard = DcGuard(mem_dc);

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
            // mem_dc itself is deleted by `_mem_dc_guard`'s Drop below, which
            // runs on this return and on the CreateDIBSection early-return
            // above alike.
            r?;
            Ok(())
        }
    }

    /// Consumes the most recent click on the overlay, if any.
    pub fn take_event(&self) -> Option<OverlayEvent> {
        match PENDING.swap(0, Ordering::Relaxed) {
            P_LEFT => Some(OverlayEvent::LeftClick),
            P_RIGHT => Some(OverlayEvent::RightClick),
            _ => None,
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
    /// Drains THIS window's messages only.
    ///
    /// Filtered by hwnd rather than peeking with a null one, which drains every message on the
    /// thread. That is not tidiness: it swallowed the tray icon's WM_TRAY - dispatching it to a
    /// wndproc that ignored it - and so made clicking the tray icon do nothing. Filtering here means
    /// the tray's own messages can only be consumed by the tray's pump.
    pub fn pump_messages(&self) {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, Some(self.hwnd), 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

/// Synthesises Win+W to open the Widgets panel. The overlay covers the Widgets
/// button while audio plays, so without this a left-click would simply do nothing
/// and the weather would be unreachable. Sending the hotkey is far more robust than
/// trying to forward a click to a window we are deliberately covering.
/// Opens `path` in whatever the user edits text with, falling back to Notepad.
///
/// The fallback is not defensive padding - `.toml` COMMONLY HAS NO ASSOCIATION AT ALL. Checked on
/// this machine: `assoc .toml` reports "File association not found", and it only opens because a
/// per-user choice happens to point at an editor. On a machine without that, the shell call fails and
/// a menu item called "Open config file" would do nothing at all, with the reason buried in the log.
/// Notepad is on every Windows install, so the item always does something.
///
/// `ShellExecuteW` reports failure by returning a pseudo-HINSTANCE at or below 32 rather than by
/// setting an HRESULT, so the check is a magnitude comparison and not the usual `.ok()`.
pub fn open_path(path: &std::path::Path) -> Result<()> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let r = unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            windows::core::w!("open"),
            windows::core::PCWSTR(wide.as_ptr()),
            None,
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    if (r.0 as usize) > 32 {
        return Ok(());
    }
    let code = r.0 as usize;
    match std::process::Command::new("notepad.exe").arg(path).spawn() {
        Ok(_) => {
            crate::log::write(&format!(
                "no editor is associated with {} (ShellExecuteW returned {code}), so it was opened                  in Notepad",
                path.display()
            ));
            Ok(())
        }
        Err(e) => Err(anyhow!("ShellExecuteW returned {code}, and Notepad failed too: {e}")),
    }
}

pub fn open_widgets_panel() -> Result<()> {
    let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                ..Default::default()
            },
        },
    };
    let seq = [
        key(VK_LWIN, false),
        key(VK_W, false),
        key(VK_W, true),
        key(VK_LWIN, true),
    ];
    let sent = unsafe { SendInput(&seq, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != seq.len() {
        return Err(anyhow!("SendInput sent {sent} of {} events", seq.len()));
    }
    Ok(())
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    match msg {
        WM_LBUTTONUP => {
            PENDING.store(P_LEFT, Ordering::Relaxed);
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_RBUTTONUP => {
            PENDING.store(P_RIGHT, Ordering::Relaxed);
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
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
