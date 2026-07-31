mod dsp;
mod geom;
mod render;
mod win;

use anyhow::Result;
use render::canvas::{Canvas, Rgba};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

fn main() -> Result<()> {
    win::dpi::set_per_monitor_v2()?;
    // S_OK and S_FALSE both map to Ok; a real Err (e.g. RPC_E_CHANGED_MODE)
    // matters because UI Automation and WASAPI both require COM initialised.
    // CoInitializeEx returns HRESULT, not Result - `.ok()` is required and maps
    // S_OK/S_FALSE to Ok(()), leaving only genuine failures as Err.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        eprintln!("CoInitializeEx failed: {e}");
    }

    let overlay = win::overlay::Overlay::new()?;

    // 15 seconds of a flat VFD-ice panel so it can be looked at and screenshotted.
    for i in 0..150 {
        let widget = win::placement::find_widget_rect()?;
        let inputs = win::visibility::Inputs {
            widget,
            notification_state: win::placement::notification_state(),
            taskbar_visible: win::placement::taskbar_visible(),
        };

        if win::visibility::should_show(&inputs) {
            let r = inputs.widget.unwrap();
            let mut canvas = Canvas::new(r.w, r.h);
            canvas.rounded_rect(1, 2, r.w - 2, r.h - 4, 4, Rgba::from_hex("#040a0e", 0.55));
            canvas.fill_rect(r.w / 2 - 30, r.h / 2 - 8, 60, 16, Rgba::from_hex("#8fe4ff", 1.0));
            canvas.bloom(6, 0.8);
            overlay.show(r, &canvas)?;
            if i % 10 == 0 {
                println!("showing at {r:?}");
            }
        } else {
            overlay.hide()?;
        }

        overlay.pump_messages();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(())
}
