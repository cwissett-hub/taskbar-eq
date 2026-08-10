//! The keystroke capture window: "press the keys you want for this control".
//!
//! One action at a time, opened by clicking that action in the tray menu, rather than a settings
//! dialog with three fields. That is a smaller thing to build and a smaller thing to get wrong, and
//! it matches how it was asked for - press the entry, press the keys.
//!
//! WHY IT RUNS ITS OWN MESSAGE LOOP RATHER THAN `DialogBoxParam`. It needs to pump EVERY message, not
//! just its own, because the render tick is driven by a `WM_TIMER` on the tray window: a loop that
//! only dispatched this window's messages would freeze the visualiser for as long as the capture
//! window was open, which is the exact bug that made right-clicking the theme menu stop the meter.
//! Pumping everything means the meter keeps running while the user thinks about which keys to press.
//!
//! WHY THE APP'S OWN HOTKEYS MUST BE RELEASED FIRST - and the caller does this, see
//! `hotkeys::Registry::release_all`. A registered hotkey CONSUMES the keystroke, so a field cannot
//! capture `Ctrl+Alt+Space` while `Ctrl+Alt+Space` is registered: the press would fire play/pause and
//! never reach this window. Every binding is therefore released for the duration and re-applied
//! afterwards.
//!
//! WHY `WM_APPCOMMAND` IS HANDLED as well as `WM_KEYDOWN`. The dedicated media keys do not
//! necessarily arrive as key presses; Windows also delivers them as `WM_APPCOMMAND` to the focused
//! window. Without that branch, pressing the Play button on a keyboard would capture nothing at all,
//! and a user trying to bind their play key would conclude the feature was broken.

use super::hotkey::{Chord, Mods};
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect,
    InvalidateRect, SelectObject, SetBkMode, SetTextColor, DT_CENTER, DT_SINGLELINE, DT_VCENTER,
    CLEARTYPE_QUALITY, FW_NORMAL, FW_SEMIBOLD, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
    PeekMessageW, RegisterClassW, SetForegroundWindow, ShowWindow,
    TranslateMessage, MSG, PM_REMOVE, SW_SHOW, WM_APPCOMMAND, WM_CLOSE,
    WM_KEYDOWN, WM_KILLFOCUS, WM_PAINT, WM_SYSKEYDOWN, WNDCLASSW, WS_BORDER, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};

/// Point sizes for the three lines, and the window's size in DIPs.
///
/// Raised from 10/15/8 on the report that the text was too small. Point sizes are DPI-independent by
/// definition, so these are the sizes the user actually sees at any scaling - the bug was never the
/// point size but the window being built in raw pixels around it.
const TITLE_PT: i32 = 11;
const CHORD_PT: i32 = 22;
const HINT_PT: i32 = 10;
/// Window size in device-independent pixels, scaled by the real DPI at creation.
const DIP_W: i32 = 440;
const DIP_H: i32 = 170;

/// What the user decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Captured {
    /// Bind this combination.
    Chord(Chord),
    /// Unbind this action.
    Clear,
}

/// Keys that are only ever modifiers - held, not committed.
fn is_modifier(vk: u16) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

/// `APPCOMMAND_*` values, for the media keys that arrive this way instead of as key presses.
fn vk_for_appcommand(cmd: i32) -> Option<u16> {
    match cmd {
        // APPCOMMAND_MEDIA_NEXTTRACK / PREVIOUSTRACK / STOP / PLAY_PAUSE
        11 => Some(0xB0),
        12 => Some(0xB1),
        13 => Some(0xB2),
        14 => Some(0xB3),
        _ => None,
    }
}

