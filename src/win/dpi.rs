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

        // This asserts a per-monitor awareness context, and deliberately does not try
        // to distinguish v1 from v2. DPI_AWARENESS_CONTEXT is an opaque pseudo-handle
        // (*mut c_void), which is why AreDpiAwarenessContextsEqual exists at all -
        // comparing the raw value is meaningless, and this API documents v1 and v2 as
        // equal. That is acceptable here: v2's extra behaviour is non-client-area
        // scaling, child-window DPI notifications and dialog scaling, and this overlay
        // is a single top-level WS_POPUP with none of those. The regressions that would
        // actually break the overlay are UNAWARE and SYSTEM_AWARE - the 1.25x
        // virtualisation that misreports the taskbar as 1536x48 - and this assertion
        // catches both.
        let equal = unsafe {
            AreDpiAwarenessContextsEqual(ctx, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        };
        assert!(equal.as_bool(), "process must report per-monitor-v2 awareness");
    }
}
