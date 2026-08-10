use anyhow::{anyhow, Result};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, LoadIconW, PeekMessageW, RegisterClassW, SetForegroundWindow, TrackPopupMenu,
    TranslateMessage, HMENU, IDI_APPLICATION, MF_CHECKED, MF_POPUP, MF_SEPARATOR,
    MF_STRING, MSG,
    PM_REMOVE,
    PostMessageW, SetTimer, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTALIGN, WM_APP, WM_CONTEXTMENU, WM_HOTKEY,
    WM_LBUTTONUP, WM_RBUTTONUP, WM_TIMER,
    WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
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
/// A request to render one frame, POSTED rather than timed.
///
/// This is the fix for the meter stalling while a menu or the capture window is open, and the reason
/// it works is a priority difference. `WM_TIMER` is a LOW-PRIORITY message: the system only
/// synthesises one when the thread's queue is otherwise empty, so inside a modal loop that is busy it
/// simply does not arrive. Measured over a 4s open menu, the timer alone left gaps of ~550ms, and the
/// phase breakdown showed the tick itself taking 0ms - it was not slow, it was not being CALLED.
///
/// A posted message is ordinary priority and a modal loop's `GetMessage` returns it like any other, so
/// it is delivered promptly whatever the loop is doing. The tick still runs on the main thread, inside
/// the modal loop's own dispatch, which is why this needs no cross-thread window access at all - the
/// alternative was moving the renderer to its own thread, a far larger change to a working app for the
/// same result.
const WM_TICK: u32 = WM_APP + 2;
const ID_QUIT: usize = 1000;
const ID_AUTOSTART: usize = 1001;
const ID_THEME_BASE: usize = 2000;
// Spotify controls. Kept below ID_THEME_BASE so the theme dispatch, which is an index offset from
// that base, cannot collide with them however many colourways are added.
const ID_KEYS_SUGGEST: usize = 1100;
const ID_KEYS_CLEAR: usize = 1101;
const ID_BACKEND_SESSION: usize = 1102;
const ID_BACKEND_MEDIAKEYS: usize = 1103;
const ID_KEYS_EDIT: usize = 1104;
/// One id per action, so clicking a binding starts capturing for THAT action. As wide as
/// `hotkeys::SLOTS`.
const ID_BIND_BASE: usize = 1110;
const ID_RANDOM_THEME_NOW: usize = 1120;
const ID_RANDOM_COLOURWAY_NOW: usize = 1121;

#[derive(Debug, Clone, PartialEq)]
pub enum TrayEvent {
    Quit,
    SelectTheme(String),
    ToggleAutostart,
    /// Capture a new key for one action, by index into `hotkeys::Slot::ALL`.
    BindKey(usize),
    /// Shuffle now, from the menu, without needing a key bound.
    RandomNow(crate::themes::pick::RandomKind),
    /// Bind the suggested transport keys and register them immediately.
    SuggestKeys,
    /// Release every transport key.
    ClearKeys,
    /// Switch which mechanism sends transport commands.
    SetBackend(crate::win::media::Backend),
    /// Open config.toml in whatever the user edits text with.
    EditConfig,
}

