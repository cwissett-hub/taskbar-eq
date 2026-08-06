//! Registering the chords with Windows, and routing `WM_HOTKEY` to the media thread.
//!
//! `RegisterHotKey` only, no low-level keyboard hook. A hook's one unique benefit is seeing a chord
//! another app already owns, and it costs a great deal for it: it must not block or Windows silently
//! removes it via `LowLevelHooksTimeout` (leaving the feature dead with no signal), it degrades
//! system-wide input latency, and it looks like a keylogger to security software. Conflicts are
//! surfaced honestly instead - see `Outcome::Taken`.
//!
//! WHY THE TRAY WINDOW OWNS THE REGISTRATION, and this is a trap worth naming: `RegisterHotKey`
//! accepts a null `hwnd`, which posts `WM_HOTKEY` as a THREAD message. Both of this app's pumps call
//! `PeekMessageW(.., None, ..)`, which does retrieve thread messages, and then hand everything to
//! `DispatchMessageW` - which cannot route a message with no window anywhere. The hotkey would be
//! dequeued and silently dropped: pressing the key would do nothing, with nothing in the log,
//! which is indistinguishable from the unreliable tool this feature exists to replace. The tray
//! window is the right owner because it is created once and never destroyed for the life of the
//! process, unlike the overlay which is shown and hidden every frame.

use super::hotkey::{Advisory, Chord, Reject};
use super::media::{self, Action, Backend};
use crate::log;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};

/// `HRESULT_FROM_WIN32(ERROR_HOTKEY_ALREADY_REGISTERED)`, i.e. another process owns the combination.
///
/// This is the value that makes conflict detection possible at all, and it is the reason
/// registration is attempted at BIND time rather than only at startup: the user finds out while
/// they are still looking at the dialog, instead of discovering at next logon that nothing happened.
const E_HOTKEY_TAKEN: u32 = 0x8007_0581;

/// The three things a hotkey can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    PlayPause,
    NextTrack,
    PrevTrack,
}

impl Slot {
    pub const ALL: [Slot; 3] = [Slot::PlayPause, Slot::NextTrack, Slot::PrevTrack];

    /// The `RegisterHotKey` id. Small and fixed, well inside the documented 0x0000..0xBFFF range
    /// available to an application.
    fn id(self) -> i32 {
        match self {
            Slot::PlayPause => 1,
            Slot::NextTrack => 2,
            Slot::PrevTrack => 3,
        }
    }

    fn from_id(id: usize) -> Option<Slot> {
        match id {
            1 => Some(Slot::PlayPause),
            2 => Some(Slot::NextTrack),
            3 => Some(Slot::PrevTrack),
            _ => None,
        }
    }

    pub fn action(self) -> Action {
        match self {
            Slot::PlayPause => Action::PlayPause,
            Slot::NextTrack => Action::NextTrack,
            Slot::PrevTrack => Action::PrevTrack,
        }
    }

    pub fn label(self) -> &'static str {
        self.action().label()
    }
}

/// What happened when a slot's binding was applied. Always reported, never `?`-ed away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Working. Carries any advisories, so a warning cannot be lost by being shown only in the
    /// dialog - somebody who hand-edits `config.toml` never opens the dialog, and they are exactly
    /// the person who might bind `Ctrl+Alt+E` and then wonder why AltGr stopped producing a
    /// character.
    Registered(Chord, Vec<Advisory>),
    /// Deliberately not set.
    Unbound,
    /// Could not be parsed from the config file.
    Unreadable(String),
    /// Parsed but refused by validation - e.g. a bare letter hand-written into config.toml.
    Refused(Chord, Reject),
    /// Another program already owns this combination.
    Taken(Chord),
    /// Registration failed for some other reason.
    Failed(Chord, String),
}

impl Outcome {
    pub fn is_working(&self) -> bool {
        matches!(self, Outcome::Registered(..))
    }

    /// True when the user asked for something and did not get it. Drives the tray label, so it
    /// reports REALITY rather than intent.
    pub fn is_broken(&self) -> bool {
        matches!(
            self,
            Outcome::Unreadable(_) | Outcome::Refused(..) | Outcome::Taken(_) | Outcome::Failed(..)
        )
    }
}

