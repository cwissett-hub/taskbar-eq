//! Transport control for Spotify: play/pause, next track, previous track.
//!
//! TWO BACKENDS, because they fail in completely different ways and the user picks.
//!
//! `Backend::Session` addresses Spotify's own media session through WinRT
//! (`Windows.Media.Control`), identified by its `SourceAppUserModelId`. Measured on the target
//! machine: commands return in 2-10ms, work with Spotify unfocused or minimised, and the playback
//! status can be read back so a command can be CONFIRMED rather than merely sent.
//!
//! `Backend::MediaKeys` synthesises `VK_MEDIA_PLAY_PAUSE` and friends. Measured on the same machine:
//! 76ms, and whoever currently owns the key wins - which is not deterministic. Chrome has claimed
//! the media keys since v73, so a browser tab playing audio is enough to take them.
//!
//! WHY THE SESSION PATH IS THE DEFAULT, as a mechanism rather than a preference. A media keystroke
//! has no addressable target: it goes to the input queue and is routed by focus and by hook order,
//! and UIPI can drop injected input while reporting success. A GSMTC command is an RPC to the
//! system media broker addressed by app id, so focus, hook order and UIPI are all irrelevant.
//!
//! THE AUMID IS THE WHOLE TRICK, and it is why most third-party controllers are unreliable here.
//! The Microsoft Store build of Spotify reports `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify`; the
//! classic installer build reports `Spotify.exe`. Code that matches only `Spotify.exe` - which is
//! what nearly every sample does - finds no session at all on a Store install and silently falls
//! back to broadcasting media keys. The target machine runs the Store build.

use crate::log;

/// Which mechanism actually sends the command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Backend {
    /// Address Spotify's media session directly. The default, and the reliable one.
    #[default]
    Session,
    /// Synthesise the dedicated media keys and let the system route them.
    MediaKeys,
}

/// A transport command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    PlayPause,
    NextTrack,
    PrevTrack,
}

impl Action {
    /// The virtual-key code the media-key backend synthesises for this action.
    pub fn vk(self) -> u16 {
        match self {
            // VK_MEDIA_PLAY_PAUSE / VK_MEDIA_NEXT_TRACK / VK_MEDIA_PREV_TRACK.
            Action::PlayPause => 0xB3,
            Action::NextTrack => 0xB0,
            Action::PrevTrack => 0xB1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::PlayPause => "play/pause",
            Action::NextTrack => "next track",
            Action::PrevTrack => "previous track",
        }
    }
}

/// Whether a media session's app id is Spotify's.
///
/// Deliberately three tests rather than one equality, because the two Spotify builds identify
/// themselves completely differently and only the first is ever in the documentation:
///
/// - `spotify.exe` - the classic installer build, which reports its executable name.
/// - `...!spotify` - a packaged build, whose id is `<PackageFamilyName>!<AppId>`. The measured value
///   on the target machine is `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify`. The family name
///   carries a publisher hash, so it CANNOT be hardcoded; matching the `!Spotify` app id can.
/// - a bare `spotify` substring, as a last resort for a build that names itself some third way.
///
/// The substring test is what makes the other two redundant in practice, and they are kept anyway
/// because they are the cases that are actually understood - if the loose test ever has to be
/// tightened, the specific ones must keep working.
pub fn is_spotify(aumid: &str) -> bool {
    let a = aumid.to_ascii_lowercase();
    a == "spotify.exe" || a.ends_with("!spotify") || a.contains("spotify")
}

