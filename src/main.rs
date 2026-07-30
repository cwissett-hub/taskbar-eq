mod win;

use anyhow::Result;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

fn main() -> Result<()> {
    // Order matters: DPI awareness before anything creates a window.
    win::dpi::set_per_monitor_v2()?;
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    println!("taskbar-eq: dpi + com initialised");
    Ok(())
}
