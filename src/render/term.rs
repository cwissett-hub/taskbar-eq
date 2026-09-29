//! The Terminal: 2077 family: the user's own VSCode "2077" theme (endormi.2077-theme) rendered as a
//! live shell session whose output lines ARE the meter.
//!
//! Requested 29 Sep 2026 ("a theme based on my current VSCode theme, I want it to look like a
//! terminal"). It reads two ways at once: as a terminal - a prompt `>`, a typed command, a block
//! cursor, pink line numbers, a status bar - and as a meter - each row is a band, a bar of `▮` block
//! glyphs growing with that band's level, bass at the bottom next to the prompt (like a log's newest
//! line), a `▯` outline marker held at the peak.
//!
//! # The panel is opaque
//!
//! Like every family it paints an opaque `t.panel` (the theme's terminal background) first, a 1 px
//! `t.edge` border inside it, and clips to the rounded rect last - so the Windows weather widget is
//! covered while music plays.
//!
//! # What the meter is
//!
//! The interior above the status bar is divided into `L = min(12, (interior_h - 5) / 4)` rows of 4 px
//! pitch. The bottom-most row is the PROMPT; the row above it (when `L >= 8`) is a `# bpm ~ NNN`
//! comment; the rest are METER rows. Each meter row shows a two-digit line number in `t.hot`, then a
//! bar of `▮` in the row's colour whose length is that band group's level, capped by a `▯` peak
//! marker in `t.hot`. Row `i` (0 = top) folds the band group `m-1-i` so the bass sits at the bottom.
//! The loudest row gets a line-highlight (`#1c1347`) behind it; a row over 0.9 gets selection
//! (`#310072`). After a command "executes" its fake output line replaces the newest meter row for a
//! couple of onsets before the meter reclaims it.
//!
//! # The prompt types
//!
//! The prompt types out the colourway's current command one character per onset, from a per-session
//! list (each colourway is a different KIND of session - a dev shell, network ops, git, sysadmin,
//! build & test). On a strong bass onset with the command complete it "executes": the line-number
//! base scrolls up one and the next command begins. A block cursor follows the text and blinks at
//! 1 Hz. Everything is `font3x5`, upper-cased at 3 px.
//!
//! # The flourish - `panic!`
//!
//! On a strong bass onset the session panics: every meter row is replaced by a red Rust stack trace
//! that scrolls up over ~700 ms, revealing the meter from the bottom, the cursor freezes solid and
//! the status bar turns red with `EXIT 101`; then the prompt runs `clear` and the frame is clean.
//! No trademark marks: "2077" appears only as the colourway names in menus.
//!
//! # Colourways
//!
//! `Theme` carries no per-family knob, so each colourway's character is a per-id [`Style`] matched on
//! the theme id (the `sesh` pattern; unknown ids fall back to `term-2077`). The five are five
//! different sessions in five palettes drawn from the real theme JSON.

