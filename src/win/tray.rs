use anyhow::{anyhow, Result};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, LoadIconW, PeekMessageW, RegisterClassW, SetForegroundWindow, TrackPopupMenu,
    TranslateMessage, HMENU, IDI_APPLICATION, MF_CHECKED, MF_SEPARATOR, MF_STRING, MSG, PM_REMOVE,
    TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTALIGN, WM_APP, WM_RBUTTONUP, WNDCLASSW,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

// NOTE ON THE `poll` / Quit DEVIATION FROM THE BRIEF:
//
// The brief's `poll()` pushed `TrayEvent::Quit` into a `pending` vec on
// WM_RBUTTONUP (commented "replaced by caller-driven menu"), while
// `take_right_click()` read that same vec. As written, right-clicking the
// tray icon would itself queue and eventually return `Quit` - i.e.
// right-clicking the icon would quit the app, which is not the intent.
//
// Per the controller ruling: `poll()` must never synthesise `Quit`. It only
// records that a right-click happened. `take_right_click()` consumes that
// flag; the caller then calls `show_menu(..)`, and `Quit` is produced ONLY
// when the menu returns `ID_QUIT`.

const WM_TRAY: u32 = WM_APP + 1;
const ID_QUIT: usize = 1000;
const ID_AUTOSTART: usize = 1001;
const ID_THEME_BASE: usize = 2000;

#[derive(Debug, Clone, PartialEq)]
pub enum TrayEvent {
    Quit,
    SelectTheme(String),
    ToggleAutostart,
}

pub struct Tray {
    hwnd: HWND,
    themes: Vec<(String, String)>, // (id, display name)
    // Set on WM_RBUTTONUP, consumed by `take_right_click`. NOT a TrayEvent
    // queue - see the note on `poll` below for why.
    right_clicked: bool,
}

impl Tray {
    pub fn new(themes: &[(String, String)]) -> Result<Self> {
        unsafe {
            let class = WNDCLASSW {
                lpfnWndProc: Some(tray_wndproc),
                lpszClassName: w!("TaskbarEqTray"),
                ..Default::default()
            };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("TaskbarEqTray"),
                w!("Taskbar EQ"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )?;

            let mut tip = [0u16; 128];
            for (i, ch) in "Taskbar EQ".encode_utf16().enumerate() {
                tip[i] = ch;
            }
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_TRAY,
                hIcon: LoadIconW(None, IDI_APPLICATION)?,
                szTip: tip,
                ..Default::default()
            };
            if !Shell_NotifyIconW(NIM_ADD, &mut nid).as_bool() {
                return Err(anyhow!("Shell_NotifyIconW(NIM_ADD) failed"));
            }

            Ok(Tray {
                hwnd,
                themes: themes.to_vec(),
                right_clicked: false,
            })
        }
    }

    /// Shows the context menu and returns the chosen event, if any.
    pub fn show_menu(&self, autostart: bool, current_theme: &str) -> Option<TrayEvent> {
        unsafe {
            let menu: HMENU = CreatePopupMenu().ok()?;
            for (i, (id, name)) in self.themes.iter().enumerate() {
                let flags = if id == current_theme {
                    MF_STRING | MF_CHECKED
                } else {
                    MF_STRING
                };
                let mut wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = AppendMenuW(
                    menu,
                    flags,
                    ID_THEME_BASE + i,
                    windows::core::PCWSTR(wide.as_mut_ptr()),
                );
            }
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(
                menu,
                if autostart {
                    MF_STRING | MF_CHECKED
                } else {
                    MF_STRING
                },
                ID_AUTOSTART,
                w!("Start with Windows"),
            );
            let _ = AppendMenuW(menu, MF_STRING, ID_QUIT, w!("Quit"));

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = SetForegroundWindow(self.hwnd);
            let cmd = TrackPopupMenu(
                menu,
                TPM_RIGHTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD,
                pt.x,
                pt.y,
                Some(0),
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(menu);

            let id = cmd.0 as usize;
            if id == ID_QUIT {
                Some(TrayEvent::Quit)
            } else if id == ID_AUTOSTART {
                Some(TrayEvent::ToggleAutostart)
            } else if id >= ID_THEME_BASE {
                self.themes
                    .get(id - ID_THEME_BASE)
                    .map(|(tid, _)| TrayEvent::SelectTheme(tid.clone()))
            } else {
                None
            }
        }
    }

    /// Pumps this window's message queue. Never produces `TrayEvent::Quit`
    /// itself - see the deviation note in the module docs above `Tray`.
    pub fn poll(&mut self) -> Option<TrayEvent> {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_TRAY && msg.lParam.0 as u32 == WM_RBUTTONUP {
                    self.right_clicked = true;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        None
    }

    /// True when the user right-clicked the tray icon this tick. The caller
    /// is expected to follow a `true` result with `show_menu(..)`; `Quit` is
    /// produced only by the menu returning `ID_QUIT`, never by `poll` itself.
    pub fn take_right_click(&mut self) -> bool {
        let hit = self.right_clicked;
        self.right_clicked = false;
        hit
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
        }
    }
}

unsafe extern "system" fn tray_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

    #[test]
    fn new_creates_and_drop_removes_the_tray_icon() {
        let tray = Tray::new(&[("vfd-ice".into(), "VFD Ice".into())])
            .expect("tray icon creation should succeed on a real desktop session");
        drop(tray);
        // Shell_NotifyIconW/NIM_DELETE has no queryable API to assert against
        // from within the same process - absence of a ghost icon is verified
        // by hand (Step 7 of the task brief).
    }

    #[test]
    fn right_click_sets_the_flag_but_poll_never_synthesises_quit() {
        let mut tray = Tray::new(&[])
            .expect("tray icon creation should succeed on a real desktop session");

        assert!(
            !tray.take_right_click(),
            "no right-click should be pending on a fresh tray"
        );

        unsafe {
            let _ = PostMessageW(
                Some(tray.hwnd),
                WM_TRAY,
                WPARAM(0),
                LPARAM(WM_RBUTTONUP as isize),
            );
        }

        // THE regression this guards against: the brief's literal poll()
        // pushed TrayEvent::Quit here, which would make right-clicking the
        // tray icon quit the app.
        let event = tray.poll();
        assert_eq!(
            event, None,
            "poll must never synthesise Quit from a right-click"
        );
        assert!(
            tray.take_right_click(),
            "the right-click flag should be set after WM_RBUTTONUP"
        );
        assert!(
            !tray.take_right_click(),
            "the flag must be consumed exactly once"
        );
    }
}