/// What the menu needs to know about the transport state in order to draw itself.
///
/// Passed in rather than read from a global, because the menu must report REALITY - what is actually
/// registered right now - and not the intent recorded in the config file. A chord another program
/// grabbed first is configured and not working, and a menu that reads the config cannot tell.
#[derive(Debug, Clone, Default)]
pub struct TransportState {
    /// One label per action, already resolved to something a person can read. Indexed the same way
    /// as `hotkeys::Slot::ALL`, so the menu and the registry cannot disagree about which is which.
    pub keys: [String; crate::win::hotkeys::SLOTS],
    /// True when at least one configured key failed to register.
    pub broken: bool,
    pub media_keys_backend: bool,
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
                // Our own glyph, drawn at the system's small-icon size and following the
                // light/dark setting. Falls back to the stock application icon rather than
                // refusing to start - a generic icon is a cosmetic flaw, no tray icon at all
                // means the user cannot quit the app.
                hIcon: match crate::win::icon::tray() {
                    Some(h) => h,
                    None => {
                        crate::log::write("tray icon could not be drawn; using the stock icon");
                        LoadIconW(None, IDI_APPLICATION)?
                    }
                },
                szTip: tip,
                ..Default::default()
            };
            if !Shell_NotifyIconW(NIM_ADD, &mut nid).as_bool() {
                return Err(anyhow!("Shell_NotifyIconW(NIM_ADD) failed"));
            }

            Ok(Tray {
                hwnd,
                themes: themes.to_vec(),
            })
        }
    }

    /// Shows the context menu and returns the chosen event, if any. Builds the
    /// theme list from this `Tray`'s own stored snapshot - see `set_themes` for
    /// how that snapshot is kept live across a hot reload.
    pub fn show_menu(
        &self,
        autostart: bool,
        current_theme: &str,
        transport: &TransportState,
    ) -> Option<TrayEvent> {
        self.show_menu_for(&self.themes, autostart, current_theme, transport)
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
        transport: &TransportState,
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

            // ---- Spotify controls -------------------------------------------------------------
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            if let Ok(sub) = CreatePopupMenu() {
                // The three bindings, shown as disabled labels. Informational: this is where the
                // user looks to find out what is bound, and whether it is working.
                // CLICKABLE, not informational. Selecting one opens the capture window for that
                // action - which is the whole point, and was the gap: showing the bindings without
                // any way to set them just tells the user what they cannot change.
                for (i, label) in transport.keys.iter().take(3).enumerate() {
                    let text = format!(
                        "{}:  {}",
                        ["Play / pause", "Next track", "Previous track"][i],
                        label
                    );
                    let mut wide: Vec<u16> =
                        text.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = AppendMenuW(
                        sub,
                        MF_STRING,
                        ID_BIND_BASE + i,
                        windows::core::PCWSTR(wide.as_mut_ptr()),
                    );
                }
                let _ = AppendMenuW(sub, MF_SEPARATOR, 0, None);
                let _ = AppendMenuW(sub, MF_STRING, ID_KEYS_SUGGEST, w!("Use suggested keys"));
                let _ = AppendMenuW(sub, MF_STRING, ID_KEYS_CLEAR, w!("Clear all keys"));
                let _ = AppendMenuW(sub, MF_SEPARATOR, 0, None);
                // The backend toggle the user asked for, as a checked pair so the current one is
                // obvious without opening anything else.
                let (a, b) = if transport.media_keys_backend {
                    (MF_STRING, MF_STRING | MF_CHECKED)
                } else {
                    (MF_STRING | MF_CHECKED, MF_STRING)
                };
                let _ = AppendMenuW(sub, a, ID_BACKEND_SESSION, w!("Send via Spotify session"));
                let _ = AppendMenuW(sub, b, ID_BACKEND_MEDIAKEYS, w!("Send via media keys"));
                // The parent label reports REALITY, not intent: a key that was configured but lost
                // the race for the combination says so here rather than looking configured and
                // silently doing nothing.
                let head = if transport.broken {
                    "Spotify controls: not working"
                } else if transport.keys.iter().all(|k| k == "not set") {
                    "Spotify controls: not set up"
                } else {
                    "Spotify controls"
                };
                let mut wide: Vec<u16> = head.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = AppendMenuW(
                    menu,
                    MF_POPUP,
                    sub.0 as usize,
                    windows::core::PCWSTR(wide.as_mut_ptr()),
                );
            }

            // ---- Random ----------------------------------------------------------------------
            // Both shuffles are offered as ACTIONS as well as bindings, so they work before any key
            // is set and remain usable if a key turns out to be taken by another program.
            if let Ok(sub) = CreatePopupMenu() {
                let _ = AppendMenuW(sub, MF_STRING, ID_RANDOM_THEME_NOW, w!("Any theme now"));
                let _ = AppendMenuW(
                    sub,
                    MF_STRING,
                    ID_RANDOM_COLOURWAY_NOW,
                    w!("Another colourway of this theme now"),
                );
                let _ = AppendMenuW(sub, MF_SEPARATOR, 0, None);
                for (i, label) in transport.keys.iter().enumerate().skip(3) {
                    let text =
                        format!("{}:  {}", ["Any theme key", "Colourway key"][i - 3], label);
                    let mut wide: Vec<u16> =
                        text.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = AppendMenuW(
                        sub,
                        MF_STRING,
                        ID_BIND_BASE + i,
                        windows::core::PCWSTR(wide.as_mut_ptr()),
                    );
                }
                let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, w!("Random"));
            }

            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            // Top level rather than inside the Spotify submenu: config.toml carries the theme, the
            // width and every timing as well as the key bindings, so it is not a transport setting.
            let _ = AppendMenuW(menu, MF_STRING, ID_KEYS_EDIT, w!("Open config file..."));
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
            // "Exit" rather than "Quit": it is the Windows convention for a tray application, and it
            // is what it was asked for by name.
            let _ = AppendMenuW(menu, MF_STRING, ID_QUIT, w!("Exit"));

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            // A tray menu needs its owner to be foreground or it dismisses itself on the first
            // mouse move, and it needs the WM_NULL nudge afterwards or the first selection after it
            // closes is swallowed. Both are long-standing shell requirements for this pattern.
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
            let _ = PostMessageW(Some(self.hwnd), 0x0000, WPARAM(0), LPARAM(0));

            let id = cmd.0 as usize;
            if id == ID_RANDOM_THEME_NOW {
                return Some(TrayEvent::RandomNow(crate::themes::pick::RandomKind::AnyTheme));
            }
            if id == ID_RANDOM_COLOURWAY_NOW {
                return Some(TrayEvent::RandomNow(crate::themes::pick::RandomKind::SameFamily));
            }
            if (ID_BIND_BASE..ID_BIND_BASE + crate::win::hotkeys::SLOTS).contains(&id) {
                return Some(TrayEvent::BindKey(id - ID_BIND_BASE));
            }
            if id == ID_KEYS_SUGGEST {
                return Some(TrayEvent::SuggestKeys);
            }
            if id == ID_KEYS_CLEAR {
                return Some(TrayEvent::ClearKeys);
            }
            if id == ID_BACKEND_SESSION {
                return Some(TrayEvent::SetBackend(crate::win::media::Backend::Session));
            }
            if id == ID_BACKEND_MEDIAKEYS {
                return Some(TrayEvent::SetBackend(crate::win::media::Backend::MediaKeys));
            }
            if id == ID_KEYS_EDIT {
                return Some(TrayEvent::EditConfig);
            }
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
    /// The tray window's handle.
    ///
    /// Exposed for the render-tick timer and (later) for hotkey registration. This window is the
    /// right host for both because it is created once and never destroyed for the life of the
    /// process, unlike the overlay which is shown and hidden every frame.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn poll(&mut self) -> Option<TrayEvent> {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
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
        TRAY_CLICKED.swap(false, std::sync::atomic::Ordering::Relaxed)
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