/// Sends `action` by synthesising the dedicated media key.
///
/// NOTE THE SELF-DEFEAT HAZARD this creates, proven by measurement rather than reasoned about: if
/// the app has claimed that same media key with `RegisterHotKey`, this synthesised press is captured
/// by our own hotkey instead of reaching Spotify. Measured both ways - holding the hotkey, a
/// synthesised play/pause left Spotify's status untouched; without it, Spotify acted in 76ms. So
/// whoever calls this while a media key is bound has to suspend that registration around the call.
pub fn send_media_key(action: Action) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    };
    // Same shape as `overlay::open_widgets_panel`, which is the app's existing synthesised-input
    // helper - one implementation pattern for injected keys rather than two.
    let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                ..Default::default()
            },
        },
    };
    let vk = VIRTUAL_KEY(action.vk());
    let seq = [key(vk, false), key(vk, true)];
    let sent = unsafe { SendInput(&seq, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != seq.len() {
        // A partial send is the UIPI case: injected input is refused when a
        // higher-integrity window has focus, and it reports how many events got through rather
        // than failing outright. Worth naming, because it is invisible otherwise and is one of the
        // reasons this backend is not the default.
        return Err(format!(
            "SendInput delivered {sent} of {} events for {} - injected input may be blocked by a \
             higher-integrity foreground window",
            seq.len(),
            action.label()
        ));
    }
    Ok(())
}

/// Reads the current playback status of Spotify's session, for confirming a command took effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Playing,
    Paused,
    /// A session exists but is in none of the states we act on (stopped, opening, closing).
    Other,
    /// No Spotify session at all - Spotify is not running, or has never played anything.
    NoSession,
}

type Session = windows::Media::Control::GlobalSystemMediaTransportControlsSession;

/// Finds Spotify's media session, or `None` if it has none right now.
///
/// Deliberately NOT `GetCurrentSession()`. That returns whichever session the system currently
/// considers foremost, by an undocumented rule, and it changes underneath you - so on a machine with
/// a browser tab also holding a session, a transport command could land on the browser. Enumerating
/// and matching by app id is the only way to be sure which app is being commanded.
fn find_session() -> Result<Option<(String, Session)>, String> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as Mgr;
    let mgr = Mgr::RequestAsync()
        .map_err(|e| format!("RequestAsync: {e}"))?
        // `join()`, not `get()`: windows-future 0.3.2 blocks with `join`. It waits with no timeout,
        // which is safe HERE because this only ever runs on the media thread, never the render
        // thread - a wedged broker can cost this one expendable thread, never a frame.
        .join()
        .map_err(|e| format!("RequestAsync.join: {e}"))?;
    let sessions = mgr.GetSessions().map_err(|e| format!("GetSessions: {e}"))?;
    for s in sessions {
        let id = s
            .SourceAppUserModelId()
            .map_err(|e| format!("SourceAppUserModelId: {e}"))?
            .to_string();
        if is_spotify(&id) {
            return Ok(Some((id, s)));
        }
    }
    Ok(None)
}

fn status_of(s: &Session) -> Result<Status, String> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus as PS;
    let info = s.GetPlaybackInfo().map_err(|e| format!("GetPlaybackInfo: {e}"))?;
    let st = info.PlaybackStatus().map_err(|e| format!("PlaybackStatus: {e}"))?;
    Ok(if st == PS::Playing {
        Status::Playing
    } else if st == PS::Paused {
        Status::Paused
    } else {
        Status::Other
    })
}

/// "Title - Artist", or just the title when there is no artist. What the banner shows.
fn title_and_artist(s: &Session) -> String {
    match s.TryGetMediaPropertiesAsync().and_then(|op| op.join()) {
        Ok(p) => {
            let t = p.Title().map(|t| t.to_string()).unwrap_or_default();
            let a = p.Artist().map(|a| a.to_string()).unwrap_or_default();
            match (t.trim(), a.trim()) {
                ("", _) => String::new(),
                (t, "") => t.to_string(),
                (t, a) => format!("{t} - {a}"),
            }
        }
        Err(_) => String::new(),
    }
}

/// The currently loaded track's title, used only to confirm that a skip actually skipped.
fn title_of(s: &Session) -> String {
    match s.TryGetMediaPropertiesAsync().and_then(|op| op.join()) {
        Ok(p) => p.Title().map(|t| t.to_string()).unwrap_or_default(),
        Err(_) => String::new(),
    }
}

/// One-shot probe of Spotify's session, used by `--diagnose` and by the live tests.
///
/// Returns the app id it matched alongside the status, because on a support request the AUMID is the
/// single most useful line in the log: it distinguishes "Spotify is not running" from "Spotify is
/// running but identifies itself in a way this build does not recognise".
pub fn probe() -> Result<(String, Status), String> {
    match find_session()? {
        Some((id, s)) => Ok((id, status_of(&s)?)),
        None => Ok((String::new(), Status::NoSession)),
    }
}