use crate::dsp::bands::NUM_BANDS;
use crate::dsp::flourish::{Envelope, Trigger};
use crate::dsp::onset::Flux;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::font3x5;
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Vertical pitch of a row, px (a 3 px glyph body plus a 1 px gap; the `font3x5` cell is 5 px tall so
/// rows sit close, a dense log).
const ROW_PITCH: i32 = 4;
/// The smoothing arrays are sized to the most rows any panel can show.
const MAX_ROWS: usize = 12;
/// Status bar height in px, and the minimum panel height at or above which it is drawn.
const STATUS_H: i32 = 7;
const STATUS_MIN_H: i32 = 48;
/// Line-number column width (two digits at 4 px pitch, minus the trailing gap: `4*2 - 1 = 7`, rounded
/// up to a clean 8 with a 1 px gutter before the bars).
const LINE_NO_W: i32 = 8;
/// Prompt chevron width (`>` glyph plus a 1 px gap).
const PROMPT_W: i32 = 4 + 1;
/// Cursor blink: 1 Hz, on 60 % of the cycle.
const CURSOR_HZ: f32 = 1.0;
const CURSOR_DUTY: f32 = 0.6;
/// Mean bass over `levels[0..8]` a strong onset needs to EXECUTE a completed command.
const EXEC_BASS: f32 = 0.55;
/// The flourish only fires on a bass hit: the trigger must fire AND the low bands clear this (the
/// `sesh` gate). A forced test fire bypasses it (see `Trigger::was_forced`).
const FLOURISH_BASS_MIN: f32 = 0.6;
/// Panic envelope length, ms, and how long each trace row dwells before the trace scrolls up one.
const PANIC_MS: f32 = 700.0;
const TRACE_SCROLL_MS: f32 = 60.0;
/// A fake output line stays visible for this many onsets after its command executes.
const OUTPUT_LINE_STEPS: u32 = 2;
/// The onset net that types the prompt. Copied from `sesh`. NOTE: the 200 ms refractory is why the
/// typing test drives loud frames every 12 frames (~200 ms) rather than every 6 - one onset per
/// refractory - see `prompt_types_on_onsets_and_scrolls_on_execute`.
const ONSET_RATIO: f32 = 2.8;
const ONSET_REFRACTORY_MS: f32 = 200.0;
/// The bpm readout clamps here (a plausible track tempo range).
const BPM_MIN: u16 = 60;
const BPM_MAX: u16 = 200;

/// Line highlight and selection, from the theme JSON - drawn behind the loudest / clipping rows.
const LINE_HIGHLIGHT: &str = "#1c1347";
const SELECTION: &str = "#310072";
/// The comment blue and the panic red, from the theme JSON.
const COMMENT_HEX: &str = "#0098df";
const TRACE_HEX: &str = "#ff2e97";
/// The status bar background (editor background) and its text (status text blue).
const STATUS_BG: &str = "#030d22";
const STATUS_TEXT: &str = "#4d8bee";
/// The status bar during a panic, and its centre label.
const PANIC_STATUS_BG: &str = "#ee1682";
const EXIT_LABEL: &str = "EXIT 101";
/// The command the tail of the panic "runs".
const CLEAR_CMD: &str = "clear";
/// The comment prefix; the tempo is appended as three digits.
const BPM_PREFIX: &str = "# bpm ~ ";
/// The status bar's left path and its right encoding readout.
const STATUS_LEFT: &str = "~/music";
const STATUS_RIGHT: &str = "UTF-8  LF";

/// The panic stack trace, verbatim (six lines, truncated to whole glyphs at the interior width).
const TRACE: [&str; 6] = [
    "thread 'main' panicked at src/dsp/bands.rs:64:9:",
    "index out of bounds: the len is 64",
    "note: run with RUST_BACKTRACE=1",
    "   0: taskbar_eq::render::term::draw",
    "   1: taskbar_eq::render::frame",
    "   2: std::rt::lang_start",
];

// The five sessions. Each colourway is a different KIND of shell session, per the spec table, so the
// user sees "different operations visible". Every string is `font3x5` (upper-cased at 3 px); a test
// asserts every character has a glyph, and outputs are the same length as their command list.

const CMDS_2077: &[&str] =
    &["cargo run --release", "git push", "ls -la", "npm run dev", "python analyse.py", "vim main.rs", "clear"];
const OUT_2077: &[&str] = &[
    "compiling taskbar-eq",
    "to origin main 1f2",
    "total 48 -rw-r--r--",
    "vite ready in 312 ms",
    "wrote 42 rows out.csv",
    "main.rs 120 lines ok",
    "screen cleared",
];

const CMDS_CYAN: &[&str] =
    &["ssh fab-01", "ping 10.0.0.1", "curl -I api.local", "netstat -an", "nslookup snap", "traceroute", "exit"];
const OUT_CYAN: &[&str] = &[
    "fab-01 connected",
    "64 bytes 10.0.0.1",
    "http 200 ok api.local",
    "tcp 22 listen 80 http",
    "snap 10.0.0.9 found",
    "18 hops to gateway",
    "logout connection out",
];

const CMDS_HOT: &[&str] =
    &["git status", "git add -A", "git commit -m fix", "git rebase -i main", "git log --oneline", "git push --tags"];
const OUT_HOT: &[&str] = &[
    "on branch main clean",
    "1 file staged add",
    "main 49afe39 fix",
    "rebased 3 commits ok",
    "49afe39 4b1c2d fix",
    "pushed tags to origin",
];

const CMDS_MATRIX: &[&str] =
    &["htop", "tail -f app.log", "journalctl -f", "df -h", "free -m", "uptime", "kill -9 1337"];
const OUT_MATRIX: &[&str] = &[
    "cpu 12% mem 44% ok",
    "app.log 3 new lines",
    "systemd started ok",
    "sda1 42% 18g free",
    "mem 15872 used 2048",
    "up 3 days load 0.42",
    "signal 9 sent 1337",
];

const CMDS_EDITOR: &[&str] =
    &["cargo test", "cargo clippy", "pytest -q", "npm run build", "make -j8", "cargo build --release"];
const OUT_EDITOR: &[&str] = &[
    "test result ok 672",
    "clippy 0 warnings ok",
    "pytest 128 passed ok",
    "built dist in 4.2 s",
    "make done 88 targets",
    "finished release 9.1s",
];

// Row colour cycles, bass -> treble, from the theme JSON (colourway tables override the base seven).
const ROWS_2077: &[&str] = &["#EA00D9", "#ff2e97", "#ffd400", "#3dd69c", "#0ab2fa", "#0ef3ff", "#3787d6"];
const ROWS_CYAN: &[&str] = &["#0ab2fa", "#0ef3ff", "#4bc5fa"];
// Rotated so no row coincides with the 2077 seven-cycle's magenta/red at the same height (the two
// share a palette): the loud bottom rows stay visibly a different hue.
const ROWS_HOT: &[&str] = &["#ff2e97", "#ee1682", "#EA00D9"];
const ROWS_MATRIX: &[&str] = &["#3dd69c", "#06ad00"];
const ROWS_EDITOR: &[&str] = &["#ff2cf1", "#0ef3ff", "#ffd400", "#39c0ff", "#c832ff"];

/// The prompt chevron colour: hot pink everywhere except matrix, where it is the green `hot`.
const PROMPT_PINK: &str = "#ee0077";

/// Per-colourway character, matched on the theme id (unknown ids fall back to `term-2077`).
#[derive(Clone, Copy)]
struct Style {
    /// Row colour hexes, cycled bass -> treble.
    rows: &'static [&'static str],
    /// The command list this session types.
    cmds: &'static [&'static str],
    /// One fake output line per command (same length as `cmds`).
    outputs: &'static [&'static str],
    /// The `>` colour.
    prompt_hex: &'static str,
}

