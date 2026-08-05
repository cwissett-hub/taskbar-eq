//! A log file, because on someone else's machine `eprintln!` is not evidence.
//!
//! This module exists because of a real support failure. The app was run on a Windows 10 machine,
//! showed its tray icon and never rendered, and there was NOTHING to diagnose it with: sixteen
//! `eprintln!` calls existed, but the binary is a console-subsystem app launched from Explorer, so
//! the console window closes the instant the process ends. A fatal error printed its cause and took
//! it away again in the same moment.
//!
//! Everything written here also goes to stderr, so running from a terminal is unchanged.

use std::io::Write as _;
use std::sync::Mutex;

/// Where the log lives. Next to `config.toml`, so a user asked for "the log" has one place to look.
pub fn path() -> std::path::PathBuf {
    crate::config::Config::dir().join("taskbar-eq.log")
}

struct State {
    file: Option<std::fs::File>,
    /// Last message and how many times it has repeated, so a per-frame failure cannot fill the disk.
    last: String,
    repeats: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Truncates the log and records the environment. Call once, early.
///
/// Truncated rather than appended so the file is always about the CURRENT run - a support request is
/// answered by "send me the log", and an ever-growing file makes the reader hunt for which launch
/// went wrong.
pub fn init() {
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = std::fs::File::create(&p).ok();
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(State { file, last: String::new(), repeats: 0 });
    drop(guard);

    write(&format!("taskbar-eq {} starting", env!("CARGO_PKG_VERSION")));
    write(&format!("log file: {}", p.display()));
    write(&os_summary());
}

/// OS build and product name, for every log. The failures that matter here are version-dependent:
/// the Widgets button does not exist before Windows 11, and per-monitor-v2 DPI awareness does not
/// exist before Windows 10 1703.
///
/// Read from the registry rather than from `GetVersionEx`, which lies to an unmanifested process
/// about anything past 6.2, and rather than `RtlGetVersion`, which would pull in the Wdk bindings
/// for one call. `CurrentBuild` under `Windows NT\CurrentVersion` is not shimmed.
pub fn os_summary() -> String {
    let build = reg_sz("CurrentBuild").unwrap_or_else(|| "?".into());
    let product = reg_sz("ProductName").unwrap_or_else(|| "?".into());
    let display = reg_sz("DisplayVersion").unwrap_or_default();
    // 22000 is the first Windows 11 build. Below it there is no Widgets button at all, which is why
    // the chevron fallback exists - so this line is the first thing to read in a "nothing renders"
    // report.
    // Classified on the BUILD, not on ProductName. ProductName still reads "Windows 10
    // Enterprise" on a Windows 11 build 26100 machine - Microsoft never updated it - so a log that
    // trusted the name would send anyone reading a "does not work on Windows 10" report chasing the
    // wrong OS entirely.
    let family = match build.parse::<u32>() {
        Ok(b) if b >= 22000 => "Windows 11",
        Ok(_) => "Windows 10 or older - no Widgets button, the chevron fallback is in use",
        Err(_) => "unknown build",
    };
    format!("windows: {product} {display} build {build} => {family}")
}

fn reg_sz(name: &str) -> Option<String> {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
        REG_VALUE_TYPE,
    };
    unsafe {
        let mut key = HKEY::default();
        let path: Vec<u16> = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            windows::core::PCWSTR(path.as_ptr()),
            Some(0),
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return None;
        }
        let wname: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut buf = [0u16; 128];
        let mut len = (buf.len() * 2) as u32;
        let mut kind = REG_VALUE_TYPE::default();
        let res = RegQueryValueExW(
            key,
            windows::core::PCWSTR(wname.as_ptr()),
            None,
            Some(&mut kind),
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut len),
        );
        let _ = RegCloseKey(key);
        if res.is_err() {
            return None;
        }
        let chars = (len as usize / 2).min(buf.len());
        let s: String = String::from_utf16_lossy(&buf[..chars]);
        // REG_SZ counts its own terminator in the returned length, so trim the NUL. Left in, it is
        // invisible in a log line and makes a build number compare unequal to itself.
        Some(s.trim_end_matches(char::from(0)).to_string())
    }
}

/// Writes one line, collapsing immediate repeats.
pub fn write(msg: &str) {
    eprintln!("{msg}");
    let mut guard = match STATE.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    let Some(st) = guard.as_mut() else { return };

    if msg == st.last {
        st.repeats += 1;
        // Powers of ten only: a failure that happens every frame at 60fps would otherwise write
        // 216,000 identical lines an hour and bury everything useful.
        if !st.repeats.is_power_of_two() {
            return;
        }
        if let Some(f) = st.file.as_mut() {
            let _ = writeln!(f, "  (repeated {} times)", st.repeats);
            let _ = f.flush();
        }
        return;
    }
    st.last = msg.to_string();
    st.repeats = 0;
    if let Some(f) = st.file.as_mut() {
        // Flushed every line on purpose. The interesting case is a process that is about to die, and
        // a buffered final message is exactly the one that gets lost.
        let _ = writeln!(f, "{msg}");
        let _ = f.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialises the tests that touch the process-wide logger.
    ///
    /// Cargo runs tests in parallel threads and this module's subject is a global, so without this
    /// one test's `write` resets the `last`/`repeats` pair another test is in the middle of
    /// measuring. That is exactly how the repeat-collapse test failed - intermittently, and on the
    /// test's own design rather than on the code under test.
    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn writing_before_init_does_not_panic() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        // Ordering matters: several subsystems log during construction, and some of them run before
        // main has had a chance to call init - notably the capture thread.
        write("a message with no log file behind it");
    }

    #[test]
    fn the_os_summary_names_a_windows_family() {
        let s = os_summary();
        assert!(s.starts_with("windows: "), "got {s:?}");
        assert!(
            s.contains("Windows 11") || s.contains("Windows 10 or older") || s.contains("unknown build"),
            "the summary must classify the OS, since the Widgets button only exists on 11: {s:?}"
        );
        assert!(s.contains("build "), "the build number is the actionable part: {s:?}");
    }

    #[test]
    fn repeated_messages_collapse_instead_of_filling_the_file() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("tbeq-log-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("t.log");
        {
            let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(State {
                file: std::fs::File::create(&p).ok(),
                last: String::new(),
                repeats: 0,
            });
        }
        for _ in 0..500 {
            write("the same failure every frame");
        }
        let body = std::fs::read_to_string(&p).unwrap_or_default();
        let lines = body.lines().count();
        assert!(
            lines < 20,
            "500 identical messages must collapse; wrote {lines} lines"
        );
        assert!(body.contains("repeated"), "the collapse must be visible in the log: {body:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
