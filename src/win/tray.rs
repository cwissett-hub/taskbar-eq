use anyhow::{anyhow, Result};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, LoadIconW, PeekMessageW, RegisterClassW, SetForegroundWindow, TrackPopupMenu,
    TranslateMessage, HMENU, IDI_APPLICATION, MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG,
    PM_REMOVE,
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

/// One selectable colourway in the theme menu.
///
/// Carries `family` because the menu groups colourways into a submenu per family. It was a
/// bare `(id, name)` tuple while the menu was flat; with three families and nineteen
/// colourways a flat list is unusable, and the tuple had no room for the grouping key.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub id: String,
    pub name: String,
    pub family: String,
}

impl MenuItem {
    pub fn new(id: &str, name: &str, family: &str) -> Self {
        MenuItem { id: id.into(), name: name.into(), family: family.into() }
    }
}

/// The families present in `items`, in first-appearance order.
///
/// First-appearance rather than alphabetical, so the menu follows the registry order the
/// built-ins are declared in instead of reordering itself as families are added. Extracted
/// from `show_menu_for` purely so it is testable: everything else in that function needs a
/// real window and a live HMENU.
fn families_in_order(items: &[MenuItem]) -> Vec<&str> {
    let mut families: Vec<&str> = Vec::new();
    for it in items {
        if !families.contains(&it.family.as_str()) {
            families.push(&it.family);
        }
    }
    families
}

pub struct Tray {
    hwnd: HWND,
    themes: Vec<MenuItem>,
    // Set on WM_RBUTTONUP, consumed by `take_right_click`. NOT a TrayEvent
    // queue - see the note on `poll` below for why.
    right_clicked: bool,
}

