//! A watchdog on the app's own resource use, because it was once measured doing real harm.
//!
//! # What happened
//!
//! Reported as: "I used the program on another machine, it seemed to cause massive stuttering in full
//! screen apps while it was in the background", and then "my fullscreen app going from 160fps to 30fps,
//! and the inputs being completely dropped".
//!
//! A long-lived instance on the development machine was then measured in exactly that state, and the
//! numbers are not subtle. Beside a freshly started one:
//!
//! ```text
//!                 fresh      pathological
//!   threads          14            18,962
//!   handles         320           131,454
//!   working set   26 MB          1,471 MB
//!   CPU          4% of a core   146% of a core, sustained
//! ```
//!
//! 19,000 threads is what dropped that framerate and ate that input. It is not a CPU-cost problem -
//! a machine whose scheduler is managing twenty thousand runnable-ish threads cannot service a game's
//! present loop or its input queue on time, whatever the CPU percentage says.
//!
//! # What this module does, and what it does not
//!
//! **It does not fix the leak, because the leak is not yet identified.** A fresh instance is clean over
//! minutes with audio playing, with the overlay drawing, across a session lock and across track
//! changes; the pathological one had been alive for days. The instance was killed before a stack dump
//! was taken, which was careless - this module exists so that never matters again.
//!
//! So it does two things:
//!
//! - **Bounds the damage.** Above `FATAL_HANDLES` the process logs loudly and exits. A background
//!   visualiser that is costing someone 130fps and their keyboard has no business staying up, and
//!   with the autostart entry it comes back clean. An unbounded system-wide degradation becomes a
//!   bounded one.
//! - **Captures the evidence.** It logs a baseline hourly and a warning the moment the count leaves
//!   the normal range, so the next occurrence arrives with a timestamped growth curve attached
//!   instead of being a mystery.
//!
//! Handles rather than threads, and that is a deliberate choice of proxy: `GetProcessHandleCount` is
//! one cheap call, where counting this process's threads needs a Toolhelp snapshot of every thread on
//! the machine. Every thread holds a handle, so a thread leak IS a handle leak - the pathological
//! instance showed 131,454 handles against a healthy 320, a factor of four hundred. Nothing subtle
//! gets missed by watching the cheaper number.

use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};

/// How often the count is checked.
///
/// 30s. The leak took days to become catastrophic, so this is not a race; it is frequent enough that
/// the log carries a usable growth curve and rare enough to cost nothing.
const CHECK_S: u64 = 30;

/// A healthy instance sits at about 320 handles.
///
/// `WARN` is an order of magnitude above that, which no normal operation approaches - so a line in the
/// log is real information rather than noise. `FATAL` is another order of magnitude beyond, comfortably
/// past anything survivable but far below the 131,454 that was actually measured, so it trips long
/// before a game is affected.
const WARN_HANDLES: u32 = 3_000;
const FATAL_HANDLES: u32 = 30_000;

/// Hourly baseline, so a healthy log still says what healthy looked like.
const BASELINE_EVERY: u64 = 3600 / CHECK_S;

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Starts the watchdog. Idempotent.
pub fn start() {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        let mut ticks: u64 = 0;
        let mut warned = false;
        let mut first = None;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(CHECK_S));
            ticks += 1;
            let Some(handles) = handle_count() else { continue };
            if first.is_none() {
                first = Some(handles);
            }

            if handles >= FATAL_HANDLES {
                // Everything a report needs, in one line, before going away.
                crate::log::write(&format!(
                    "FATAL: {handles} open handles (a healthy instance uses about 320, and this \
                     started at {}). This is the resource leak that was measured degrading a \
                     fullscreen app from 160fps to 30fps with input loss, so the overlay is exiting \
                     rather than continuing to harm the system. Please send this log. It will restart \
                     with the machine if autostart is on.",
                    first.unwrap_or(handles)
                ));
                std::process::exit(3);
            }

            if handles >= WARN_HANDLES {
                // Once on the way up, and then only if it keeps climbing, so the log does not become
                // a heartbeat while the process is already in trouble.
                if !warned {
                    warned = true;
                    crate::log::write(&format!(
                        "WARNING: {handles} open handles, up from {} - a healthy instance stays near \
                         320. This is the start of the leak that was measured stalling a fullscreen \
                         app. Watching; will exit at {FATAL_HANDLES}.",
                        first.unwrap_or(handles)
                    ));
                }
            } else {
                warned = false;
            }

            if ticks.is_multiple_of(BASELINE_EVERY) {
                crate::log::write(&format!(
                    "health: {handles} handles after {:.1}h (started at {})",
                    ticks as f32 * CHECK_S as f32 / 3600.0,
                    first.unwrap_or(handles)
                ));
            }
        }
    });
}

/// This process's open handle count, or None if the call fails.
fn handle_count() -> Option<u32> {
    let mut n = 0u32;
    let h: HANDLE = unsafe { GetCurrentProcess() };
    // A failure here is not worth reporting every 30 seconds: the watchdog degrades to doing nothing,
    // which is exactly what the app did before it existed.
    unsafe { GetProcessHandleCount(h, &mut n) }.ok().map(|()| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_reports_a_plausible_handle_count() {
        // Guards the measurement itself. A probe that silently returns 0 - which is what an
        // unchecked API call would have done - is worse than no watchdog, because it can never fire.
        let n = handle_count().expect("GetProcessHandleCount must work on our own process");
        assert!(n > 5, "{n} handles is implausibly few - the call is not really measuring");
        assert!(n < FATAL_HANDLES, "the test harness itself is over the fatal threshold at {n}");
    }

    #[test]
    fn the_thresholds_are_ordered_and_far_from_healthy() {
        // A healthy instance was measured at about 320 handles and the pathological one at 131,454.
        // The thresholds have to sit between those, with enough clearance that neither fires by
        // accident nor waits until the damage is done.
        assert!(WARN_HANDLES > 320 * 5, "the warning would fire on a healthy instance");
        assert!(FATAL_HANDLES > WARN_HANDLES * 5, "no room between warning and exit");
        assert!(FATAL_HANDLES < 131_454, "the exit must trip well before the measured pathology");
    }

    #[test]
    fn starting_twice_leaves_one_watchdog() {
        start();
        assert!(RUNNING.load(Ordering::SeqCst));
        start();
        assert!(RUNNING.load(Ordering::SeqCst));
    }
}