/// Timer id for the render-tick backstop. Scoped to the tray window, so it cannot collide with
/// anything outside this module.
const ID_TICK_TIMER: usize = 1;

/// Set when the shell tells us the icon was clicked. Read and cleared by `take_right_click`.
///
/// A static because it is written from a bare `extern "system"` wndproc, and IN THE WNDPROC because
/// that is the only place it can be seen reliably. It used to be detected by inspecting messages
/// inside `poll`'s own `PeekMessageW` loop, and that broke the moment anything else on the thread
/// pumped: `overlay::pump_messages` peeks with a null hwnd, so it drains EVERY message including
/// WM_TRAY and hands it to `DispatchMessageW`, which routed it to a wndproc that ignored it. The
/// click was consumed and the flag never set, so clicking the tray icon did nothing at all.
///
/// That became reachable when the render tick moved behind a WM_TIMER: the timer handler calls the
/// tick, the tick pumps the overlay, and the timer is dispatched from inside `poll`'s own loop - so
/// the overlay's pump now runs in the middle of the very loop that was looking for the click.
/// Handling the message here instead makes it independent of who pumps.
static TRAY_CLICKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The render tick, installed by `main`.
///
/// A `fn()` rather than a closure because it has to be reachable from a `wndproc`, which is a bare
/// `extern "system"` function with no place to carry state. The tick's own state lives in a
/// thread-local in `main`; this is only the doorbell.
static TICK_HOOK: std::sync::OnceLock<fn()> = std::sync::OnceLock::new();

