//! Display power state, so the overlay can stop entirely when there is nothing to look at.
//!
//! # Why this exists
//!
//! Measured, not suspected. `powercfg /srumutil` over eight days put this app **second on the machine**
//! for energy - above VS Code, 5.3x Chrome across its thirty processes, 18.6x the editor agent - beaten
//! only by Spotify. It was reported alongside a battery that drained in about 45 minutes with the laptop
//! **lid closed**, which is the case this module addresses: with the lid shut nothing can be seen, so
//! every frame composited and every shell call made is pure waste.
//!
//! The cost is also worse than the app's own CPU share suggests. A process that wakes every 16ms keeps
//! the machine out of the deep idle states that make a closed lid cheap in the first place, so the damage
//! is not the few percent of a core it spends but the low-power state it denies everything else.
//!
//! # Why a notification and not a poll
//!
//! There is no cheap query for "is the display on". `GUID_CONSOLE_DISPLAY_STATE` is the documented
//! mechanism and it is push-only, which is why the state lives in an atomic in `shell_state` beside the
//! other suspend signals rather than being read on demand.
//!
//! It only fires on a CHANGE, so a process that starts with the display already on is never told - which
//! is exactly why the default is "on". Defaulting to "off" would hide the meter from launch until the
//! next time the screen happened to sleep.
//!
//! # What it deliberately does not do
//!
//! It does not treat DIMMED as off. Windows reports three states, and a dimmed screen is still a screen
//! someone is looking at - it dims before it sleeps, and blanking the meter a few seconds early on every
//! idle timeout would be a visible regression in exchange for nothing.

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Power::{
    RegisterPowerSettingNotification, HPOWERNOTIFY, POWERBROADCAST_SETTING,
};
use windows::Win32::UI::WindowsAndMessaging::DEVICE_NOTIFY_WINDOW_HANDLE;

/// `GUID_CONSOLE_DISPLAY_STATE` - the console session's display power state.
///
/// Written out rather than imported because the `windows` crate exposes the power-setting GUIDs
/// inconsistently across versions, and a hand-written constant cannot break on a bump.
/// {6FE69556-704A-47A0-8F24-C28D936FDA47}
const GUID_CONSOLE_DISPLAY_STATE: windows::core::GUID = windows::core::GUID::from_values(
    0x6FE6_9556,
    0x704A,
    0x47A0,
    [0x8F, 0x24, 0xC2, 0x8D, 0x93, 0x6F, 0xDA, 0x47],
);

/// The three values `GUID_CONSOLE_DISPLAY_STATE` reports.
pub const DISPLAY_OFF: u32 = 0;
pub const DISPLAY_ON: u32 = 1;
pub const DISPLAY_DIMMED: u32 = 2;

/// Asks for `WM_POWERBROADCAST` on display power changes. Returns the handle to keep alive.
///
/// The handle is deliberately returned rather than dropped: unregistering is what stops the
/// notifications, so dropping it here would register and immediately cancel. `main` holds it for the
/// life of the process, which is also why there is no unregister path - the process exiting is the
/// unregister.
pub fn watch_display(hwnd: HWND) -> Option<HPOWERNOTIFY> {
    unsafe {
        RegisterPowerSettingNotification(
            windows::Win32::Foundation::HANDLE(hwnd.0),
            &GUID_CONSOLE_DISPLAY_STATE,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )
    }
    .ok()
}

