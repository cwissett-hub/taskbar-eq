use crate::dsp::bands::{BandMapper, FFT_SIZE, HOP, NUM_BANDS};
use anyhow::Result;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};

/// Capacity of the capture->render handoff queue. Bounded so a stalled
/// consumer (a slow UI Automation call, a hung `pump_messages`, etc.) makes
/// the capture thread drop frames via `try_send` instead of growing the
/// queue without limit; frames are analysed at HOP/rate ~ 10.7ms apart, so 8
/// slots is a little under 100ms of slack - comfortably more than one
/// render tick, without letting a real stall accumulate unbounded memory.
const QUEUE_CAPACITY: usize = 8;

#[derive(Clone)]
pub struct Frame {
    pub bands: [f32; NUM_BANDS],
    pub waveform: [f32; 256],
    pub rms_l: f32,
    pub rms_r: f32,
    pub rms: f32,
}

impl Default for Frame {
    fn default() -> Self {
        Frame {
            bands: [0.0; NUM_BANDS],
            waveform: [0.0; 256],
            rms_l: 0.0,
            rms_r: 0.0,
            rms: 0.0,
        }
    }
}

/// Downmixes interleaved float frames to mono. Handles any channel count, since
/// the default endpoint on this machine is a virtual device and may not be stereo.
pub fn interleaved_to_mono(src: &[f32], channels: usize) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    src.chunks_exact(channels)
        .map(|f| f.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Per-channel RMS. For mono input both values are the same; the VU family needs
/// them separate.
pub fn channel_rms(src: &[f32], channels: usize) -> (f32, f32) {
    if channels == 0 {
        return (0.0, 0.0);
    }
    let frames = src.chunks_exact(channels);
    let n = frames.len().max(1) as f32;
    let (mut sl, mut sr) = (0.0f32, 0.0f32);
    for f in src.chunks_exact(channels) {
        let l = f[0];
        let r = if channels > 1 { f[1] } else { f[0] };
        sl += l * l;
        sr += r * r;
    }
    ((sl / n).sqrt(), (sr / n).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmixes_stereo_by_averaging() {
        let src = [1.0, 0.0, 0.5, 0.5, -1.0, 1.0];
        assert_eq!(interleaved_to_mono(&src, 2), vec![0.5, 0.5, 0.0]);
    }

    #[test]
    fn passes_mono_through_unchanged() {
        let src = [0.1, -0.2, 0.3];
        assert_eq!(interleaved_to_mono(&src, 1), vec![0.1, -0.2, 0.3]);
    }

    #[test]
    fn handles_surround_channel_counts() {
        // 6ch: all ones must average to one, not overflow.
        let src = vec![1.0f32; 12];
        assert_eq!(interleaved_to_mono(&src, 6), vec![1.0, 1.0]);
    }

    #[test]
    fn tolerates_a_truncated_final_frame() {
        // WASAPI can hand back a partial frame; must not panic.
        let src = [1.0, 1.0, 1.0];
        let out = interleaved_to_mono(&src, 2);
        assert_eq!(out.len(), 1, "partial trailing frame is dropped, not panicked on");
    }

    #[test]
    fn zero_channels_is_survivable() {
        assert!(interleaved_to_mono(&[1.0, 2.0], 0).is_empty());
        assert_eq!(channel_rms(&[1.0, 2.0], 0), (0.0, 0.0));
    }

    #[test]
    fn rms_of_silence_is_zero() {
        assert_eq!(channel_rms(&vec![0.0; 64], 2), (0.0, 0.0));
    }

    #[test]
    fn rms_separates_the_two_channels() {
        // Left full-scale DC, right silent.
        let src: Vec<f32> = (0..64).map(|i| if i % 2 == 0 { 1.0 } else { 0.0 }).collect();
        let (l, r) = channel_rms(&src, 2);
        assert!((l - 1.0).abs() < 1e-6, "left {l}");
        assert!(r.abs() < 1e-6, "right {r}");
    }
}

use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};

/// Reads a WASAPI-owned `PWSTR` into an owned `String` and frees the
/// CoTaskMem allocation backing it.
///
/// `IMMDevice::GetId` (and friends) document that the caller owns the
/// returned string and must free it with `CoTaskMemFree`. The brief's
/// original device-change poll called `GetId` on every spin of the capture
/// loop - many times a second - and never freed the result, leaking one
/// string allocation per poll for the life of the process. This wrapper
/// makes the free unconditional (even if the UTF-16 decode fails) so every
/// call site that turns a `PWSTR` into a `String` is leak-free.
///
/// # Safety
/// `s` must be a valid, null-terminated `PWSTR` owned by the caller (as
/// returned by a WASAPI/MMDevice API), not a borrowed or already-freed one.
unsafe fn pwstr_to_string_and_free(s: windows::core::PWSTR) -> String {
    let out = unsafe { s.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(s.0 as *const _)) };
    out
}

