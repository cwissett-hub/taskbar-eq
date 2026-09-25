//! "What song is this?" - the on-demand Shazam identification.
//!
//! Runs on its own thread so neither the render tick nor the wndproc ever waits on the network.
//! `request()` is the only entry point and is safe to call from a message handler: it flips a flag
//! and spawns. Results reach the user through `banner()`, polled by the render tick the same way
//! `media::now_playing()` is, and through `songs::append`.
//!
//! Timing follows SongRec: post at 4 s, again at 8 s and 12 s if there was no match. Most songs
//! match on the first post; the later ones catch quiet intros.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::dsp::shazam_sig::{resample_to_16k, Signature};
use crate::net::shazam::{self, Outcome};

static BUSY: AtomicBool = AtomicBool::new(false);
/// (text, change counter, sticky). Same protocol as `media::NOW_PLAYING`.
static BANNER: Mutex<(String, u64, bool)> = Mutex::new((String::new(), 0, false));

const POST_AT_SECS: [f32; 3] = [4.0, 8.0, 12.0];
const OVERALL_DEADLINE: Duration = Duration::from_secs(14);

pub fn banner() -> (String, u64, bool) {
    BANNER.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn publish(text: &str, sticky: bool) {
    let mut g = BANNER.lock().unwrap_or_else(|e| e.into_inner());
    if g.0 != text || g.2 != sticky {
        g.0 = text.to_string();
        g.1 = g.1.wrapping_add(1);
        g.2 = sticky;
    }
}

fn try_begin() -> bool {
    BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed).is_ok()
}

fn end() {
    BUSY.store(false, Ordering::Release);
}