impl Tray {
    pub fn new(themes: &[MenuItem]) -> Result<Self> {
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

    /// Shows the context menu and returns the chosen event, if any. Builds the
    /// theme list from this `Tray`'s own stored snapshot - see `set_themes` for
    /// how that snapshot is kept live across a hot reload.
    pub fn show_menu(&self, autostart: bool, current_theme: &str) -> Option<TrayEvent> {
        self.show_menu_for(&self.themes, autostart, current_theme)
    }

    /// Shows the context menu built from an explicit theme list. This is the
    /// one Win32 menu implementation shared by both entry points: the tray
    /// icon's right-click (via `show_menu`, above) and a right-click on the
    /// equaliser overlay itself, which has no `Tray` state of its own to keep
    /// in sync and so passes the current registry straight through.
    pub fn show_menu_for(
        &self,
        items: &[MenuItem],
        autostart: bool,
        current_theme: &str,
    ) -> Option<TrayEvent> {
        unsafe {
            // Before CreatePopupMenu: the theme is resolved when the menu is created, so
            // setting the mode afterwards would only take effect on the NEXT right-click.
            crate::win::darkmode::apply();

            let menu: HMENU = CreatePopupMenu().ok()?;

            // Group into one submenu per family, in first-appearance order so the built-in
            // registry order is preserved rather than alphabetised.
            //
            // Command ids stay indices into the flat `items` slice, NOT per-submenu
            // positions, so the dispatch below is unchanged by the nesting and a submenu
            // that fails to build cannot silently shift another family's ids.
            let families = families_in_order(items);

            // Submenu handles must outlive TrackPopupMenu. Destroying the parent destroys
            // attached submenus, so these are not separately freed - but one that is
            // created and never attached WOULD leak, hence attaching immediately below.
            for family in &families {
                let sub: HMENU = match CreatePopupMenu() {
                    Ok(h) => h,
                    // Skipping one family is better than abandoning the whole menu: the
                    // other families, autostart and Quit all still work.
                    Err(_) => continue,
                };
                let mut family_holds_current = false;
                for (i, it) in items.iter().enumerate() {
                    if it.family != *family {
                        continue;
                    }
                    let selected = it.id == current_theme;
                    family_holds_current |= selected;
                    let flags = if selected { MF_STRING | MF_CHECKED } else { MF_STRING };
                    let mut wide: Vec<u16> =
                        it.name.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = AppendMenuW(
                        sub,
                        flags,
                        ID_THEME_BASE + i,
                        windows::core::PCWSTR(wide.as_mut_ptr()),
                    );
                }
                // Check the family too, so the active one is identifiable without opening
                // every submenu to hunt for the tick.
                let label = crate::themes::family_label(family);
                let mut wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
                let flags = if family_holds_current {
                    MF_POPUP | MF_CHECKED
                } else {
                    MF_POPUP
                };
                let _ = AppendMenuW(
                    menu,
                    flags,
                    sub.0 as usize,
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
                items
                    .get(id - ID_THEME_BASE)
                    .map(|it| TrayEvent::SelectTheme(it.id.clone()))
            } else {
                None
            }
        }
    }

    /// Replaces the stored theme snapshot used by `show_menu`, so a hot reload
    /// (which can add, remove or rename colourways) is reflected the next time
    /// either the tray icon or the overlay is right-clicked, without needing a
    /// restart.
    pub fn set_themes(&mut self, items: &[MenuItem]) {
        self.themes = items.to_vec();
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
        let tray = Tray::new(&[MenuItem::new("vfd-ice", "VFD Ice", "segmented")])
            .expect("tray icon creation should succeed on a real desktop session");
        drop(tray);
        // Shell_NotifyIconW/NIM_DELETE has no queryable API to assert against
        // from within the same process - absence of a ghost icon is verified
        // by hand (Step 7 of the task brief).
    }

    #[test]
    fn families_group_in_first_appearance_order_not_alphabetically() {
        let items = [
            MenuItem::new("vfd-ice", "VFD ice", "segmented"),
            MenuItem::new("p1-green", "P1 green", "scope"),
            MenuItem::new("matrix", "Matrix", "segmented"),
            MenuItem::new("vu-cream", "Warm cream", "vu"),
            MenuItem::new("p7", "P7", "scope"),
        ];
        // Alphabetical would be [scope, segmented, vu] - asserting the interleaved input
        // still yields declaration order is what makes this test worth having.
        assert_eq!(families_in_order(&items), vec!["segmented", "scope", "vu"]);
    }

    #[test]
    fn every_theme_lands_in_exactly_one_family_submenu() {
        // The menu builds command ids as indices into the FLAT slice while iterating
        // per-family. If the grouping ever dropped or duplicated an item, a colourway
        // would be unreachable or two entries would resolve to the same theme.
        let items: Vec<MenuItem> = crate::themes::builtin::all()
            .iter()
            .map(|t| MenuItem::new(&t.id, &t.name, &t.family))
            .collect();
        let mut seen = 0;
        for fam in families_in_order(&items) {
            seen += items.iter().filter(|it| it.family == fam).count();
        }
        assert_eq!(seen, items.len(), "every colourway must appear under exactly one family");
        assert!(items.len() > 15, "sanity: the registry should be non-trivial");
    }

    #[test]
    fn family_labels_are_readable_and_unknown_families_still_get_a_name() {
        assert_eq!(crate::themes::family_label("scope"), "Oscilloscope");
        assert_eq!(crate::themes::family_label("vu"), "VU dials");
        // The important case: a family this table has never heard of must still be
        // presentable, because a TOML file can introduce one and the menu must not drop it.
        assert_eq!(crate::themes::family_label("vaporwave"), "Vaporwave");
        assert_eq!(crate::themes::family_label(""), "Other");
    }

    #[test]
    fn set_themes_replaces_the_stored_list_so_a_hot_reload_is_reflected() {
        let mut tray = Tray::new(&[MenuItem::new("old", "Old", "segmented")])
            .expect("tray icon creation should succeed on a real desktop session");
        assert_eq!(tray.themes, vec![MenuItem::new("old", "Old", "segmented")]);

        tray.set_themes(&[
            MenuItem::new("new", "New", "segmented"),
            MenuItem::new("newer", "Newer", "scope"),
        ]);

        assert_eq!(
            tray.themes,
            vec![
                MenuItem::new("new", "New", "segmented"),
                MenuItem::new("newer", "Newer", "scope"),
            ],
            "set_themes must replace the snapshot show_menu reads from"
        );
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