/// True while a `WM_TICK` is already in the queue.
///
/// Without this the poster thread would push messages faster than a busy main thread could drain
/// them, and the queue would grow without bound - a frame's worth of work per message, arriving
/// whether or not the previous one has been done. One in flight at a time means the poster naturally
/// runs at whatever rate the main thread can actually service.
static TICK_PENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Registers the function `WM_TIMER` should call, and starts the timer.
///
/// WHY THIS EXISTS: `TrackPopupMenu` runs its own modal message loop and does not return until the
/// menu is dismissed, so while the theme menu is open the main loop cannot reach its render call -
/// the reported "right-clicking freezes the visualiser". A modal loop still PUMPS messages, so a
/// timer on this window keeps the tick running through it. Measured in
/// `tests::wm_timer_is_delivered_during_a_popup_menu_modal_loop`: 42 ticks across 1194ms of open
/// menu. The same mechanism covers a settings dialog's title-bar drag, which is also a nested loop.
///
/// Safe to call more than once; only the first hook is kept.
pub fn install_tick(hwnd: HWND, interval_ms: u32, tick: fn()) {
    let _ = TICK_HOOK.set(tick);

    // The poster. Its only job is to put a WM_TICK in the queue when there is not one already; the
    // work happens on the main thread when it dispatches. HWND is not Send, hence the isize.
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        let hwnd = HWND(raw as *mut std::ffi::c_void);
        loop {
            if !TICK_PENDING.swap(true, std::sync::atomic::Ordering::AcqRel) {
                let posted =
                    unsafe { PostMessageW(Some(hwnd), WM_TICK, WPARAM(0), LPARAM(0)) }.is_ok();
                if !posted {
                    // The window has gone, i.e. the app is closing.
                    TICK_PENDING.store(false, std::sync::atomic::Ordering::Release);
                    return;
                }
            }
            // The suspended interval, not the installed one, while a fullscreen app is on top:
            // otherwise this thread keeps waking the main thread sixty times a second to do nothing,
            // which is exactly the interference the suspend exists to remove.
            let wait = crate::win::shell_state::tick_interval_ms().max(interval_ms.min(1)).max(1);
            std::thread::sleep(std::time::Duration::from_millis(wait as u64));
        }
    });

    unsafe {
        // The timer stays as a BACKSTOP. It costs nothing, and it means a poster thread that somehow
        // dies leaves the app exactly as it behaved before this change - correct, but stalling while a
        // menu is open - rather than with a frozen meter.
        if SetTimer(Some(hwnd), ID_TICK_TIMER, interval_ms, None) == 0 {
            // Not fatal, and deliberately so: without the timer the app behaves exactly as it did
            // before this backstop existed - correct, but frozen while a menu is open. That is a
            // far better outcome than refusing to start, and it is the same policy as the overlay
            // draw failure that caused the Windows 10 report.
            crate::log::write(
                "SetTimer for the render tick failed; the meter will freeze while a menu is open",
            );
        }
    }
}

