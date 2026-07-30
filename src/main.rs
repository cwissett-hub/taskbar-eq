mod geom;
mod win;

use anyhow::Result;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

fn main() -> Result<()> {
    win::dpi::set_per_monitor_v2()?;
    // S_OK and S_FALSE both map to Ok; a real Err (e.g. RPC_E_CHANGED_MODE)
    // matters because UI Automation and WASAPI both require COM initialised.
    // CoInitializeEx returns HRESULT, not Result, so .ok() converts it first.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        eprintln!("CoInitializeEx failed: {e}");
    }

    for _ in 0..5 {
        let widget = win::placement::find_widget_rect()?;
        let inputs = win::visibility::Inputs {
            widget,
            notification_state: win::placement::notification_state(),
            taskbar_visible: win::placement::taskbar_visible(),
        };
        println!(
            "rect={:?} notif={} taskbar={} -> show={}",
            inputs.widget,
            inputs.notification_state,
            inputs.taskbar_visible,
            win::visibility::should_show(&inputs)
        );
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    Ok(())
}