/// How long to watch for a command to visibly take effect before calling it unconfirmed.
const CONFIRM_MS: u64 = 750;
/// How often to re-read while watching.
const CONFIRM_POLL_MS: u64 = 50;

/// Sends `action` through Spotify's media session and reports whether it visibly took effect.
///
/// WHY THE RETURNED BOOL IS NOT THE ANSWER, and this is the subtlest thing in the module: the
/// `Try*Async` calls return `true` for DELIVERED, not for ACTED UPON. Measured against this very
/// session, six commands returned `true` while the session's own capability flags said the command
/// was not even enabled. So success is defined as an OBSERVED change - the playback status flipping
/// for play/pause, the track title changing for a skip.
///
/// AND IT IS NEVER RETRIED. A skip that appears to have failed may simply have been slow, and a
/// second attempt would skip two tracks. An unconfirmed command is logged and dropped, because a
/// silent double-skip is a worse failure than a missed press.
fn send_via_session(action: Action) -> Result<(), String> {
    let Some((_, s)) = find_session()? else {
        return Err("no Spotify session - is Spotify running?".into());
    };
    let before_status = status_of(&s)?;
    // Only fetched for a skip: it costs an async round trip, and play/pause is confirmed by the
    // status alone.
    let before_title = if action == Action::PlayPause { String::new() } else { title_of(&s) };

    let op = match action {
        Action::PlayPause => s.TryTogglePlayPauseAsync(),
        Action::NextTrack => s.TrySkipNextAsync(),
        Action::PrevTrack => s.TrySkipPreviousAsync(),
    }
    .map_err(|e| format!("{}: {e}", action.label()))?;
    let delivered = op.join().map_err(|e| format!("{}: join: {e}", action.label()))?;

    // THE SESSION HANDLE MUST BE RE-RESOLVED ON EVERY POLL, and this was found by running it
    // against the real Spotify rather than by reading the docs.
    //
    // Re-reading `GetPlaybackInfo()` on the handle held from before the command returns a STALE
    // snapshot forever on this thread. The command itself had plainly worked - a fresh `probe()`
    // straight afterwards reported `Playing` - but the confirmation loop watched the old handle,
    // never saw a change, and reported every single command as unconfirmed. Shipped, that would have
    // filled the log with false failures for commands that all succeeded, which is precisely the
    // "it feels unreliable" symptom this feature exists to remove.
    //
    // The reason it is not obvious: the same held-handle check DOES observe the change from
    // PowerShell, which runs STA and pumps messages, so the proxy gets updated. This thread is MTA
    // and pumps nothing, so nothing refreshes it. Re-resolving is the cheap, correct fix - it costs
    // one `RequestAsync` per 50ms poll on a thread that has nothing else to do.
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(CONFIRM_MS) {
        std::thread::sleep(std::time::Duration::from_millis(CONFIRM_POLL_MS));
        let Ok(Some((_, fresh))) = find_session() else {
            continue;
        };
        let changed = match action {
            Action::PlayPause => status_of(&fresh).map(|now| now != before_status).unwrap_or(false),
            _ => {
                let now = title_of(&fresh);
                !now.is_empty() && now != before_title
            }
        };
        if changed {
            return Ok(());
        }
    }
    Err(format!(
        "{} was delivered={delivered} but nothing changed within {CONFIRM_MS}ms; not retrying, \
         because a repeated skip would skip twice",
        action.label()
    ))
}

/// Sends `action` using `backend`.
///
/// The backend is chosen ONCE, by the user, and never switched in response to a failed command. An
/// automatic fallback would be actively dangerous here: a skip that was merely slow to confirm
/// would be followed by a media key, and the track would advance twice.
pub fn send(action: Action, backend: Backend) -> Result<(), String> {
    match backend {
        Backend::Session => send_via_session(action),
        Backend::MediaKeys => send_media_key(action),
    }
}

/// The track currently loaded, and a counter that changes whenever the track does.
///
/// A counter rather than a callback: the render loop reads this once per tick and compares, which
/// needs no synchronisation beyond the mutex and cannot fire from the wrong thread.
static NOW_PLAYING: std::sync::Mutex<(String, u64)> = std::sync::Mutex::new((String::new(), 0));