unsafe extern "system" fn tray_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_TICK {
        TICK_PENDING.store(false, std::sync::atomic::Ordering::Release);
        if let Some(tick) = TICK_HOOK.get() {
            tick();
        }
        return LRESULT(0);
    }
    if msg == WM_TRAY {
        // Every way the shell can ask for this icon's menu. WM_CONTEXTMENU is what a keyboard
        // context-menu press sends and what some Windows 11 shell builds send for a right-click;
        // WM_LBUTTONUP is accepted because this app has no window to show on a left click, and an
        // icon that does nothing when clicked reads as broken.
        let which = lparam.0 as u32;
        let wanted = which == WM_RBUTTONUP || which == WM_CONTEXTMENU || which == WM_LBUTTONUP;
        // Logged unconditionally, because "clicking the icon does nothing" has two completely
        // different causes and they are indistinguishable without this: either the shell never sent
        // us anything, or it sent a message this list does not accept. `log::write` collapses
        // repeats, so a stream of mouse-move callbacks cannot flood the file.
        crate::log::write(&format!(
            "tray callback {which:#06X}{}",
            if wanted { " -> opening the menu" } else { " (ignored)" }
        ));
        if wanted {
            TRAY_CLICKED.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        return LRESULT(0);
    }
    if msg == WM_HOTKEY {
        // Routed here because a null-hwnd registration would post a THREAD message that
        // DispatchMessageW cannot deliver anywhere - see the note in `win::hotkeys`.
        crate::win::hotkeys::on_wm_hotkey(wparam.0);
        return LRESULT(0);
    }
    if msg == WM_TIMER && wparam.0 == ID_TICK_TIMER {
        if let Some(tick) = TICK_HOOK.get() {
            // The tick guards its own re-entrancy (see `main::tick_now`), so a WM_TIMER that
            // arrives while the main loop is already mid-tick is dropped rather than nested.
            tick();
        }
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

    /// Proves the one mechanism a timer-driven render tick depends on: `WM_TIMER` IS delivered
    /// while a popup menu's own modal loop is running.
    ///
    /// This is load-bearing for the reported bug "right-clicking for the menu freezes the
    /// visualiser". `TrackPopupMenu` does not return until the menu is dismissed (measured below),
    /// so while a menu is open the main loop cannot reach its render call at the bottom of the
    /// body, and it is not draining the capture channel either. Driving the tick from a timer
    /// fixes that only if a menu's message pump dispatches `WM_TIMER` to other windows.
    ///
    /// That is widely believed and easy to assume. This repo's repeated lesson is that the
    /// assumption about what actually reaches the screen is the thing that turns out to be wrong,
    /// and a 240-line restructure of the main loop was about to be built on top of it, so it is
    /// measured instead. Ignored by default because it creates a real window and a real menu.
    ///
    /// Run: cargo test --release wm_timer_is_delivered -- --ignored --nocapture
    #[test]
    #[ignore]
    fn wm_timer_is_delivered_during_a_popup_menu_modal_loop() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use windows::Win32::UI::WindowsAndMessaging::{KillTimer, WM_CANCELMODE};

        static TICKS: AtomicU32 = AtomicU32::new(0);

        unsafe extern "system" fn probe_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
            if m == WM_TIMER {
                TICKS.fetch_add(1, Ordering::Relaxed);
            }
            unsafe { DefWindowProcW(h, m, w, l) }
        }

        unsafe {
            let class = WNDCLASSW {
                lpfnWndProc: Some(probe_proc),
                lpszClassName: w!("TaskbarEqTimerProbe"),
                ..Default::default()
            };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("TaskbarEqTimerProbe"),
                w!("probe"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
            .expect("probe window creation should succeed on a real desktop session");

            assert_ne!(SetTimer(Some(hwnd), 1, 16, None), 0, "SetTimer failed");

            // Baseline: pump the ordinary way and confirm the timer fires at all, so a zero
            // during the menu cannot be blamed on a dead timer.
            let mut msg = MSG::default();
            let t0 = std::time::Instant::now();
            while t0.elapsed().as_millis() < 300 {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    DispatchMessageW(&msg);
                }
            }
            let before = TICKS.load(Ordering::Relaxed);
            assert!(before > 5, "the timer is not firing even outside a menu ({before} ticks)");

            // The menu blocks its caller, so it has to be dismissed from elsewhere. HWND is not
            // Send, hence the isize round trip.
            let raw = hwnd.0 as isize;
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(1200));
                let _ = PostMessageW(
                    Some(HWND(raw as *mut std::ffi::c_void)),
                    WM_CANCELMODE,
                    WPARAM(0),
                    LPARAM(0),
                );
            });

            let menu = CreatePopupMenu().expect("popup menu creation should succeed");
            let _ = AppendMenuW(menu, MF_STRING, 1, w!("probe"));
            let _ = SetForegroundWindow(hwnd);
            let t1 = std::time::Instant::now();
            // BLOCKS until dismissed. That blocking is exactly the reported bug.
            let _ = TrackPopupMenu(menu, TPM_RETURNCMD, 0, 0, Some(0), hwnd, None);
            let blocked_ms = t1.elapsed().as_millis();
            let during = TICKS.load(Ordering::Relaxed) - before;
            let _ = DestroyMenu(menu);
            let _ = KillTimer(Some(hwnd), 1);

            println!(
                "TrackPopupMenu blocked its caller for {blocked_ms}ms; \
                 WM_TIMER was dispatched {during} times during that window"
            );
            assert!(
                blocked_ms >= 500,
                "the menu did not actually block ({blocked_ms}ms), so this test is not measuring \
                 what it claims - the modal loop never ran"
            );
            assert!(
                during >= 10,
                "WM_TIMER fired only {during} times across {blocked_ms}ms of menu modal loop; a \
                 timer-driven render tick would NOT survive an open menu and the fix must change"
            );
        }
    }

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
