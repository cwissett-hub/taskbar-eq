mod geom;
mod win;

use anyhow::Result;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

fn main() -> Result<()> {
    // Order matters: DPI awareness before anything creates a window.
    win::dpi::set_per_monitor_v2()?;
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        eprintln!("CoInitializeEx failed: {e}");
    }
    println!("taskbar-eq: dpi + com initialised");
    Ok(())
}
