use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, GetThreadDpiAwarenessContext, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    DPI_AWARENESS_CONTEXT_SYSTEM_AWARE,
};

/// The contexts worth having, best first.
fn candidates() -> [(windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT, &'static str); 3] {
    [
        (DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, "per-monitor-v2"),
        (DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE, "per-monitor-v1"),
        (DPI_AWARENESS_CONTEXT_SYSTEM_AWARE, "system-aware"),
    ]
}

/// MUST be called before any window is created. A DPI-unaware process reads the taskbar as 1536x48
/// instead of the true 1920x60 at 125% scaling, so every coordinate this app computes would be wrong.
///
/// Tries per-monitor-v2, then per-monitor-v1, then system-aware, and gives up without failing.
///
/// It used to be a single call whose error was propagated with `?` straight out of `main` - so on any
/// Windows older than 10 version 1703, where `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` does not
/// exist, the app exited on its FIRST line and printed the reason to a console that closed in the
/// same instant. Refusing to start over a positioning nicety is the wrong trade: an app that runs and
/// is slightly mispositioned can be reported and diagnosed, one that vanishes cannot.
///
/// Returns the name of the context actually in effect, for the log.
pub fn set_per_monitor_v2() -> String {
    for (ctx, name) in candidates() {
        if unsafe { SetProcessDpiAwarenessContext(ctx) }.is_ok() {
            return name.to_string();
        }
    }
    // Every attempt failed - but that does NOT mean the process is unaware. Windows rejects a SECOND
    // change with ERROR_ACCESS_DENIED, so a repeat call fails on all three while the awareness set by
    // the first call is still perfectly in force. Reporting "UNAWARE" there would put a falsehood in
    // the log, and this log exists precisely to be trusted about DPI on a machine nobody can inspect.
    // So ask what is actually in effect instead of inferring it from the failures.
    query()
}

/// The awareness context currently in effect, by name.
pub fn query() -> String {
    let ctx = unsafe { GetThreadDpiAwarenessContext() };
    for (candidate, name) in candidates() {
        if unsafe { AreDpiAwarenessContextsEqual(ctx, candidate) }.as_bool() {
            return format!("{name} (already set)");
        }
    }
    "UNAWARE - coordinates are virtualised, the overlay may be mispositioned".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serialises these tests. Awareness is PROCESS-wide and can only be set once, so with cargo
    /// running tests in parallel the two below race: whichever loses sees ERROR_ACCESS_DENIED on
    /// every candidate. That is what made this suite fail intermittently - the tests' own design,
    /// not the code.
    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn sets_per_monitor_v2_awareness() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let got = set_per_monitor_v2();
        // Either this call set it, or an earlier one in the same process did - both are per-monitor-v2
        // on a modern Windows. The fallbacks exist for older Windows and are asserted only by not
        // being reached here.
        assert!(
            got.starts_with("per-monitor-v2"),
            "expected per-monitor-v2 on a modern Windows, got {got:?}"
        );

        let ctx = unsafe { GetThreadDpiAwarenessContext() };
        // AreDpiAwarenessContextsEqual DOES discriminate v1 from v2, so this is a real check rather
        // than a tautology: GetThreadDpiAwarenessContext returns an opaque handle (observed as 0x22)
        // that is not the same pointer value as the named constant.
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
    fn a_repeat_call_reports_what_is_in_effect_rather_than_claiming_unaware() {
        // The bug this guards would have put a falsehood in the log on the machine least able to be
        // inspected: Windows rejects a second awareness change with ERROR_ACCESS_DENIED, so a naive
        // fallback chain walks past all three candidates and concludes the process is unaware, when
        // in fact the first call's setting is still in force.
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let _ = set_per_monitor_v2();
        let again = set_per_monitor_v2();
        assert!(
            !again.starts_with("UNAWARE"),
            "a repeat call must report the awareness actually in effect, got {again:?}"
        );
        assert!(again.starts_with("per-monitor-v2"), "got {again:?}");
    }
}
