use crate::geom::Rect;
use anyhow::Result;
use windows::core::{w, BSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants};
use windows::Win32::UI::Shell::SHQueryUserNotificationState;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, IsWindowVisible};

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

pub fn notification_state() -> i32 {
    unsafe { SHQueryUserNotificationState().map(|s| s.0).unwrap_or(0) }
}

pub fn taskbar_visible() -> bool {
    match tray_hwnd() {
        Ok(h) => unsafe { IsWindowVisible(h).as_bool() },
        Err(_) => false,
    }
}