/// Decides the display state from a power-setting GUID and its raw payload bytes.
///
/// The pure core, and pure for a specific reason: `POWERBROADCAST_SETTING::Data` is declared `[u8; 1]`
/// as a stand-in for a variable-length tail, so the four bytes of the display state cannot be read
/// through the struct field at all - indexing past the first is a compile-time panic. Reading it needs a
/// raw pointer, and a raw pointer cannot be handed a payload in a test. Taking bytes instead means the
/// three-state decision is testable with real four-byte data, without a window, a power event, or a
/// laptop lid.
///
/// `None` means "not an answer": a different power setting, a truncated payload, or a state Windows has
/// not documented. The caller must not treat it as either state - guessing "off" blanks the meter.
pub fn display_off_from_bytes(guid: &windows::core::GUID, data: &[u8]) -> Option<bool> {
    if *guid != GUID_CONSOLE_DISPLAY_STATE {
        return None;
    }
    // The length is the only thing that says how much of the tail is real. Reading four bytes out of a
    // shorter payload would be reading whatever the system happened to leave there.
    if data.len() < 4 {
        return None;
    }
    match u32::from_le_bytes([data[0], data[1], data[2], data[3]]) {
        DISPLAY_OFF => Some(true),
        // ON and DIMMED both count as visible - see the module note on why dimmed is not off.
        DISPLAY_ON | DISPLAY_DIMMED => Some(false),
        _ => None,
    }
}

/// Reads the display state out of a `WM_POWERBROADCAST` whose wparam is `PBT_POWERSETTINGCHANGE`.
///
/// # Safety
///
/// `lparam` must be the pointer Windows passed with that message, pointing at a
/// `POWERBROADCAST_SETTING` whose `DataLength` describes a tail that is really there.
pub unsafe fn display_off_from_lparam(lparam: LPARAM) -> Option<bool> {
    if lparam.0 == 0 {
        return None;
    }
    let setting = unsafe { &*(lparam.0 as *const POWERBROADCAST_SETTING) };
    // The tail starts at `Data` and runs for `DataLength`. Bounded to 8 bytes because this only ever
      // reads four and a hostile or corrupt length must not turn into a huge slice.
    let len = (setting.DataLength as usize).min(8);
    let bytes = unsafe { std::slice::from_raw_parts(setting.Data.as_ptr(), len) };
    display_off_from_bytes(&setting.PowerSetting, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real four-byte payload, little-endian, as Windows sends it.
    fn payload(state: u32) -> [u8; 4] {
        state.to_le_bytes()
    }

    #[test]
    fn only_a_display_off_message_suspends_the_overlay() {
        // The three-state decision, without needing a laptop lid. DIMMED is the interesting one: it is
        // NOT off, because Windows dims before it sleeps and blanking the meter a few seconds early on
        // every idle timeout would be a visible regression for nothing.
        assert_eq!(
            display_off_from_bytes(&GUID_CONSOLE_DISPLAY_STATE, &payload(DISPLAY_OFF)),
            Some(true),
            "an off display must suspend the overlay"
        );
        assert_eq!(
            display_off_from_bytes(&GUID_CONSOLE_DISPLAY_STATE, &payload(DISPLAY_ON)),
            Some(false),
            "an on display must resume it"
        );
        assert_eq!(
            display_off_from_bytes(&GUID_CONSOLE_DISPLAY_STATE, &payload(DISPLAY_DIMMED)),
            Some(false),
            "a DIMMED display is still a display someone is looking at"
        );
    }

    #[test]
    fn another_power_setting_is_not_an_answer_either_way() {
        // Windows delivers every power setting the process has subscribed to, and more can arrive from
        // elsewhere in the system. Returning Some(false) for an unrelated GUID would resume the overlay
        // on, say, a battery-saver change while the lid was still shut.
        let other = windows::core::GUID::from_values(0x1234_5678, 0x0000, 0x0000, [0; 8]);
        assert_eq!(display_off_from_bytes(&other, &payload(DISPLAY_OFF)), None);
        // And an unknown state for the RIGHT guid is also not evidence: guessing "off" blanks the meter.
        assert_eq!(display_off_from_bytes(&GUID_CONSOLE_DISPLAY_STATE, &payload(99)), None);
    }

    #[test]
    fn a_truncated_payload_is_rejected_rather_than_read() {
        // The payload is variable-length and the length is the only thing saying how much of it is real.
        // Reading four bytes out of a shorter one would be reading whatever the system left there.
        let full = payload(DISPLAY_OFF);
        assert_eq!(
            display_off_from_bytes(&GUID_CONSOLE_DISPLAY_STATE, &full[..2]),
            None,
            "a short payload must not be interpreted"
        );
    }
}
