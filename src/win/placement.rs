use crate::geom::Rect;
use anyhow::Result;
use windows::core::{w, BSTR};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants};
use windows::Win32::UI::Shell::SHQueryUserNotificationState;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect, IsWindowVisible};

/// The Widgets button's automation name embeds the live weather, e.g.
/// "Widgets 19C Partly cloudy" or "Widgets 20C Mostly cloudy". Match on the
/// stable prefix only - the rest changes every few minutes.
pub fn is_widget_name(name: &str) -> bool {
    name.starts_with("Widgets")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_real_observed_names() {
        // Both captured live from this machine, 30 minutes apart.
        assert!(is_widget_name("Widgets 20\u{b0}C Mostly cloudy"));
        assert!(is_widget_name("Widgets 19\u{b0}C Partly cloudy"));
        assert!(is_widget_name("Widgets"));
    }

    #[test]
    fn rejects_other_tray_items() {
        for other in [
            "Start",
            "Search",
            "Task View",
            "Show Hidden Icons",
            "Clock 21:08",
            "Network Lenses - Primary",
            "Power Battery status: 99% available",
            "Spotify - 1 running window",
        ] {
            assert!(!is_widget_name(other), "{other} must not match");
        }
    }

    #[test]
    fn is_case_sensitive_on_the_prefix() {
        assert!(!is_widget_name("widgets 19C"));
    }
}

fn tray_hwnd() -> Result<HWND> {
    Ok(unsafe { FindWindowW(w!("Shell_TrayWnd"), None)? })
}

/// Walks the taskbar's UIA subtree for the Widgets button. Returns physical pixels.
/// Call this on a timer - the rect moves as the weather text changes width.
pub fn find_widget_rect() -> Result<Option<Rect>> {
    let tray = tray_hwnd()?;
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let root = unsafe { automation.ElementFromHandle(tray)? };
    let cond = unsafe { automation.CreateTrueCondition()? };
    let all = unsafe { root.FindAll(TreeScope_Descendants, &cond)? };

    for i in 0..unsafe { all.Length()? } {
        let el = unsafe { all.GetElement(i)? };
        let name: BSTR = unsafe { el.CurrentName().unwrap_or_default() };
        if is_widget_name(&name.to_string()) {
            let r = unsafe { el.CurrentBoundingRectangle()? };
            return Ok(Some(Rect {
                x: r.left,
                y: r.top,
                w: r.right - r.left,
                h: r.bottom - r.top,
            }));
        }
    }
    Ok(None)
}

/// The taskbar's own rect, in true physical pixels (the process is per-monitor-v2
/// aware, so GetWindowRect is not virtualised here).
/// Finds the overflow chevron's rect via the same UIA walk as the Widgets button.
pub fn find_chevron_rect() -> Result<Option<Rect>> {
    let tray = tray_hwnd()?;
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let root = unsafe { automation.ElementFromHandle(tray)? };
    let cond = unsafe { automation.CreateTrueCondition()? };
    let all = unsafe { root.FindAll(TreeScope_Descendants, &cond)? };
    for i in 0..unsafe { all.Length()? } {
        let el = unsafe { all.GetElement(i)? };
        let name: BSTR = unsafe { el.CurrentName().unwrap_or_default() };
        if is_chevron_name(&name.to_string()) {
            let r = unsafe { el.CurrentBoundingRectangle()? };
            return Ok(Some(Rect {
                x: r.left,
                y: r.top,
                w: r.right - r.left,
                h: r.bottom - r.top,
            }));
        }
    }
    Ok(None)
}

pub fn taskbar_rect() -> Option<Rect> {
    let h = tray_hwnd().ok()?;
    let mut r = RECT::default();
    if unsafe { GetWindowRect(h, &mut r) }.is_err() {
        return None;
    }
    Some(Rect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top })
}

/// True for the taskbar's overflow chevron - "Show Hidden Icons" on Windows 11,
/// "Show hidden icons" on Windows 10. Matched case-insensitively on the stable part
/// of the phrase precisely because those differ.
pub fn is_chevron_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("hidden icons")
}

/// Places the display to the LEFT of the overflow chevron.
///
/// This is the Windows 10 answer as well as the "put it somewhere sensible" answer.
/// Win10 has no Widgets button at all - it has News and interests, a differently
/// named element - so on that OS `find_widget_rect` always returns None and the
/// overlay would otherwise run and show nothing. The chevron exists on both, sits in
/// the same part of the tray on both, and moves with the tray as icons come and go,
/// so anchoring to it gives a consistent position without hardcoding coordinates.
pub fn rect_left_of(anchor: Rect, taskbar: Rect, gap: i32, width: i32) -> Rect {
    let width = width.clamp(40, taskbar.w.max(40));
    let gap = gap.clamp(0, taskbar.w);
    // right edge sits `gap` px left of the anchor, then clamp into the taskbar
    let mut x = anchor.x - gap - width;
    if x < taskbar.x {
        x = taskbar.x;
    }
    Rect { x, y: taskbar.y, w: width, h: taskbar.h }
}

pub fn notification_state() -> i32 {
    unsafe { SHQueryUserNotificationState().map(|s| s.0).unwrap_or(0) }
}

pub fn taskbar_visible() -> bool {
    match tray_hwnd() {
        Ok(h) => unsafe { IsWindowVisible(h).as_bool() },
        Err(_) => false,
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    fn bar() -> Rect {
        Rect { x: 0, y: 1140, w: 1920, h: 60 }
    }

    #[test]
    fn chevron_matches_both_windows_spellings() {
        assert!(is_chevron_name("Show Hidden Icons"), "Windows 11");
        assert!(is_chevron_name("Show hidden icons"), "Windows 10");
        assert!(is_chevron_name("SHOW HIDDEN ICONS"));
    }

    #[test]
    fn chevron_predicate_rejects_other_tray_items() {
        for other in ["Widgets 19C Partly cloudy", "Clock 21:08", "Start", "Volume", "Network"] {
            assert!(!is_chevron_name(other), "{other} must not match");
        }
    }

    #[test]
    fn sits_immediately_left_of_the_chevron() {
        // measured chevron on the reference machine
        let chev = Rect { x: 1590, y: 1140, w: 40, h: 60 };
        let r = rect_left_of(chev, bar(), 4, 190);
        assert_eq!(r.x + r.w, chev.x - 4, "right edge sits `gap` px left of the chevron");
        assert_eq!(r.w, 190);
        assert_eq!(r.y, 1140);
        assert_eq!(r.h, 60, "takes the taskbar's full height");
    }

    #[test]
    fn clamps_into_the_taskbar_rather_than_going_off_the_left_edge() {
        let chev = Rect { x: 60, y: 1140, w: 40, h: 60 };
        let r = rect_left_of(chev, bar(), 4, 190);
        assert_eq!(r.x, bar().x, "clamped flush left instead of negative");
        assert!(r.is_plausible_widget());
    }

    #[test]
    fn fallback_rect_is_plausible_so_should_show_accepts_it() {
        let chev = Rect { x: 1590, y: 1140, w: 40, h: 60 };
        assert!(rect_left_of(chev, bar(), 4, 190).is_plausible_widget());
    }
}

