use crate::dsp::bands::{BandMapper, FFT_SIZE, HOP, NUM_BANDS};
use anyhow::Result;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Capacity of the capture->render handoff queue. Bounded so a stalled
/// consumer (a slow UI Automation call, a hung `pump_messages`, etc.) makes
/// the capture thread evict old frames instead of growing the queue without
/// limit; frames are produced roughly once per WASAPI packet (~10ms), so 8
/// slots is a little under 100ms of slack - comfortably more than one
/// render tick, without letting a real stall accumulate unbounded memory.
const QUEUE_CAPACITY: usize = 8;

/// Bounded single-producer/single-consumer handoff of analysed [`Frame`]s
/// from the capture thread to the render loop.
///
/// `std::sync::mpsc::sync_channel`'s `SyncSender` cannot give the newest
/// frame priority over stale ones: once its bounded queue is full, the only
/// thing a sender can do is fail the send it was attempting
/// (`TrySendError::Full`) - there is no API for a sender to reach into the
/// queue and evict something, only a receiver can pop. So under a burst,
/// the old code's only option on a full queue was to drop the frame it was
/// holding - the FRESHEST one just analysed - while older, already-queued
/// frames survived to be shown later, stale. This tiny ring buffer keeps
/// the eviction power on the producer side instead: on a full queue it
/// evicts the single oldest frame before pushing, so the newest analysis
/// always wins.
struct Inner {
    frames: Mutex<VecDeque<Frame>>,
    capacity: usize,
}

pub struct FrameSender {
    inner: Arc<Inner>,
}

pub struct FrameReceiver {
    inner: Arc<Inner>,
}

fn frame_channel(capacity: usize) -> (FrameSender, FrameReceiver) {
    let inner = Arc::new(Inner { frames: Mutex::new(VecDeque::with_capacity(capacity)), capacity });
    (FrameSender { inner: inner.clone() }, FrameReceiver { inner })
}

impl FrameSender {
    /// Never blocks waiting for the consumer to drain: the critical section
    /// is a bounded, in-memory `VecDeque` push/pop with no I/O, so the only
    /// way this could block is if the *receiver* held the lock across a
    /// slow operation - and `FrameReceiver::try_recv` never does that, it
    /// only ever holds the lock for the pop itself. A slow UI Automation
    /// call or a hung `pump_messages` on the render side happens entirely
    /// outside that lock, so it can never stall the capture thread here -
    /// preserving the guarantee the original `try_send`-based code relied
    /// on.
    ///
    /// If the queue is full, evicts the single oldest frame before pushing
    /// - atomically, under the one lock acquisition, so this can never
    /// fail the way a separate check-then-send could race - so the newest
    /// frame always gets in rather than being dropped in favour of frames
    /// the consumer hasn't looked at yet.
    ///
    /// Returns `false` once the receiver has been dropped (the render
    /// thread/process is gone for good), mirroring
    /// `TrySendError::Disconnected`, so the caller can stop capturing
    /// instead of pushing into the void forever.
    pub fn send_freshest(&self, frame: Frame) -> bool {
        if Arc::strong_count(&self.inner) < 2 {
            return false;
        }
        let mut q = self.inner.frames.lock().unwrap();
        if q.len() >= self.inner.capacity {
            q.pop_front(); // evict the oldest stale frame - never the newest
        }
        q.push_back(frame);
        true
    }
}

impl FrameReceiver {
    pub fn try_recv(&self) -> Result<Frame, TryRecvError> {
        let mut q = self.inner.frames.lock().unwrap();
        match q.pop_front() {
            Some(f) => Ok(f),
            // strong_count < 2 means the capture thread's FrameSender is
            // gone (thread panicked/exited) and nothing will ever be
            // queued again, distinct from "just momentarily empty".
            None if Arc::strong_count(&self.inner) < 2 => Err(TryRecvError::Disconnected),
            None => Err(TryRecvError::Empty),
        }
    }
}