/// The current track and its change counter.
pub fn now_playing() -> (String, u64) {
    NOW_PLAYING.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn publish(title: &str) {
    let mut g = NOW_PLAYING.lock().unwrap_or_else(|e| e.into_inner());
    if g.0 != title {
        g.0 = title.to_string();
        g.1 = g.1.wrapping_add(1);
    }
}

/// How often the media thread looks for a track change while idle.
///
/// 400ms is well inside the time it takes to notice a new track starting, and the poll is one cheap
/// cross-process read - measured at well under a millisecond once the session is resolved.
const POLL_MS: u64 = 400;

/// A handle for asking the media thread to do something. Cheap to clone.
#[derive(Clone)]
pub struct Handle {
    tx: std::sync::mpsc::Sender<(Action, Backend)>,
}

impl Handle {
    /// Queues a command. Never blocks and never fails loudly.
    ///
    /// The caller is a window procedure handling a hotkey press, which must return promptly and has
    /// nowhere to report an error to. Every outcome is logged by the thread instead.
    pub fn send(&self, action: Action, backend: Backend) {
        if self.tx.send((action, backend)).is_err() {
            // The thread is gone, which means its WinRT activation failed at startup and it logged
            // why. Say so once per press rather than silently doing nothing, because "I pressed the
            // key and nothing happened" with an empty log is the failure mode this whole feature
            // exists to avoid.
            log::write(&format!(
                "{}: the media thread is not running, so the command was dropped",
                action.label()
            ));
        }
    }
}

/// Starts the media thread and returns a handle to it.
///
/// EVERYTHING WinRT HAPPENS ON THIS THREAD, and that is the design's load-bearing constraint rather
/// than tidiness. `join()` on a WinRT async operation waits on an event with no timeout, so a wedged
/// media broker blocks its caller forever. On this thread that costs one expendable thread and the
/// visualiser keeps running; on the render thread it would be a hung app. The confirmation watch
/// also sleeps for up to 750ms per command, which is by itself far too long to spend in a frame.
///
/// Mirrors `win::capture::start` deliberately, including doing its own `CoInitializeEx` - a new
/// thread starts with no apartment regardless of what `main` did.
pub fn start() -> Handle {
    let (tx, rx) = std::sync::mpsc::channel::<(Action, Backend)>();
    std::thread::spawn(move || {
        // MTA, matching the rest of the app (src/main.rs). WinRT's own `RoInitialize`
        // multithreaded mode is documented as equivalent, and GSMTC's classes are agile, so this is
        // compatible with the UI Automation and WASAPI use elsewhere. Logged, never fatal.
        if let Err(e) = unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            )
        }
        .ok()
        {
            log::write(&format!("media: CoInitializeEx failed: {e}"));
        }
        loop {
            // `recv_timeout`, not `recv`: the thread also has to NOTICE things, not only be told
            // them. Without a timeout it slept until the next hotkey, so a track change was invisible
            // until the user pressed something.
            match rx.recv_timeout(std::time::Duration::from_millis(POLL_MS)) {
                Ok((action, backend)) => {
                    let t0 = std::time::Instant::now();
                    match send(action, backend) {
                        Ok(()) => log::write(&format!(
                            "{} confirmed in {}ms via {backend:?}",
                            action.label(),
                            t0.elapsed().as_millis()
                        )),
                        Err(e) => log::write(&format!("{}: {e}", action.label())),
                    }
                    // Straight after a command the track has very likely just changed.
                    if let Ok(Some((_, s))) = find_session() {
                        publish(&title_and_artist(&s));
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    match find_session() {
                        Ok(Some((_, s))) => publish(&title_and_artist(&s)),
                        // Nothing playing: clear it, so a banner cannot be re-shown for a track that
                        // stopped ages ago when playback resumes.
                        Ok(None) => publish(""),
                        Err(_) => {}
                    }
                }
                // The sender is gone, i.e. the app is shutting down.
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    Handle { tx }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_real_spotify_app_ids_are_recognised_and_other_players_are_not() {
        // The Store id is the value MEASURED on the target machine, not a guess. Keeping it here
        // verbatim is the point of the test: the family-name hash is machine-specific, so a matcher
        // that accidentally became an equality check against it would pass on the dev box and fail
        // on every other install.
        assert!(is_spotify("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"));
        assert!(is_spotify("Spotify.exe"));
        assert!(is_spotify("spotify.exe"));
        // Case must not matter: the id's casing is the app's choice, not a contract.
        assert!(is_spotify("SPOTIFYAB.SPOTIFYMUSIC_ZPDNEKDRZREA0!SPOTIFY"));

        // Real ids of other media apps that commonly hold a session at the same time. If any of
        // these matched, the app would happily send Spotify's transport commands to a browser.
        for other in [
            "Chrome",
            "chrome.exe",
            "msedge.exe",
            "Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic",
            "vlc.exe",
            "Microsoft.Teams_8wekyb3d8bbwe!MSTeams",
            "firefox.exe",
        ] {
            assert!(!is_spotify(other), "{other} must not be taken for Spotify");
        }
    }

    #[test]
    fn the_three_actions_map_to_the_three_distinct_media_keys() {
        // A copy-paste slip here would silently bind two actions to the same key, which is exactly
        // the kind of defect that looks like "the app is unreliable" rather than like a bug.
        let vks: Vec<u16> =
            [Action::PlayPause, Action::NextTrack, Action::PrevTrack].iter().map(|a| a.vk()).collect();
        assert_eq!(vks, vec![0xB3, 0xB0, 0xB1]);
        let mut sorted = vks.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 3, "two actions share a virtual key: {vks:?}");
    }

    #[test]
    fn the_session_backend_is_the_default() {
        // Not cosmetic. The default decides what a config file with no `backend` key gets, which is
        // every existing install - and the media-key backend is the unreliable one.
        assert_eq!(Backend::default(), Backend::Session);
    }

    /// Live end-to-end check of the session backend: play, confirm, pause, confirm.
    ///
    /// Self-reversing on purpose - it leaves playback exactly as it found it - and ignored, because
    /// it commands the real Spotify. Run it only when nobody is listening.
    ///
    /// Run: cargo test --release live_session_round_trip -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_session_round_trip_toggles_and_restores_playback() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            );
        }
        let (id, before) = probe().expect("probe should succeed");
        println!("session {id:?} starts {before:?}");
        assert!(before != Status::NoSession, "no Spotify session - start Spotify first");

        for pass in 1..=2 {
            let t0 = std::time::Instant::now();
            let r = send(Action::PlayPause, Backend::Session);
            let st = probe().map(|(_, s)| s).unwrap_or(Status::NoSession);
            println!("  toggle {pass}: {:?} in {}ms -> {st:?}", r, t0.elapsed().as_millis());
            assert!(r.is_ok(), "toggle {pass} was not confirmed: {r:?}");
        }
        let (_, after) = probe().expect("probe should succeed");
        assert_eq!(after, before, "playback state must be left exactly as it was found");
        println!("restored to {after:?}");
    }

    /// Live end-to-end check of a SKIP, which is the command whose confirmation cannot use the
    /// playback status. Returns to the original track. Ignored for the same reason.
    ///
    /// Run: cargo test --release live_skip_round_trip -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_skip_round_trip_changes_the_track_and_comes_back() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            );
        }
        let Some((_, s)) = find_session().expect("find_session") else {
            println!("no Spotify session - skipping");
            return;
        };
        let first = title_of(&s);
        println!("starting track: {first:?}");
        let t0 = std::time::Instant::now();
        let r = send(Action::NextTrack, Backend::Session);
        println!("  next: {:?} in {}ms -> {:?}", r, t0.elapsed().as_millis(), title_of(&s));
        assert!(r.is_ok(), "next was not confirmed: {r:?}");
        // Two previous presses: the first restarts the current track, the second goes back one.
        // That is Spotify's own behaviour and not something this code can change.
        for _ in 0..2 {
            let _ = send(Action::PrevTrack, Backend::Session);
        }
        std::thread::sleep(std::time::Duration::from_millis(600));
        println!("ended on: {:?} (started on {first:?})", title_of(&s));
    }

    /// Live probe against whatever Spotify is doing right now. Ignored: it needs a real session.
    ///
    /// Run: cargo test --release live_probe -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_probe_finds_the_running_spotify_session() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            );
        }
        match probe() {
            Ok((id, st)) => println!("session id={id:?} status={st:?}"),
            Err(e) => println!("probe failed: {e}"),
        }
    }
}
