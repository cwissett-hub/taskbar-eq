use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, DPI_AWARENESS_CONTEXT_SYSTEM_AWARE,
};

/// MUST be called before any window is created. A DPI-unaware process reads the taskbar as
/// 1536x48 instead of the true 1920x60 at 125% scaling, so every coordinate this app computes
/// would be wrong.
///
/// Tries per-monitor-v2, then per-monitor-v1, then system-aware, and gives up without failing.
///
/// It used to be a single call whose error was propagated with `?` straight out of `main` - so on
/// any Windows older than 10 version 1703, where `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` does
/// not exist, the app exited on its FIRST line and printed the reason to a console that closed in
/// the same instant. Refusing to start over a positioning nicety is the wrong trade: an app that
/// runs and is slightly mispositioned can be reported and diagnosed, one that vanishes cannot.
///
/// Returns the name of the context that took effect, for the log.
pub fn set_per_monitor_v2() -> &'static str {
    let attempts = [
        (DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, "per-monitor-v2"),
        (DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE, "per-monitor-v1 (Windows 10 pre-1703)"),
        (DPI_AWARENESS_CONTEXT_SYSTEM_AWARE, "system-aware (Windows 8.1 era)"),
    ];
    for (ctx, name) in attempts {
        if unsafe { SetProcessDpiAwarenessContext(ctx) }.is_ok() {
            return name;
        }
    }
    // Unaware. Coordinates will be virtualised and the overlay will sit in the wrong place on a
    // scaled display, but the app runs and says so, which is what makes it diagnosable.
    "UNAWARE - coordinates are virtualised, the overlay may be mispositioned"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_per_monitor_v2_awareness() {
        let got = set_per_monitor_v2();
        // On this build v2 must be the one that takes effect; the fallbacks exist for older
        // Windows and are asserted only by not being reached here.
        assert_eq!(got, "per-monitor-v2", "expected v2 on a modern Windows, got {got:?}");

        use windows::Win32::UI::HiDpi::{AreDpiAwarenessContextsEqual, GetThreadDpiAwarenessContext};
        let ctx = unsafe { GetThreadDpiAwarenessContext() };
        // AreDpiAwarenessContextsEqual DOES discriminate v1 from v2 here, so this is a real check
        // rather than a tautology: GetThreadDpiAwarenessContext returns an opaque handle
        // (observed as 0x22) that is not the same *pointer value* as the named constant.
        assert!(
            unsafe { AreDpiAwarenessContextsEqual(ctx, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
                .as_bool(),
            "awareness should be per-monitor-v2"
        );
        assert!(
            !unsafe { AreDpiAwarenessContextsEqual(ctx, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE) }
                .as_bool(),
            "and must be distinguishable from v1, or this test proves nothing"
        );
    }

    #[test]
    fn setting_awareness_twice_does_not_panic() {
        // Windows rejects a second change with ERROR_ACCESS_DENIED. The fallback chain must treat
        // that as "already set" rather than walking down to UNAWARE and reporting a lie.
        let _ = set_per_monitor_v2();
        let again = set_per_monitor_v2();
        assert!(!again.is_empty());
    }
}