/// The live state of one capture, reachable from the wndproc.
struct State {
    /// What is being bound, for the prompt.
    label: String,
    /// Set once the user commits or cancels; ends the loop.
    done: bool,
    result: Option<Captured>,
    /// Modifiers currently held, so the prompt can show "Ctrl + Alt + ..." before a key lands.
    held: Mods,
    dark: bool,
    /// The chords bound to the OTHER actions, so a duplicate is caught here.
    others: Vec<Chord>,
    /// The DPI the window was SIZED for, so painting scales by the same number the geometry used.
    ///
    /// Resolved before the window exists and then remembered, rather than each paint calling
    /// `GetDpiForWindow`. The two must not be allowed to disagree: the first version hardcoded the
    /// size at 380x130 with a `dpi_guess` of 96 that did nothing, while the font DID scale with the
    /// real DPI - so on this 125% display the text was correctly sized inside a window 20% too small
    /// for it, which is what "doesn't scale right" looked like.
    dpi: i32,
    /// Why the last attempt was refused, shown in the window.
    ///
    /// Refusing HERE rather than after the window closes is the point. It used to accept anything,
    /// store it, and let the startup path reject it - so the menu ended up showing "F9 (not allowed)"
    /// with no reason anywhere the user could see, and the only way to find out why was to ask. Now
    /// the window says so and stays open for another try.
    error: Option<&'static str>,
}

thread_local! {
    static STATE: std::cell::RefCell<Option<State>> = const { std::cell::RefCell::new(None) };
}

/// Reads the modifiers that are physically down right now.
///
/// `GetKeyState` rather than `GetAsyncKeyState`: inside a message handler the former reports the
/// state as of the message being processed, which is the state the user actually pressed, while the
/// latter reports the state now and can disagree if they have already let go.
fn current_mods() -> Mods {
    let down = |vk: i32| unsafe { (GetKeyState(vk) as u16 & 0x8000) != 0 };
    Mods {
        ctrl: down(0x11),
        alt: down(0x12),
        shift: down(0x10),
        win: down(0x5B) || down(0x5C),
    }
}

