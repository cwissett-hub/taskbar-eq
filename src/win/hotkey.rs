//! Key combinations: parsing, canonical formatting, and validation.
//!
//! Pure logic, no Win32 calls, so all of it is testable without a window or a keyboard. The
//! registration that consumes it lives in `win::hotkeys`.
//!
//! WHY A STRING IN THE CONFIG FILE rather than `{ vk = 179, mods = 3 }`: blast radius. `Config`
//! falls back to `Config::default()` for the WHOLE document if any field fails to deserialise, so a
//! hand-mangled `vk = "P"` in a struct-of-integers would silently reset the user's theme, width and
//! every timing. A `String` field cannot fail to deserialise, so the worst a typo can cost is one
//! unbound action - which is then reported in the log and in the dialog. The README invites editing
//! `config.toml` by hand, so this is not a hypothetical.
//!
//! WHY A STATIC NAME TABLE rather than `GetKeyNameTextW`: that API is keyboard-layout aware, which
//! is a feature for a label and a liability for a stored value. A config file written on a UK layout
//! must mean the same thing when the same exe runs on a US one, and it must round-trip byte for byte.
//! The table is layout-independent by construction.
//!
//! WHAT IS DELIBERATELY NOT MODELLED: the extended-key bit. `RegisterHotKey` takes a virtual key and
//! nothing else, so it genuinely cannot tell Numpad Enter from Enter. Modelling a distinction the OS
//! will not honour would only create a value that cannot round-trip through registration.

/// The four modifier keys a hotkey can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Mods {
    pub fn count(self) -> u32 {
        self.ctrl as u32 + self.alt as u32 + self.shift as u32 + self.win as u32
    }

    /// The `fsModifiers` bits `RegisterHotKey` wants, with `MOD_NOREPEAT` always set.
    ///
    /// `MOD_NOREPEAT` (0x4000) is not optional here: without it, holding Next Track down auto-repeats
    /// and skips dozens of tracks before the key comes back up.
    pub fn to_win32(self) -> u32 {
        (if self.alt { 0x0001 } else { 0 })
            | (if self.ctrl { 0x0002 } else { 0 })
            | (if self.shift { 0x0004 } else { 0 })
            | (if self.win { 0x0008 } else { 0 })
            | 0x4000
    }
}

/// A modifier combination plus one trigger key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub mods: Mods,
    pub vk: u16,
}

/// Why a chord was refused. Refusal is final - the chord is not stored and not registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// Only modifiers were pressed; there is no key to trigger on.
    NoTriggerKey,
    /// A bare printable key would fire while typing anywhere on the machine.
    NeedsModifier,
    /// Shift alone does not make a key non-typing: Shift+A is just `A`.
    ShiftAloneIsNotEnough,
    /// Already bound to another action in this app.
    DuplicateOfOtherAction,
}

impl Reject {
    pub fn message(self) -> &'static str {
        match self {
            Reject::NoTriggerKey => "press a key as well as the modifiers",
            Reject::NeedsModifier => {
                "a key on its own would fire while you were typing - add Ctrl, Alt or Win"
            }
            Reject::ShiftAloneIsNotEnough => {
                "Shift on its own still types a character - add Ctrl, Alt or Win"
            }
            Reject::DuplicateOfOtherAction => "already used by another control here",
        }
    }
}

/// A chord that is allowed but worth warning about. Returned, never merely documented.
///
/// This is an enum rather than README prose on purpose: a warning that lives only in documentation is
/// a warning no code reads, and this repo has shipped that twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advisory {
    /// `Ctrl+Alt+<key>` is what AltGr produces in hardware.
    AltGrCollision,
    /// Claiming a bare media key takes it from every other app on the machine.
    MediaKeySeizure,
    /// Any other bare key - a function key - is claimed system-wide too.
    BareKeySeizure,
    /// F12 is the debugger's break key in most tooling.
    F12Debugger,
    /// Windows reserves a number of `Win+<key>` combinations and will refuse them.
    WinKeyMayBeReserved,
}