fn style(t: &Theme) -> Style {
    match t.id.as_str() {
        "term-2077-cyan" => Style { rows: ROWS_CYAN, cmds: CMDS_CYAN, outputs: OUT_CYAN, prompt_hex: PROMPT_PINK },
        "term-2077-hot" => Style { rows: ROWS_HOT, cmds: CMDS_HOT, outputs: OUT_HOT, prompt_hex: PROMPT_PINK },
        // Matrix: the prompt takes the green `hot` (the pink would be an off-palette accent here).
        "term-2077-matrix" => {
            Style { rows: ROWS_MATRIX, cmds: CMDS_MATRIX, outputs: OUT_MATRIX, prompt_hex: "#06ad00" }
        }
        "term-2077-editor" => {
            Style { rows: ROWS_EDITOR, cmds: CMDS_EDITOR, outputs: OUT_EDITOR, prompt_hex: PROMPT_PINK }
        }
        _ => Style { rows: ROWS_2077, cmds: CMDS_2077, outputs: OUT_2077, prompt_hex: PROMPT_PINK },
    }
}

/// The geometry of one frame, kept so the tests can ask where a meter row's bar area landed.
#[derive(Clone, Copy)]
struct Layout {
    /// Bar-area boxes (x0, x1, y0, y1) for each METER row, top -> bottom.
    rows: [(i32, i32, i32, i32); MAX_ROWS],
    /// Number of meter rows. Read only by the test that asks how many rows the meter has.
    #[cfg_attr(not(test), allow(dead_code))]
    n_meter: usize,
}

impl Default for Layout {
    fn default() -> Self {
        Layout { rows: [(0, 0, 0, 0); MAX_ROWS], n_meter: 0 }
    }
}

pub struct Term {
    /// Fires the panic on a rare, exceptional bass hit. `pub(crate)` so the family tests can force it.
    pub(crate) flourish: Trigger,
    /// The panic's decay envelope.
    panic: Envelope,
    /// The onset net that types the prompt.
    onset: Flux,
    /// Smoothed level and peak-hold per meter row (indexed by row, 0 = top).
    fill: [f32; MAX_ROWS],
    peak: [f32; MAX_ROWS],
    /// Line-number base; scrolls up one on each execute.
    line_base: u32,
    /// The command being typed, and how much of it is typed.
    cmd_idx: usize,
    typed: usize,
    /// The fake output line and how many onsets it stays visible for.
    output_idx: usize,
    output_steps_left: u32,
    /// Cursor blink accumulator, ms.
    blink_ms: f32,
    /// Onset gaps for the bpm readout, and the refresh accumulator.
    onset_gaps: [f32; 8],
    gap_head: usize,
    gap_n: usize,
    since_onset_ms: f32,
    bpm: u16,
    bpm_refresh_ms: f32,
    /// Panic scroll state.
    trace_row_offset: usize,
    trace_scroll_ms: f32,
    /// This frame's geometry.
    layout: Layout,
}

impl Default for Term {
    fn default() -> Self {
        Term {
            flourish: Default::default(),
            panic: Default::default(),
            onset: Default::default(),
            fill: [0.0; MAX_ROWS],
            peak: [0.0; MAX_ROWS],
            line_base: 0,
            cmd_idx: 0,
            typed: 0,
            output_idx: 0,
            output_steps_left: 0,
            blink_ms: 0.0,
            onset_gaps: [500.0; 8],
            gap_head: 0,
            gap_n: 0,
            since_onset_ms: 0.0,
            bpm: 120,
            bpm_refresh_ms: 0.0,
            trace_row_offset: 0,
            trace_scroll_ms: 0.0,
            layout: Layout::default(),
        }
    }
}

impl Term {
    #[cfg(test)]
    pub fn typed_len_for_test(&self) -> usize {
        self.typed
    }
    #[cfg(test)]
    pub fn line_base_for_test(&self) -> u32 {
        self.line_base
    }
    #[cfg(test)]
    pub fn rows_for_test(&self) -> usize {
        self.layout.n_meter
    }
    #[cfg(test)]
    pub fn row_box_for_test(&self, row: usize) -> (i32, i32, i32, i32) {
        self.layout.rows[row.min(MAX_ROWS - 1)]
    }

    /// Median of the recorded onset gaps, in ms - what an ordinary beat interval is on this track.
    fn median_gap(&self) -> f32 {
        let n = self.gap_n.min(self.onset_gaps.len());
        if n == 0 {
            return 500.0;
        }
        let mut buf = [0.0f32; 8];
        buf[..n].copy_from_slice(&self.onset_gaps[..n]);
        let s = &mut buf[..n];
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) * 0.5 };
        if mid.is_finite() && mid > 0.0 {
            mid
        } else {
            500.0
        }
    }
}