/// The text shown under the prompt: the chord so far, or a hint.
fn pending_text(held: Mods) -> String {
    if held.count() == 0 {
        return "...".into();
    }
    let mut s = String::new();
    for (on, name) in [(held.ctrl, "Ctrl"), (held.alt, "Alt"), (held.shift, "Shift"), (held.win, "Win")] {
        if on {
            s.push_str(name);
            s.push_str(" + ");
        }
    }
    s.push_str("...");
    s
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            // WM_SYSKEYDOWN is how any Alt combination arrives; handling only WM_KEYDOWN would make
            // every Alt chord uncapturable.
            let vk = (wp.0 & 0xFFFF) as u16;
            STATE.with(|s| {
                let mut g = s.borrow_mut();
                let Some(st) = g.as_mut() else { return };
                st.held = current_mods();
                if is_modifier(vk) {
                    // Not a commit - just update the "Ctrl + ..." echo so the user can see it is
                    // listening.
                    let _ = unsafe { InvalidateRect(Some(hwnd), None, true) };
                    return;
                }
                match vk {
                    // Bare Escape cancels. With a modifier it is a legitimate binding, so only the
                    // bare press is treated as "get me out of here".
                    0x1B if st.held.count() == 0 => {
                        st.result = None;
                        st.done = true;
                    }
                    // Bare Backspace or Delete unbinds, which is the only way to clear a key from
                    // this window.
                    0x08 | 0x2E if st.held.count() == 0 => {
                        st.result = Some(Captured::Clear);
                        st.done = true;
                    }
                    _ => {
                        let c = Chord { mods: st.held, vk };
                        match c.validate(&st.others) {
                            Ok(_) => {
                                st.result = Some(Captured::Chord(c));
                                st.done = true;
                            }
                            Err(why) => {
                                // Stay open. The user pressed something reasonable-looking and needs
                                // to know what was wrong with it, not to have the window vanish.
                                st.error = Some(why.message());
                                st.held = Mods::default();
                                let _ = unsafe { InvalidateRect(Some(hwnd), None, true) };
                            }
                        }
                    }
                }
            });
            LRESULT(0)
        }
        WM_APPCOMMAND => {
            // The media keys, which do not always arrive as key presses. The command is in the high
            // word of lParam, with flags in the low word.
            let cmd = ((lp.0 >> 16) & 0x0FFF) as i32;
            if let Some(vk) = vk_for_appcommand(cmd) {
                STATE.with(|s| {
                    if let Some(st) = s.borrow_mut().as_mut() {
                        st.result = Some(Captured::Chord(Chord { mods: current_mods(), vk }));
                        st.done = true;
                    }
                });
                return LRESULT(1);
            }
            unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
        }
        // Clicking away or closing the window is a cancel, not a silent hang. Without this the
        // caller's hotkeys would stay released for the life of the process.
        WM_KILLFOCUS | WM_CLOSE => {
            STATE.with(|s| {
                if let Some(st) = s.borrow_mut().as_mut() {
                    st.result = None;
                    st.done = true;
                }
            });
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            STATE.with(|s| {
                let g = s.borrow();
                let Some(st) = g.as_ref() else { return };
                let (bg, fg, dim) = if st.dark {
                    (0x00201F1Eu32, 0x00F0F0F0u32, 0x00A0A0A0u32)
                } else {
                    (0x00FAFAFAu32, 0x00202020u32, 0x00606060u32)
                };
                let mut rc = RECT::default();
                let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rc) };
                unsafe {
                    let brush = CreateSolidBrush(COLORREF(bg));
                    FillRect(hdc, &rc, brush);
                    let _ = DeleteObject(brush.into());
                    SetBkMode(hdc, TRANSPARENT);

                    let px = |pt: i32| -(pt * st.dpi / 72);
                    let title_font = CreateFontW(
                        px(TITLE_PT), 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0,
                        Default::default(), // charset
                        Default::default(), // output precision
                        Default::default(), // clip precision
                        // ClearType explicitly. DEFAULT_QUALITY does not guarantee it, and
                        // unhinted greyscale antialiasing at these sizes is exactly what
                        // "looks blurry" describes.
                        CLEARTYPE_QUALITY,
                        Default::default(), // pitch and family
                        w!("Segoe UI"),
                    );
                    let big_font = CreateFontW(
                        px(CHORD_PT), 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0,
                        Default::default(), // charset
                        Default::default(), // output precision
                        Default::default(), // clip precision
                        // ClearType explicitly. DEFAULT_QUALITY does not guarantee it, and
                        // unhinted greyscale antialiasing at these sizes is exactly what
                        // "looks blurry" describes.
                        CLEARTYPE_QUALITY,
                        Default::default(), // pitch and family
                        w!("Segoe UI"),
                    );
                    let small_font = CreateFontW(
                        px(HINT_PT), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                        Default::default(), // charset
                        Default::default(), // output precision
                        Default::default(), // clip precision
                        // ClearType explicitly. DEFAULT_QUALITY does not guarantee it, and
                        // unhinted greyscale antialiasing at these sizes is exactly what
                        // "looks blurry" describes.
                        CLEARTYPE_QUALITY,
                        Default::default(), // pitch and family
                        w!("Segoe UI"),
                    );

                    let third = (rc.bottom - rc.top) / 3;
                    let draw = |text: &str, font, colour: u32, top: i32, bottom: i32| {
                        let mut wide: Vec<u16> = text.encode_utf16().collect();
                        let old = SelectObject(hdc, font);
                        SetTextColor(hdc, COLORREF(colour));
                        let mut r = RECT { left: rc.left, top, right: rc.right, bottom };
                        DrawTextW(hdc, &mut wide, &mut r, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
                        SelectObject(hdc, old);
                    };
                    draw(&format!("Press the keys for {}", st.label), title_font.into(), dim, rc.top, third);
                    draw(&pending_text(st.held), big_font.into(), fg, third, third * 2);
                    match st.error {
                        Some(why) => draw(
                            why,
                            small_font.into(),
                            if st.dark { 0x005A5AFF } else { 0x000000C8 },
                            third * 2,
                            rc.bottom,
                        ),
                        None => draw(
                            "Esc cancels Backspace clears",
                            small_font.into(),
                            dim,
                            third * 2,
                            rc.bottom,
                        ),
                    }
                    let _ = DeleteObject(title_font.into());
                    let _ = DeleteObject(big_font.into());
                    let _ = DeleteObject(small_font.into());
                }
            });
            let _ = unsafe { EndPaint(hwnd, &ps) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

/// Shows the capture window and blocks until the user commits, clears, or cancels.
///
/// The caller MUST have released the app's own hotkeys first, or the very combinations most worth
/// binding will fire instead of being captured.
pub fn capture(owner: HWND, label: &str, dark: bool, others: &[Chord]) -> Option<Captured> {
    unsafe {
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            lpszClassName: w!("TaskbarEqCapture"),
            ..Default::default()
        };
        RegisterClassW(&class);

        // THE REAL DPI, resolved before the window exists.
        //
        // `GetDpiForWindow` needs a window, which is the trap: the first version therefore used a
        // hardcoded 96 for the geometry while the font scaled with the real DPI, so on a 125%
        // display the text was the right size inside a window 20% too small for it. The monitor
        // under the cursor is the one the menu was just clicked on, so it is the one this will
        // appear on.
        let mut cur = POINT::default();
        let _ = GetCursorPos(&mut cur);
        let mon = MonitorFromPoint(cur, MONITOR_DEFAULTTONEAREST);
        let (mut dx, mut dy) = (96u32, 96u32);
        let _ = GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        let dpi = (dx as i32).max(96);
        let (w0, h0) = (DIP_W * dpi / 96, DIP_H * dpi / 96);

        // Centred on THAT monitor's work area, not on the primary screen's metrics, so a
        // multi-monitor desktop does not put it on the wrong display.
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let (cx, cy) = if GetMonitorInfoW(mon, &mut mi).as_bool() {
            let r = mi.rcWork;
            (r.left + (r.right - r.left - w0) / 2, r.top + (r.bottom - r.top - h0) / 2)
        } else {
            (0, 0)
        };
        let hwnd = CreateWindowExW(
            // TOPMOST because the overlay is topmost and re-asserts itself every frame, so a normal
            // window would be painted over. TOOLWINDOW so this does not get a taskbar button -
            // which matters more than it sounds: a new taskbar button changes the clearance the
            // overlay measures, so the meter would visibly narrow while this window was open.
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("TaskbarEqCapture"),
            w!("Set key"),
            WS_POPUP | WS_BORDER,
            cx,
            cy,
            w0,
            h0,
            // Owned by the tray window, so it is destroyed with it and always sits above it.
            Some(owner),
            None,
            None,
            None,
        )
        .ok()?;

        STATE.with(|s| {
            *s.borrow_mut() =
                Some(State {
                    label: label.into(),
                    done: false,
                    result: None,
                    held: Mods::default(),
                    dark,
                    others: others.to_vec(),
                    dpi,
                    error: None,
                })
        });

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));

        // Own message loop, pumping EVERYTHING so the render tick's WM_TIMER still gets through and
        // the visualiser keeps running while this is open.
        let mut msg = MSG::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let done = STATE.with(|s| s.borrow().as_ref().map(|st| st.done).unwrap_or(true));
            if done {
                break;
            }
            // A timeout, so a window that somehow loses every route to a decision cannot leave the
            // app with its hotkeys released forever.
            if std::time::Instant::now() > deadline {
                crate::log::write("key capture timed out after 30s; nothing was changed");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(8));
        }

        let out = STATE.with(|s| s.borrow_mut().take().and_then(|st| st.result));
        let _ = DestroyWindow(hwnd);
        // Give the owner its focus back, or the taskbar is left in an odd state.
        let _ = SetForegroundWindow(owner);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialises the two live tests. Both create a window of the same class and then locate it with
    /// `FindWindowW`, so run in parallel - which is `cargo test`'s default - each can find the
    /// other's window and post its key into it. Run together without this they fail; run one at a
    /// time they pass, which is the most misleading shape a test failure can have. Same remedy the
    /// registry and config tests already use.
    static LIVE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Drives the real capture window: opens it, posts a key to it from another thread, and checks what
    /// comes back. Ignored because it creates a window.
    ///
    /// Posts the message rather than synthesising input, deliberately: this is testing the window and
    /// its state machine, and injected input would additionally depend on focus, which is exactly the
    /// part a test cannot control reliably.
    ///
    /// Run: cargo test --release live_capture -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_capture_window_returns_what_was_pressed() {
        let _serial = LIVE.lock().unwrap_or_else(|e| e.into_inner());
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};

        // A key, a clear, and a cancel - the three routes out. 0x78 is F9, which is the exact key
        // that was reported as wrongly refused, so it doubles as the regression test for that.
        for (vk, want) in [
            (0x78u16, Some(Captured::Chord(Chord { mods: Mods::default(), vk: 0x78 }))),
            (0x08, Some(Captured::Clear)),
            (0x1B, None),
        ] {
            let poster = std::thread::spawn(move || {
                // Wait for the window to exist, then post into it.
                for _ in 0..200 {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    let h = unsafe { FindWindowW(w!("TaskbarEqCapture"), None) };
                    if let Ok(h) = h {
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        let _ = unsafe {
                            PostMessageW(Some(h), WM_KEYDOWN, WPARAM(vk as usize), LPARAM(0))
                        };
                        return true;
                    }
                }
                false
            });
            let got = capture(HWND(std::ptr::null_mut()), "test", true, &[]);
            assert!(poster.join().unwrap(), "the capture window never appeared");
            println!("  posted {vk:#04X} -> {got:?}");
            assert_eq!(got, want, "posting {vk:#04X} gave the wrong result");
        }
    }

    /// A chord the rules refuse must NOT close the window or get stored.
    ///
    /// The failure this pins is what was reported: an unusable binding was accepted here, written to
    /// the config, and refused later by the startup path, so the only evidence was "(not allowed)" in
    /// a menu label with the reason nowhere at all.
    ///
    /// Run: cargo test --release live_capture_refuses -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_capture_refuses_a_bare_letter_and_stays_open() {
        let _serial = LIVE.lock().unwrap_or_else(|e| e.into_inner());
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};
        let poster = std::thread::spawn(move || {
            for _ in 0..200 {
                std::thread::sleep(std::time::Duration::from_millis(20));
                if let Ok(h) = unsafe { FindWindowW(w!("TaskbarEqCapture"), None) } {
                    std::thread::sleep(std::time::Duration::from_millis(80));
                    // A bare letter: legitimately refused, because it would fire mid-sentence.
                    let _ = unsafe {
                        PostMessageW(Some(h), WM_KEYDOWN, WPARAM(0x50), LPARAM(0))
                    };
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    // The window must still be there to receive this.
                    let still = unsafe { FindWindowW(w!("TaskbarEqCapture"), None) };
                    let alive = still.is_ok();
                    if let Ok(h2) = still {
                        let _ = unsafe {
                            PostMessageW(Some(h2), WM_KEYDOWN, WPARAM(0x1B), LPARAM(0))
                        };
                    }
                    return alive;
                }
            }
            false
        });
        let got = capture(HWND(std::ptr::null_mut()), "test", true, &[]);
        let stayed_open = poster.join().unwrap();
        println!("  bare 'P' -> refused, window still open = {stayed_open}, result {got:?}");
        assert!(stayed_open, "a refused chord closed the window instead of reporting why");
        assert_eq!(got, None, "a refused chord must not be committed");
    }

    /// Prints the DPI the window is actually sized for, and the resulting pixel size.
    ///
    /// Exists because the bug it guards was invisible: the geometry used a hardcoded 96 while the
    /// font used the real DPI, and on a 100% display those agree, so nothing looks wrong on a
    /// developer machine that happens not to be scaled.
    ///
    /// Run: cargo test --release probe_capture_dpi -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_capture_dpi() {
        use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};
        use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        // THE PROCESS MUST BE DPI-AWARE OR THIS MEASURES A FICTION. `main` sets per-monitor-v2
        // before it creates anything; the test harness has its own entry point and does not, so
        // without this line Windows virtualises the answer and reports 96 on a 125% display - which
        // it did, and which is the same lie that made a 1920x1200 monitor look like 1536x960 when
        // measured from PowerShell. Any DPI number obtained from an unaware process is worthless.
        crate::win::dpi::set_per_monitor_v2();
        unsafe {
            let mut cur = POINT::default();
            let _ = GetCursorPos(&mut cur);
            let mon = MonitorFromPoint(cur, MONITOR_DEFAULTTONEAREST);
            let (mut dx, mut dy) = (96u32, 96u32);
            let _ = GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
            let dpi = (dx as i32).max(96);
            println!(
                "monitor dpi = {dpi} ({}% scaling)
  window  {}x{} px (from {DIP_W}x{DIP_H} dip)
                   fonts title {}px  chord {}px  hint {}px",
                dpi * 100 / 96,
                DIP_W * dpi / 96,
                DIP_H * dpi / 96,
                TITLE_PT * dpi / 72,
                CHORD_PT * dpi / 72,
                HINT_PT * dpi / 72
            );
            assert!(dpi >= 96);
        }
    }

        #[test]
    fn modifier_keys_are_never_treated_as_a_trigger() {
        // Committing on a modifier would make every chord impossible: the first thing a user presses
        // for Ctrl+Alt+P is Ctrl.
        for vk in [0x10u16, 0x11, 0x12, 0x5B, 0x5C, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5] {
            assert!(is_modifier(vk), "{vk:#04X} must be held, not committed");
        }
        for vk in [0x41u16, 0x20, 0x0D, 0x70, 0xB3] {
            assert!(!is_modifier(vk), "{vk:#04X} must be able to commit");
        }
    }

    #[test]
    fn the_media_keys_can_be_captured_from_wm_appcommand() {
        // Without this route a user pressing the Play button on their keyboard captures nothing,
        // because the media keys do not reliably arrive as WM_KEYDOWN.
        assert_eq!(vk_for_appcommand(14), Some(0xB3), "play/pause");
        assert_eq!(vk_for_appcommand(11), Some(0xB0), "next");
        assert_eq!(vk_for_appcommand(12), Some(0xB1), "previous");
        assert_eq!(vk_for_appcommand(13), Some(0xB2), "stop");
        // And unrelated app commands are ignored rather than captured as a mystery key.
        for other in [1, 2, 5, 15, 20, 30] {
            assert_eq!(vk_for_appcommand(other), None, "appcommand {other}");
        }
    }

    #[test]
    fn the_prompt_echoes_held_modifiers_so_it_is_visibly_listening() {
        assert_eq!(pending_text(Mods::default()), "...");
        assert_eq!(
            pending_text(Mods { ctrl: true, alt: true, ..Default::default() }),
            "Ctrl + Alt + ..."
        );
        assert_eq!(
            pending_text(Mods { ctrl: true, alt: true, shift: true, win: true }),
            "Ctrl + Alt + Shift + Win + ..."
        );
        // The order matches `Chord`'s canonical spelling, so the echo and the committed value read
        // the same way round.
        assert_eq!(pending_text(Mods { win: true, ..Default::default() }), "Win + ...");
    }
}