/// Polls `snapshot` until it holds `want_secs` of audio or `deadline` passes. Returns whatever
/// arrived, or `None` if nothing did - a dead capture thread must end as "no match", not a hang.
pub fn wait_for_samples(
    deadline: Duration,
    want_secs: f32,
    mut snapshot: impl FnMut() -> Option<(u32, Vec<f32>)>,
) -> Option<(u32, Vec<f32>)> {
    let t0 = Instant::now();
    let mut last: Option<(u32, Vec<f32>)> = None;
    loop {
        if let Some((rate, buf)) = snapshot() {
            if rate > 0 && buf.len() as f32 >= want_secs * rate as f32 {
                return Some((rate, buf));
            }
            if !buf.is_empty() && rate > 0 {
                last = Some((rate, buf));
            }
        }
        if t0.elapsed() >= deadline {
            return last;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Starts an identification, or does nothing if one is already running.
pub fn request() {
    if !try_begin() {
        crate::log::write("identify: already listening, press ignored");
        return;
    }
    if let Err(e) = std::thread::Builder::new().name("identify".into()).spawn(run) {
        crate::log::write(&format!("identify: could not start thread: {e}"));
        end();
    }
}

fn run() {
    crate::log::write("identify: listening");
    publish("listening...", true);
    crate::win::capture::record_start();
    let t0 = Instant::now();
    let mut result: Option<Outcome> = None;
    for (i, &at) in POST_AT_SECS.iter().enumerate() {
        let remaining = OVERALL_DEADLINE.saturating_sub(t0.elapsed());
        let Some((rate, buf)) =
            wait_for_samples(remaining, at, crate::win::capture::record_snapshot)
        else {
            break;
        };
        let secs = buf.len() as f32 / rate as f32;
        let mono16 = resample_to_16k(&buf, rate);
        let sig = Signature::from_mono_16k(&mono16);
        let peaks: usize = sig.bands.iter().map(|b| b.len()).sum();
        crate::log::write(&format!(
            "identify: post {} with {secs:.1}s @ {rate} Hz, {peaks} peaks",
            i + 1
        ));
        if peaks == 0 {
            // Silence. Posting it is a wasted call and a guaranteed no-match.
            result = Some(Outcome::NoMatch);
            if t0.elapsed() >= OVERALL_DEADLINE {
                break;
            }
            continue;
        }
        let out = shazam::recognize(&sig);
        let done = matches!(out, Outcome::Match(_));
        result = Some(out);
        if done || t0.elapsed() >= OVERALL_DEADLINE {
            break;
        }
    }
    crate::win::capture::record_stop();
    match result {
        Some(Outcome::Match(f)) => {
            crate::log::write(&format!(
                "identify: MATCH {} - {} (key {})",
                f.title, f.artist, f.shazam_key
            ));
            publish(&format!("{} - {}", f.title, f.artist), false);
            if let Err(e) = crate::songs::append(&f) {
                crate::log::write(&format!("identify: could not save the find: {e}"));
            }
        }
        Some(Outcome::NoMatch) => {
            crate::log::write("identify: no match");
            publish("no match", false);
        }
        Some(Outcome::Error(e)) => {
            crate::log::write(&format!("identify: failed: {e}"));
            publish("no match", false);
        }
        None => {
            crate::log::write("identify: no audio arrived from the capture thread");
            publish("no match", false);
        }
    }
    end();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn wait_gives_up_when_no_audio_ever_arrives() {
        let t0 = std::time::Instant::now();
        let got = wait_for_samples(Duration::from_millis(120), 1.0, || None);
        assert!(got.is_none());
        assert!(t0.elapsed() >= Duration::from_millis(100));
    }

    #[test]
    fn wait_returns_as_soon_as_enough_arrives() {
        let mut calls = 0;
        let got = wait_for_samples(Duration::from_secs(5), 0.5, || {
            calls += 1;
            Some((16_000, vec![0.0; 4_000 * calls]))
        });
        let (rate, buf) = got.unwrap();
        assert_eq!(rate, 16_000);
        assert!(buf.len() >= 8_000);
        assert!(calls <= 3);
    }

    #[test]
    fn wait_returns_what_it_has_at_the_deadline_if_any() {
        let got = wait_for_samples(Duration::from_millis(80), 100.0, || {
            Some((48_000, vec![0.0; 10]))
        });
        assert_eq!(got.unwrap().1.len(), 10);
    }

    /// The whole pipeline against the real loopback device and the real Shazam endpoint, with
    /// Spotify as the reference audio (started if paused, and paused again afterwards). Run with
    /// `cargo test live_identify -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_identify_via_shazam() {
        use crate::win::media::{self, Action, Backend, Status};
        let (id, status) = media::probe().expect("Spotify media session");
        eprintln!("spotify session {id:?}: {status:?}");
        let started = !matches!(status, Status::Playing);
        if started {
            media::send(Action::PlayPause, Backend::Session).expect("play");
            std::thread::sleep(Duration::from_secs(3));
        }
        let _rx = crate::win::capture::start();
        crate::win::capture::record_start();
        let got = wait_for_samples(Duration::from_secs(12), 6.0, crate::win::capture::record_snapshot);
        crate::win::capture::record_stop();
        if started {
            let _ = media::send(Action::PlayPause, Backend::Session);
        }
        let (rate, buf) = got.expect("no audio arrived from the capture thread");
        eprintln!("recorded {:.1}s @ {rate} Hz", buf.len() as f32 / rate as f32);
        let sig = Signature::from_mono_16k(&resample_to_16k(&buf, rate));
        let peaks: usize = sig.bands.iter().map(|b| b.len()).sum();
        eprintln!("signature: {peaks} peaks, {} ms", sig.sample_ms());
        match shazam::recognize(&sig) {
            Outcome::Match(f) => eprintln!(
                "MATCH: {} - {} [{}] key={} spotify={:?} apple={:?}",
                f.title, f.artist, f.album.as_deref().unwrap_or("-"), f.shazam_key, f.spotify_uri, f.apple_music_url
            ),
            Outcome::NoMatch => panic!("Shazam: no match"),
            Outcome::Error(e) => panic!("Shazam: {e}"),
        }
    }

    #[test]
    fn busy_flag_blocks_a_second_run_and_clears_after() {
        assert!(try_begin());
        assert!(!try_begin(), "second press while running must be refused");
        end();
        assert!(try_begin());
        end();
    }

    #[test]
    fn banner_publish_bumps_the_counter_only_on_change() {
        let (_, s0, _) = banner();
        publish("listening...", true);
        let (t, s1, sticky) = banner();
        assert_eq!(t, "listening...");
        assert!(sticky);
        assert_ne!(s0, s1);
        publish("listening...", true);
        assert_eq!(banner().1, s1);
        publish("no match", false);
        assert_ne!(banner().1, s1);
        assert!(!banner().2);
    }
}