/// Owns the live registrations.
///
/// A plain struct that `main` holds, NOT a process-global with a thread-id assertion. `RegisterHotKey`
/// and `UnregisterHotKey` are thread-affine, and the tempting way to express that is a static plus a
/// `debug_assert` on the owning thread - which would make this module's own unit tests panic
/// nondeterministically, because `cargo test` runs them on several threads at once. Not being
/// `Send`-shared is a better enforcement than an assertion that only fires in a test harness.
pub struct Registry {
    hwnd: HWND,
    live: [Option<Chord>; 3],
}

/// The Win32 calls, behind a trait so the decision logic can be tested without a window.
///
/// Without this seam the interesting parts - what gets validated, what gets unregistered before what,
/// which outcome a failure maps to - would only be reachable through a real message loop and a real
/// keyboard, i.e. not testable at all.
pub trait Registrar {
    fn register(&mut self, id: i32, mods: u32, vk: u16) -> Result<(), u32>;
    fn unregister(&mut self, id: i32);
}

struct Win32Registrar {
    hwnd: HWND,
}

impl Registrar for Win32Registrar {
    fn register(&mut self, id: i32, mods: u32, vk: u16) -> Result<(), u32> {
        unsafe { RegisterHotKey(Some(self.hwnd), id, HOT_KEY_MODIFIERS(mods), vk as u32) }
            .map_err(|e| e.code().0 as u32)
    }
    fn unregister(&mut self, id: i32) {
        // Failure here means it was not registered, which is the state being asked for anyway.
        let _ = unsafe { UnregisterHotKey(Some(self.hwnd), id) };
    }
}

/// Applies one slot's binding, given the chords already bound to the OTHER slots.
///
/// Pure decision logic over an injectable `Registrar`, which is what makes it testable. Always
/// unregisters first: the documented behaviour of registering the same id twice is that BOTH
/// registrations are kept, so rebinding without unregistering leaks the old combination and leaves
/// the app holding a key the user thinks they have released.
pub fn apply_one(
    r: &mut dyn Registrar,
    slot: Slot,
    text: &str,
    others: &[Chord],
) -> Outcome {
    r.unregister(slot.id());
    if text.trim().is_empty() {
        return Outcome::Unbound;
    }
    let chord = match Chord::parse(text) {
        Ok(c) => c,
        Err(e) => return Outcome::Unreadable(format!("{text:?}: {e}")),
    };
    // VALIDATION RUNS HERE, not only in the dialog. The README invites hand-editing config.toml, so
    // without this a `play_pause = "P"` would seize a bare letter machine-wide at every logon.
    let advisories = match chord.validate(others) {
        Ok(a) => a,
        Err(why) => return Outcome::Refused(chord, why),
    };
    match r.register(slot.id(), chord.mods.to_win32(), chord.vk) {
        Ok(()) => Outcome::Registered(chord, advisories),
        Err(code) if code == E_HOTKEY_TAKEN => Outcome::Taken(chord),
        Err(code) => Outcome::Failed(chord, format!("{code:#010X}")),
    }
}

impl Registry {
    pub fn new(hwnd: HWND) -> Self {
        Registry { hwnd, live: [None; 3] }
    }

    fn idx(slot: Slot) -> usize {
        match slot {
            Slot::PlayPause => 0,
            Slot::NextTrack => 1,
            Slot::PrevTrack => 2,
        }
    }

    /// Applies all three bindings and logs what happened to each.
    pub fn apply_all(&mut self, texts: [&str; 3]) -> [Outcome; 3] {
        let mut reg = Win32Registrar { hwnd: self.hwnd };
        let mut out = [Outcome::Unbound, Outcome::Unbound, Outcome::Unbound];
        for (i, slot) in Slot::ALL.iter().enumerate() {
            // Chords already accepted in THIS pass, so a duplicate inside one config file is caught.
            let others: Vec<Chord> = self.live.iter().flatten().copied().collect();
            let o = apply_one(&mut reg, *slot, texts[i], &others);
            self.live[Self::idx(*slot)] = match &o {
                Outcome::Registered(c, _) => Some(*c),
                _ => None,
            };
            log::write(&format!("hotkey {}: {}", slot.label(), describe(&o)));
            out[i] = o;
        }
        out
    }

    /// Releases every registration. Called before the process exits so nothing is left held.
    pub fn release_all(&mut self) {
        let mut reg = Win32Registrar { hwnd: self.hwnd };
        for slot in Slot::ALL {
            reg.unregister(slot.id());
            self.live[Self::idx(slot)] = None;
        }
    }
}