/// Mirrors `std::sync::mpsc::TryRecvError`'s shape so callers can pattern
/// match the same way; the render loop's `while let Ok(f) = rx.try_recv()`
/// only inspects `Ok`, but both `Err` arms are real and distinguishable.
pub enum TryRecvError {
    Empty,
    Disconnected,
}

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
pub fn start() -> FrameReceiver {
    let (tx, rx) = frame_channel(QUEUE_CAPACITY);
    std::thread::spawn(move || {
        // CoInitializeEx returns HRESULT, not Result - `.ok()` is required.
        if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
            eprintln!("capture: CoInitializeEx failed: {e}");
        }
        let mut fails: u64 = 0;
        loop {
            if let Err(e) = capture_loop(&tx) {
                // The LOG, not stderr. This is a windows-subsystem binary, so `eprintln!` goes
                // nowhere at all - which means a retry storm after a sleep/resume or a device change
                // was completely invisible, and "the meter stopped reacting" would have arrived with
                // an empty log. Rate-limited, because the retry is once a second and an unbounded
                // error line per attempt would itself fill the disk over days.
                if fails == 0 || fails.is_multiple_of(60) {
                    crate::log::write(&format!(
                        "capture: {e}; reopening in 1s (attempt {})",
                        fails + 1
                    ));
                }
                fails += 1;
            } else {
                fails = 0;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
            // send_freshest never blocks waiting for the consumer to drain
            // (see FrameSender::send_freshest); it only reports `false`
            // once the receiver (main thread) is gone for good, which is
            // when this reset marker should stop being sent too.
            if !tx.send_freshest(Frame::default()) {
                return;
            }
        }
    });
    rx
}

/// Requested WASAPI buffer duration, in 100ns units - this is a latency
/// CEILING we're asking the audio engine for, not a capacity target: the
/// old 1s (10_000_000) request alone permitted up to a full second of
/// undetected backlog to build up inside WASAPI itself, before the ring
/// buffer or queue even entered the picture. 200ms is comfortably above
/// any real device's minimum period.
const REQUESTED_BUFFER_DURATION_HNS: i64 = 2_000_000;
/// Fallback if a device rejects the 200ms request outright - the previous,
/// known-good behaviour - so a stricter latency ask never stops capture
/// from starting at all.
const FALLBACK_BUFFER_DURATION_HNS: i64 = 10_000_000;

fn capture_loop(tx: &FrameSender) -> Result<()> {
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
        let mut init_result = client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            REQUESTED_BUFFER_DURATION_HNS,
            0,
            fmt,
            None,
        );
        if let Err(e) = &init_result {
            eprintln!(
                "capture: device rejected a {}ms buffer ({e}); falling back to {}ms",
                REQUESTED_BUFFER_DURATION_HNS / 10_000,
                FALLBACK_BUFFER_DURATION_HNS / 10_000
            );
            init_result = client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                FALLBACK_BUFFER_DURATION_HNS,
                0,
                fmt,
                None,
            );
        }
        CoTaskMemFree(Some(fmt as *const _));
        init_result?;

        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;

        let mut mapper = BandMapper::new(rate);
        // Capacity hint matches the tight FFT_SIZE + HOP bound enforced
        // below, plus one packet's worth of headroom so a typical packet
        // doesn't force an immediate reallocation.
        let mut ring: Vec<f32> = Vec::with_capacity(FFT_SIZE + HOP * 2);
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

            if ring.len() >= FFT_SIZE {
                // Analyse the NEWEST FFT_SIZE samples - the tail of the
                // ring - never the oldest. The old code analysed
                // `ring[..FFT_SIZE]`, the OLDEST window, while the ring was
                // permitted to hold up to FFT_SIZE * 2 samples; that let
                // the display persistently lag the music by up to 4096
                // samples (~85ms at 48kHz) and never catch up within a
                // burst, since every additional buffered sample pushed the
                // "oldest window" further into the past. Always reading
                // from the tail means the reported spectrum is always the
                // most recent audio available, regardless of how much
                // backlog is sitting in front of it.
                let start = ring.len() - FFT_SIZE;
                mapper.process(&ring[start..], &mut frame.bands);
                for i in 0..256 {
                    frame.waveform[i] = ring[start + i * FFT_SIZE / 256];
                }
                // send_freshest never blocks the capture thread waiting for
                // the consumer to drain (see FrameSender::send_freshest for
                // why), and it never drops the frame just analysed in
                // favour of stale queued ones - it's the newest data, so it
                // always gets in, evicting the oldest queued frame instead
                // if the queue is full. `false` means the receiver (main
                // thread) is gone for good, same as the old
                // `TrySendError::Disconnected`.
                if !tx.send_freshest(frame.clone()) {
                    client.Stop()?;
                    return Ok(());
                }
            }
            // Keep the ring tight: never retain more than FFT_SIZE + HOP
            // samples, trimming from the FRONT (oldest) only - the newest
            // samples must never be discarded. This bounds how much stale
            // backlog can sit in the ring to about one HOP (~10.7ms at
            // 48kHz) instead of the old FFT_SIZE * 2 (~85ms) bound. Since
            // analysis above always reads from the tail, this trim is
            // purely a memory bound, not a latency one - but keeping it
            // tight also means a burst can't leave megabytes of stale audio
            // sitting around between device reconnects.
            if ring.len() > FFT_SIZE + HOP {
                let excess = ring.len() - (FFT_SIZE + HOP);
                ring.drain(..excess);
            }
        }
    }
}