impl Advisory {
    pub fn message(self) -> &'static str {
        match self {
            // The dev machine is a UK layout, where AltGr is LeftCtrl+RightAlt in hardware, so this
            // is not exotic - binding Ctrl+Alt+E stops the user typing the character AltGr+E makes.
            Advisory::AltGrCollision => {
                "Ctrl+Alt is how AltGr is sent, so this may stop you typing some characters"
            }
            Advisory::MediaKeySeizure => {
                "this takes the key from every other app until taskbar-eq is closed"
            }
            Advisory::BareKeySeizure => {
                "with no modifier this key is taken from every other app while taskbar-eq runs"
            }
            Advisory::F12Debugger => "F12 is the break key in most developer tools",
            Advisory::WinKeyMayBeReserved => "Windows reserves some Win key combinations",
        }
    }
}

/// The dedicated transport keys, allowed with no modifier at all.
///
/// The FUNCTION KEYS are allowed bare too - see `is_bare_allowed`. They are not in this list only
/// because they are a range rather than a set.
///
/// Deliberately narrow beyond that. An earlier draft also waved through the volume, browser and
/// launch keys on the same reasoning, and that is a much worse trade: binding bare Volume Up takes
/// volume control away from the entire machine including the Windows volume overlay, and the volume
/// and media keys are adjacent on most keyboards and share the Fn row on laptops, so it is an easy
/// mis-press in a capture field.
const BARE_ALLOWED: [u16; 4] = [
    0xB3, // VK_MEDIA_PLAY_PAUSE
    0xB0, // VK_MEDIA_NEXT_TRACK
    0xB1, // VK_MEDIA_PREV_TRACK
    0xB2, // VK_MEDIA_STOP
];

/// Whether `vk` may be bound with no modifier at all.
///
/// THE RULE IS "DOES IT TYPE", NOT "IS IT UNUSUAL". The first version of this allowed only the media
/// keys and F13-F24, and it refused a bare F9 - which was reported, correctly, as wrong: F9 does not
/// produce a character, so the justification for demanding a modifier ("a key on its own would fire
/// while you were typing") does not apply to it at all. F13-F24 had been allowed on the different and
/// much weaker ground that most keyboards do not have them, which is a reason a binding is unlikely
/// to be USED rather than a reason it is safe.
///
/// So: the whole function-key range, and the dedicated transport keys. Everything that produces text
/// still needs a modifier, because those genuinely would fire mid-sentence - and that includes Space,
/// Enter and Tab, which do not print a glyph but are pressed constantly while typing.
///
/// A bare binding still takes the key from every other application while this app runs, which is a
/// real cost and not one to hide, so it carries `Advisory::BareKeySeizure` rather than being silently
/// accepted.
fn is_bare_allowed(vk: u16) -> bool {
    // F1..F24.
    (0x70..=0x87).contains(&vk) || BARE_ALLOWED.contains(&vk)
}

/// Virtual keys that are only ever modifiers, so they can never be a trigger.
fn is_modifier_vk(vk: u16) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

/// `(vk, canonical name)`. The canonical name is what `Display` writes and what the config stores.
const NAMES: &[(u16, &str)] = &[
    (0x08, "Backspace"),
    (0x09, "Tab"),
    (0x0D, "Enter"),
    (0x1B, "Esc"),
    (0x20, "Space"),
    (0x21, "PageUp"),
    (0x22, "PageDown"),
    (0x23, "End"),
    (0x24, "Home"),
    (0x25, "Left"),
    (0x26, "Up"),
    (0x27, "Right"),
    (0x28, "Down"),
    (0x2C, "PrintScreen"),
    (0x2D, "Insert"),
    (0x2E, "Delete"),
    (0x60, "Numpad0"),
    (0x61, "Numpad1"),
    (0x62, "Numpad2"),
    (0x63, "Numpad3"),
    (0x64, "Numpad4"),
    (0x65, "Numpad5"),
    (0x66, "Numpad6"),
    (0x67, "Numpad7"),
    (0x68, "Numpad8"),
    (0x69, "Numpad9"),
    (0x6A, "NumpadMultiply"),
    (0x6B, "NumpadAdd"),
    (0x6D, "NumpadSubtract"),
    (0x6E, "NumpadDecimal"),
    (0x6F, "NumpadDivide"),
    (0xA6, "BrowserBack"),
    (0xA7, "BrowserForward"),
    (0xAD, "VolumeMute"),
    (0xAE, "VolumeDown"),
    (0xAF, "VolumeUp"),
    (0xB0, "MediaNext"),
    (0xB1, "MediaPrev"),
    (0xB2, "MediaStop"),
    (0xB3, "MediaPlayPause"),
    // OEM punctuation. `VK_OEM_COMMA`, `VK_OEM_PERIOD`, `VK_OEM_MINUS` and `VK_OEM_PLUS` are the
    // only OEM keys documented as identical "for any country/region", which is why the suggested
    // bindings use them and the VK_OEM_1..8 range is left unnamed.
    (0xBC, "Comma"),
    (0xBE, "Period"),
    (0xBD, "Minus"),
    (0xBB, "Plus"),
];

