use anyhow::Result;
use windows::Win32::UI::HiDpi::{
    GetThreadDpiAwarenessContext, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
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
    use windows::Win32::UI::HiDpi::AreDpiAwarenessContextsEqual;

    #[test]
    fn sets_per_monitor_v2_awareness() {
        set_per_monitor_v2().expect("should set awareness");
        let ctx = unsafe { GetThreadDpiAwarenessContext() };
        let equal = unsafe {
            AreDpiAwarenessContextsEqual(ctx, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        };
        assert!(equal.as_bool(), "process must report per-monitor-v2 awareness");
    }
}
