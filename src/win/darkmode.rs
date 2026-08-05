//! Makes Win32 menus follow the user's Windows light/dark app preference.
//!
//! A plain `CreatePopupMenu` menu is always light, regardless of the system setting or of
//! anything the app does to its own windows: `DwmSetWindowAttribute`'s dark-mode attribute
//! applies to window frames, not to the menus a window owns, and there is no documented
//! per-menu equivalent.
//!
//! The two entry points that do work are undocumented uxtheme exports, available by
//! ORDINAL only - they have no names in the export table, so they cannot be linked
//! against and must be resolved at runtime:
//!
//! - ordinal 135, `SetPreferredAppMode` (as of Windows 10 1903; on 1809 the same ordinal
//!   was `AllowDarkModeForApp`, which takes a BOOL instead of an enum - the two happen to
//!   be call-compatible for the values used here, since 1 means "allow dark"/"AllowDark"
//!   in both)
//! - ordinal 136, `FlushMenuThemes`, which makes an already-cached menu theme re-resolve
//!
//! Being undocumented, this is written to degrade rather than fail: if the library or
//! either ordinal is missing, `apply` silently leaves the menu light. A light menu on a
//! dark desktop is a cosmetic flaw; a crash or a failed launch is not an acceptable price
//! for it.
//!
//! Deliberately NOT owner-drawn. Owner-draw is fully documented and would avoid the
//! undocumented calls, but it means reimplementing item metrics, checkmarks, submenu
//! arrows, accelerator alignment, keyboard focus rendering and the high-contrast
//! accessibility modes by hand - far more code, and far more ways to look subtly wrong,
//! than the thing it avoids.

use std::sync::OnceLock;
use windows::core::{s, PCSTR};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_VALUE_TYPE,
};

/// Values for uxtheme's `PreferredAppMode`.
#[allow(dead_code)]
#[repr(i32)]
enum PreferredAppMode {
    Default = 0,
    AllowDark = 1,
    ForceDark = 2,
    ForceLight = 3,
}

/// True when the user has apps set to dark under Settings > Personalisation > Colours.
///
/// Reads `AppsUseLightTheme`, which is the apps-specific preference; `SystemUsesLightTheme`
/// is the separate taskbar/Start setting and is the wrong one to key a menu off. A missing
/// value means dark: the key is absent on installs that have never changed the default,
/// and Windows treats absent as... light, actually - but this app's own overlay is
/// dark-only by design, so on the unusual absent-value path a dark menu is the consistent
/// choice.
pub fn windows_prefers_dark() -> bool {
    unsafe {
        let mut key = HKEY::default();
        let path = windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        if RegOpenKeyExW(HKEY_CURRENT_USER, path, Some(0), KEY_READ, &mut key).is_err() {
            return true;
        }
        let mut data = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut kind = REG_VALUE_TYPE::default();
        let res = RegQueryValueExW(
            key,
            windows::core::w!("AppsUseLightTheme"),
            None,
            Some(&mut kind),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if res.is_err() {
            return true;
        }
        data == 0
    }
}

type SetPreferredAppModeFn = unsafe extern "system" fn(i32) -> i32;
type FlushMenuThemesFn = unsafe extern "system" fn();

struct UxTheme {
    set_mode: Option<SetPreferredAppModeFn>,
    flush: Option<FlushMenuThemesFn>,
}

/// Resolved once. `LoadLibraryW` on an already-loaded module just bumps its refcount, but
/// resolving two ordinals on every right-click is pointless work, and caching also means a
/// missing export is reported to `eprintln` once rather than on every menu.
fn uxtheme() -> &'static UxTheme {
    static CELL: OnceLock<UxTheme> = OnceLock::new();
    CELL.get_or_init(|| unsafe {
        let module = match LoadLibraryW(windows::core::w!("uxtheme.dll")) {
            Ok(m) if !m.is_invalid() => m,
            _ => {
                eprintln!("darkmode: uxtheme.dll unavailable; menus will use the light theme");
                return UxTheme { set_mode: None, flush: None };
            }
        };
        // Ordinals, not names: these exports are unnamed, so PCSTR carries the ordinal in
        // the low word rather than a string pointer. This is what `MAKEINTRESOURCEA` does.
        let by_ordinal = |n: u16| PCSTR(n as usize as *const u8);
        let set_mode = GetProcAddress(module, by_ordinal(135))
            .map(|p| std::mem::transmute::<_, SetPreferredAppModeFn>(p));
        let flush =
            GetProcAddress(module, by_ordinal(136)).map(|p| std::mem::transmute::<_, FlushMenuThemesFn>(p));
        if set_mode.is_none() {
            eprintln!("darkmode: uxtheme ordinal 135 missing; menus will use the light theme");
        }
        let _ = s!("");
        UxTheme { set_mode, flush }
    })
}

/// Points the process's menu theming at the user's current preference.
///
/// Call before building a menu, every time rather than once at startup: the user can flip
/// the setting while the app runs, and `FlushMenuThemes` is what makes an already-themed
/// menu pick up the change.
pub fn apply() {
    let ux = uxtheme();
    let Some(set_mode) = ux.set_mode else { return };
    let mode = if windows_prefers_dark() {
        // AllowDark rather than ForceDark, so a system on a high-contrast theme keeps the
        // accessibility colours it asked for instead of being overridden to dark.
        PreferredAppMode::AllowDark
    } else {
        PreferredAppMode::ForceLight
    };
    unsafe {
        set_mode(mode as i32);
        if let Some(flush) = ux.flush {
            flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_the_preference_does_not_panic_and_is_stable() {
        // Cannot assert WHICH value without dictating the tester's Windows settings, so
        // this covers the two things that are actually invariant: the registry read
        // completes without panicking on any machine, and it is a pure read - calling it
        // twice must agree, which would catch it accidentally writing or consuming state.
        let a = windows_prefers_dark();
        let b = windows_prefers_dark();
        assert_eq!(a, b, "reading the theme preference must be side-effect free");
    }

    #[test]
    fn apply_is_safe_to_call_repeatedly_and_before_any_menu_exists() {
        // The undocumented ordinals are the risk being covered here: if either one is
        // absent or has changed signature on this Windows build, `apply` must no-op rather
        // than fault. Calling it with no menu in existence is also the real startup order.
        apply();
        apply();
        apply();
    }
}