/// The canonical name of a virtual key.
pub fn key_name(vk: u16) -> String {
    if let Some((_, n)) = NAMES.iter().find(|(v, _)| *v == vk) {
        return (*n).to_string();
    }
    // Letters and digits name themselves.
    if (0x30..=0x39).contains(&vk) || (0x41..=0x5A).contains(&vk) {
        return (vk as u8 as char).to_string();
    }
    if (0x70..=0x87).contains(&vk) {
        return format!("F{}", vk - 0x6F);
    }
    // Anything else keeps its number, so a chord captured on hardware this table does not know
    // still round-trips exactly instead of being silently dropped.
    format!("VK({vk:#04X})")
}

/// Parses a canonical name back to a virtual key.
fn key_from_name(s: &str) -> Option<u16> {
    let t = s.trim();
    if let Some((v, _)) = NAMES.iter().find(|(_, n)| n.eq_ignore_ascii_case(t)) {
        return Some(*v);
    }
    // Aliases a hand-editor is likely to write.
    for (alias, vk) in [
        ("Escape", 0x1Bu16),
        ("Return", 0x0D),
        ("Del", 0x2E),
        ("Ins", 0x2D),
        ("PgUp", 0x21),
        ("PgDn", 0x22),
        ("Spacebar", 0x20),
        ("ArrowLeft", 0x25),
        ("ArrowRight", 0x27),
        ("ArrowUp", 0x26),
        ("ArrowDown", 0x28),
    ] {
        if alias.eq_ignore_ascii_case(t) {
            return Some(vk);
        }
    }
    if t.len() == 1 {
        let c = t.chars().next()?.to_ascii_uppercase();
        if c.is_ascii_digit() || c.is_ascii_uppercase() {
            return Some(c as u16);
        }
    }
    if let Some(rest) = t.strip_prefix(['F', 'f']) {
        if let Ok(n) = rest.parse::<u16>() {
            if (1..=24).contains(&n) {
                return Some(0x6F + n);
            }
        }
    }
    if let Some(rest) = t.strip_prefix("VK(").and_then(|r| r.strip_suffix(')')) {
        let hex = rest.trim_start_matches("0x").trim_start_matches("0X");
        if let Ok(v) = u16::from_str_radix(hex, 16) {
            return Some(v);
        }
    }
    None
}

/// Why a stored string could not be understood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The string was empty, i.e. deliberately unbound.
    Empty,
    /// A `+`-separated token was not a modifier or a known key name.
    UnknownToken(String),
    /// Modifiers only, with no trigger key.
    NoTriggerKey,
    /// More than one non-modifier key.
    TooManyKeys,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty"),
            ParseError::UnknownToken(t) => write!(f, "unknown key name {t:?}"),
            ParseError::NoTriggerKey => write!(f, "modifiers with no key"),
            ParseError::TooManyKeys => write!(f, "more than one key"),
        }
    }
}

