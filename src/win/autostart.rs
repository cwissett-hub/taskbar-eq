use anyhow::Result;
use windows::core::{w, HSTRING};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};

const VALUE_NAME: windows::core::PCWSTR = w!("TaskbarEQ");

fn run_key(access: u32) -> Result<HKEY> {
    let mut key = HKEY::default();
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            windows::Win32::System::Registry::REG_SAM_FLAGS(access),
            &mut key,
        )
        .ok()?;
    }
    Ok(key)
}

pub fn is_enabled() -> bool {
    match run_key(KEY_READ.0) {
        Ok(key) => {
            let mut size = 0u32;
            let present = unsafe {
                RegQueryValueExW(key, VALUE_NAME, None, None, None, Some(&mut size)).is_ok()
            };
            unsafe {
                let _ = RegCloseKey(key);
            }
            present
        }
        Err(_) => false,
    }
}

/// HKCU only - never HKLM. This must work without elevation.
pub fn set(enabled: bool) -> Result<()> {
    let key = run_key(KEY_WRITE.0 | KEY_READ.0)?;
    unsafe {
        if enabled {
            let exe = std::env::current_exe()?;
            let quoted = format!("\"{}\"", exe.display());
            let h = HSTRING::from(quoted);
            let bytes: &[u8] = std::slice::from_raw_parts(
                h.as_ptr() as *const u8,
                (h.len() + 1) * 2, // include the NUL
            );
            RegSetValueExW(key, VALUE_NAME, Some(0), REG_SZ, Some(bytes)).ok()?;
        } else {
            let _ = RegDeleteValueW(key, VALUE_NAME);
        }
        let _ = RegCloseKey(key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // All three tests below mutate the one shared HKCU Run value for this
    // app. The brief's own note ("--test-threads=1 matters") only covers the
    // standalone `cargo test autostart` invocation - a plain `cargo test`
    // (which the task's own verification step requires running) executes the
    // whole suite in parallel by default, and these tests DO race under that:
    // observed live on this machine, a parallel `cargo test` run left this
    // exe's own registry value permanently enabled afterwards even though
    // every test reported green. This mutex serialises them regardless of
    // `--test-threads`, so a plain `cargo test` cannot leave the real
    // developer registry mutated.
    static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn toggling_autostart_round_trips() {
        let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let original = is_enabled();

        set(true).expect("enable should not need elevation");
        assert!(is_enabled(), "should report enabled after set(true)");

        set(false).expect("disable should succeed");
        assert!(!is_enabled(), "should report disabled after set(false)");

        // Leave the machine as we found it.
        set(original).ok();
    }

    #[test]
    fn disabling_when_absent_is_not_an_error() {
        let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let original = is_enabled();
        set(false).ok();
        assert!(set(false).is_ok(), "deleting a missing value must be idempotent");
        set(original).ok();
    }

    /// The task-11 brief's manual Step 7 checks the persisted command line
    /// with `reg query`; this pins the same thing deterministically - that
    /// what `set(true)` actually writes is a quoted, absolute path to this
    /// exe, which is what "survives restart" depends on being correct.
    #[test]
    fn the_persisted_command_line_is_a_quoted_absolute_exe_path() {
        let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let original = is_enabled();
        set(true).expect("enable should not need elevation");

        let key = run_key(KEY_READ.0).expect("Run key should open for read");
        let mut size = 0u32;
        unsafe {
            RegQueryValueExW(key, VALUE_NAME, None, None, None, Some(&mut size))
                .ok()
                .expect("value should exist after set(true)");
        }
        let mut buf = vec![0u8; size as usize];
        unsafe {
            RegQueryValueExW(
                key,
                VALUE_NAME,
                None,
                None,
                Some(buf.as_mut_ptr()),
                Some(&mut size),
            )
            .ok()
            .expect("value should be readable");
            let _ = RegCloseKey(key);
        }
        let u16s: Vec<u16> = buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let s = String::from_utf16_lossy(&u16s);
        let s = s.trim_end_matches('\0');

        assert!(
            s.starts_with('"') && s.ends_with('"'),
            "expected a quoted path, got {s:?}"
        );
        // Under `cargo test` this is the test harness binary, not
        // taskbar-eq.exe - what matters is that `set()` faithfully quotes
        // whatever `current_exe()` reports, not a hardcoded literal name.
        let expected = format!("\"{}\"", std::env::current_exe().unwrap().display());
        assert_eq!(s, expected, "the persisted command line should be a quoted current_exe()");

        set(original).ok();
    }
}
