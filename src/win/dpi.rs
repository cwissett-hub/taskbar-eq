use anyhow::Result;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

/// MUST be called before any window is created. A DPI-unaware process reads the
/// taskbar as 1536x48 instead of the true 1920x60 at 125% scaling.
pub fn set_per_monitor_v2() -> Result<()> {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)? };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::HiDpi::{AreDpiAwarenessContextsEqual, GetThreadDpiAwarenessContext};

    #[test]
    fn sets_per_monitor_v2_awareness() {
        set_per_monitor_v2().expect("should set awareness");
        let ctx = unsafe { GetThreadDpiAwarenessContext() };

        // One assertion, and it is stronger than it looks. Both claims below were
        // measured on this machine on 2026-07-30, not taken from the docs:
        //
        //  - AreDpiAwarenessContextsEqual DOES discriminate v1 from v2 here. With
        //    PER_MONITOR_AWARE (v1) deliberately set instead, this returned false.
        //    So this genuinely pins v2, and a regression to v1 fails the test. (A
        //    review claimed the API treats v1 and v2 as equal; the experiment
        //    disproved that. Do not weaken this assertion on the strength of the
        //    documentation's ambiguity.)
        //
        //  - Do NOT be tempted to compare the raw values instead. It looks more
        //    precise and it does not work: DPI_AWARENESS_CONTEXT wraps an opaque
        //    *mut c_void, and GetThreadDpiAwarenessContext returned handle 0x22
        //    while the v2 sentinel is -4, so a raw comparison fails even when the
        //    awareness is correct. It cannot even be hex-formatted.
        let equal = unsafe {
            AreDpiAwarenessContextsEqual(ctx, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        };
        assert!(equal.as_bool(), "process must report per-monitor-v2 awareness");
    }
}