impl Chord {
    /// Parses `"Ctrl+Alt+Space"`. Order-insensitive and case-insensitive.
    pub fn parse(s: &str) -> Result<Chord, ParseError> {
        if s.trim().is_empty() {
            return Err(ParseError::Empty);
        }
        let mut mods = Mods::default();
        let mut vk: Option<u16> = None;
        for raw in s.split('+') {
            let t = raw.trim();
            if t.is_empty() {
                continue;
            }
            let lower = t.to_ascii_lowercase();
            match lower.as_str() {
                "ctrl" | "control" | "ctl" => mods.ctrl = true,
                "alt" | "menu" => mods.alt = true,
                "shift" => mods.shift = true,
                "win" | "windows" | "super" | "meta" | "cmd" => mods.win = true,
                _ => match key_from_name(t) {
                    Some(v) if vk.is_some() => {
                        let _ = v;
                        return Err(ParseError::TooManyKeys);
                    }
                    Some(v) => vk = Some(v),
                    None => return Err(ParseError::UnknownToken(t.to_string())),
                },
            }
        }
        match vk {
            Some(v) => Ok(Chord { mods, vk: v }),
            None => Err(ParseError::NoTriggerKey),
        }
    }

    /// Checks a chord, returning any advisories or the reason it is refused.
    ///
    /// `others` is every chord already bound to a DIFFERENT action, so a duplicate is caught here
    /// rather than by the OS - `RegisterHotKey` would happily accept the same combination twice
    /// under two ids and then deliver it to only one of them.
    ///
    /// This runs in BOTH the dialog and the startup path. Validating only in the dialog would leave
    /// a hand-edited `play_pause = "P"` seizing a bare letter machine-wide at every logon, and the
    /// README actively invites hand-editing.
    pub fn validate(&self, others: &[Chord]) -> Result<Vec<Advisory>, Reject> {
        if is_modifier_vk(self.vk) {
            return Err(Reject::NoTriggerKey);
        }
        if others.contains(self) {
            return Err(Reject::DuplicateOfOtherAction);
        }
        if self.mods.count() == 0 && !is_bare_allowed(self.vk) {
            return Err(Reject::NeedsModifier);
        }
        if !self.mods.ctrl && !self.mods.alt && !self.mods.win && self.mods.shift {
            return Err(Reject::ShiftAloneIsNotEnough);
        }
        let mut out = Vec::new();
        if self.mods.ctrl && self.mods.alt && !self.mods.win {
            out.push(Advisory::AltGrCollision);
        }
        if self.mods.count() == 0 {
            // Any bare key is claimed exclusively and system-wide. The media keys get their own
            // wording because losing them is what a user notices first.
            if BARE_ALLOWED.contains(&self.vk) {
                out.push(Advisory::MediaKeySeizure);
            } else {
                out.push(Advisory::BareKeySeizure);
            }
        }
        if self.vk == 0x7B {
            out.push(Advisory::F12Debugger);
        }
        if self.mods.win {
            out.push(Advisory::WinKeyMayBeReserved);
        }
        Ok(out)
    }
}