/// One log line per outcome, in plain words rather than an HRESULT the user cannot act on.
pub fn describe(o: &Outcome) -> String {
    match o {
        Outcome::Registered(c, adv) if adv.is_empty() => format!("{c} - working"),
        Outcome::Registered(c, adv) => format!(
            "{c} - working, but: {}",
            adv.iter().map(|a| a.message()).collect::<Vec<_>>().join("; ")
        ),
        Outcome::Unbound => "not set".into(),
        Outcome::Unreadable(what) => format!("could not read {what} - left unbound"),
        Outcome::Refused(c, why) => format!("refusing {c} - {}", why.message()),
        Outcome::Taken(c) => {
            format!("{c} is already used by another program, so it was not bound here")
        }
        Outcome::Failed(c, code) => format!("{c} could not be registered ({code})"),
    }
}

/// The media thread's handle, and the backend the user picked.
///
/// Statics because `WM_HOTKEY` arrives in a bare `extern "system"` wndproc with nowhere to carry
/// state. The send is on an unbounded channel, so it is lossless and per-press - a bitmask of
/// pending actions was considered and rejected because it cannot represent "Next pressed three
/// times" and would silently merge a run of presses into one.
static MEDIA: OnceLock<media::Handle> = OnceLock::new();
static BACKEND: AtomicU8 = AtomicU8::new(0);

pub fn install_media(handle: media::Handle, backend: Backend) {
    let _ = MEDIA.set(handle);
    set_backend(backend);
}

pub fn set_backend(backend: Backend) {
    BACKEND.store(match backend { Backend::Session => 0, Backend::MediaKeys => 1 }, Ordering::Relaxed);
}

fn backend() -> Backend {
    match BACKEND.load(Ordering::Relaxed) {
        1 => Backend::MediaKeys,
        _ => Backend::Session,
    }
}