/// A finite level, clamped 0..1.
fn lvl(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Two digits of `v` (mod 100), right-aligned, into `buf`.
fn fmt_2(v: u32, buf: &mut [u8; 2]) {
    let v = v % 100;
    buf[0] = b'0' + (v / 10) as u8;
    buf[1] = b'0' + (v % 10) as u8;
}

/// Three digits of `v` (mod 1000) into `buf`.
fn fmt_3(v: u32, buf: &mut [u8; 3]) {
    let v = v % 1000;
    buf[0] = b'0' + (v / 100) as u8;
    buf[1] = b'0' + ((v / 10) % 10) as u8;
    buf[2] = b'0' + (v % 10) as u8;
}

/// Truncate `text` to the whole glyphs that fit in `max_w` px at the 4 px pitch (never past its box).
fn clip_label(text: &str, max_w: i32) -> &str {
    if max_w <= 0 {
        return "";
    }
    let n = (((max_w + 1) / 4) as usize).min(text.len());
    &text[..n]
}

impl Family for Term {
    fn id(&self) -> &'static str {
        "term"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // ---- the opaque panel + interior + 1 px border ----
        let panel = Rgba::from_hex(&t.panel, 1.0);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        let (ix0, iy0) = (3i32, 4i32);
        let (ix1, iy1) = (w - 3, h - 4);
        let iw = ix1 - ix0;
        let ih = iy1 - iy0;
        if iw < 6 || ih < 6 {
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return; // too small for a terminal
        }
        let edge = Rgba::from_hex(&t.edge, t.edge_alpha.clamp(0.0, 1.0));
        c.fill_rect(ix0, iy0, iw, 1, edge);
        c.fill_rect(ix0, iy1 - 1, iw, 1, edge);
        c.fill_rect(ix0, iy0, 1, ih, edge);
        c.fill_rect(ix1 - 1, iy0, 1, ih, edge);

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        self.blink_ms = (self.blink_ms + dt).rem_euclid(1000.0 / CURSOR_HZ);
        if !self.blink_ms.is_finite() {
            self.blink_ms = 0.0;
        }

        let st = style(t);
        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);

        // ---- layout: rows, status bar, roles ----
        let status = h >= STATUS_MIN_H;
        let bottom = if status { iy1 - STATUS_H } else { iy1 };
        let n = ((bottom - iy0 - 5) / ROW_PITCH).clamp(0, MAX_ROWS as i32) as usize;
        if n < 2 {
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return; // no room for even a prompt + one meter row
        }
        let comment = n >= 8;
        let m = n - 1 - if comment { 1 } else { 0 }; // meter rows
        let prompt_r = n - 1;
        let comment_r = if comment { n - 2 } else { usize::MAX };

        // Bar area x-range and per-row y.
        let barx0 = ix0 + 1 + LINE_NO_W + 1;
        let barx1 = ix1 - 2;
        let row_y = |r: usize| iy0 + 1 + r as i32 * ROW_PITCH;
        let mut layout = Layout { rows: [(0, 0, 0, 0); MAX_ROWS], n_meter: m };
        for i in 0..m {
            let y0 = row_y(i);
            layout.rows[i] = (barx0, barx1, y0, y0 + 5);
        }
        self.layout = layout;

        // ---- subtle CRT scanlines: a DARKENING of the panel every 3rd row (a subtractive gap, so it
        // never reads as a lit bar in an empty meter row) ----
        let scan = Rgba::lerp_linear(panel, Rgba::new(0, 0, 0, 255), t.ghost.clamp(0.0, 1.0));
        let mut y = iy0 + 1;
        while y < bottom {
            if (y - iy0) % 3 == 0 {
                c.fill_rect(ix0 + 1, y, iw - 2, 1, scan);
            }
            y += 1;
        }

        // ---- fold 64 bands into m meter rows and smooth ----
        let attack = t.ballistics.attack.clamp(0.0, 1.0);
        let decay = t.ballistics.decay.clamp(0.0, 1.0);
        let peak_fall = t.ballistics.peak_fall.max(0.0);
        let mut bass = 0.0f32;
        for &v in &d.levels[0..8] {
            bass += lvl(v);
        }
        bass /= 8.0;
        let m1 = m.max(1);
        for i in 0..m {
            let g = m1 - 1 - i; // 0 = bass group
            let lo = g * NUM_BANDS / m1;
            let hi = (g + 1) * NUM_BANDS / m1;
            let mut sum = 0.0f32;
            for &v in &d.levels[lo..hi.max(lo + 1).min(NUM_BANDS)] {
                sum += lvl(v);
            }
            let target = sum / (hi - lo).max(1) as f32;
            let cur = self.fill[i];
            let kf = if target > cur { attack } else { decay };
            let mut f = cur + (target - cur) * kf;
            if !f.is_finite() {
                f = 0.0;
            }
            self.fill[i] = f;
            let mut pk = (self.peak[i] * (1.0 - peak_fall * dt).max(0.0)).max(f);
            if !pk.is_finite() {
                pk = f;
            }
            self.peak[i] = pk;
        }

        // ---- onsets: type the prompt, execute, drive the bpm ----
        let onset = self.onset.update(&d.levels, dt, ONSET_RATIO, ONSET_REFRACTORY_MS);
        let strong = onset && bass > EXEC_BASS;
        self.since_onset_ms += dt;
        if !self.since_onset_ms.is_finite() {
            self.since_onset_ms = 0.0;
        }
        let cmd_len = st.cmds[self.cmd_idx.min(st.cmds.len() - 1)].chars().count();
        if onset {
            // Record the gap for the tempo readout.
            self.onset_gaps[self.gap_head] = self.since_onset_ms.clamp(1.0, 5000.0);
            self.gap_head = (self.gap_head + 1) % self.onset_gaps.len();
            self.gap_n = (self.gap_n + 1).min(self.onset_gaps.len());
            self.since_onset_ms = 0.0;
            // Type, or execute when complete.
            if self.typed < cmd_len {
                self.typed += 1;
            } else if strong {
                self.line_base = self.line_base.wrapping_add(1);
                self.output_idx = self.cmd_idx;
                self.output_steps_left = OUTPUT_LINE_STEPS;
                self.cmd_idx = (self.cmd_idx + 1) % st.cmds.len();
                self.typed = 0;
            }
            if self.output_steps_left > 0 {
                self.output_steps_left -= 1;
            }
        }
        // Refresh the bpm once a second from the median onset gap.
        self.bpm_refresh_ms += dt;
        if self.bpm_refresh_ms >= 1000.0 {
            self.bpm_refresh_ms = 0.0;
            let g = self.median_gap().max(1.0);
            self.bpm = (60000.0 / g).round().clamp(BPM_MIN as f32, BPM_MAX as f32) as u16;
        }

        // ---- flourish: a panic! ----
        let triggered = self.flourish.update(&d.levels, dt, t.flourish);
        let bass_gate = triggered && bass >= FLOURISH_BASS_MIN;
        #[cfg(test)]
        let fired = bass_gate || (triggered && self.flourish.was_forced());
        #[cfg(not(test))]
        let fired = bass_gate;
        let env = self.panic.update(fired, dt, PANIC_MS);
        if fired {
            self.trace_row_offset = 0;
            self.trace_scroll_ms = 0.0;
        } else if env > 0.25 {
            self.trace_scroll_ms += dt;
            if self.trace_scroll_ms > TRACE_SCROLL_MS {
                self.trace_scroll_ms = 0.0;
                self.trace_row_offset = (self.trace_row_offset + 1).min(TRACE.len() + MAX_ROWS);
            }
        }
        let tracing = env > 0.25;
        let clearing = env > 0.0 && env <= 0.25;

        // ---- meter rows ----
        let loudest = {
            let (mut bi, mut bv) = (0usize, -1.0f32);
            for i in 0..m {
                if self.fill[i] > bv {
                    bv = self.fill[i];
                    bi = i;
                }
            }
            bi
        };
        let cells = ((barx1 - barx0) / 4).max(1);
        for i in 0..m {
            let (_, _, y0, _) = layout.rows[i];
            // Row background: selection for a clipping row, else line-highlight for the loudest.
            if self.fill[i] >= 0.9 {
                c.fill_rect(ix0 + 1, y0, iw - 2, 5, Rgba::from_hex(SELECTION, 1.0));
            } else if i == loudest && self.fill[i] > 0.02 {
                c.fill_rect(ix0 + 1, y0, iw - 2, 5, Rgba::from_hex(LINE_HIGHLIGHT, 1.0));
            }
            // Line number.
            let mut ln = [0u8; 2];
            fmt_2(self.line_base.wrapping_add(i as u32), &mut ln);
            if let Ok(s) = std::str::from_utf8(&ln) {
                font3x5::draw(c, ix0 + 1, y0, s, hot);
            }
            // The newest (bottom) meter row shows the fake output line after an execute.
            if i == m - 1 && self.output_steps_left > 0 && !tracing {
                let out = st.outputs[self.output_idx.min(st.outputs.len() - 1)];
                font3x5::draw(c, barx0, y0, clip_label(out, barx1 - barx0), lit);
                continue;
            }
            // The bar: `▮` cells in the row colour, `▯` peak marker in hot.
            let col_hex = st.rows[(m1 - 1 - i) % st.rows.len()];
            let col = Rgba::from_hex(col_hex, 1.0);
            let filled = (lvl(self.fill[i]) * cells as f32).round() as i32;
            for cell in 0..filled.min(cells) {
                font3x5::draw(c, barx0 + cell * 4, y0, "▮", col);
            }
            let peak_cell = (lvl(self.peak[i]) * cells as f32).round() as i32;
            if peak_cell > 0 && peak_cell <= cells {
                font3x5::draw(c, barx0 + (peak_cell - 1) * 4, y0, "▯", hot);
            }
        }

        // ---- comment row ----
        if comment && !tracing {
            let y0 = row_y(comment_r);
            let mut ln = [0u8; 2];
            fmt_2(self.line_base.wrapping_add(comment_r as u32), &mut ln);
            if let Ok(s) = std::str::from_utf8(&ln) {
                font3x5::draw(c, ix0 + 1, y0, s, hot);
            }
            let mut bpm = [0u8; 3];
            fmt_3(self.bpm as u32, &mut bpm);
            let comment_col = Rgba::from_hex(COMMENT_HEX, 1.0);
            let cx = font3x5::draw(c, barx0, y0, clip_label(BPM_PREFIX, barx1 - barx0), comment_col);
            let bx = barx0 + cx + 1;
            if let Ok(s) = std::str::from_utf8(&bpm) {
                font3x5::draw(c, bx, y0, clip_label(s, barx1 - bx), comment_col);
            }
        }

        // ---- prompt row ----
        {
            let y0 = row_y(prompt_r);
            let mut ln = [0u8; 2];
            fmt_2(self.line_base.wrapping_add(prompt_r as u32), &mut ln);
            if let Ok(s) = std::str::from_utf8(&ln) {
                font3x5::draw(c, ix0 + 1, y0, s, hot);
            }
            font3x5::draw(c, barx0, y0, ">", Rgba::from_hex(st.prompt_hex, 1.0));
            let tx0 = barx0 + PROMPT_W;
            // During the panic tail the prompt is running `clear`, solid cursor.
            let (text, typed, cursor_solid) = if clearing {
                (CLEAR_CMD, CLEAR_CMD.chars().count(), true)
            } else {
                (st.cmds[self.cmd_idx.min(st.cmds.len() - 1)], self.typed, tracing)
            };
            let shown = char_prefix(text, typed);
            let drawn = font3x5::draw(c, tx0, y0, clip_label(shown, barx1 - tx0), lit);
            let cx = tx0 + drawn + if typed > 0 { 2 } else { 0 };
            let blink_on = cursor_solid || self.blink_ms < 1000.0 / CURSOR_HZ * CURSOR_DUTY;
            if blink_on && cx + 3 <= barx1 {
                font3x5::draw(c, cx, y0, "▮", hot);
            }
        }

        // ---- the panic trace overpaints the meter rows ----
        if tracing {
            let trace_col = Rgba::from_hex(TRACE_HEX, 1.0);
            for i in 0..m {
                let ti = i + self.trace_row_offset;
                if ti < TRACE.len() {
                    let (_, _, y0, _) = layout.rows[i];
                    // Panel-coloured wipe over the bar area, then the trace line.
                    c.fill_rect(barx0, y0, barx1 - barx0, 5, panel);
                    font3x5::draw(c, barx0, y0, clip_label(TRACE[ti], barx1 - barx0), trace_col);
                }
            }
        }

        // ---- status bar ----
        if status {
            let bg = if tracing { Rgba::from_hex(PANIC_STATUS_BG, 1.0) } else { Rgba::from_hex(STATUS_BG, 1.0) };
            c.fill_rect(ix0, bottom, iw, STATUS_H, bg);
            let sy = bottom + 1;
            let third = iw / 3;
            let text = Rgba::from_hex(STATUS_TEXT, 1.0);
            if tracing {
                let ex = ix0 + third + (third - font3x5::width(EXIT_LABEL)).max(0) / 2;
                font3x5::draw(c, ex, sy, clip_label(EXIT_LABEL, third), Rgba::new(255, 255, 255, 255));
            } else {
                // left: ~/music
                font3x5::draw(c, ix0 + 2, sy, clip_label(STATUS_LEFT, third - 2), text);
                // centre: ▮ + rms percent + %
                let rms = (((d.rms_l + d.rms_r) * 0.5).max(0.0)).min(1.0);
                let rms = if rms.is_finite() { rms } else { 0.0 };
                let mut pc = [0u8; 2];
                fmt_2((rms * 100.0).round() as u32, &mut pc);
                let mut cx = ix0 + third + 2;
                cx += font3x5::draw(c, cx, sy, "▮", text) + 2;
                if let Ok(s) = std::str::from_utf8(&pc) {
                    cx += font3x5::draw(c, cx, sy, s, text) + 1;
                }
                font3x5::draw(c, cx, sy, "%", text);
                // right: UTF-8  LF
                let rx = ix1 - 2 - font3x5::width(STATUS_RIGHT);
                font3x5::draw(c, rx.max(ix0 + 2 * third), sy, clip_label(STATUS_RIGHT, third), text);
            }
        }

        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

/// The first `n` characters of `text` as a sub-slice (ASCII strings only, so char = byte).
fn char_prefix(text: &str, n: usize) -> &str {
    let end = text.char_indices().nth(n).map(|(i, _)| i).unwrap_or(text.len());
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    fn frames(fam: &mut Term, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
        let mut c = Canvas::new(w, h);
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = level * (1.0 - i as f32 / 96.0);
        }
        d.peaks = d.levels;
        d.rms_l = level;
        d.rms_r = level;
        d.dt_ms = 16.7;
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, t, &d);
        }
        c
    }
    fn drew_over_panel(px: Rgba, panel: Rgba) -> bool {
        let d = (px.r as i32 - panel.r as i32).abs()
            + (px.g as i32 - panel.g as i32).abs()
            + (px.b as i32 - panel.b as i32).abs();
        px.a > 8 && d > 24
    }
    fn lit(c: &Canvas, t: &Theme) -> usize {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let mut n = 0;
        for y in 0..c.height() {
            for x in 0..c.width() {
                if drew_over_panel(c.get(x, y), panel) {
                    n += 1;
                }
            }
        }
        n
    }

    /// Every `&'static str` the family can draw, for the glyph-coverage guard.
    fn all_strings() -> Vec<&'static str> {
        let mut v = Vec::new();
        for lists in [
            CMDS_2077, OUT_2077, CMDS_CYAN, OUT_CYAN, CMDS_HOT, OUT_HOT, CMDS_MATRIX, OUT_MATRIX, CMDS_EDITOR,
            OUT_EDITOR,
        ] {
            v.extend_from_slice(lists);
        }
        v.extend_from_slice(&TRACE);
        v.extend_from_slice(&[
            STATUS_LEFT, STATUS_RIGHT, EXIT_LABEL, CLEAR_CMD, BPM_PREFIX, "▮▯>", "0123456789%",
        ]);
        v
    }
    #[allow(non_snake_case)]
    fn ALL_STRINGS() -> Vec<&'static str> {
        all_strings()
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"term"));
        assert_eq!(crate::render::family_for("term").id(), "term");
        assert_ne!(crate::themes::family_label("term"), "term");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "term").count(), 5);
    }

    #[test]
    fn rows_are_the_meter() {
        let t = theme("term-2077");
        let mut fam = Term::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData::default();
        for v in d.levels[0..8].iter_mut() {
            *v = 0.9;
        }
        d.peaks = d.levels;
        d.dt_ms = 16.7;
        for k in 0..40 {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let fill = |row: usize| -> f32 {
            let (x0, x1, y0, y1) = fam.row_box_for_test(row);
            let painted = (y0..y1)
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .filter(|&(x, y)| drew_over_panel(c.get(x, y), panel))
                .count();
            painted as f32 / ((x1 - x0) * (y1 - y0)) as f32
        };
        let rows = fam.rows_for_test();
        assert!(fill(rows - 1) >= 0.45, "bottom (bass) row {:.2}", fill(rows - 1));
        assert!(fill(0) <= 0.10, "top (treble) row {:.2}", fill(0));
    }

    #[test]
    fn prompt_types_on_onsets_and_scrolls_on_execute() {
        let t = theme("term-2077-hot");
        let mut fam = Term::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData::default();
        d.dt_ms = 16.7;
        let base0 = fam.line_base_for_test();
        // Alternate loud/quiet so every loud frame is an onset (the Flux detector needs a RISE). The
        // cadence is every 12 frames (~200 ms) not 6, to match the copied ONSET_REFRACTORY_MS of
        // 200 ms - one onset per refractory - see the const's note.
        for k in 0..80 {
            let loud = k % 12 == 0;
            for v in d.levels.iter_mut() {
                *v = if loud { 0.9 } else { 0.05 };
            }
            d.peaks = d.levels;
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        assert!(fam.typed_len_for_test() > 0 || fam.line_base_for_test() > base0, "nothing typed and nothing executed");
        for k in 80..400 {
            let loud = k % 12 == 0;
            for v in d.levels.iter_mut() {
                *v = if loud { 0.9 } else { 0.05 };
            }
            d.peaks = d.levels;
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        assert!(fam.line_base_for_test() > base0, "no command executed in 400 frames");
    }

    #[test]
    fn every_command_and_label_char_has_a_glyph() {
        for s in ALL_STRINGS() {
            for ch in s.chars() {
                assert!(crate::render::font3x5::glyph(ch).is_some(), "{s:?} {ch:?}");
            }
        }
    }

    #[test]
    fn status_bar_dropped_below_48_rows() {
        let t = theme("term-2077");
        let bar = Rgba::from_hex("#030d22", 1.0);
        let is_bar = |c: &Canvas, y: i32| {
            (10..370)
                .filter(|&x| {
                    let p = c.get(x, y);
                    (p.r as i32 - bar.r as i32).abs() < 6
                        && (p.g as i32 - bar.g as i32).abs() < 6
                        && (p.b as i32 - bar.b as i32).abs() < 6
                })
                .count()
                > 300
        };
        let tall = frames(&mut Term::default(), &t, 380, 60, 0.3, 5);
        assert!((50..56).any(|y| is_bar(&tall, y)), "no status bar at 380x60");
        let short = frames(&mut Term::default(), &t, 380, 44, 0.3, 5);
        assert!(!(34..40).any(|y| is_bar(&short, y)), "status bar present at 380x44");
    }

    #[test]
    fn cursor_blinks_without_onsets() {
        let t = theme("term-2077");
        let mut fam = Term::default();
        let a = frames(&mut fam, &t, 380, 60, 0.0, 10); // ~167 ms in
        let b = frames(&mut fam, &t, 380, 60, 0.0, 30); // ~667 ms in: other half of the 1 Hz blink
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let diff = (0..60)
            .flat_map(|y| (0..380).map(move |x| (x, y)))
            .filter(|&(x, y)| drew_over_panel(a.get(x, y), panel) != drew_over_panel(b.get(x, y), panel))
            .count();
        assert!(diff >= 9, "cursor did not blink: {diff} px differ");
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("term-2077-matrix");
        let c = frames(&mut Term::default(), &t, 380, 60, 0.0, 10);
        assert!(lit(&c, &t) as f32 >= 0.02 * (380 * 60) as f32, "prompt + numbers + status: {}", lit(&c, &t));
    }

    #[test]
    fn panic_trace_never_exceeds_the_interior() {
        for id in ["term-2077", "term-2077-cyan", "term-2077-hot", "term-2077-matrix", "term-2077-editor"] {
            let t = theme(id);
            for (w, h) in [(190, 48), (128, 44)] {
                let mut fam = Term::default();
                let _ = frames(&mut fam, &t, w, h, 0.5, 5);
                fam.flourish.force_next();
                let c = frames(&mut fam, &t, w, h, 0.5, 45);
                assert!(lit(&c, &t) > 0, "{id} {w}x{h}");
                let panel = Rgba::from_hex(&t.panel, 1.0);
                for y in 0..h {
                    for x in [0, 1, w - 2, w - 1] {
                        let p = c.get(x, y);
                        assert!(
                            p.a == 0 || !drew_over_panel(p, panel) || y < 2 || y >= h - 2,
                            "{id}: paint at the edge {x},{y}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_five_colourways_are_visibly_different() {
        let ids = ["term-2077", "term-2077-cyan", "term-2077-hot", "term-2077-matrix", "term-2077-editor"];
        let renders: Vec<Canvas> = ids.iter().map(|id| frames(&mut Term::default(), &theme(id), 380, 60, 0.7, 30)).collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let (a, b) = (&renders[i], &renders[j]);
                let n = (4..56)
                    .flat_map(|y| (3..377).map(move |x| (x, y)))
                    .filter(|&(x, y)| {
                        let (p, q) = (a.get(x, y), b.get(x, y));
                        (p.r as i32 - q.r as i32).abs() + (p.g as i32 - q.g as i32).abs() + (p.b as i32 - q.b as i32).abs()
                            > 24
                    })
                    .count();
                assert!(
                    n as f32 >= 0.15 * (374.0 * 52.0),
                    "{} vs {}: {:.1}%",
                    ids[i],
                    ids[j],
                    100.0 * n as f32 / (374.0 * 52.0)
                );
            }
        }
    }

    /// A frame shaper: a shaped spectrum with a bass kick on beat frames, which is a genuine RISE out
    /// of the between-beat floor so the onset net fires and the prompt types.
    fn shape(level: f32, t_s: f32, beat: bool) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / NUM_BANDS as f32;
            let base = (1.0 - f).powf(1.2) * 0.55 + 0.06;
            let wob = 1.0 + 0.25 * (t_s * 2.4 + f * 6.0).sin();
            let kick = if beat { 1.0 + 1.1 * (1.0 - f) } else { 0.55 };
            *v = ((base * wob * kick) * level).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = level * 0.6;
        d.rms_r = level * 0.6;
        d
    }

    /// Dumps for the eye test, composited over `#202020` like every family's dump.
    ///
    /// Run: cargo test --release dump_term -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_term() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let bg = Rgba::from_hex("#202020", 1.0);
        let write = |name: String, c: &Canvas| {
            let mut out = Vec::with_capacity((c.width() * c.height() * 4) as usize);
            for y in 0..c.height() {
                for x in 0..c.width() {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for (ch, base) in [(px.r, bg.r), (px.g, bg.g), (px.b, bg.b)] {
                        out.push((ch as f32 * a + base as f32 * (1.0 - a)).round() as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("{name}.rgba")), &out).unwrap();
        };
        let ids = ["term-2077", "term-2077-cyan", "term-2077-hot", "term-2077-matrix", "term-2077-editor"];
        for id in ids {
            let t = theme(id);
            let short = &id["term-".len()..];
            // A beat every 15 frames is an onset; ~120 frames types a few commands and executes.
            for (tag, level) in [("calm", 0.30f32), ("loud", 0.85)] {
                let mut fam = Term::default();
                let mut c = Canvas::new(380, 60);
                for k in 0..140 {
                    c.clear();
                    fam.draw(&mut c, &t, &shape(level, k as f32 * 0.0167, k % 15 == 0));
                }
                write(format!("term-{short}-{tag}"), &c);
            }
            // Panic: settle, fire, capture the peak (a couple of frames in) and a mid-decay frame.
            let mut fam = Term::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..140 {
                c.clear();
                fam.draw(&mut c, &t, &shape(0.6, k as f32 * 0.0167, k % 15 == 0));
            }
            fam.flourish.force_next();
            for k in 140..146 {
                c.clear();
                fam.draw(&mut c, &t, &shape(0.45, k as f32 * 0.0167, false));
            }
            write(format!("term-{short}-panic-peak"), &c);
            for k in 146..175 {
                c.clear();
                fam.draw(&mut c, &t, &shape(0.3, k as f32 * 0.0167, false));
            }
            write(format!("term-{short}-panic-decay"), &c);
        }
        // term-2077 at the two narrow sizes.
        for (w, h) in [(190, 48), (128, 44)] {
            let t = theme("term-2077");
            let mut fam = Term::default();
            let mut c = Canvas::new(w, h);
            for k in 0..140 {
                c.clear();
                fam.draw(&mut c, &t, &shape(0.8, k as f32 * 0.0167, k % 15 == 0));
            }
            write(format!("term-2077-{w}x{h}"), &c);
        }
        println!("wrote term dumps to {}", dir.display());
    }

    /// Per-frame cost, steady and during the trace, at 380x60.
    ///
    /// Run: cargo test --release probe_term_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_term_cost() {
        let t = theme("term-2077-editor");
        let mut fam = Term::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = 0.6 * (1.0 - i as f32 / 96.0);
        }
        d.peaks = d.levels;
        for _ in 0..60 {
            fam.draw(&mut c, &t, &d);
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        let steady = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("term steady: {steady:.3} ms/frame at 380x60");

        fam.flourish.force_next();
        let m = 42; // ~700 ms of trace at 16.7 ms/frame
        let t1 = std::time::Instant::now();
        for k in n..(n + m) {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        let flourish = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
        println!("term trace: {flourish:.3} ms/frame at 380x60");
    }
}