impl std::fmt::Display for Chord {
    /// Canonical form. The modifier order is fixed so a chord round-trips byte for byte; the parser
    /// accepts any order.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (on, name) in [
            (self.mods.ctrl, "Ctrl"),
            (self.mods.alt, "Alt"),
            (self.mods.shift, "Shift"),
            (self.mods.win, "Win"),
        ] {
            if on {
                write!(f, "{name}+")?;
            }
        }
        write!(f, "{}", key_name(self.vk))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chord_the_type_can_hold_round_trips_through_its_own_text() {
        // EXHAUSTIVE, not a handful of examples. The space is 256 keys x 16 modifier subsets = 4096,
        // which is small enough to walk, and walking it is the only way to be sure no key formats to
        // a name the parser cannot read back. A hand-picked set of examples is the weak version of
        // this property and would miss exactly the keys nobody thought of.
        let mut checked = 0;
        for vk in 0u16..=0xFF {
            if is_modifier_vk(vk) {
                continue;
            }
            for bits in 0u8..16 {
                let mods = Mods {
                    ctrl: bits & 1 != 0,
                    alt: bits & 2 != 0,
                    shift: bits & 4 != 0,
                    win: bits & 8 != 0,
                };
                let c = Chord { mods, vk };
                let text = c.to_string();
                let back = Chord::parse(&text)
                    .unwrap_or_else(|e| panic!("{text:?} (vk {vk:#04X}) did not parse back: {e}"));
                assert_eq!(back, c, "{text:?} round-tripped to a different chord");
                assert_eq!(back.to_string(), text, "{text:?} is not a canonical spelling");
                checked += 1;
            }
        }
        // Derived from `is_modifier_vk` rather than hand-counted: the first version of this line
        // said (256 - 12) and the real figure is 11, so the arithmetic was the only thing that
        // failed while all 3920 chords round-tripped.
        let skipped = (0u16..=0xFF).filter(|v| is_modifier_vk(*v)).count();
        assert_eq!(checked, (256 - skipped) * 16, "unexpected number of chords checked");
    }

    #[test]
    fn the_parser_is_order_and_case_insensitive_and_accepts_the_obvious_aliases() {
        let want = Chord {
            mods: Mods { ctrl: true, alt: true, shift: false, win: false },
            vk: 0x20,
        };
        for s in ["Ctrl+Alt+Space", "alt+ctrl+space", "CONTROL + ALT + SPACEBAR", "ctl+menu+Space"] {
            assert_eq!(Chord::parse(s).expect(s), want, "{s:?}");
        }
        // And the canonical spelling is one specific one of those.
        assert_eq!(want.to_string(), "Ctrl+Alt+Space");
    }

    #[test]
    fn a_mangled_binding_is_refused_by_name_rather_than_guessed_at() {
        // The point is that a hand-edited typo produces a NAMED error that can be logged, not a
        // wrong chord and not a panic.
        assert_eq!(Chord::parse(""), Err(ParseError::Empty));
        assert_eq!(Chord::parse("   "), Err(ParseError::Empty));
        assert_eq!(Chord::parse("Ctrl+Alt"), Err(ParseError::NoTriggerKey));
        assert_eq!(Chord::parse("Ctrl+A+B"), Err(ParseError::TooManyKeys));
        assert_eq!(
            Chord::parse("Ault+Right"),
            Err(ParseError::UnknownToken("Ault".into())),
            "a near-miss modifier must be reported, not silently dropped"
        );
    }

    #[test]
    fn a_bare_printable_key_is_refused_however_it_arrives() {
        // The failure this prevents: `play_pause = "P"` hand-written into config.toml, registered at
        // every logon, and the letter P stops working across the whole machine.
        // Space, Enter and Tab are in here even though they print no glyph: they are pressed
        // constantly while typing, which is the actual test. Function keys are NOT - see the next
        // test.
        for s in ["P", "1", "Space", "Enter", "Tab", "Comma", "Period"] {
            let c = Chord::parse(s).expect(s);
            assert_eq!(
                c.validate(&[]),
                Err(Reject::NeedsModifier),
                "{s:?} must not be bindable without a modifier"
            );
        }
        // Shift is not a real guard: Shift+P is just P.
        assert_eq!(
            Chord::parse("Shift+P").unwrap().validate(&[]),
            Err(Reject::ShiftAloneIsNotEnough)
        );
    }

    #[test]
    fn the_bare_keys_that_are_allowed_are_the_transport_keys_and_every_function_key() {
        // THE REPORTED BUG: a bare F9 was refused. It was refused because the first version of this
        // rule allowed only F13-F24, on the ground that most keyboards do not have them - which is a
        // reason a binding is unlikely to be used, not a reason it is safe. F9 does not type a
        // character, so the "it would fire while you were typing" justification never applied to it.
        for s in [
            "F1", "F2", "F5", "F9", "F12", "F13", "F24", "MediaPlayPause", "MediaNext", "MediaPrev",
            "MediaStop",
        ] {
            let c = Chord::parse(s).expect(s);
            assert!(c.validate(&[]).is_ok(), "{s:?} should be bindable bare");
        }
        // Every function key, not just the ones spot-checked above.
        for n in 1..=24 {
            let s = format!("F{n}");
            let c = Chord::parse(&s).expect(&s);
            assert!(c.validate(&[]).is_ok(), "bare {s} should be bindable");
        }
        // Refused: an earlier draft allowed these on the same reasoning, and binding bare Volume Up
        // takes volume off the whole machine including the Windows overlay. They sit next to the
        // media keys on most keyboards, so it is an easy mis-press in a capture field.
        for s in ["VolumeUp", "VolumeDown", "VolumeMute", "BrowserBack", "BrowserForward"] {
            let c = Chord::parse(s).expect(s);
            assert_eq!(
                c.validate(&[]),
                Err(Reject::NeedsModifier),
                "{s:?} must not be bindable bare"
            );
        }
    }

    #[test]
    fn advisories_are_returned_as_values_so_the_ui_cannot_forget_them() {
        // Each of these was a line of prose in the design. Prose is not read by code; this is.
        let altgr = Chord::parse("Ctrl+Alt+E").unwrap().validate(&[]).unwrap();
        assert!(altgr.contains(&Advisory::AltGrCollision), "{altgr:?}");

        let media = Chord::parse("MediaPlayPause").unwrap().validate(&[]).unwrap();
        assert!(media.contains(&Advisory::MediaKeySeizure), "{media:?}");

        // A bare function key is allowed, but it IS taken from the whole machine, and saying so is
        // the difference between an informed choice and a surprise.
        let bare = Chord::parse("F9").unwrap().validate(&[]).unwrap();
        assert!(bare.contains(&Advisory::BareKeySeizure), "{bare:?}");
        // With a modifier there is nothing to warn about.
        let modded = Chord::parse("Ctrl+Shift+F9").unwrap().validate(&[]).unwrap();
        assert!(!modded.contains(&Advisory::BareKeySeizure), "{modded:?}");

        let f12 = Chord::parse("Ctrl+F12").unwrap().validate(&[]).unwrap();
        assert!(f12.contains(&Advisory::F12Debugger), "{f12:?}");

        let winkey = Chord::parse("Win+Ctrl+Right").unwrap().validate(&[]).unwrap();
        assert!(winkey.contains(&Advisory::WinKeyMayBeReserved), "{winkey:?}");

        // And a clean chord carries none, so the warnings mean something when they do appear.
        let clean = Chord::parse("Ctrl+Shift+F9").unwrap().validate(&[]).unwrap();
        assert!(clean.is_empty(), "{clean:?}");
    }

    #[test]
    fn the_suggested_bindings_are_clean_and_do_not_collide_with_each_other() {
        // The set the dialog's one-click button offers. Win+Ctrl rather than Ctrl+Alt precisely to
        // avoid the AltGr advisory, and the OEM comma/period because they are the only punctuation
        // keys documented identical across layouts.
        let set: Vec<Chord> = ["Win+Ctrl+Space", "Win+Ctrl+Period", "Win+Ctrl+Comma"]
            .iter()
            .map(|s| Chord::parse(s).expect(s))
            .collect();
        for (i, c) in set.iter().enumerate() {
            let others: Vec<Chord> =
                set.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, c)| *c).collect();
            let adv = c.validate(&others).unwrap_or_else(|e| panic!("{c} refused: {e:?}"));
            assert!(
                !adv.contains(&Advisory::AltGrCollision),
                "{c} carries the AltGr warning the suggested set exists to avoid"
            );
        }
        // And a duplicate IS caught, rather than left for the OS to deliver to one id only.
        let dup = set[0];
        assert_eq!(dup.validate(&set), Err(Reject::DuplicateOfOtherAction));
    }

    #[test]
    fn mod_norepeat_is_always_set_because_holding_next_track_must_not_skip_a_playlist() {
        for bits in 0u8..16 {
            let mods = Mods {
                ctrl: bits & 1 != 0,
                alt: bits & 2 != 0,
                shift: bits & 4 != 0,
                win: bits & 8 != 0,
            };
            assert_eq!(mods.to_win32() & 0x4000, 0x4000, "MOD_NOREPEAT missing for {mods:?}");
        }
        // And the individual bits are the documented ones, in case a copy-paste swapped them.
        assert_eq!(Mods { alt: true, ..Default::default() }.to_win32() & 0xFF, 0x0001);
        assert_eq!(Mods { ctrl: true, ..Default::default() }.to_win32() & 0xFF, 0x0002);
        assert_eq!(Mods { shift: true, ..Default::default() }.to_win32() & 0xFF, 0x0004);
        assert_eq!(Mods { win: true, ..Default::default() }.to_win32() & 0xFF, 0x0008);
    }
}