/// Handles a `WM_HOTKEY`. Returns true if it was one of ours.
///
/// The id is MATCHED, never used to index or shift. An earlier draft did
/// `pending.fetch_or(1 << wparam)`, which shifts by an unvalidated message parameter - a single
/// stray or future `WM_HOTKEY` with an unexpected id would then panic inside a wndproc that until
/// then could not fail, taking the visualiser down with it.
pub fn on_wm_hotkey(id: usize) -> bool {
    let Some(slot) = Slot::from_id(id) else {
        log::write(&format!("unexpected WM_HOTKEY id {id}, ignored"));
        return false;
    };
    match MEDIA.get() {
        Some(h) => h.send(slot.action(), backend()),
        None => log::write(&format!("{}: no media thread to send to", slot.label())),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records what was asked of Win32 and returns whatever the test wants.
    #[derive(Default)]
    struct FakeRegistrar {
        calls: Vec<String>,
        fail_with: Option<u32>,
    }

    impl Registrar for FakeRegistrar {
        fn register(&mut self, id: i32, mods: u32, vk: u16) -> Result<(), u32> {
            self.calls.push(format!("register({id},{mods:#06X},{vk:#04X})"));
            match self.fail_with {
                Some(c) => Err(c),
                None => Ok(()),
            }
        }
        fn unregister(&mut self, id: i32) {
            self.calls.push(format!("unregister({id})"));
        }
    }

    #[test]
    fn a_rebind_always_unregisters_before_it_registers() {
        // Not cosmetic ordering. Registering the same id twice is documented to KEEP BOTH, so a
        // rebind that skipped the unregister would leave the app still holding the old combination
        // while the user believes they have released it.
        let mut f = FakeRegistrar::default();
        let o = apply_one(&mut f, Slot::PlayPause, "Ctrl+Alt+Space", &[]);
        assert!(o.is_working(), "{o:?}");
        assert_eq!(f.calls.len(), 2, "{:?}", f.calls);
        assert!(f.calls[0].starts_with("unregister("), "{:?}", f.calls);
        assert!(f.calls[1].starts_with("register("), "{:?}", f.calls);
    }

    #[test]
    fn an_empty_binding_releases_the_key_and_registers_nothing() {
        let mut f = FakeRegistrar::default();
        assert_eq!(apply_one(&mut f, Slot::NextTrack, "   ", &[]), Outcome::Unbound);
        assert_eq!(f.calls, vec!["unregister(2)".to_string()]);
    }

    #[test]
    fn a_hand_edited_bare_letter_is_refused_before_it_ever_reaches_the_os() {
        // The scenario: the README tells the user to edit config.toml, and they write
        // `play_pause = "P"`. Validating only in the dialog would let this seize the letter P
        // machine-wide at every logon.
        let mut f = FakeRegistrar::default();
        let o = apply_one(&mut f, Slot::PlayPause, "P", &[]);
        assert!(matches!(o, Outcome::Refused(_, Reject::NeedsModifier)), "{o:?}");
        assert!(
            !f.calls.iter().any(|c| c.starts_with("register(")),
            "a refused chord must never reach RegisterHotKey: {:?}",
            f.calls
        );
    }

    #[test]
    fn an_unreadable_binding_costs_one_action_and_is_named_in_the_outcome() {
        let mut f = FakeRegistrar::default();
        let o = apply_one(&mut f, Slot::PrevTrack, "Ault+Left", &[]);
        match &o {
            Outcome::Unreadable(what) => {
                assert!(what.contains("Ault"), "the bad token must be named: {what}")
            }
            other => panic!("{other:?}"),
        }
        assert!(o.is_broken());
    }

    #[test]
    fn the_taken_error_code_is_classified_rather_than_shown_as_a_number() {
        // 1409 is the whole basis of conflict reporting, so the mapping is pinned.
        let mut f = FakeRegistrar { fail_with: Some(E_HOTKEY_TAKEN), ..Default::default() };
        let o = apply_one(&mut f, Slot::PlayPause, "Ctrl+Alt+Space", &[]);
        assert!(matches!(o, Outcome::Taken(_)), "{o:?}");
        assert!(describe(&o).contains("already used by another program"), "{}", describe(&o));

        // Any other failure keeps its code, because it is not something we can explain.
        let mut f2 = FakeRegistrar { fail_with: Some(0x8007_0005), ..Default::default() };
        let o2 = apply_one(&mut f2, Slot::PlayPause, "Ctrl+Alt+Space", &[]);
        assert!(matches!(o2, Outcome::Failed(..)), "{o2:?}");
        assert!(describe(&o2).contains("0x80070005"), "{}", describe(&o2));
    }

    #[test]
    fn a_duplicate_inside_one_config_file_is_caught_here_not_left_to_the_os() {
        // RegisterHotKey would accept the same combination under two ids and then deliver it to only
        // one of them, so one control would silently do nothing.
        let mut f = FakeRegistrar::default();
        let first = Chord::parse("Ctrl+Alt+Space").unwrap();
        let o = apply_one(&mut f, Slot::NextTrack, "Ctrl+Alt+Space", &[first]);
        assert!(matches!(o, Outcome::Refused(_, Reject::DuplicateOfOtherAction)), "{o:?}");
    }

    #[test]
    fn mod_norepeat_reaches_the_os() {
        // The end-to-end version of the unit test in `hotkey`: whatever the chord, the bits that
        // actually get passed to RegisterHotKey must carry MOD_NOREPEAT, or holding Next Track
        // skips a playlist.
        let mut f = FakeRegistrar::default();
        let _ = apply_one(&mut f, Slot::NextTrack, "Ctrl+Alt+Period", &[]);
        let call = f.calls.iter().find(|c| c.starts_with("register(")).expect("registered");
        let mods = u32::from_str_radix(
            call.split(',').nth(1).unwrap().trim_start_matches("0x"),
            16,
        )
        .expect("mods hex");
        assert_eq!(mods & 0x4000, 0x4000, "MOD_NOREPEAT missing from {call}");
    }

    #[test]
    fn an_unexpected_hotkey_id_is_logged_and_ignored_rather_than_indexed() {
        // The mutation this guards is real: `1 << wparam` on an unvalidated id panics inside a
        // wndproc, which would take the whole app down with it.
        assert!(!on_wm_hotkey(0));
        assert!(!on_wm_hotkey(99));
        assert!(!on_wm_hotkey(usize::MAX));
    }

    #[test]
    fn slot_ids_are_distinct_and_map_back_to_themselves() {
        let mut seen = Vec::new();
        for s in Slot::ALL {
            assert_eq!(Slot::from_id(s.id() as usize), Some(s));
            seen.push(s.id());
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 3, "two slots share an id");
    }
}
