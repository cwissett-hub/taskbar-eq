mod dsp;
mod geom;
mod render;
mod themes;
mod win;

use anyhow::Result;
use dsp::ballistics::Smoother;
use dsp::gate::{Gate, GateConfig};
use render::canvas::Canvas;
use render::FrameData;
use win::capture::Frame;
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

    let theme = themes::builtin::vfd_ice();
    let mut family = render::family_for(&theme.family);
    let mut smoother = Smoother::new(theme.ballistics);
    let mut gate = Gate::new(GateConfig::default());
    let overlay = win::overlay::Overlay::new()?;
    let rx = win::capture::start();

    let mut latest = Frame::default();
    let mut rect_tick = 0u32;
    let mut rect = None;

    loop {
        while let Ok(f) = rx.try_recv() {
            latest = f;
        }

        // Re-discover the rect once a second - it moves with the weather text.
        if rect_tick == 0 {
            rect = win::placement::find_widget_rect().unwrap_or(None);
        }
        rect_tick = (rect_tick + 1) % 60;

        let inputs = win::visibility::Inputs {
            widget: rect,
            notification_state: win::placement::notification_state(),
            taskbar_visible: win::placement::taskbar_visible(),
        };

        let opacity = gate.update(latest.rms, 16);
        smoother.update(&latest.bands);

        if win::visibility::should_show(&inputs) && gate.is_visible() {
            let r = rect.unwrap();
            let mut canvas = Canvas::new(r.w, r.h);
            let data = FrameData {
                levels: *smoother.levels(),
                peaks: *smoother.peaks(),
                waveform: latest.waveform,
                rms_l: latest.rms_l,
                rms_r: latest.rms_r,
            };
            family.draw(&mut canvas, &theme, &data);
            let _ = opacity; // applied via theme alpha in Task 11
            overlay.show(r, &canvas)?;
        } else {
            overlay.hide()?;
        }

        overlay.pump_messages();
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