/// Spawns the capture thread. Returns a receiver of analysed frames.
/// The thread owns its own COM apartment and re-opens the endpoint if the
/// default device changes (the reference machine's default is a virtual device,
/// so this path is exercised in normal use, not just on unplug).
pub fn start() -> Receiver<Frame> {
    let (tx, rx) = sync_channel::<Frame>(QUEUE_CAPACITY);
    std::thread::spawn(move || {
        // CoInitializeEx returns HRESULT, not Result - `.ok()` is required.
        if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
            eprintln!("capture: CoInitializeEx failed: {e}");
        }
        loop {
            if let Err(e) = capture_loop(&tx) {
                eprintln!("capture: {e}; reopening in 1s");
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
            // try_send: a full queue just means the consumer is stalled and
            // will catch up later, which is fine for a reset marker - the
            // capture thread must never block waiting for it to drain.
            // Disconnected still means the main thread is gone for good.
            match tx.try_send(Frame::default()) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
    });
    rx
}

fn capture_loop(tx: &SyncSender<Frame>) -> Result<()> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let device_id = pwstr_to_string_and_free(device.GetId()?);
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let fmt = client.GetMixFormat()?;
        let channels = (*fmt).nChannels as usize;
        let rate = (*fmt).nSamplesPerSec as f32;

        // GetMixFormat's WAVEFORMATEX is also CoTaskMem-owned; free it as soon
        // as Initialize (which only reads through the pointer) is done with
        // it, rather than leaking it on every device reconnect.
        let init_result = client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            10_000_000,
            0,
            fmt,
            None,
        );
        CoTaskMemFree(Some(fmt as *const _));
        init_result?;

        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;

        let mut mapper = BandMapper::new(rate);
        let mut ring: Vec<f32> = Vec::with_capacity(FFT_SIZE * 2);
        let mut frame = Frame::default();

        loop {
            // Bail out and let start() reopen us if the default device changed.
            let current = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .and_then(|d| d.GetId())
                .map(|s| pwstr_to_string_and_free(s))
                .unwrap_or_default();
            if current != device_id {
                client.Stop()?;
                return Ok(());
            }

            let avail = capture.GetNextPacketSize()?;
            if avail == 0 {
                std::thread::sleep(std::time::Duration::from_millis(4));
                continue;
            }

            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frames = 0u32;
            let mut flags = 0u32;
            capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;

            let n = frames as usize * channels;
            let slice: &[f32] = if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                &[]
            } else {
                std::slice::from_raw_parts(data as *const f32, n)
            };

            let (l, r) = channel_rms(slice, channels);
            frame.rms_l = l;
            frame.rms_r = r;
            frame.rms = ((l * l + r * r) * 0.5).sqrt();
            ring.extend_from_slice(&interleaved_to_mono(slice, channels));

            capture.ReleaseBuffer(frames)?;

            while ring.len() >= FFT_SIZE {
                mapper.process(&ring[..FFT_SIZE], &mut frame.bands);
                for i in 0..256 {
                    frame.waveform[i] = ring[i * FFT_SIZE / 256];
                }
                // try_send, not send: the queue is bounded, so a stalled
                // consumer (slow UI Automation call, hung pump_messages,
                // etc.) must never block the capture thread. Full just
                // drops this frame - the analysed audio is still fresh next
                // time around - while Disconnected means the receiver
                // (main thread) is gone for good and we should stop.
                match tx.try_send(frame.clone()) {
                    Ok(()) | Err(TrySendError::Full(_)) => {}
                    Err(TrySendError::Disconnected(_)) => {
                        client.Stop()?;
                        return Ok(());
                    }
                }
                ring.drain(..HOP);
            }
            if ring.len() > FFT_SIZE * 2 {
                let keep = ring.len() - FFT_SIZE;
                ring.drain(..keep); // never let a stalled consumer grow the buffer
            }
        }
    }
}
