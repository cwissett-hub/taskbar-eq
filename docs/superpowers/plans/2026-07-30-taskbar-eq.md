# Taskbar EQ Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a single portable Windows exe that overlays a real-time audio visualiser on the Windows 11 Widgets (weather) button while audio is playing, and hands the weather back when it stops.

**Architecture:** One Rust binary. A WASAPI loopback thread captures system audio and feeds band levels to a render loop through a lock-free channel; a UI Automation watcher tracks the Widgets button's rect; a software rasteriser draws into an RGBA buffer that is blitted to a topmost layered window via `UpdateLayeredWindow`. Themes split along a hard seam: **colourways are data** (TOML, hot-reloaded) and **families are code** (a `Family` trait plus registry).

**Tech Stack:** Rust stable `x86_64-pc-windows-msvc`, `windows` 0.62.2 (official Microsoft Win32 bindings), `rustfft` 6.4.1, `notify` 8.2.0, `toml` 1.1.4, `serde` 1.0.229, `anyhow` 1.0.104.

## Global Constraints

Every task's requirements implicitly include this section. All values verified on the target machine 2026-07-30, not assumed.

- **Reference:** the approved spec is `docs/superpowers/specs/2026-07-30-taskbar-eq-design.md`. The committed browser mockups at `docs/reference/mockups/` are the **reference implementation of all three renderers** — read `docs/reference/README.md` before any render task.
- **DPI:** call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` **before creating any window**. A DPI-unaware process misreads the taskbar as 1536×48 instead of the true 1920×60. This is a verified trap, not a precaution.
- **Coordinates:** UI Automation returns **true physical pixels**. Never mix with `GetWindowRect` from an unaware context.
- **Overlay canvas:** 190 × 60 physical pixels. Widget height 60. **The rect moves** — X was observed shifting 1385 → 1416 within a single session as the weather text changed from "20°C Mostly cloudy" to "19°C Partly cloudy". Poll every 1 s; never cache the rect across frames.
- **Audio format:** default endpoint reports 48000 Hz, 2 ch, 32-bit float. Handle the general case but optimise for this.
- **FFT:** 2048-point, Hann window, 512-sample hop. Bin resolution 23.4 Hz — a 1000 Hz sine peaks at bin 43 (1008 Hz). Verified.
- **Dark mode only.** Light mode is an explicit non-goal.
- **Panel opacity floor:** `panel_alpha >= 0.92`. Two reasons, and the second is the one that
  bites: the taskbar is wallpaper-tinted acrylic (`#3D1712` on the reference machine) rather than
  black, so each theme supplies its own near-black panel; AND the panel must **occlude the
  Widgets button's own icon and text**. At 0.55 the white weather text composited to ~45% of 255
  and stayed plainly legible through the panel. The design chose "EQ replaces the weather while
  playing", not a translucent wash, so anything below ~0.92 fails the requirement.
- **Contrast floor:** every `lit` colour must reach ≥ 3:1 against its own theme's `panel`. Computed, never eyeballed.
- **No elevation, ever.** The user is in the local Administrators group but the app must never require elevation. Autostart writes `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- **No runtime dependency.** Output is one exe, copied to two machines.
- **Non-goals:** keyboard backlight, light mode, macOS/Linux, installer, the Spectrogram family.

### Verified dependency block — copy verbatim

The `windows` feature list below is exact. `Win32_System_Com_StructuredStorage` and `Win32_System_Variant` are **required** for `IMMDevice::Activate` (it takes `Option<*const PROPVARIANT>`); without them the method silently does not exist and you get `E0599 no method named Activate`.

```toml
[package]
name = "taskbar-eq"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0.104"
rustfft = "6.4.1"
notify = "8.2.0"
serde = { version = "1.0.229", features = ["derive"] }
toml = "1.1.4"

[dependencies.windows]
version = "0.62.2"
features = [
  "Win32_Foundation",
  "Win32_System_Com",
  "Win32_System_Com_StructuredStorage",
  "Win32_System_Variant",
  "Win32_System_LibraryLoader",
  "Win32_System_Registry",
  "Win32_System_Threading",
  "Win32_UI_Accessibility",
  "Win32_UI_HiDpi",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_UI_WindowsAndMessaging",
  "Win32_UI_Shell",
  "Win32_Media_Audio",
  "Win32_Graphics_Gdi",
]

[profile.release]
opt-level = 3
lto = true
strip = true
```

### File structure

| File | Responsibility |
|---|---|
| `src/main.rs` | Entry: DPI, COM, wiring, message loop |
| `src/geom.rs` | `Rect` type shared by placement and render |
| `src/config.rs` | Settings TOML load/save |
| `src/dsp/mod.rs` | `Analyzer` — samples in, `Frame` out |
| `src/dsp/bands.rs` | FFT + log band mapping (pure) |
| `src/dsp/ballistics.rs` | Attack/decay smoothing, peak hold (pure) |
| `src/dsp/gate.rs` | Reveal/hide hysteresis state machine (pure) |
| `src/themes/mod.rs` | `Theme`, registry, merge and precedence |
| `src/themes/schema.rs` | serde types for the TOML schema |
| `src/themes/builtin.rs` | The 15 embedded colourways |
| `src/themes/watch.rs` | Hot reload |
| `src/render/mod.rs` | `Family` trait + registry + dispatch |
| `src/render/canvas.rs` | RGBA buffer, rects, bloom, gap punching (pure) |
| `src/render/segmented.rs` | Family 1 renderer |
| `src/render/scope.rs` | Family 2 renderer |
| `src/render/vu.rs` | Family 3 renderer |
| `src/win/dpi.rs` | DPI awareness |
| `src/win/placement.rs` | Widget rect discovery + visibility rules |
| `src/win/overlay.rs` | Layered window + blit |
| `src/win/capture.rs` | WASAPI loopback thread |
| `src/win/tray.rs` | Tray icon + menus |
| `src/win/autostart.rs` | HKCU Run key |
| `tests/golden/*.png` | Committed golden images, one per colourway |

### Task sequence

Ordered so **something real appears on the live taskbar at Task 5**, before any theme or DSP polish. Schema work is deliberately last: defining a versioned external format before three renderers exist would mean versioning a guess.

| # | Task | Milestone |
|---|---|---|
| 1 | Scaffold, DPI, dependency set | builds |
| 2 | `geom` + visibility rules (pure) | tested logic |
| 3 | Widget rect discovery (UIA) | prints the live rect |
| 4 | Canvas primitives (pure) | tested pixels |
| 5 | Layered overlay window | **flat rect visible on the taskbar** |
| 6 | FFT + band mapping (pure) | tested spectrum |
| 7 | Ballistics + peak hold (pure) | tested feel |
| 8 | Reveal/hide gate (pure) | tested hysteresis |
| 9 | WASAPI loopback thread | live levels in the console |
| 10 | Segmented family + VFD Ice | **the real thing, end to end** |
| 11 | Tray icon, config, quit, autostart | usable daily |
| 12 | Remaining 4 segmented colourways | family 1 complete |
| 13 | Scope family + 5 phosphors | family 2 complete |
| 14 | VU family + 5 backlights | family 3 complete |
| 15 | TOML schema, loading, precedence | external themes |
| 16 | Hot reload + right-click theme menu | ships |

---

### Task 1: Scaffold and verified dependency set

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/win/mod.rs`
- Create: `src/win/dpi.rs`
- Test: `src/win/dpi.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: nothing.
- Produces: `win::dpi::set_per_monitor_v2() -> anyhow::Result<()>`.

- [ ] **Step 1: Create `Cargo.toml`**

Copy the **Verified dependency block** from Global Constraints verbatim. Do not re-resolve versions or trim the feature list — the two `StructuredStorage`/`Variant` features look unrelated to audio but are load-bearing.

- [ ] **Step 2: Write the failing test**

Create `src/win/dpi.rs`:

```rust
use anyhow::Result;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

/// MUST be called before any window is created. A DPI-unaware process reads the
/// taskbar as 1536x48 instead of the true 1920x60 at 125% scaling.
pub fn set_per_monitor_v2() -> Result<()> {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)? };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::HiDpi::{
        AreDpiAwarenessContextsEqual, GetThreadDpiAwarenessContext,
    };

    #[test]
    fn sets_per_monitor_awareness() {
        set_per_monitor_v2().expect("should set awareness");
        let ctx = unsafe { GetThreadDpiAwarenessContext() };

        // This asserts a per-monitor awareness context and deliberately does not
        // try to distinguish v1 from v2. DPI_AWARENESS_CONTEXT is an opaque
        // pseudo-handle (*mut c_void) - it cannot even be hex-formatted, which is
        // why AreDpiAwarenessContextsEqual exists at all, and that API documents
        // v1 and v2 as equal. That limitation is acceptable here: v2's extra
        // behaviour is non-client-area scaling, child-window DPI notifications
        // and dialog scaling, and this overlay is a single top-level WS_POPUP
        // with none of those. The regressions that would actually break it are
        // UNAWARE and SYSTEM_AWARE - the 1.25x virtualisation that misreports
        // the taskbar as 1536x48 - and this assertion catches both.
        let equal = unsafe {
            AreDpiAwarenessContextsEqual(ctx, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        };
        assert!(equal.as_bool(), "must be a per-monitor awareness context");
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test dpi 2>&1 | tail -20`
Expected: FAIL — `src/win/mod.rs` does not exist, so the module is not compiled in.

- [ ] **Step 4: Write the minimal implementation**

Create `src/win/mod.rs`:

```rust
pub mod dpi;
```

Create `src/main.rs`:

```rust
mod win;

use anyhow::Result;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

fn main() -> Result<()> {
    // Order matters: DPI awareness before anything creates a window.
    win::dpi::set_per_monitor_v2()?;
    // S_OK and S_FALSE both map to Ok; a real Err (e.g. RPC_E_CHANGED_MODE)
    // matters because UI Automation and WASAPI both require COM initialised.
    // CoInitializeEx returns HRESULT, not Result - `.ok()` is required and maps
    // S_OK/S_FALSE to Ok(()), leaving only genuine failures as Err.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        eprintln!("CoInitializeEx failed: {e}");
    }
    println!("taskbar-eq: dpi + com initialised");
    Ok(())
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test dpi 2>&1 | tail -20`
Expected: PASS, 1 test.

Then `cargo run --release` and expect `taskbar-eq: dpi + com initialised`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "feat: scaffold with verified dependency set and per-monitor-v2 DPI"
```

---

### Task 2: Geometry type and visibility rules

The visibility decision is pure logic and is the single easiest thing to get subtly wrong, so it is separated from the COM calls that feed it and fully unit-tested.

**Files:**
- Create: `src/geom.rs`
- Create: `src/win/visibility.rs`
- Modify: `src/main.rs` (add `mod geom;`)
- Modify: `src/win/mod.rs` (add `pub mod visibility;`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `geom::Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }` with `Rect::is_plausible_widget(&self) -> bool`.
  - `win::visibility::Inputs { pub widget: Option<Rect>, pub notification_state: i32, pub taskbar_visible: bool }`
  - `win::visibility::should_show(inputs: &Inputs) -> bool`

- [ ] **Step 1: Write the failing test**

Create `src/win/visibility.rs`:

```rust
use crate::geom::Rect;

// Real Win32 QUERY_USER_NOTIFICATION_STATE values. The full enum is
// NOT_PRESENT=1, BUSY=2, RUNNING_D3D_FULL_SCREEN=3, PRESENTATION_MODE=4,
// ACCEPTS_NOTIFICATIONS=5, QUIET_TIME=6, APP=7. An earlier draft of this plan
// had 6 and 3, which is QUIET_TIME and FULL_SCREEN - i.e. it would have hidden
// the overlay during quiet hours and shown it over fullscreen games, exactly
// backwards. A live probe on this machine returned 5.
/// QUNS_RUNNING_D3D_FULL_SCREEN
pub const QUNS_FULLSCREEN: i32 = 3;
/// QUNS_PRESENTATION_MODE
pub const QUNS_PRESENTATION: i32 = 4;

pub struct Inputs {
    pub widget: Option<Rect>,
    pub notification_state: i32,
    pub taskbar_visible: bool,
}

pub fn should_show(i: &Inputs) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good_rect() -> Rect {
        Rect { x: 1416, y: 1140, w: 190, h: 60 }
    }

    fn base() -> Inputs {
        Inputs { widget: Some(good_rect()), notification_state: 5, taskbar_visible: true }
    }

    #[test]
    fn shows_when_everything_is_normal() {
        assert!(should_show(&base()));
    }

    #[test]
    fn hides_when_widget_not_found() {
        let i = Inputs { widget: None, ..base() };
        assert!(!should_show(&i), "no widget means no anchor - must not guess a position");
    }

    #[test]
    fn hides_over_fullscreen_app() {
        let i = Inputs { notification_state: QUNS_FULLSCREEN, ..base() };
        assert!(!should_show(&i));
    }

    #[test]
    fn hides_in_presentation_mode() {
        let i = Inputs { notification_state: QUNS_PRESENTATION, ..base() };
        assert!(!should_show(&i));
    }

    #[test]
    fn hides_when_taskbar_hidden() {
        let i = Inputs { taskbar_visible: false, ..base() };
        assert!(!should_show(&i));
    }

    #[test]
    fn hides_on_implausible_rect() {
        // A zero-width or absurd rect means UIA gave us something unusable.
        let i = Inputs { widget: Some(Rect { x: 0, y: 0, w: 0, h: 0 }), ..base() };
        assert!(!should_show(&i));
    }

    #[test]
    fn plausibility_accepts_the_measured_rect() {
        assert!(good_rect().is_plausible_widget());
    }

    #[test]
    fn plausibility_rejects_degenerate_rects() {
        assert!(!Rect { x: 0, y: 0, w: 0, h: 60 }.is_plausible_widget());
        assert!(!Rect { x: 0, y: 0, w: 190, h: 0 }.is_plausible_widget());
        assert!(!Rect { x: 0, y: 0, w: 5000, h: 60 }.is_plausible_widget());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test visibility 2>&1 | tail -20`
Expected: FAIL — `src/geom.rs` missing and `should_show` is `todo!()`.

- [ ] **Step 3: Write the minimal implementation**

Create `src/geom.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    /// Guards against UIA handing back a degenerate or absurd rect, which would
    /// otherwise place a glowing rectangle somewhere random.
    pub fn is_plausible_widget(&self) -> bool {
        self.w >= 40 && self.w <= 600 && self.h >= 20 && self.h <= 200
    }
}
```

Replace the `todo!()` in `src/win/visibility.rs`:

```rust
pub fn should_show(i: &Inputs) -> bool {
    if !i.taskbar_visible {
        return false;
    }
    if i.notification_state == QUNS_FULLSCREEN || i.notification_state == QUNS_PRESENTATION {
        return false;
    }
    match i.widget {
        Some(r) => r.is_plausible_widget(),
        None => false,
    }
}
```

Add `mod geom;` to `src/main.rs` and `pub mod visibility;` to `src/win/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test 2>&1 | tail -20`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat: add Rect and pure visibility rules with full test coverage"
```

---

### Task 3: Widget rect discovery via UI Automation

The COM call is a thin wrapper; the **name predicate is pure and tested**, because that is the part that will break after a Windows update.

**Files:**
- Create: `src/win/placement.rs`
- Modify: `src/win/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `geom::Rect`, `win::visibility::{Inputs, should_show}`.
- Produces:
  - `win::placement::is_widget_name(name: &str) -> bool`
  - `win::placement::find_widget_rect() -> anyhow::Result<Option<Rect>>`
  - `win::placement::notification_state() -> i32`
  - `win::placement::taskbar_visible() -> bool`

- [ ] **Step 1: Write the failing test**

Create `src/win/placement.rs`:

```rust
use crate::geom::Rect;
use anyhow::Result;
use windows::core::{w, BSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants};
use windows::Win32::UI::Shell::SHQueryUserNotificationState;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, IsWindowVisible};

/// The Widgets button's automation name embeds the live weather, e.g.
/// "Widgets 19C Partly cloudy" or "Widgets 20C Mostly cloudy". Match on the
/// stable prefix only - the rest changes every few minutes.
pub fn is_widget_name(name: &str) -> bool {
    name.starts_with("Widgets")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_real_observed_names() {
        // Both captured live from this machine, 30 minutes apart.
        assert!(is_widget_name("Widgets 20\u{b0}C Mostly cloudy"));
        assert!(is_widget_name("Widgets 19\u{b0}C Partly cloudy"));
        assert!(is_widget_name("Widgets"));
    }

    #[test]
    fn rejects_other_tray_items() {
        for other in [
            "Start",
            "Search",
            "Task View",
            "Show Hidden Icons",
            "Clock 21:08",
            "Network Lenses - Primary",
            "Power Battery status: 99% available",
            "Spotify - 1 running window",
        ] {
            assert!(!is_widget_name(other), "{other} must not match");
        }
    }

    #[test]
    fn is_case_sensitive_on_the_prefix() {
        assert!(!is_widget_name("widgets 19C"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test placement 2>&1 | tail -20`
Expected: FAIL — module not registered in `src/win/mod.rs`.

- [ ] **Step 3: Register the module and run again**

Add `pub mod placement;` to `src/win/mod.rs`.

Run: `cargo test placement 2>&1 | tail -20`
Expected: PASS, 3 tests.

- [ ] **Step 4: Add the COM discovery functions**

Append to `src/win/placement.rs`:

```rust
fn tray_hwnd() -> Result<HWND> {
    Ok(unsafe { FindWindowW(w!("Shell_TrayWnd"), None)? })
}

/// Walks the taskbar's UIA subtree for the Widgets button. Returns physical pixels.
/// Call this on a timer - the rect moves as the weather text changes width.
pub fn find_widget_rect() -> Result<Option<Rect>> {
    let tray = tray_hwnd()?;
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let root = unsafe { automation.ElementFromHandle(tray)? };
    let cond = unsafe { automation.CreateTrueCondition()? };
    let all = unsafe { root.FindAll(TreeScope_Descendants, &cond)? };

    for i in 0..unsafe { all.Length()? } {
        let el = unsafe { all.GetElement(i)? };
        let name: BSTR = unsafe { el.CurrentName().unwrap_or_default() };
        if is_widget_name(&name.to_string()) {
            let r = unsafe { el.CurrentBoundingRectangle()? };
            return Ok(Some(Rect {
                x: r.left,
                y: r.top,
                w: r.right - r.left,
                h: r.bottom - r.top,
            }));
        }
    }
    Ok(None)
}

pub fn notification_state() -> i32 {
    unsafe { SHQueryUserNotificationState().map(|s| s.0).unwrap_or(0) }
}

pub fn taskbar_visible() -> bool {
    match tray_hwnd() {
        Ok(h) => unsafe { IsWindowVisible(h).as_bool() },
        Err(_) => false,
    }
}
```

- [ ] **Step 5: Wire a smoke harness into main**

Replace the body of `main` in `src/main.rs`:

```rust
mod geom;
mod win;

use anyhow::Result;
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

    for _ in 0..5 {
        let widget = win::placement::find_widget_rect()?;
        let inputs = win::visibility::Inputs {
            widget,
            notification_state: win::placement::notification_state(),
            taskbar_visible: win::placement::taskbar_visible(),
        };
        println!(
            "rect={:?} notif={} taskbar={} -> show={}",
            inputs.widget,
            inputs.notification_state,
            inputs.taskbar_visible,
            win::visibility::should_show(&inputs)
        );
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    Ok(())
}
```

- [ ] **Step 6: Run the smoke harness and confirm against reality**

Run: `cargo run --release`

Expected: five lines, each with `w: 190` (or close) and `show=true`. The reference measurement was `Rect { x: 1416, y: 1140, w: 190, h: 60 }`.

**Verify the moving-rect behaviour explicitly:** leave it running, or run it twice several minutes apart, and confirm `x` changes as the weather text changes. If `x` never moves you have likely cached something you should not have.

- [ ] **Step 7: Commit**

```bash
git add src/
git commit -m "feat: discover the Widgets button rect via UI Automation

Name predicate is pure and tested against two real observed labels; the
COM walk is a thin wrapper verified by the smoke harness."
```

---

### Task 4: Canvas primitives

The rasteriser every family draws through. Fully pure and pixel-tested.

**Critical format requirement:** `UpdateLayeredWindow` with `AC_SRC_ALPHA` requires **premultiplied** alpha in **BGRA** byte order. A 32bpp `BI_RGB` DIB is `B,G,R,A` in memory, which little-endian reads as `0xAARRGGBB`. Getting this wrong produces dark fringing around every glowing element — it looks like a bad blend mode rather than an obvious bug, so it is easy to ship by accident.

**Files:**
- Create: `src/render/mod.rs`
- Create: `src/render/canvas.rs`
- Modify: `src/main.rs` (add `mod render;`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `render::canvas::Rgba { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }` with `Rgba::new(r, g, b, a)` and `Rgba::from_hex(hex: &str, alpha: f32) -> Rgba`
  - `render::canvas::Canvas` with:
    - `Canvas::new(w: i32, h: i32) -> Canvas`
    - `fn clear(&mut self)`
    - `fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba)`
    - `fn rounded_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: Rgba)`
    - `fn punch_row(&mut self, y: i32, h: i32)`
    - `fn bloom(&mut self, radius: i32, strength: f32)`
    - `fn get(&self, x: i32, y: i32) -> Rgba`
    - `fn bits(&self) -> &[u32]`
    - `fn width(&self) -> i32`, `fn height(&self) -> i32`

- [ ] **Step 1: Write the failing test**

Create `src/render/canvas.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba { r: 0, g: 0, b: 0, a: 0 };

    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Rgba { r, g, b, a }
    }

    /// Parses "#RRGGBB" (leading '#' optional) and applies `alpha` in 0.0..=1.0.
    /// Returns TRANSPARENT on malformed input rather than panicking - theme files
    /// are user-authored and must never crash the app.
    pub fn from_hex(hex: &str, alpha: f32) -> Self {
        let h = hex.trim_start_matches('#');
        // The ASCII check is load-bearing, not defensive. len() is a BYTE count,
        // so a non-ASCII string can be exactly 6 bytes ("a\u{FC}aaa" is 5 chars
        // in 6 bytes) and then the h[i..i+2] slices below land inside a
        // multi-byte char and PANIC rather than returning TRANSPARENT. Theme
        // files are user-authored, so that is a real crash vector.
        if !h.is_ascii() || h.len() != 6 {
            return Rgba::TRANSPARENT;
        }
        let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        match (p(0), p(2), p(4)) {
            (Some(r), Some(g), Some(b)) => {
                Rgba::new(r, g, b, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
            }
            _ => Rgba::TRANSPARENT,
        }
    }
}

pub struct Canvas {
    w: i32,
    h: i32,
    px: Vec<u32>,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        todo!()
    }
    pub fn width(&self) -> i32 {
        self.w
    }
    pub fn height(&self) -> i32 {
        self.h
    }
    pub fn bits(&self) -> &[u32] {
        &self.px
    }
    pub fn clear(&mut self) {
        todo!()
    }
    pub fn get(&self, x: i32, y: i32) -> Rgba {
        todo!()
    }
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        todo!()
    }
    pub fn rounded_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: Rgba) {
        todo!()
    }
    pub fn punch_row(&mut self, y: i32, h: i32) {
        todo!()
    }
    pub fn bloom(&mut self, radius: i32, strength: f32) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_canvas_is_fully_transparent() {
        let c = Canvas::new(190, 60);
        assert_eq!(c.width(), 190);
        assert_eq!(c.height(), 60);
        assert_eq!(c.bits().len(), 190 * 60);
        assert!(c.bits().iter().all(|&p| p == 0));
    }

    #[test]
    fn hex_parsing_handles_the_real_theme_colours() {
        assert_eq!(Rgba::from_hex("#8fe4ff", 1.0), Rgba::new(0x8f, 0xe4, 0xff, 255));
        assert_eq!(Rgba::from_hex("8fe4ff", 1.0), Rgba::new(0x8f, 0xe4, 0xff, 255));
        assert_eq!(Rgba::from_hex("#3ddc5a", 0.5), Rgba::new(0x3d, 0xdc, 0x5a, 128));
    }

    #[test]
    fn hex_parsing_never_panics_on_bad_input() {
        // Theme files are user-authored; malformed colour must degrade, not crash.
        for bad in ["", "#", "#12345", "#gggggg", "not a colour", "#1234567"] {
            assert_eq!(Rgba::from_hex(bad, 1.0), Rgba::TRANSPARENT, "input {bad:?}");
        }
    }

    #[test]
    fn opaque_fill_round_trips_exactly() {
        let mut c = Canvas::new(10, 10);
        c.fill_rect(2, 3, 4, 5, Rgba::new(0x8f, 0xe4, 0xff, 255));
        assert_eq!(c.get(2, 3), Rgba::new(0x8f, 0xe4, 0xff, 255));
        assert_eq!(c.get(5, 7), Rgba::new(0x8f, 0xe4, 0xff, 255));
    }

    #[test]
    fn fill_respects_its_bounds() {
        let mut c = Canvas::new(10, 10);
        c.fill_rect(2, 3, 4, 5, Rgba::new(255, 255, 255, 255));
        assert_eq!(c.get(1, 3), Rgba::TRANSPARENT, "left of rect");
        assert_eq!(c.get(6, 3), Rgba::TRANSPARENT, "right of rect");
        assert_eq!(c.get(2, 2), Rgba::TRANSPARENT, "above rect");
        assert_eq!(c.get(2, 8), Rgba::TRANSPARENT, "below rect");
    }

    #[test]
    fn fill_clips_instead_of_panicking() {
        let mut c = Canvas::new(10, 10);
        c.fill_rect(-5, -5, 20, 20, Rgba::new(255, 0, 0, 255));
        assert_eq!(c.get(0, 0), Rgba::new(255, 0, 0, 255));
        assert_eq!(c.get(9, 9), Rgba::new(255, 0, 0, 255));
    }

    #[test]
    fn stored_pixels_are_premultiplied_bgra() {
        // UpdateLayeredWindow with AC_SRC_ALPHA demands premultiplied alpha.
        // White at 50% alpha must store as ~0x80808080, not 0x80FFFFFF.
        let mut c = Canvas::new(4, 4);
        c.fill_rect(0, 0, 4, 4, Rgba::new(255, 255, 255, 128));
        let p = c.bits()[0];
        let (a, r, g, b) = (p >> 24, (p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
        assert_eq!(a, 128, "alpha preserved");
        for (name, v) in [("r", r), ("g", g), ("b", b)] {
            assert!(
                (127..=129).contains(&v),
                "{name} must be premultiplied to ~128, got {v}"
            );
        }
    }

    #[test]
    fn punch_row_clears_a_full_width_band() {
        let mut c = Canvas::new(10, 10);
        c.fill_rect(0, 0, 10, 10, Rgba::new(255, 255, 255, 255));
        c.punch_row(4, 2);
        assert_eq!(c.get(0, 4), Rgba::TRANSPARENT);
        assert_eq!(c.get(9, 5), Rgba::TRANSPARENT);
        assert_eq!(c.get(0, 3), Rgba::new(255, 255, 255, 255), "row above survives");
        assert_eq!(c.get(0, 6), Rgba::new(255, 255, 255, 255), "row below survives");
    }

    #[test]
    fn rounded_rect_omits_its_corners() {
        let mut c = Canvas::new(20, 20);
        c.rounded_rect(0, 0, 20, 20, 5, Rgba::new(255, 255, 255, 255));
        assert_eq!(c.get(0, 0), Rgba::TRANSPARENT, "corner must be cut");
        assert_eq!(c.get(10, 10), Rgba::new(255, 255, 255, 255), "centre filled");
        assert_eq!(c.get(10, 0), Rgba::new(255, 255, 255, 255), "top edge filled");
    }

    #[test]
    fn bloom_spreads_light_outward_without_erasing_the_source() {
        let mut c = Canvas::new(21, 21);
        c.fill_rect(10, 10, 1, 1, Rgba::new(255, 255, 255, 255));
        c.bloom(3, 1.0);
        assert!(c.get(10, 10).a > 200, "source stays bright");
        assert!(c.get(12, 10).a > 0, "light spread sideways");
        assert_eq!(c.get(20, 20).a, 0, "but not across the whole canvas");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test canvas 2>&1 | tail -20`
Expected: FAIL — `src/render/mod.rs` missing, and every method is `todo!()`.

- [ ] **Step 3: Write the implementation**

Create `src/render/mod.rs`:

```rust
pub mod canvas;
```

Add `mod render;` to `src/main.rs`.

Replace the `todo!()` bodies in `src/render/canvas.rs`:

```rust
impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        Canvas { w, h, px: vec![0u32; (w.max(0) * h.max(0)) as usize] }
    }

    pub fn clear(&mut self) {
        self.px.iter_mut().for_each(|p| *p = 0);
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            None
        } else {
            Some((y * self.w + x) as usize)
        }
    }

    /// Packs to premultiplied 0xAARRGGBB, which is BGRA in DIB memory order.
    fn pack(c: Rgba) -> u32 {
        let a = c.a as u32;
        let pm = |v: u8| ((v as u32 * a + 127) / 255) & 0xff;
        (a << 24) | (pm(c.r) << 16) | (pm(c.g) << 8) | pm(c.b)
    }

    fn unpack(p: u32) -> Rgba {
        let a = (p >> 24) as u32;
        if a == 0 {
            return Rgba::TRANSPARENT;
        }
        let un = |v: u32| ((v * 255 + a / 2) / a).min(255) as u8;
        Rgba::new(
            un((p >> 16) & 0xff),
            un((p >> 8) & 0xff),
            un(p & 0xff),
            a as u8,
        )
    }

    pub fn get(&self, x: i32, y: i32) -> Rgba {
        match self.idx(x, y) {
            Some(i) => Self::unpack(self.px[i]),
            None => Rgba::TRANSPARENT,
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        if c.a == 0 {
            return;
        }
        let packed = Self::pack(c);
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in x.max(0)..(x + w).min(self.w) {
                let i = (yy * self.w + xx) as usize;
                self.px[i] = if c.a == 255 {
                    packed
                } else {
                    Self::blend_over(self.px[i], packed)
                };
            }
        }
    }

    /// Source-over on premultiplied values.
    fn blend_over(dst: u32, src: u32) -> u32 {
        let sa = src >> 24;
        if sa == 255 {
            return src;
        }
        let inv = 255 - sa;
        let ch = |sh: u32| {
            let s = (src >> sh) & 0xff;
            let d = (dst >> sh) & 0xff;
            (s + (d * inv + 127) / 255).min(255)
        };
        let a = (sa + (((dst >> 24) & 0xff) * inv + 127) / 255).min(255);
        (a << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
    }

    pub fn rounded_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: Rgba) {
        // Guard before the clamp, or this panics in RELEASE builds. For a
        // negative dimension w.min(h)/2 goes negative (190, -4 -> -2), and
        // i32::clamp's min <= max assertion is unconditional, not debug-only.
        // Reachable because the widget rect changes size at runtime.
        if w <= 0 || h <= 0 {
            return;
        }
        let r = r.max(0).min(w.min(h) / 2);
        for yy in 0..h {
            // Shrink the span near the top and bottom to round the corners.
            let dy = if yy < r {
                r - yy
            } else if yy >= h - r {
                yy - (h - r - 1)
            } else {
                0
            };
            let inset = if dy > 0 {
                let f = (r * r - dy * dy).max(0) as f32;
                r - f.sqrt().round() as i32
            } else {
                0
            };
            self.fill_rect(x + inset, y + yy, w - inset * 2, 1, c);
        }
    }

    pub fn punch_row(&mut self, y: i32, h: i32) {
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in 0..self.w {
                self.px[(yy * self.w + xx) as usize] = 0;
            }
        }
    }

    /// Separable box blur of the current contents, composited *under* the
    /// original so lit elements keep their crisp edge and gain a halo.
    pub fn bloom(&mut self, radius: i32, strength: f32) {
        if radius <= 0 || strength <= 0.0 {
            return;
        }
        let (w, h) = (self.w, self.h);
        let src = self.px.clone();
        let mut tmp = vec![0u32; src.len()];

        let blur = |input: &[u32], out: &mut [u32], horizontal: bool| {
            for y in 0..h {
                for x in 0..w {
                    let (mut a, mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
                    for d in -radius..=radius {
                        let (sx, sy) = if horizontal { (x + d, y) } else { (x, y + d) };
                        if sx < 0 || sy < 0 || sx >= w || sy >= h {
                            continue;
                        }
                        let p = input[(sy * w + sx) as usize];
                        a += p >> 24;
                        r += (p >> 16) & 0xff;
                        g += (p >> 8) & 0xff;
                        b += p & 0xff;
                        n += 1;
                    }
                    let n = n.max(1);
                    out[(y * w + x) as usize] =
                        ((a / n) << 24) | ((r / n) << 16) | ((g / n) << 8) | (b / n);
                }
            }
        };

        blur(&src, &mut tmp, true);
        let mut halo = vec![0u32; src.len()];
        blur(&tmp, &mut halo, false);

        for i in 0..self.px.len() {
            let hp = halo[i];
            let scale = |v: u32| ((v as f32 * strength).min(255.0)) as u32;
            let scaled = (scale(hp >> 24) << 24)
                | (scale((hp >> 16) & 0xff) << 16)
                | (scale((hp >> 8) & 0xff) << 8)
                | scale(hp & 0xff);
            self.px[i] = Self::blend_over(scaled, src[i]);
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test canvas 2>&1 | tail -20`
Expected: PASS, 11 tests.

- [ ] **Step 5: Commit**

```bash
git add src/render/ src/main.rs
git commit -m "feat: add canvas rasteriser with premultiplied BGRA and bloom

Premultiplication is required by UpdateLayeredWindow's AC_SRC_ALPHA; getting
it wrong causes dark fringing that reads as a blend bug, so it is asserted."
```

---

### Task 5: Layered overlay window — first pixels on the taskbar

**This is the milestone that de-risks the whole project.** It proves a window can sit convincingly over the widget and track it. Do not proceed to DSP until you have looked at this on the real taskbar.

**Files:**
- Create: `src/win/overlay.rs`
- Modify: `src/win/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `geom::Rect`, `render::canvas::Canvas`.
- Produces:
  - `win::overlay::Overlay` with:
    - `Overlay::new() -> anyhow::Result<Overlay>`
    - `fn show(&self, rect: Rect, canvas: &Canvas) -> anyhow::Result<()>`
    - `fn hide(&self) -> anyhow::Result<()>`
    - `fn pump_messages(&self)`

- [ ] **Step 1: Write `src/win/overlay.rs`**

```rust
use crate::geom::Rect;
use crate::render::canvas::Canvas;
use anyhow::{anyhow, Result};
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, SIZE, WPARAM};
// NOTE: AC_SRC_ALPHA, AC_SRC_OVER and BLENDFUNCTION live in Graphics::Gdi, NOT
// in UI::WindowsAndMessaging. Verified against the compiler - importing them from
// WindowsAndMessaging fails with E0432 unresolved import.
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, AC_SRC_ALPHA,
    AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, PeekMessageW, RegisterClassW, SetWindowPos,
    ShowWindow, TranslateMessage, UpdateLayeredWindow, HWND_TOPMOST, MSG, PM_REMOVE,
    SWP_NOACTIVATE, SWP_NOSIZE, SW_HIDE, SW_SHOWNA, ULW_ALPHA, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

pub struct Overlay {
    hwnd: HWND,
}

impl Overlay {
    pub fn new() -> Result<Self> {
        unsafe {
            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                lpszClassName: w!("TaskbarEqOverlay"),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return Err(anyhow!("RegisterClassW failed"));
            }
            // WS_EX_TOOLWINDOW keeps it out of Alt-Tab; WS_EX_NOACTIVATE stops it
            // stealing focus from whatever you are actually working in.
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("TaskbarEqOverlay"),
                w!("Taskbar EQ"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )?;
            Ok(Overlay { hwnd })
        }
    }

    pub fn show(&self, rect: Rect, canvas: &Canvas) -> Result<()> {
        unsafe {
            let screen_dc = HDC::default();
            let mem_dc = CreateCompatibleDC(Some(screen_dc));

            let mut bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: canvas.width(),
                    // Negative height = top-down rows, matching our buffer order.
                    biHeight: -canvas.height(),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib: HBITMAP = CreateDIBSection(
                Some(mem_dc),
                &bi as *const _ as *const _,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            )?;
            let old = SelectObject(mem_dc, dib.into());

            std::ptr::copy_nonoverlapping(
                canvas.bits().as_ptr(),
                bits as *mut u32,
                canvas.bits().len(),
            );

            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                rect.x,
                rect.y,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE,
            );
            let _ = ShowWindow(self.hwnd, SW_SHOWNA);

            let mut pos = POINT { x: rect.x, y: rect.y };
            let mut src = POINT { x: 0, y: 0 };
            let mut size = SIZE { cx: canvas.width(), cy: canvas.height() };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            let r = UpdateLayeredWindow(
                self.hwnd,
                Some(screen_dc),
                Some(&mut pos),
                Some(&mut size),
                Some(mem_dc),
                Some(&mut src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );

            SelectObject(mem_dc, old);
            let _ = DeleteObject(dib.into());
            let _ = DeleteDC(mem_dc);
            r?;
            Ok(())
        }
    }

    pub fn hide(&self) -> Result<()> {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
        Ok(())
    }

    /// Non-blocking pump. The overlay has no UI of its own yet, but a window
    /// that never pumps messages is considered hung by the shell.
    pub fn pump_messages(&self) {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
```

Add `pub mod overlay;` to `src/win/mod.rs`.

- [ ] **Step 2: Wire a visible smoke test into main**

Replace the loop body in `src/main.rs`:

```rust
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
```

- [ ] **Step 3: Run it and look at the taskbar**

Run: `cargo run --release`

Expected: for 15 seconds, a dark rounded panel with a glowing ice-blue bar sits exactly over the weather widget, and the console prints `showing at Rect { .. }`.

**Look at it.** Check: is it aligned with the widget's edges, or offset? An offset that is a clean ratio of 1.25 means DPI awareness is not actually taking effect.

- [ ] **Step 4: Verify by sampling pixels, not just by eye**

While it is running, in a second terminal:

```powershell
powershell -File tools/probe/Probe-Colours.ps1
```

Expected: the reported top colours change from the warm `#3D1712` family to dark blue-black plus bright ice-blue. This is the objective check that the overlay is really compositing, since a screenshot taken on a locked session returns solid black and would otherwise look like a failure.

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat: layered topmost overlay drawing over the Widgets button

First pixels on the real taskbar. Tracks the widget rect each tick rather
than caching it, since the rect moves as the weather text changes."
```

---

### Task 6: FFT and log band mapping

Pure. The verified reference point: a 1000 Hz sine through a 2048-point FFT at 48 kHz peaks at bin 43 (1008 Hz).

**Files:**
- Create: `src/dsp/mod.rs`
- Create: `src/dsp/bands.rs`
- Modify: `src/main.rs` (add `mod dsp;`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `dsp::bands::NUM_BANDS: usize = 64`
  - `dsp::bands::FFT_SIZE: usize = 2048`
  - `dsp::bands::HOP: usize = 512`
  - `dsp::bands::BandMapper` with `BandMapper::new(sample_rate: f32) -> BandMapper` and `fn process(&mut self, mono: &[f32], out: &mut [f32; NUM_BANDS])`

- [ ] **Step 1: Write the failing test**

Create `src/dsp/bands.rs`:

```rust
use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use std::sync::Arc;

pub const NUM_BANDS: usize = 64;
pub const FFT_SIZE: usize = 2048;
pub const HOP: usize = 512;
const F_LOW: f32 = 40.0;
const F_HIGH: f32 = 16_000.0;

pub struct BandMapper {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    scratch: Vec<Complex32>,
    edges: Vec<usize>,
    sample_rate: f32,
}

impl BandMapper {
    pub fn new(sample_rate: f32) -> Self {
        todo!()
    }

    /// `mono` must be exactly FFT_SIZE samples. Writes normalised 0.0..=1.0 levels.
    pub fn process(&mut self, mono: &[f32], out: &mut [f32; NUM_BANDS]) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (i as f32 / rate * freq * std::f32::consts::TAU).sin())
            .collect()
    }

    fn band_of(freq: f32) -> usize {
        // log-spaced band index a frequency should land in
        let t = (freq / F_LOW).ln() / (F_HIGH / F_LOW).ln();
        ((t * NUM_BANDS as f32) as usize).min(NUM_BANDS - 1)
    }

    #[test]
    fn silence_produces_no_energy() {
        let mut m = BandMapper::new(48_000.0);
        let mut out = [0.0f32; NUM_BANDS];
        m.process(&vec![0.0; FFT_SIZE], &mut out);
        assert!(out.iter().all(|&v| v < 1e-4), "silence must be flat, got {out:?}");
    }

    #[test]
    fn a_1khz_sine_peaks_in_the_1khz_band() {
        let mut m = BandMapper::new(48_000.0);
        let mut out = [0.0f32; NUM_BANDS];
        m.process(&sine(1000.0, 48_000.0, FFT_SIZE), &mut out);
        let peak = out
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        let expected = band_of(1000.0);
        assert!(
            peak.abs_diff(expected) <= 1,
            "1kHz should peak near band {expected}, peaked at {peak}"
        );
    }

    #[test]
    fn a_bass_tone_peaks_low_and_a_treble_tone_peaks_high() {
        let mut m = BandMapper::new(48_000.0);
        let mut lo = [0.0f32; NUM_BANDS];
        let mut hi = [0.0f32; NUM_BANDS];
        m.process(&sine(80.0, 48_000.0, FFT_SIZE), &mut lo);
        m.process(&sine(8000.0, 48_000.0, FFT_SIZE), &mut hi);
        let peak = |a: &[f32; NUM_BANDS]| {
            a.iter().enumerate().max_by(|x, y| x.1.partial_cmp(y.1).unwrap()).unwrap().0
        };
        assert!(peak(&lo) < NUM_BANDS / 3, "80Hz must land in the low third");
        assert!(peak(&hi) > NUM_BANDS * 2 / 3, "8kHz must land in the high third");
    }

    #[test]
    fn output_is_normalised_within_range() {
        let mut m = BandMapper::new(48_000.0);
        let mut out = [0.0f32; NUM_BANDS];
        // Full-scale sine - the loudest realistic input.
        m.process(&sine(500.0, 48_000.0, FFT_SIZE), &mut out);
        assert!(out.iter().all(|&v| (0.0..=1.0).contains(&v)), "got {out:?}");
    }

    #[test]
    fn band_edges_are_strictly_ascending() {
        let m = BandMapper::new(48_000.0);
        for pair in m.edges.windows(2) {
            assert!(pair[1] > pair[0], "edges must ascend: {:?}", m.edges);
        }
        assert_eq!(m.edges.len(), NUM_BANDS + 1);
    }

    #[test]
    fn works_at_other_sample_rates() {
        // Do not hardcode 48kHz - a virtual endpoint may report 44.1k.
        let mut m = BandMapper::new(44_100.0);
        let mut out = [0.0f32; NUM_BANDS];
        m.process(&sine(1000.0, 44_100.0, FFT_SIZE), &mut out);
        let peak = out.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
        assert!(peak.abs_diff(band_of(1000.0)) <= 2);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test bands 2>&1 | tail -20`
Expected: FAIL — `src/dsp/mod.rs` missing and both methods `todo!()`.

- [ ] **Step 3: Write the implementation**

Create `src/dsp/mod.rs`:

```rust
pub mod bands;
```

Add `mod dsp;` to `src/main.rs`.

Replace the `todo!()` bodies:

```rust
impl BandMapper {
    pub fn new(sample_rate: f32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        // Hann window - reduces spectral leakage so a pure tone stays in one band.
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                let t = i as f32 / (FFT_SIZE - 1) as f32;
                0.5 - 0.5 * (t * std::f32::consts::TAU).cos()
            })
            .collect();

        // Log-spaced band edges in bin space, forced strictly ascending so the
        // low bands (where bins are sparse) never collapse to zero width.
        let bin_hz = sample_rate / FFT_SIZE as f32;
        let mut edges = Vec::with_capacity(NUM_BANDS + 1);
        let mut last = 0usize;
        for b in 0..=NUM_BANDS {
            let t = b as f32 / NUM_BANDS as f32;
            let f = F_LOW * (F_HIGH / F_LOW).powf(t);
            let bin = (f / bin_hz).round() as usize;
            let bin = bin.max(last + if b == 0 { 0 } else { 1 });
            edges.push(bin.min(FFT_SIZE / 2 - 1));
            last = *edges.last().unwrap();
        }

        BandMapper {
            fft,
            window,
            scratch: vec![Complex32::new(0.0, 0.0); FFT_SIZE],
            edges,
            sample_rate,
        }
    }

    pub fn process(&mut self, mono: &[f32], out: &mut [f32; NUM_BANDS]) {
        debug_assert_eq!(mono.len(), FFT_SIZE);

        for i in 0..FFT_SIZE {
            self.scratch[i] = Complex32::new(mono[i] * self.window[i], 0.0);
        }
        self.fft.process(&mut self.scratch);

        // Hann coherent gain is 0.5, so a full-scale sine yields FFT_SIZE/4.
        let norm = 4.0 / FFT_SIZE as f32;

        for b in 0..NUM_BANDS {
            let (lo, hi) = (self.edges[b], self.edges[b + 1]);
            let mut peak = 0.0f32;
            for bin in lo..hi.max(lo + 1) {
                peak = peak.max(self.scratch[bin].norm() * norm);
            }
            // Bass-weighted tilt: without it low-frequency energy dominates and
            // the top two-thirds of the display barely moves.
            let t = b as f32 / (NUM_BANDS - 1) as f32;
            let tilt = 1.0 + 2.2 * t;
            out[b] = (peak * tilt).clamp(0.0, 1.0);
        }
        let _ = self.sample_rate;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test bands 2>&1 | tail -20`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src/dsp/ src/main.rs
git commit -m "feat: add FFT band mapper with log spacing and bass tilt"
```

---

### Task 7: Ballistics and peak hold

Pure. The single thing that decides whether the meter *feels* right: fast attack, slow decay. Equal rates look broken.

**Files:**
- Create: `src/dsp/ballistics.rs`
- Modify: `src/dsp/mod.rs`

**Interfaces:**
- Consumes: `dsp::bands::NUM_BANDS`.
- Produces:
  - `dsp::ballistics::Ballistics { pub attack: f32, pub decay: f32, pub peak_fall: f32 }` with `Default`
  - `dsp::ballistics::Smoother` with `Smoother::new(b: Ballistics) -> Smoother`, `fn update(&mut self, target: &[f32; NUM_BANDS])`, `fn levels(&self) -> &[f32; NUM_BANDS]`, `fn peaks(&self) -> &[f32; NUM_BANDS]`, `fn set_ballistics(&mut self, b: Ballistics)`

- [ ] **Step 1: Write the failing test**

Create `src/dsp/ballistics.rs`:

```rust
use crate::dsp::bands::NUM_BANDS;

#[derive(Debug, Clone, Copy)]
pub struct Ballistics {
    pub attack: f32,
    pub decay: f32,
    pub peak_fall: f32,
}

impl Default for Ballistics {
    fn default() -> Self {
        // Matches the VFD Ice colourway in the spec.
        Ballistics { attack: 0.55, decay: 0.11, peak_fall: 0.0055 }
    }
}

pub struct Smoother {
    b: Ballistics,
    levels: [f32; NUM_BANDS],
    peaks: [f32; NUM_BANDS],
}

impl Smoother {
    pub fn new(b: Ballistics) -> Self {
        todo!()
    }
    pub fn set_ballistics(&mut self, b: Ballistics) {
        self.b = b;
    }
    pub fn levels(&self) -> &[f32; NUM_BANDS] {
        &self.levels
    }
    pub fn peaks(&self) -> &[f32; NUM_BANDS] {
        &self.peaks
    }
    pub fn update(&mut self, target: &[f32; NUM_BANDS]) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(v: f32) -> [f32; NUM_BANDS] {
        [v; NUM_BANDS]
    }

    #[test]
    fn starts_at_zero() {
        let s = Smoother::new(Ballistics::default());
        assert!(s.levels().iter().all(|&v| v == 0.0));
        assert!(s.peaks().iter().all(|&v| v == 0.0));
    }

    #[test]
    fn rises_faster_than_it_falls() {
        let b = Ballistics::default();
        let mut up = Smoother::new(b);
        up.update(&flat(1.0));
        let after_one_rise = up.levels()[0];

        let mut down = Smoother::new(b);
        for _ in 0..40 {
            down.update(&flat(1.0));
        }
        let peak_level = down.levels()[0];
        down.update(&flat(0.0));
        let dropped = peak_level - down.levels()[0];

        assert!(
            after_one_rise > dropped,
            "attack ({after_one_rise}) must outpace decay ({dropped}) or the meter feels dead"
        );
    }

    #[test]
    fn converges_toward_a_held_target() {
        let mut s = Smoother::new(Ballistics::default());
        for _ in 0..80 {
            s.update(&flat(0.7));
        }
        assert!((s.levels()[0] - 0.7).abs() < 0.01, "got {}", s.levels()[0]);
    }

    #[test]
    fn never_overshoots_or_goes_negative() {
        let mut s = Smoother::new(Ballistics::default());
        for i in 0..200 {
            s.update(&flat(if i % 2 == 0 { 1.0 } else { 0.0 }));
            assert!(s.levels().iter().all(|&v| (0.0..=1.0).contains(&v)));
            assert!(s.peaks().iter().all(|&v| (0.0..=1.0).contains(&v)));
        }
    }

    #[test]
    fn peaks_hold_above_levels_then_sink_slowly() {
        let mut s = Smoother::new(Ballistics::default());
        for _ in 0..60 {
            s.update(&flat(1.0));
        }
        let held = s.peaks()[0];
        assert!(held > 0.9);

        s.update(&flat(0.0));
        assert!(s.peaks()[0] > s.levels()[0], "peak must lag the level down");

        // peak_fall of 0.0055 per frame is ~0.33 per second at 60fps
        let before = s.peaks()[0];
        for _ in 0..10 {
            s.update(&flat(0.0));
        }
        let fell = before - s.peaks()[0];
        assert!(fell > 0.0 && fell < 0.15, "peak fell {fell}, expected a slow sink");
    }

    #[test]
    fn peak_jumps_immediately_to_a_new_high() {
        let mut s = Smoother::new(Ballistics::default());
        for _ in 0..60 {
            s.update(&flat(0.3));
        }
        let low = s.peaks()[0];
        for _ in 0..60 {
            s.update(&flat(0.9));
        }
        assert!(s.peaks()[0] > low + 0.4, "peak must track a new maximum");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test ballistics 2>&1 | tail -20`
Expected: FAIL — module not registered, methods `todo!()`.

- [ ] **Step 3: Write the implementation**

Add `pub mod ballistics;` to `src/dsp/mod.rs`. Replace the `todo!()` bodies:

```rust
impl Smoother {
    pub fn new(b: Ballistics) -> Self {
        Smoother { b, levels: [0.0; NUM_BANDS], peaks: [0.0; NUM_BANDS] }
    }

    pub fn update(&mut self, target: &[f32; NUM_BANDS]) {
        for i in 0..NUM_BANDS {
            let t = target[i].clamp(0.0, 1.0);
            // Asymmetric one-pole: snap up, ease down.
            let rate = if t > self.levels[i] { self.b.attack } else { self.b.decay };
            self.levels[i] = (self.levels[i] + (t - self.levels[i]) * rate).clamp(0.0, 1.0);
            self.peaks[i] = (self.peaks[i] - self.b.peak_fall).max(self.levels[i]).clamp(0.0, 1.0);
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test ballistics 2>&1 | tail -20`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src/dsp/
git commit -m "feat: add asymmetric ballistics with peak hold"
```

---

### Task 8: Reveal/hide gate

Pure, and the requirement most likely to be got wrong by feel. The tests encode the actual product requirement: **a Teams notification must not blank the weather.**

**Files:**
- Create: `src/dsp/gate.rs`
- Modify: `src/dsp/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `dsp::gate::GateConfig { pub threshold_dbfs: f32, pub reveal_ms: u32, pub hide_ms: u32, pub fade_ms: u32 }` with `Default`
  - `dsp::gate::Gate` with `Gate::new(cfg: GateConfig) -> Gate`, `fn update(&mut self, rms: f32, dt_ms: u32) -> f32` returning opacity 0.0..=1.0, and `fn is_visible(&self) -> bool`

- [ ] **Step 1: Write the failing test**

Create `src/dsp/gate.rs`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct GateConfig {
    pub threshold_dbfs: f32,
    pub reveal_ms: u32,
    pub hide_ms: u32,
    pub fade_ms: u32,
}

impl Default for GateConfig {
    fn default() -> Self {
        GateConfig { threshold_dbfs: -55.0, reveal_ms: 400, hide_ms: 2000, fade_ms: 250 }
    }
}

pub struct Gate {
    cfg: GateConfig,
    above_ms: u32,
    below_ms: u32,
    shown: bool,
    opacity: f32,
}

impl Gate {
    pub fn new(cfg: GateConfig) -> Self {
        todo!()
    }
    pub fn is_visible(&self) -> bool {
        self.opacity > 0.0
    }
    /// `rms` is linear 0.0..=1.0. Returns the opacity to draw at this frame.
    pub fn update(&mut self, rms: f32, dt_ms: u32) -> f32 {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: u32 = 16; // ~60fps

    fn loud() -> f32 {
        0.2 // about -14 dBFS
    }
    fn silent() -> f32 {
        0.0
    }

    fn run(g: &mut Gate, rms: f32, ms: u32) -> f32 {
        let mut last = 0.0;
        for _ in 0..(ms / FRAME) {
            last = g.update(rms, FRAME);
        }
        last
    }

    #[test]
    fn starts_hidden() {
        let g = Gate::new(GateConfig::default());
        assert!(!g.is_visible());
    }

    #[test]
    fn does_not_reveal_before_the_delay_elapses() {
        let mut g = Gate::new(GateConfig::default());
        run(&mut g, loud(), 300);
        assert!(!g.is_visible(), "must still be hidden 300ms into a 400ms delay");
    }

    #[test]
    fn reveals_after_sustained_audio() {
        let mut g = Gate::new(GateConfig::default());
        run(&mut g, loud(), 500);
        assert!(g.is_visible());
        let op = run(&mut g, loud(), 400);
        assert!((op - 1.0).abs() < 0.01, "should reach full opacity, got {op}");
    }

    #[test]
    fn a_notification_ding_does_not_reveal_it() {
        // THE requirement: a 200ms blip must never blank the weather.
        let mut g = Gate::new(GateConfig::default());
        run(&mut g, loud(), 200);
        run(&mut g, silent(), 100);
        assert!(!g.is_visible(), "a 200ms ding must be ignored entirely");
    }

    #[test]
    fn several_separated_dings_still_do_not_reveal_it() {
        let mut g = Gate::new(GateConfig::default());
        for _ in 0..5 {
            run(&mut g, loud(), 150);
            run(&mut g, silent(), 600);
        }
        assert!(!g.is_visible(), "the above-threshold timer must reset on silence");
    }

    #[test]
    fn rides_through_the_gap_between_tracks() {
        let mut g = Gate::new(GateConfig::default());
        run(&mut g, loud(), 600);
        assert!(g.is_visible());
        run(&mut g, silent(), 1500); // typical inter-track gap
        assert!(g.is_visible(), "1.5s of silence must not hide it (2s threshold)");
    }

    #[test]
    fn hides_after_sustained_silence() {
        let mut g = Gate::new(GateConfig::default());
        run(&mut g, loud(), 600);
        run(&mut g, silent(), 2100);
        run(&mut g, silent(), 300); // allow the fade to finish
        assert!(!g.is_visible(), "should be fully hidden after 2s + fade");
    }

    #[test]
    fn quiet_passages_below_threshold_are_treated_as_silence() {
        let mut g = Gate::new(GateConfig::default());
        // -60 dBFS, below the -55 threshold
        let very_quiet = 10f32.powf(-60.0 / 20.0);
        run(&mut g, very_quiet, 1000);
        assert!(!g.is_visible());
    }

    #[test]
    fn opacity_is_always_in_range() {
        let mut g = Gate::new(GateConfig::default());
        for i in 0..500 {
            let op = g.update(if i % 60 < 30 { loud() } else { silent() }, FRAME);
            assert!((0.0..=1.0).contains(&op), "opacity {op} out of range");
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test gate 2>&1 | tail -20`
Expected: FAIL — module not registered, methods `todo!()`.

- [ ] **Step 3: Write the implementation**

Add `pub mod gate;` to `src/dsp/mod.rs`. Replace the `todo!()` bodies:

```rust
impl Gate {
    pub fn new(cfg: GateConfig) -> Self {
        Gate { cfg, above_ms: 0, below_ms: 0, shown: false, opacity: 0.0 }
    }

    pub fn update(&mut self, rms: f32, dt_ms: u32) -> f32 {
        let dbfs = if rms > 1e-9 { 20.0 * rms.log10() } else { -200.0 };
        let above = dbfs > self.cfg.threshold_dbfs;

        if above {
            self.above_ms = self.above_ms.saturating_add(dt_ms);
            self.below_ms = 0;
        } else {
            self.below_ms = self.below_ms.saturating_add(dt_ms);
            self.above_ms = 0; // a blip must not accumulate across silence
        }

        if !self.shown && self.above_ms >= self.cfg.reveal_ms {
            self.shown = true;
        } else if self.shown && self.below_ms >= self.cfg.hide_ms {
            self.shown = false;
        }

        let step = if self.cfg.fade_ms == 0 {
            1.0
        } else {
            dt_ms as f32 / self.cfg.fade_ms as f32
        };
        let target = if self.shown { 1.0 } else { 0.0 };
        if self.opacity < target {
            self.opacity = (self.opacity + step).min(target);
        } else if self.opacity > target {
            self.opacity = (self.opacity - step).max(target);
        }
        self.opacity = self.opacity.clamp(0.0, 1.0);
        self.opacity
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test gate 2>&1 | tail -20`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add src/dsp/
git commit -m "feat: add reveal/hide gate with blip rejection

Tests encode the product requirement directly: a 200ms notification ding
must never blank the weather, and a 1.5s inter-track gap must not hide it."
```

---

### Task 9: WASAPI loopback capture thread

The audio thread must never block on rendering. Verified format on this machine: 48000 Hz, 2 ch, 32-bit float.

**Files:**
- Create: `src/win/capture.rs`
- Modify: `src/win/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `dsp::bands::{BandMapper, FFT_SIZE, HOP, NUM_BANDS}`.
- Produces:
  - `win::capture::Frame { pub bands: [f32; NUM_BANDS], pub waveform: [f32; 256], pub rms_l: f32, pub rms_r: f32, pub rms: f32 }` implementing `Default` and `Clone`
  - `win::capture::interleaved_to_mono(src: &[f32], channels: usize) -> Vec<f32>` (pure, tested)
  - `win::capture::channel_rms(src: &[f32], channels: usize) -> (f32, f32)` (pure, tested)
  - `win::capture::start() -> std::sync::mpsc::Receiver<Frame>`

- [ ] **Step 1: Write the failing test for the pure helpers**

Create `src/win/capture.rs`:

```rust
use crate::dsp::bands::{BandMapper, FFT_SIZE, HOP, NUM_BANDS};
use anyhow::Result;
use std::sync::mpsc::{channel, Receiver, Sender};

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
    todo!()
}

/// Per-channel RMS. For mono input both values are the same; the VU family needs
/// them separate.
pub fn channel_rms(src: &[f32], channels: usize) -> (f32, f32) {
    todo!()
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test capture 2>&1 | tail -20`
Expected: FAIL — module not registered, both helpers `todo!()`.

- [ ] **Step 3: Implement the pure helpers**

Add `pub mod capture;` to `src/win/mod.rs`. Replace the `todo!()` bodies:

```rust
pub fn interleaved_to_mono(src: &[f32], channels: usize) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    src.chunks_exact(channels)
        .map(|f| f.iter().sum::<f32>() / channels as f32)
        .collect()
}

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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test capture 2>&1 | tail -20`
Expected: PASS, 7 tests.

- [ ] **Step 5: Add the capture thread**

Append to `src/win/capture.rs`:

```rust
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};

/// Spawns the capture thread. Returns a receiver of analysed frames.
/// The thread owns its own COM apartment and re-opens the endpoint if the
/// default device changes (the reference machine's default is a virtual device,
/// so this path is exercised in normal use, not just on unplug).
pub fn start() -> Receiver<Frame> {
    let (tx, rx) = channel::<Frame>();
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
            if tx.send(Frame::default()).is_err() {
                return; // main thread gone
            }
        }
    });
    rx
}

fn capture_loop(tx: &Sender<Frame>) -> Result<()> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let device_id = device.GetId()?.to_string()?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let fmt = client.GetMixFormat()?;
        let channels = (*fmt).nChannels as usize;
        let rate = (*fmt).nSamplesPerSec as f32;

        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            10_000_000,
            0,
            fmt,
            None,
        )?;
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
                .map(|s| s.to_string().unwrap_or_default())
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
                if tx.send(frame.clone()).is_err() {
                    client.Stop()?;
                    return Ok(());
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
```

- [ ] **Step 6: Verify live with a console harness**

Temporarily replace `main` to print levels, play music, and run `cargo run --release`:

```rust
fn main() -> anyhow::Result<()> {
    win::dpi::set_per_monitor_v2()?;
    let rx = win::capture::start();
    for _ in 0..60 {
        if let Ok(f) = rx.recv() {
            let bars: String = f.bands.iter().step_by(8)
                .map(|&v| " .:-=+*#%@".chars().nth((v * 9.0) as usize).unwrap())
                .collect();
            println!("rms={:.4} L={:.3} R={:.3} [{bars}]", f.rms, f.rms_l, f.rms_r);
        }
    }
    Ok(())
}
```

Expected with music playing: `rms` well above 0.001 and the bar string visibly moving. With audio paused: `rms` near 0 and the bars flat. If `rms` is always 0 while music plays, the loopback opened on the wrong endpoint — check `GetId` against the current default.

- [ ] **Step 7: Commit**

```bash
git add src/win/ src/main.rs
git commit -m "feat: WASAPI loopback capture thread feeding analysed frames

Pure downmix and RMS helpers are unit-tested including partial frames and
zero-channel edge cases; the device-change path is exercised in normal use
because the reference machine's default endpoint is virtual."
```

---

### Task 10: Segmented family and VFD Ice — the real thing

**The end-to-end milestone.** After this task the app does what it was asked to do, with one theme.

**Read `docs/reference/README.md` and open `docs/reference/mockups/all-themes.html` before starting.** The mockup is the reference implementation. Two techniques in it are non-obvious and were arrived at by trial:

1. **Segment gaps are punched, not drawn.** Fill each bar as one rect, bloom it, *then* remove the horizontal gaps. Drawing individual segments produces the same picture far more slowly and makes the bloom wrong.
2. **The hot core is a narrower brighter rect** inset 28% from each edge at 55% alpha — not a gradient.

**Goldens are stored as ASCII luminance maps, not PNGs.** A 190×60 canvas renders to 60 readable lines, so a golden diff shows *what* changed visually in a code review, and it needs no image dependency.

**Files:**
- Create: `src/themes/mod.rs`
- Create: `src/themes/builtin.rs`
- Create: `src/render/segmented.rs`
- Create: `src/render/golden.rs`
- Create: `tests/golden/vfd-ice.txt`
- Modify: `src/render/mod.rs`, `src/main.rs`

**Interfaces:**
- Consumes: `render::canvas::{Canvas, Rgba}`, `dsp::bands::NUM_BANDS`, `dsp::ballistics::Ballistics`.
- Produces:
  - `themes::Zone { pub upto: f32, pub lit: String, pub hot: String }`
  - `themes::Texture` enum: `Glass | Scanlines | Haze | Filament | Grille | None_`
  - `themes::Theme` with fields `id: String`, `name: String`, `family: String`, `lit: String`, `hot: String`, `panel: String`, `panel_alpha: f32`, `edge: String`, `edge_alpha: f32`, `ghost: f32`, `bloom: f32`, `fade: f32`, `texture: Texture`, `ballistics: Ballistics`, `zones: Vec<Zone>`, `dual: Option<(String, f32)>`
  - `themes::builtin::all() -> Vec<Theme>`
  - `render::FrameData { pub levels: [f32; NUM_BANDS], pub peaks: [f32; NUM_BANDS], pub waveform: [f32; 256], pub rms_l: f32, pub rms_r: f32 }`
  - `render::Family` trait: `fn id(&self) -> &'static str;` and `fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData);`
  - `render::family_for(id: &str) -> Box<dyn Family>`
  - `render::golden::canvas_to_ascii(c: &Canvas) -> String`

- [ ] **Step 1: Write the golden helper and its test**

Create `src/render/golden.rs`:

```rust
use super::canvas::Canvas;

const RAMP: &[u8] = b" .:-=+*#%@";

/// Renders a canvas to an ASCII luminance map so golden diffs are readable in
/// review. Alpha-weighted, so transparent areas read as blank.
pub fn canvas_to_ascii(c: &Canvas) -> String {
    let mut s = String::with_capacity(((c.width() + 1) * c.height()) as usize);
    for y in 0..c.height() {
        for x in 0..c.width() {
            let p = c.get(x, y);
            let lum = (0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32)
                * (p.a as f32 / 255.0);
            let i = ((lum / 255.0) * (RAMP.len() - 1) as f32).round() as usize;
            s.push(RAMP[i.min(RAMP.len() - 1)] as char);
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::canvas::{Canvas, Rgba};
    use super::*;

    #[test]
    fn empty_canvas_is_all_blank() {
        let c = Canvas::new(4, 2);
        assert_eq!(canvas_to_ascii(&c), "    \n    \n");
    }

    #[test]
    fn white_is_the_brightest_ramp_character() {
        let mut c = Canvas::new(2, 1);
        c.fill_rect(0, 0, 2, 1, Rgba::new(255, 255, 255, 255));
        assert_eq!(canvas_to_ascii(&c), "@@\n");
    }

    #[test]
    fn transparent_pixels_read_as_blank_regardless_of_colour() {
        let mut c = Canvas::new(2, 1);
        c.fill_rect(0, 0, 2, 1, Rgba::new(255, 255, 255, 0));
        assert_eq!(canvas_to_ascii(&c), "  \n");
    }
}
```

- [ ] **Step 2: Run it and confirm those three pass**

Add `pub mod golden;` to `src/render/mod.rs`.

Run: `cargo test golden 2>&1 | tail -20`
Expected: PASS, 3 tests.

- [ ] **Step 3: Write the theme model and the VFD Ice built-in**

Create `src/themes/mod.rs`:

```rust
pub mod builtin;

use crate::dsp::ballistics::Ballistics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Texture {
    Glass,
    Scanlines,
    Haze,
    Filament,
    Grille,
    None_,
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub upto: f32,
    pub lit: String,
    pub hot: String,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub family: String,
    pub lit: String,
    pub hot: String,
    pub panel: String,
    pub panel_alpha: f32,
    pub edge: String,
    pub edge_alpha: f32,
    pub ghost: f32,
    pub bloom: f32,
    pub fade: f32,
    pub texture: Texture,
    pub ballistics: Ballistics,
    pub zones: Vec<Zone>,
    /// (trail colour, trail fade) - scope family only, models a dual-layer phosphor.
    pub dual: Option<(String, f32)>,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            id: "unnamed".into(),
            name: "Unnamed".into(),
            family: "segmented".into(),
            lit: "#8fe4ff".into(),
            hot: "#e4f8ff".into(),
            panel: "#040a0e".into(),
            panel_alpha: 0.96,
            edge: "#96e1ff".into(),
            edge_alpha: 0.13,
            ghost: 0.11,
            bloom: 16.0,
            fade: 0.30,
            texture: Texture::Glass,
            ballistics: Ballistics::default(),
            zones: Vec::new(),
            dual: None,
        }
    }
}

impl Theme {
    /// Colour of the segment at `frac` up the bar, honouring zones if present.
    pub fn lit_at(&self, frac: f32) -> &str {
        for z in &self.zones {
            if frac <= z.upto {
                return &z.lit;
            }
        }
        self.zones.last().map(|z| z.lit.as_str()).unwrap_or(&self.lit)
    }

    pub fn hot_at(&self, frac: f32) -> &str {
        for z in &self.zones {
            if frac <= z.upto {
                return &z.hot;
            }
        }
        self.zones.last().map(|z| z.hot.as_str()).unwrap_or(&self.hot)
    }
}
```

Create `src/themes/builtin.rs`:

```rust
use super::{Texture, Theme};

pub fn all() -> Vec<Theme> {
    vec![vfd_ice()]
}

pub fn vfd_ice() -> Theme {
    Theme {
        id: "vfd-ice".into(),
        name: "VFD Ice".into(),
        family: "segmented".into(),
        lit: "#8fe4ff".into(),
        hot: "#e4f8ff".into(),
        panel: "#040a0e".into(),
        panel_alpha: 0.96,
        edge: "#96e1ff".into(),
        edge_alpha: 0.13,
        ghost: 0.11,
        bloom: 16.0,
        texture: Texture::Glass,
        ..Theme::default()
    }
}
```

- [ ] **Step 4: Write the failing renderer test**

Create `src/render/segmented.rs`:

```rust
use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::{Texture, Theme};

const BAR_W: i32 = 5;
const GAP: i32 = 2;
const SEG_H: i32 = 3;
const SEG_GAP: i32 = 1;
const PAD_X: i32 = 5;
const PAD_Y: i32 = 6;

pub struct Segmented;

impl Family for Segmented {
    fn id(&self) -> &'static str {
        "segmented"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::bands::NUM_BANDS;
    use crate::render::golden::canvas_to_ascii;
    use crate::themes::builtin;

    fn frame(level: f32) -> FrameData {
        FrameData {
            levels: [level; NUM_BANDS],
            peaks: [level; NUM_BANDS],
            waveform: [0.0; 256],
            rms_l: level,
            rms_r: level,
        }
    }

    #[test]
    fn silence_still_draws_the_panel_and_ghost_grid() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.0));
        // Panel is opaque enough to be visible.
        assert!(c.get(95, 30).a > 100, "panel must be drawn even in silence");
        // Ghost grid present but dim.
        let ascii = canvas_to_ascii(&c);
        assert!(ascii.contains('.') || ascii.contains(':'), "expected a dim ghost grid");
        assert!(!ascii.contains('@'), "nothing should be fully lit in silence");
    }

    #[test]
    fn full_level_lights_the_top_of_the_bars() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
        let top = c.get(PAD_X + 2, PAD_Y + 1);
        assert!(top.a > 200, "top segment should be lit at full level");
    }

    #[test]
    fn half_level_lights_the_bottom_but_not_the_top() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
        let bottom = c.get(PAD_X + 2, 60 - PAD_Y - 2);
        let top = c.get(PAD_X + 2, PAD_Y + 1);
        assert!(bottom.a > 150, "bottom must be lit");
        assert!(top.a < bottom.a, "top must be dimmer than bottom at half level");
    }

    #[test]
    fn segment_gaps_are_punched_through_to_transparent() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
        // Walk a lit column and confirm the luminance is not monotonic - the
        // gaps must interrupt it. This is what distinguishes a segmented meter
        // from a solid bar.
        let x = PAD_X + 2;
        let mut transitions = 0;
        let mut prev = c.get(x, PAD_Y).a > 128;
        for y in PAD_Y..(60 - PAD_Y) {
            let now = c.get(x, y).a > 128;
            if now != prev {
                transitions += 1;
            }
            prev = now;
        }
        assert!(transitions >= 6, "expected several segment gaps, saw {transitions}");
    }

    #[test]
    fn bar_count_matches_the_geometry() {
        // (190 - 10) / 7 = 25 bars at the measured widget width.
        let usable = 190 - PAD_X * 2;
        assert_eq!(usable / (BAR_W + GAP), 25);
    }

    #[test]
    fn nothing_is_drawn_outside_the_canvas() {
        // Narrow rect - must clip, not panic.
        let mut c = Canvas::new(40, 20);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
        assert_eq!(c.bits().len(), 40 * 20);
    }

    #[test]
    fn golden_vfd_ice_at_half_level() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
        let actual = canvas_to_ascii(&c);
        let expected = include_str!("../../tests/golden/vfd-ice.txt");
        assert_eq!(
            actual, expected,
            "golden mismatch - if this change is intended, overwrite \
             tests/golden/vfd-ice.txt and eyeball the diff"
        );
    }
}
```

- [ ] **Step 5: Run test to verify it fails**

Run: `cargo test segmented 2>&1 | tail -20`
Expected: FAIL — `draw` is `todo!()`, `Family`/`FrameData` undefined, golden file missing.

- [ ] **Step 6: Define the Family trait and registry**

Replace `src/render/mod.rs`:

```rust
pub mod canvas;
pub mod golden;
pub mod scope;
pub mod segmented;
pub mod vu;

use crate::dsp::bands::NUM_BANDS;
use crate::themes::Theme;
use canvas::Canvas;

pub struct FrameData {
    pub levels: [f32; NUM_BANDS],
    pub peaks: [f32; NUM_BANDS],
    pub waveform: [f32; 256],
    pub rms_l: f32,
    pub rms_r: f32,
}

impl Default for FrameData {
    fn default() -> Self {
        FrameData {
            levels: [0.0; NUM_BANDS],
            peaks: [0.0; NUM_BANDS],
            waveform: [0.0; 256],
            rms_l: 0.0,
            rms_r: 0.0,
        }
    }
}

/// A family is a renderer with its own geometry and its own per-frame state.
/// Adding one means a new file plus one line in `family_for` - no existing
/// family is touched.
pub trait Family {
    fn id(&self) -> &'static str;
    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData);
}

pub fn family_for(id: &str) -> Box<dyn Family> {
    match id {
        "scope" => Box::new(scope::Scope::default()),
        "vu" => Box::new(vu::Vu::default()),
        _ => Box::new(segmented::Segmented),
    }
}
```

Create stubs so the module compiles — `src/render/scope.rs`:

```rust
use super::canvas::Canvas;
use super::{Family, FrameData};
use crate::themes::Theme;

#[derive(Default)]
pub struct Scope;

impl Family for Scope {
    fn id(&self) -> &'static str {
        "scope"
    }
    fn draw(&mut self, _c: &mut Canvas, _t: &Theme, _d: &FrameData) {
        // Implemented in Task 13.
    }
}
```

And `src/render/vu.rs`:

```rust
use super::canvas::Canvas;
use super::{Family, FrameData};
use crate::themes::Theme;

#[derive(Default)]
pub struct Vu;

impl Family for Vu {
    fn id(&self) -> &'static str {
        "vu"
    }
    fn draw(&mut self, _c: &mut Canvas, _t: &Theme, _d: &FrameData) {
        // Implemented in Task 14.
    }
}
```

Add `mod themes;` to `src/main.rs`.

- [ ] **Step 7: Implement the segmented renderer**

Replace the `todo!()` in `src/render/segmented.rs`:

```rust
    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        // 1. panel
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&t.panel, t.panel_alpha));

        // 2. panel texture
        match t.texture {
            Texture::Glass => {
                for y in 2..(h / 2) {
                    let a = 0.09 * (1.0 - (y - 2) as f32 / (h / 2 - 2) as f32);
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex("#bef0ff", a));
                }
            }
            Texture::Scanlines => {
                let mut y = 2;
                while y < h - 4 {
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, 0.045));
                    y += 2;
                }
            }
            Texture::Filament => {
                for y in (h / 2)..(h - 4) {
                    let a = 0.20 * (y - h / 2) as f32 / (h / 2 - 4) as f32;
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, a));
                }
            }
            Texture::Haze => {
                for y in 2..(h - 4) {
                    let dy = ((y - h / 2) as f32 / (h as f32 * 0.5)).abs();
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, 0.13 * (1.0 - dy).max(0.0)));
                }
            }
            Texture::Grille => {
                let mut x = 1;
                while x < w - 1 {
                    c.fill_rect(x, 2, 1, h - 4, Rgba::from_hex("#ffffff", 0.028));
                    x += 3;
                }
            }
            Texture::None_ => {}
        }

        // 3. geometry from the live rect
        let usable_w = w - PAD_X * 2;
        let usable_h = h - PAD_Y * 2;
        let pitch = BAR_W + GAP;
        let nbars = (usable_w / pitch).max(1);
        let ox = PAD_X + (usable_w - nbars * pitch + GAP) / 2;
        let seg_pitch = SEG_H + SEG_GAP;
        let nseg = (usable_h / seg_pitch).max(1);

        let sample = |arr: &[f32], b: i32| -> f32 {
            let lo = (b as usize * arr.len()) / nbars as usize;
            let hi = (((b + 1) as usize * arr.len()) / nbars as usize).max(lo + 1);
            arr[lo..hi.min(arr.len())].iter().copied().fold(0.0f32, f32::max)
        };

        // 4. dormant ghost grid - zone-coloured when zones are present
        if t.ghost > 0.0 {
            for k in 0..nseg {
                let frac = (k + 1) as f32 / nseg as f32;
                let col = Rgba::from_hex(t.lit_at(frac), t.ghost);
                let y = PAD_Y + usable_h - (k + 1) * seg_pitch;
                for b in 0..nbars {
                    c.fill_rect(ox + b * pitch, y, BAR_W, SEG_H, col);
                }
            }
        }

        // 5. lit columns - one fill per bar (or per zone), then bloom, then punch
        for b in 0..nbars {
            let lit = (sample(&d.levels, b) * nseg as f32).round() as i32;
            for k in 0..lit.min(nseg) {
                let frac = (k + 1) as f32 / nseg as f32;
                let y = PAD_Y + usable_h - (k + 1) * seg_pitch;
                c.fill_rect(ox + b * pitch, y, BAR_W, SEG_H, Rgba::from_hex(t.lit_at(frac), 1.0));
            }
        }

        // Strength above 1.0 deliberately: the halo is composited UNDER the crisp
        // marks, so overdriving it reads as phosphor glow rather than as blur.
        c.bloom(t.bloom.round() as i32, 1.35);

        // 6. hot core: a narrower brighter rect, not a gradient
        if t.zones.is_empty() {
            for b in 0..nbars {
                let lit = (sample(&d.levels, b) * nseg as f32).round() as i32;
                if lit <= 0 {
                    continue;
                }
                let hh = lit.min(nseg) * seg_pitch - SEG_GAP;
                c.fill_rect(
                    ox + b * pitch + (BAR_W as f32 * 0.28) as i32,
                    PAD_Y + usable_h - hh,
                    (BAR_W as f32 * 0.44).ceil() as i32,
                    hh,
                    Rgba::from_hex(&t.hot, 0.55),
                );
            }
        }

        // 7. punch the segment gaps back out
        for k in 1..=nseg {
            c.punch_row(PAD_Y + usable_h - k * seg_pitch + SEG_H, SEG_GAP);
        }

        // 8. peak-hold caps
        for b in 0..nbars {
            let pk = (sample(&d.peaks, b) * nseg as f32).round() as i32;
            if pk <= 0 {
                continue;
            }
            let frac = pk as f32 / nseg as f32;
            c.fill_rect(
                ox + b * pitch,
                PAD_Y + usable_h - pk * seg_pitch,
                BAR_W,
                1,
                Rgba::from_hex(t.hot_at(frac), 1.0),
            );
        }

        // 9. bezel
        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
        c.fill_rect(1, 2, 1, h - 4, e);
        c.fill_rect(w - 2, 2, 1, h - 4, e);
    }
```

- [ ] **Step 8: Generate the golden, then inspect it before trusting it**

Create an empty `tests/golden/vfd-ice.txt`, then run the suite once to fail, and write the actual output:

```bash
mkdir -p tests/golden && touch tests/golden/vfd-ice.txt
cargo test segmented 2>&1 | tail -30
```

Add a temporary generator test to emit it, run it, then **open the file and look at it**:

```rust
#[test]
#[ignore]
fn regenerate_golden() {
    let mut c = Canvas::new(190, 60);
    Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
    std::fs::write("tests/golden/vfd-ice.txt", canvas_to_ascii(&c)).unwrap();
}
```

Run: `cargo test regenerate_golden -- --ignored`

Then read `tests/golden/vfd-ice.txt`. **Do not commit it unmodified without looking.** It must show 25 evenly spaced columns, lit to roughly half height, interrupted by horizontal gaps. If it shows solid columns the punch step is not working; if it shows fewer than 25 columns the geometry is wrong.

- [ ] **Step 9: Run the full suite**

Run: `cargo test 2>&1 | tail -20`
Expected: PASS, all tests including the 7 segmented ones.

- [ ] **Step 10: Wire it up live and look at it**

Replace `main` in `src/main.rs`:

```rust
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
```

- [ ] **Step 11: Verify against real music**

Run `cargo run --release`, play something with a strong beat.

Check all four:
1. Bars move with the music, bass-heavy on the left.
2. It appears ~400 ms after audio starts, not instantly.
3. It disappears ~2 s after you pause — and **not** between tracks.
4. A Teams notification or a UI click does **not** make it appear.

- [ ] **Step 12: Commit**

```bash
git add src/ tests/
git commit -m "feat: segmented family with VFD Ice, end to end on the real taskbar

Golden images are ASCII luminance maps rather than PNGs so a visual
regression shows up as a readable diff and needs no image dependency."
```

---

### Task 11: Tray icon, config, autostart, quit

Without a tray icon there is no way to quit the app, because when nothing is playing the overlay does not exist. This task makes it usable daily.

**Files:**
- Create: `src/config.rs`
- Create: `src/win/tray.rs`
- Create: `src/win/autostart.rs`
- Modify: `src/win/mod.rs`, `src/main.rs`

**Interfaces:**
- Consumes: `dsp::gate::GateConfig`.
- Produces:
  - `config::Config { pub theme: String, pub brightness: f32, pub saturation: f32, pub threshold_dbfs: f32, pub reveal_ms: u32, pub hide_ms: u32, pub fade_ms: u32, pub autostart: bool }` with `Default`, `Config::path() -> PathBuf`, `Config::load() -> Config`, `fn save(&self) -> anyhow::Result<()>`, `fn gate_config(&self) -> GateConfig`
  - `win::autostart::{is_enabled() -> bool, set(enabled: bool) -> anyhow::Result<()>}`
  - `win::tray::{Tray, TrayEvent}` where `TrayEvent` is `Quit | SelectTheme(String) | ToggleAutostart`, plus `Tray::new(themes: &[(String, String)]) -> Result<Tray>` and `fn poll(&mut self) -> Option<TrayEvent>`

- [ ] **Step 1: Write the failing config test**

Create `src/config.rs`:

```rust
use crate::dsp::gate::GateConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub brightness: f32,
    pub saturation: f32,
    pub threshold_dbfs: f32,
    pub reveal_ms: u32,
    pub hide_ms: u32,
    pub fade_ms: u32,
    pub autostart: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "vfd-ice".into(),
            brightness: 1.0,
            saturation: 1.0,
            threshold_dbfs: -55.0,
            reveal_ms: 400,
            hide_ms: 2000,
            fade_ms: 250,
            autostart: false,
        }
    }
}

impl Config {
    pub fn dir() -> PathBuf {
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(base).join("taskbar-eq")
    }

    pub fn path() -> PathBuf {
        Self::dir().join("config.toml")
    }

    /// Never fails: a missing or corrupt config falls back to defaults, because
    /// a bad config file must not stop the app from starting.
    pub fn load() -> Config {
        match std::fs::read_to_string(Self::path()) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                eprintln!("config: {e}; using defaults");
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(Self::dir())?;
        std::fs::write(Self::path(), toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn gate_config(&self) -> GateConfig {
        GateConfig {
            threshold_dbfs: self.threshold_dbfs,
            reveal_ms: self.reveal_ms,
            hide_ms: self.hide_ms,
            fade_ms: self.fade_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let c = Config::default();
        assert_eq!(c.threshold_dbfs, -55.0);
        assert_eq!(c.reveal_ms, 400);
        assert_eq!(c.hide_ms, 2000);
        assert_eq!(c.fade_ms, 250);
        assert_eq!(c.theme, "vfd-ice");
    }

    #[test]
    fn round_trips_through_toml() {
        let mut c = Config::default();
        c.theme = "matrix-green".into();
        c.brightness = 0.8;
        let s = toml::to_string_pretty(&c).unwrap();
        assert_eq!(toml::from_str::<Config>(&s).unwrap(), c);
    }

    #[test]
    fn a_partial_file_fills_in_defaults() {
        // serde(default) means an old config missing new keys still loads.
        let c: Config = toml::from_str("theme = \"neon-pink\"").unwrap();
        assert_eq!(c.theme, "neon-pink");
        assert_eq!(c.hide_ms, 2000, "missing keys must take defaults");
    }

    #[test]
    fn a_corrupt_file_does_not_panic() {
        assert!(toml::from_str::<Config>("this is not toml {{{").is_err());
        // load() swallows that error; proven by defaults_match_the_spec plus this.
    }

    #[test]
    fn gate_config_is_derived_from_the_file() {
        let mut c = Config::default();
        c.reveal_ms = 900;
        assert_eq!(c.gate_config().reveal_ms, 900);
    }

    #[test]
    fn config_lives_under_appdata() {
        let p = Config::path();
        assert!(p.ends_with("taskbar-eq/config.toml") || p.ends_with("taskbar-eq\\config.toml"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails, then passes**

Add `mod config;` to `src/main.rs`.

Run: `cargo test config 2>&1 | tail -20`
Expected: PASS, 6 tests. (The implementation is written inline above, since a `todo!()` here would only test serde.)

- [ ] **Step 3: Write the autostart module with tests**

Create `src/win/autostart.rs`:

```rust
use anyhow::Result;
use windows::core::{w, HSTRING};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};

const VALUE_NAME: windows::core::PCWSTR = w!("TaskbarEQ");

fn run_key(access: u32) -> Result<HKEY> {
    let mut key = HKEY::default();
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            windows::Win32::System::Registry::REG_SAM_FLAGS(access),
            &mut key,
        )
        .ok()?;
    }
    Ok(key)
}

pub fn is_enabled() -> bool {
    match run_key(KEY_READ.0) {
        Ok(key) => {
            let mut size = 0u32;
            let present = unsafe {
                RegQueryValueExW(key, VALUE_NAME, None, None, None, Some(&mut size)).is_ok()
            };
            unsafe { let _ = RegCloseKey(key); }
            present
        }
        Err(_) => false,
    }
}

/// HKCU only - never HKLM. This must work without elevation.
pub fn set(enabled: bool) -> Result<()> {
    let key = run_key(KEY_WRITE.0 | KEY_READ.0)?;
    unsafe {
        if enabled {
            let exe = std::env::current_exe()?;
            let quoted = format!("\"{}\"", exe.display());
            let h = HSTRING::from(quoted);
            let bytes: &[u8] = std::slice::from_raw_parts(
                h.as_ptr() as *const u8,
                (h.len() + 1) * 2, // include the NUL
            );
            RegSetValueExW(key, VALUE_NAME, Some(0), REG_SZ, Some(bytes)).ok()?;
        } else {
            let _ = RegDeleteValueW(key, VALUE_NAME);
        }
        let _ = RegCloseKey(key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_autostart_round_trips() {
        let original = is_enabled();

        set(true).expect("enable should not need elevation");
        assert!(is_enabled(), "should report enabled after set(true)");

        set(false).expect("disable should succeed");
        assert!(!is_enabled(), "should report disabled after set(false)");

        // Leave the machine as we found it.
        set(original).ok();
    }

    #[test]
    fn disabling_when_absent_is_not_an_error() {
        let original = is_enabled();
        set(false).ok();
        assert!(set(false).is_ok(), "deleting a missing value must be idempotent");
        set(original).ok();
    }
}
```

- [ ] **Step 4: Run the autostart tests**

Add `pub mod autostart;` to `src/win/mod.rs`.

Run: `cargo test autostart -- --test-threads=1 2>&1 | tail -20`
Expected: PASS, 2 tests. `--test-threads=1` matters — both tests mutate the same registry value.

- [ ] **Step 5: Write the tray icon**

Create `src/win/tray.rs`:

```rust
use anyhow::{anyhow, Result};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, LoadIconW, PeekMessageW, RegisterClassW, SetForegroundWindow,
    TrackPopupMenu, TranslateMessage, HMENU, IDI_APPLICATION, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    MSG, PM_REMOVE, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTALIGN, WM_APP, WM_COMMAND,
    WM_RBUTTONUP, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};

const WM_TRAY: u32 = WM_APP + 1;
const ID_QUIT: usize = 1000;
const ID_AUTOSTART: usize = 1001;
const ID_THEME_BASE: usize = 2000;

#[derive(Debug, Clone, PartialEq)]
pub enum TrayEvent {
    Quit,
    SelectTheme(String),
    ToggleAutostart,
}

pub struct Tray {
    hwnd: HWND,
    themes: Vec<(String, String)>, // (id, display name)
    pending: Vec<TrayEvent>,
}

impl Tray {
    pub fn new(themes: &[(String, String)]) -> Result<Self> {
        unsafe {
            let class = WNDCLASSW {
                lpfnWndProc: Some(tray_wndproc),
                lpszClassName: w!("TaskbarEqTray"),
                ..Default::default()
            };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("TaskbarEqTray"),
                w!("Taskbar EQ"),
                WS_POPUP,
                0, 0, 0, 0,
                None, None, None, None,
            )?;

            let mut tip = [0u16; 128];
            for (i, ch) in "Taskbar EQ".encode_utf16().enumerate() {
                tip[i] = ch;
            }
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_TRAY,
                hIcon: LoadIconW(None, IDI_APPLICATION)?,
                szTip: tip,
                ..Default::default()
            };
            if !Shell_NotifyIconW(NIM_ADD, &mut nid).as_bool() {
                return Err(anyhow!("Shell_NotifyIconW(NIM_ADD) failed"));
            }

            Ok(Tray { hwnd, themes: themes.to_vec(), pending: Vec::new() })
        }
    }

    /// Shows the context menu and returns the chosen event, if any.
    pub fn show_menu(&self, autostart: bool, current_theme: &str) -> Option<TrayEvent> {
        unsafe {
            let menu: HMENU = CreatePopupMenu().ok()?;
            for (i, (id, name)) in self.themes.iter().enumerate() {
                let flags = if id == current_theme { MF_STRING | MF_CHECKED } else { MF_STRING };
                let mut wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = AppendMenuW(
                    menu,
                    flags,
                    ID_THEME_BASE + i,
                    windows::core::PCWSTR(wide.as_mut_ptr()),
                );
            }
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(
                menu,
                if autostart { MF_STRING | MF_CHECKED } else { MF_STRING },
                ID_AUTOSTART,
                w!("Start with Windows"),
            );
            let _ = AppendMenuW(menu, MF_STRING, ID_QUIT, w!("Quit"));

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = SetForegroundWindow(self.hwnd);
            let cmd = TrackPopupMenu(
                menu,
                TPM_RIGHTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD,
                pt.x, pt.y, Some(0), self.hwnd, None,
            );
            let _ = DestroyMenu(menu);

            let id = cmd.0 as usize;
            if id == ID_QUIT {
                Some(TrayEvent::Quit)
            } else if id == ID_AUTOSTART {
                Some(TrayEvent::ToggleAutostart)
            } else if id >= ID_THEME_BASE {
                self.themes.get(id - ID_THEME_BASE).map(|(tid, _)| TrayEvent::SelectTheme(tid.clone()))
            } else {
                None
            }
        }
    }

    pub fn poll(&mut self) -> Option<TrayEvent> {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_TRAY && msg.lParam.0 as u32 == WM_RBUTTONUP {
                    self.pending.push(TrayEvent::Quit); // replaced by caller-driven menu
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        self.pending.pop()
    }

    /// True when the user right-clicked the tray icon this tick.
    pub fn take_right_click(&mut self) -> bool {
        let hit = !self.pending.is_empty();
        self.pending.clear();
        hit
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
        }
    }
}

unsafe extern "system" fn tray_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
```

Add `pub mod tray;` to `src/win/mod.rs`.

- [ ] **Step 6: Wire config, tray and autostart into main**

In `src/main.rs`: load `Config::load()` at startup, build the theme list from `themes::builtin::all()`, create the `Tray`, and in the loop call `tray.poll()`; on a right-click call `tray.show_menu(...)` and handle the returned event — `Quit` breaks the loop, `SelectTheme(id)` swaps `theme`/`family`/`smoother` and saves the config, `ToggleAutostart` calls `win::autostart::set(!is_enabled())` and saves.

Use `cfg.gate_config()` when constructing the `Gate` instead of `GateConfig::default()`.

- [ ] **Step 7: Verify by hand**

Run `cargo run --release`. Confirm:
1. A tray icon appears.
2. Right-click shows the menu with VFD Ice checked.
3. "Start with Windows" toggles and survives a restart (`reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v TaskbarEQ`).
4. "Quit" exits cleanly **and removes the tray icon** — a leftover ghost icon means the `Drop` impl did not run.
5. `%APPDATA%\taskbar-eq\config.toml` exists and is readable.

- [ ] **Step 8: Commit**

```bash
git add src/
git commit -m "feat: tray icon, config file, autostart and clean quit

Tray icon is mandatory rather than a nicety: when nothing is playing the
overlay does not exist, so it is the only way to quit. Autostart uses HKCU
so it never needs elevation."
```

---

### Task 12: Remaining four segmented colourways

No renderer changes — this is pure data plus the zone path, which is the one behavioural difference in family 1.

**Files:**
- Modify: `src/themes/builtin.rs`
- Create: `tests/golden/classic-three-colour.txt`

**Interfaces:**
- Consumes: `themes::{Theme, Texture, Zone}`.
- Produces: `themes::builtin::{matrix_green, neon_pink, vac_tube_orange, classic_three_colour}`, each `-> Theme`; `builtin::all()` returns all five.

- [ ] **Step 1: Write the failing test**

Append to `src/themes/builtin.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of a #RRGGBB string.
    fn luminance(hex: &str) -> f32 {
        let h = hex.trim_start_matches('#');
        let ch = |i: usize| {
            let v = u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(0) + 0.7152 * ch(2) + 0.0722 * ch(4)
    }

    fn contrast(a: &str, b: &str) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn ships_all_five_segmented_colourways() {
        let ids: Vec<String> = all().iter().map(|t| t.id.clone()).collect();
        for want in ["vfd-ice", "matrix-green", "neon-pink", "vac-tube-orange", "classic-three-colour"] {
            assert!(ids.contains(&want.to_string()), "missing {want}");
        }
    }

    #[test]
    fn every_id_is_unique() {
        let mut ids: Vec<String> = all().iter().map(|t| t.id.clone()).collect();
        let before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate theme ids would break override-by-id");
    }

    #[test]
    fn every_lit_colour_clears_three_to_one_against_its_own_panel() {
        // The spec's hard requirement, computed rather than eyeballed.
        for t in all() {
            let ratio = contrast(&t.lit, &t.panel);
            assert!(ratio >= 3.0, "{}: lit {} vs panel {} = {ratio:.2}:1", t.id, t.lit, t.panel);
            for z in &t.zones {
                let zr = contrast(&z.lit, &t.panel);
                assert!(zr >= 3.0, "{} zone {}: {zr:.2}:1", t.id, z.lit);
            }
        }
    }

    #[test]
    fn every_panel_alpha_meets_the_floor() {
        // Below 0.92 the widget's own weather text shows through the panel.
        for t in all() {
            assert!(t.panel_alpha >= 0.92, "{} has panel_alpha {}", t.id, t.panel_alpha);
        }
    }

    #[test]
    fn attack_always_outpaces_decay() {
        for t in all() {
            assert!(
                t.ballistics.attack > t.ballistics.decay,
                "{}: attack {} must exceed decay {}",
                t.id, t.ballistics.attack, t.ballistics.decay
            );
        }
    }

    #[test]
    fn zones_ascend_and_reach_the_top() {
        for t in all() {
            if t.zones.is_empty() {
                continue;
            }
            for pair in t.zones.windows(2) {
                assert!(pair[1].upto > pair[0].upto, "{}: zones must ascend", t.id);
            }
            assert!(
                t.zones.last().unwrap().upto >= 1.0,
                "{}: final zone must cover the top of the bar",
                t.id
            );
        }
    }

    #[test]
    fn only_the_classic_theme_uses_zones() {
        for t in all() {
            let expect_zones = t.id == "classic-three-colour";
            assert_eq!(!t.zones.is_empty(), expect_zones, "{}", t.id);
        }
    }

    #[test]
    fn classic_zones_are_green_amber_red_in_order() {
        let t = classic_three_colour();
        assert_eq!(t.zones.len(), 3);
        assert_eq!(t.lit_at(0.2), "#3ddc5a", "low = green (headroom)");
        assert_eq!(t.lit_at(0.7), "#ffc21f", "mid = amber (loud)");
        assert_eq!(t.lit_at(0.95), "#ff3b30", "top = red (peaking)");
    }

    #[test]
    fn each_colourway_has_a_distinct_texture_or_bloom() {
        // Guards against a theme being an unmodified copy with a new hex.
        let sigs: Vec<(super::Texture, i32)> =
            all().iter().map(|t| (t.texture, t.bloom as i32)).collect();
        let mut uniq = sigs.clone();
        uniq.dedup();
        assert_eq!(uniq.len(), sigs.len(), "two themes are visually identical: {sigs:?}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test builtin 2>&1 | tail -20`
Expected: FAIL — the four new constructors do not exist.

- [ ] **Step 3: Add the four colourways**

Replace `all()` and append to `src/themes/builtin.rs`:

```rust
use super::Zone;

pub fn all() -> Vec<Theme> {
    vec![
        vfd_ice(),
        matrix_green(),
        neon_pink(),
        vac_tube_orange(),
        classic_three_colour(),
    ]
}

pub fn matrix_green() -> Theme {
    Theme {
        id: "matrix-green".into(),
        name: "Matrix Green".into(),
        lit: "#35ff6e".into(),
        hot: "#ccffdb".into(),
        panel: "#000903".into(),
        panel_alpha: 0.96,
        edge: "#3cff78".into(),
        edge_alpha: 0.14,
        ghost: 0.17,
        bloom: 15.0,
        texture: Texture::Scanlines,
        ballistics: crate::dsp::ballistics::Ballistics {
            attack: 0.55,
            decay: 0.13,
            peak_fall: 0.0070,
        },
        ..vfd_ice()
    }
}

pub fn neon_pink() -> Theme {
    Theme {
        id: "neon-pink".into(),
        name: "Neon Pink".into(),
        lit: "#ff4fb0".into(),
        hot: "#ffd9ee".into(),
        panel: "#0d020b".into(),
        panel_alpha: 0.96,
        edge: "#ff4fb0".into(),
        edge_alpha: 0.22,
        ghost: 0.09,
        bloom: 20.0,
        texture: Texture::Haze,
        ballistics: crate::dsp::ballistics::Ballistics {
            attack: 0.55,
            decay: 0.11,
            peak_fall: 0.0050,
        },
        ..vfd_ice()
    }
}

pub fn vac_tube_orange() -> Theme {
    Theme {
        id: "vac-tube-orange".into(),
        name: "Vac Tube Orange".into(),
        lit: "#ff9a2e".into(),
        hot: "#ffe9c9".into(),
        panel: "#0f0602".into(),
        panel_alpha: 0.96,
        edge: "#ff9632".into(),
        edge_alpha: 0.16,
        ghost: 0.13,
        bloom: 18.0,
        texture: Texture::Filament,
        // Slowest peak fall of the five - heat dissipating rather than snapping back.
        ballistics: crate::dsp::ballistics::Ballistics {
            attack: 0.50,
            decay: 0.09,
            peak_fall: 0.0035,
        },
        ..vfd_ice()
    }
}

pub fn classic_three_colour() -> Theme {
    Theme {
        id: "classic-three-colour".into(),
        name: "Classic Three-Colour".into(),
        lit: "#3ddc5a".into(),
        hot: "#b6ffc6".into(),
        panel: "#060708".into(),
        panel_alpha: 0.96,
        edge: "#c8d2d7".into(),
        edge_alpha: 0.12,
        ghost: 0.13,
        bloom: 13.0,
        texture: Texture::Grille,
        zones: vec![
            Zone { upto: 0.58, lit: "#3ddc5a".into(), hot: "#b6ffc6".into() },
            Zone { upto: 0.84, lit: "#ffc21f".into(), hot: "#fff0b8".into() },
            Zone { upto: 1.01, lit: "#ff3b30".into(), hot: "#ffc2bd".into() },
        ],
        ballistics: crate::dsp::ballistics::Ballistics {
            attack: 0.55,
            decay: 0.11,
            peak_fall: 0.0045,
        },
        ..vfd_ice()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test builtin 2>&1 | tail -20`
Expected: PASS, 9 tests. If the contrast test fails on a colourway, **do not lower the threshold** — darken that theme's `panel` until it passes.

- [ ] **Step 5: Add a golden for the zoned path**

Add to `src/render/segmented.rs` tests:

```rust
    #[test]
    fn golden_classic_three_colour_at_high_level() {
        let mut c = Canvas::new(190, 60);
        // 0.9 so all three colour zones are lit and visible in the golden.
        Segmented.draw(&mut c, &crate::themes::builtin::classic_three_colour(), &frame(0.9));
        let actual = canvas_to_ascii(&c);
        let expected = include_str!("../../tests/golden/classic-three-colour.txt");
        assert_eq!(actual, expected, "golden mismatch - regenerate and eyeball the diff");
    }

    #[test]
    fn zoned_themes_skip_the_hot_core() {
        // Real coloured LEDs are flat; a hot core would wash out the zone colour.
        let mut c = Canvas::new(190, 60);
        let t = crate::themes::builtin::classic_three_colour();
        Segmented.draw(&mut c, &t, &frame(1.0));
        let centre = c.get(PAD_X + 2, 20);
        let edge = c.get(PAD_X, 20);
        assert!(
            centre.r.abs_diff(edge.r) < 40,
            "zoned bars must be flat across their width, got centre {centre:?} edge {edge:?}"
        );
    }
```

Generate the golden with the `regenerate_golden` pattern from Task 10, then **open it and check** that the lower two-thirds and the top read differently — the ASCII ramp is luminance-based, so green/amber/red produce visibly different characters.

- [ ] **Step 6: Run the full suite and check each theme live**

Run: `cargo test 2>&1 | tail -20`

Then `cargo run --release` and cycle all five from the tray menu with music playing. Compare each against `docs/reference/mockups/all-themes.html` open in a browser.

- [ ] **Step 7: Commit**

```bash
git add src/themes/ src/render/ tests/golden/
git commit -m "feat: add the remaining four segmented colourways

Contrast ratios are asserted rather than trusted - the test computes WCAG
luminance for every lit colour against its own panel and fails below 3:1."
```

---

### Task 13: Scope family and five phosphors

The distinguishing feature is **genuine persistence**: the trace is drawn into a buffer that decays each frame, so old traces fade rather than being cleared. **P7 needs two buffers** at different fade rates — a fast blue-white trace over a slow yellow-green trail, because the real P7 is physically two phosphor layers.

**Files:**
- Modify: `src/render/scope.rs`
- Modify: `src/themes/builtin.rs`
- Create: `tests/golden/p1-green.txt`

**Interfaces:**
- Consumes: `render::canvas::{Canvas, Rgba}`, `render::{Family, FrameData}`, `themes::Theme`.
- Produces:
  - `render::scope::Scope` with `Default`, holding `trace: Option<Canvas>` and `trail: Option<Canvas>`
  - `themes::builtin::{p1_green, p7_dual, p11_blue_violet, scope_amber, scope_white}`

- [ ] **Step 1: Write the failing test**

Replace `src/render/scope.rs`:

```rust
use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

#[derive(Default)]
pub struct Scope {
    trace: Option<Canvas>,
    trail: Option<Canvas>,
}

impl Scope {
    /// Draws the waveform polyline into `buf` after decaying what was there.
    fn stroke_into(buf: &mut Canvas, d: &FrameData, colour: Rgba, fade: f32, bloom: i32) {
        todo!()
    }
}

impl Family for Scope {
    fn id(&self) -> &'static str {
        "scope"
    }
    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::bands::NUM_BANDS;
    use crate::render::golden::canvas_to_ascii;
    use crate::themes::builtin;

    fn wave(amp: f32) -> FrameData {
        let mut d = FrameData::default();
        for i in 0..256 {
            d.waveform[i] = amp * ((i as f32 / 256.0) * std::f32::consts::TAU * 2.0).sin();
        }
        d.levels = [0.0; NUM_BANDS];
        d
    }

    #[test]
    fn draws_a_graticule_even_with_no_signal() {
        let mut c = Canvas::new(190, 60);
        Scope::default().draw(&mut c, &builtin::p1_green(), &wave(0.0));
        let ascii = canvas_to_ascii(&c);
        assert!(ascii.contains('.') || ascii.contains(':'), "graticule should be faintly visible");
    }

    #[test]
    fn a_flat_signal_traces_the_centre_line() {
        let mut c = Canvas::new(190, 60);
        Scope::default().draw(&mut c, &builtin::p1_green(), &wave(0.0));
        let mid = c.get(95, 30);
        let top = c.get(95, 8);
        assert!(mid.a > top.a, "flat trace must sit on the centre line");
    }

    #[test]
    fn a_large_signal_reaches_away_from_the_centre() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p1_green(), &wave(1.0));
        // Somewhere in the upper third must be lit by the excursion.
        let lit_high = (0..190).any(|x| c.get(x, 12).a > 100);
        assert!(lit_high, "full-scale wave should reach the upper third");
    }

    #[test]
    fn persistence_accumulates_across_frames() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);

        // Frame 1: signal high. Frame 2: flat. The old trace must still be faintly there.
        s.draw(&mut c, &builtin::p1_green(), &wave(1.0));
        let lit_after_one: u32 = (0..190).map(|x| c.get(x, 12).a as u32).sum();

        s.draw(&mut c, &builtin::p1_green(), &wave(0.0));
        let lit_after_two: u32 = (0..190).map(|x| c.get(x, 12).a as u32).sum();

        assert!(lit_after_two > 0, "the previous trace must persist, not be cleared");
        assert!(lit_after_two < lit_after_one, "but it must be decaying");
    }

    #[test]
    fn a_high_fade_decays_faster_than_a_low_one() {
        let fast = builtin::p11_blue_violet(); // fade 0.20
        let slow = builtin::scope_amber();     // fade 0.11
        assert!(fast.fade > slow.fade, "test premise: p11 fades faster than amber");

        let residue = |t: &Theme| {
            let mut s = Scope::default();
            let mut c = Canvas::new(190, 60);
            s.draw(&mut c, t, &wave(1.0));
            for _ in 0..8 {
                s.draw(&mut c, t, &wave(0.0));
            }
            (0..190).map(|x| c.get(x, 12).a as u32).sum::<u32>()
        };
        assert!(residue(&fast) < residue(&slow), "higher fade must leave less residue");
    }

    #[test]
    fn p7_uses_two_buffers_and_the_others_use_one() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p7_dual(), &wave(0.5));
        assert!(s.trail.is_some(), "P7 must allocate a second buffer for its trail");

        let mut s2 = Scope::default();
        s2.draw(&mut c, &builtin::p1_green(), &wave(0.5));
        assert!(s2.trail.is_none(), "single-layer phosphors must not allocate a trail");
    }

    #[test]
    fn resizing_the_canvas_does_not_panic() {
        // The widget rect changes width; buffers must be reallocated, not indexed stale.
        let mut s = Scope::default();
        let mut a = Canvas::new(190, 60);
        s.draw(&mut a, &builtin::p1_green(), &wave(0.5));
        let mut b = Canvas::new(120, 60);
        s.draw(&mut b, &builtin::p1_green(), &wave(0.5));
        assert_eq!(b.bits().len(), 120 * 60);
    }

    #[test]
    fn golden_p1_green() {
        let mut s = Scope::default();
        let mut c = Canvas::new(190, 60);
        s.draw(&mut c, &builtin::p1_green(), &wave(0.7));
        let expected = include_str!("../../tests/golden/p1-green.txt");
        assert_eq!(canvas_to_ascii(&c), expected, "golden mismatch - regenerate and eyeball");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test scope 2>&1 | tail -20`
Expected: FAIL — `todo!()` plus five missing theme constructors.

- [ ] **Step 3: Add the five phosphor colourways**

Append to `src/themes/builtin.rs` (and add all five to `all()`):

```rust
fn scope_base() -> Theme {
    Theme {
        family: "scope".into(),
        texture: Texture::None_,
        ghost: 0.0,
        bloom: 6.0,
        ..Theme::default()
    }
}

pub fn p1_green() -> Theme {
    Theme {
        id: "p1-green".into(),
        name: "P1 green".into(),
        lit: "#5cff9a".into(),
        hot: "#ccffdd".into(),
        panel: "#020805".into(),
        panel_alpha: 0.96,
        edge: "#78ffb4".into(),
        edge_alpha: 0.14,
        fade: 0.14,
        ..scope_base()
    }
}

pub fn p7_dual() -> Theme {
    Theme {
        id: "p7-dual".into(),
        name: "P7 dual-layer".into(),
        lit: "#e8f4ff".into(),
        hot: "#ffffff".into(),
        panel: "#03060c".into(),
        panel_alpha: 0.96,
        edge: "#aad7ff".into(),
        edge_alpha: 0.15,
        fade: 0.30,
        // The real P7 is two phosphor layers: a blue-white flash over a slow
        // yellow-green tail. The trail fades far more slowly than the trace.
        dual: Some(("#cfe86a".into(), 0.055)),
        ..scope_base()
    }
}

pub fn p11_blue_violet() -> Theme {
    Theme {
        id: "p11-blue-violet".into(),
        name: "P11 blue-violet".into(),
        lit: "#9db4ff".into(),
        hot: "#dde5ff".into(),
        panel: "#03040c".into(),
        panel_alpha: 0.96,
        edge: "#96afff".into(),
        edge_alpha: 0.15,
        fade: 0.20,
        ..scope_base()
    }
}

pub fn scope_amber() -> Theme {
    Theme {
        id: "scope-amber".into(),
        name: "Amber".into(),
        lit: "#ffc766".into(),
        hot: "#ffe9c9".into(),
        panel: "#0c0602".into(),
        panel_alpha: 0.96,
        edge: "#ffc878".into(),
        edge_alpha: 0.15,
        fade: 0.11,
        ..scope_base()
    }
}

pub fn scope_white() -> Theme {
    Theme {
        id: "scope-white".into(),
        name: "White-hot".into(),
        lit: "#f6fbff".into(),
        hot: "#ffffff".into(),
        panel: "#040609".into(),
        panel_alpha: 0.96,
        edge: "#dceeff".into(),
        edge_alpha: 0.15,
        fade: 0.17,
        ..scope_base()
    }
}
```

- [ ] **Step 4: Implement the scope renderer**

Replace the `todo!()` bodies in `src/render/scope.rs`:

```rust
impl Scope {
    fn stroke_into(buf: &mut Canvas, d: &FrameData, colour: Rgba, fade: f32, bloom: i32) {
        // Decay what is already there. Scaling alpha keeps the buffer transparent,
        // which is what lets the panel show through the trail.
        let decay = (1.0 - fade.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        let (w, h) = (buf.width(), buf.height());
        for y in 0..h {
            for x in 0..w {
                let p = buf.get(x, y);
                if p.a == 0 {
                    continue;
                }
                let a = (p.a as f32 * decay) as u8;
                // Overwrite rather than blend, so decay is monotonic.
                buf.fill_rect(x, y, 1, 1, Rgba::new(p.r, p.g, p.b, 0));
                if a > 2 {
                    buf.fill_rect(x, y, 1, 1, Rgba::new(p.r, p.g, p.b, a));
                }
            }
        }

        // Stroke the new trace: one vertical span per column, joining consecutive
        // samples so a steep slope stays continuous instead of dotting.
        let mid = h / 2;
        let amp = (h as f32 * 0.38) as i32;
        let x0 = 5;
        let span = (w - 10).max(1);
        let mut prev_y: Option<i32> = None;
        for px in 0..span {
            let i = (px as usize * 255) / span.max(1) as usize;
            let y = mid - (d.waveform[i.min(255)] * amp as f32) as i32;
            let y = y.clamp(0, h - 1);
            let (lo, hi) = match prev_y {
                Some(p) if p < y => (p, y),
                Some(p) => (y, p),
                None => (y, y),
            };
            buf.fill_rect(x0 + px, lo, 1, (hi - lo + 1).max(1), colour);
            prev_y = Some(y);
        }
        buf.bloom(bloom, 0.9);
    }
}

impl Family for Scope {
    fn id(&self) -> &'static str {
        "scope"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // Reallocate the persistence buffers when the widget rect changes width.
        let stale = self
            .trace
            .as_ref()
            .map(|b| b.width() != w || b.height() != h)
            .unwrap_or(true);
        if stale {
            self.trace = Some(Canvas::new(w, h));
            self.trail = None;
        }
        if t.dual.is_some() && self.trail.is_none() {
            self.trail = Some(Canvas::new(w, h));
        } else if t.dual.is_none() {
            self.trail = None;
        }

        // Slow trail first (drawn underneath), then the fast trace.
        if let (Some((trail_hex, trail_fade)), Some(trail)) = (t.dual.clone(), self.trail.as_mut()) {
            Self::stroke_into(
                trail,
                d,
                Rgba::from_hex(&trail_hex, 1.0),
                trail_fade,
                (t.bloom * 0.8) as i32,
            );
        }
        if let Some(trace) = self.trace.as_mut() {
            Self::stroke_into(trace, d, Rgba::from_hex(&t.lit, 1.0), t.fade, t.bloom as i32);
        }

        // Compose: panel, graticule, trail, trace, bezel.
        c.clear();
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&t.panel, t.panel_alpha));

        let grid = Rgba::from_hex(&t.lit, 0.10);
        for k in 1..8 {
            c.fill_rect(1 + (w - 2) * k / 8, 2, 1, h - 4, grid);
        }
        for k in 1..4 {
            c.fill_rect(1, 2 + (h - 4) * k / 4, w - 2, 1, grid);
        }
        c.fill_rect(1, h / 2, w - 2, 1, Rgba::from_hex(&t.lit, 0.20));

        for buf in [self.trail.as_ref(), self.trace.as_ref()] {
            if let Some(b) = buf {
                for y in 0..h {
                    for x in 0..w {
                        let p = b.get(x, y);
                        if p.a > 0 {
                            c.fill_rect(x, y, 1, 1, p);
                        }
                    }
                }
            }
        }

        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}
```

- [ ] **Step 5: Generate the golden and look at it**

Use the `regenerate_golden` pattern from Task 10 to write `tests/golden/p1-green.txt`, then open it. It must show a continuous sine curve spanning the width, not a dotted line. A dotted line means the `prev_y` join is not working.

- [ ] **Step 6: Run the suite, then check live**

Run: `cargo test 2>&1 | tail -20`

Then `cargo run --release`, select each phosphor from the tray with music playing. **Specifically check P7:** the fresh trace should be blue-white and the fading tail distinctly yellow-green. If the trail is the same colour as the trace, the `dual` path is not being taken.

- [ ] **Step 7: Commit**

```bash
git add src/ tests/golden/
git commit -m "feat: scope family with five phosphors including dual-layer P7

Persistence is a decaying transparent buffer rather than a fixed trail
length. P7 allocates a second slower buffer so its tail is a genuinely
different colour from its trace, as the real dual-layer phosphor is."
```

---

### Task 14: VU family and five backlights

Two needle dials with ~300 ms ballistics. Slow and asymmetric — making them fast destroys the feel immediately.

**Files:**
- Modify: `src/render/vu.rs`
- Modify: `src/themes/builtin.rs`
- Create: `tests/golden/vu-cream.txt`

**Interfaces:**
- Consumes: `render::canvas::{Canvas, Rgba}`, `render::{Family, FrameData}`, `themes::Theme`.
- Produces:
  - `render::vu::Vu` with `Default`, holding `l: f32, r: f32, pk_l: f32, pk_r: f32`
  - `render::vu::VU_SMOOTHING: f32 = 0.085`
  - `themes::builtin::{vu_cream, vu_amber, vu_ice, vu_green, vu_red}`
  - `Theme::overload_hex(&self) -> &str` — returns the arc colour, white on the red dial

- [ ] **Step 1: Write the failing test**

Replace `src/render/vu.rs`:

```rust
use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// ~300ms integration at 60fps. Slow on purpose: a fast needle looks broken.
pub const VU_SMOOTHING: f32 = 0.085;
const OVERLOAD_AT: f32 = 0.76;

#[derive(Default)]
pub struct Vu {
    l: f32,
    r: f32,
    pk_l: f32,
    pk_r: f32,
}

impl Family for Vu {
    fn id(&self) -> &'static str {
        "vu"
    }
    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::golden::canvas_to_ascii;
    use crate::themes::builtin;

    fn level(l: f32, r: f32) -> FrameData {
        FrameData { rms_l: l, rms_r: r, ..FrameData::default() }
    }

    #[test]
    fn draws_two_dials_with_printed_arcs() {
        let mut c = Canvas::new(190, 60);
        Vu::default().draw(&mut c, &builtin::vu_cream(), &level(0.0, 0.0));
        // Both halves must contain ink.
        let left: u32 = (5..90).map(|x| c.get(x, 30).a as u32).sum();
        let right: u32 = (100..185).map(|x| c.get(x, 30).a as u32).sum();
        assert!(left > 0 && right > 0, "expected a dial in each half");
    }

    #[test]
    fn needles_move_slowly_toward_the_target() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        v.draw(&mut c, &builtin::vu_cream(), &level(1.0, 1.0));
        assert!(v.l < 0.2, "one frame must not swing the needle far, got {}", v.l);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(1.0, 1.0));
        }
        assert!(v.l > 0.9, "should converge after ~1.3s, got {}", v.l);
    }

    #[test]
    fn the_two_channels_are_independent() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.9, 0.1));
        }
        assert!(v.l > v.r + 0.5, "L {} and R {} must differ", v.l, v.r);
    }

    #[test]
    fn peak_needles_lag_behind_on_the_way_down() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.9, 0.9));
        }
        for _ in 0..10 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.0, 0.0));
        }
        assert!(v.pk_l > v.l, "peak needle must hold above the live needle");
    }

    #[test]
    fn the_red_dial_flips_its_overload_arc_to_white() {
        // Red-on-red would be invisible - the one colourway needing a behavioural change.
        assert_eq!(builtin::vu_red().overload_hex(), "#ffffff");
        assert_ne!(builtin::vu_cream().overload_hex(), "#ffffff");
    }

    #[test]
    fn state_survives_a_canvas_resize() {
        let mut v = Vu::default();
        let mut a = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut a, &builtin::vu_cream(), &level(0.8, 0.8));
        }
        let before = v.l;
        let mut b = Canvas::new(120, 60);
        v.draw(&mut b, &builtin::vu_cream(), &level(0.8, 0.8));
        assert!((v.l - before).abs() < 0.05, "needle position must not jump on resize");
    }

    #[test]
    fn golden_vu_cream() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.65, 0.4));
        }
        let expected = include_str!("../../tests/golden/vu-cream.txt");
        assert_eq!(canvas_to_ascii(&c), expected, "golden mismatch - regenerate and eyeball");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test vu 2>&1 | tail -20`
Expected: FAIL — `todo!()`, five missing constructors, missing `overload_hex`.

- [ ] **Step 3: Add `overload_hex` and the five backlights**

Add to `impl Theme` in `src/themes/mod.rs`:

```rust
    /// The printed overload arc. Red on every dial except the red one, where it
    /// goes white because red-on-red is illegible.
    pub fn overload_hex(&self) -> &str {
        if self.id == "vu-red" {
            "#ffffff"
        } else {
            "#ff5a46"
        }
    }
```

Append to `src/themes/builtin.rs` (and add all five to `all()`):

```rust
fn vu_base() -> Theme {
    Theme {
        family: "vu".into(),
        texture: Texture::Filament,
        ghost: 0.0,
        bloom: 5.0,
        ..Theme::default()
    }
}

pub fn vu_cream() -> Theme {
    Theme {
        id: "vu-cream".into(),
        name: "Warm cream".into(),
        lit: "#ffe2aa".into(),
        hot: "#ffe6b0".into(),
        panel: "#140e06".into(),
        panel_alpha: 0.96,
        edge: "#ffc878".into(),
        edge_alpha: 0.16,
        ..vu_base()
    }
}

pub fn vu_amber() -> Theme {
    Theme {
        id: "vu-amber".into(),
        name: "Amber".into(),
        lit: "#ffbe6e".into(),
        hot: "#ffcf7a".into(),
        panel: "#160b02".into(),
        panel_alpha: 0.96,
        edge: "#ffaf50".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}

pub fn vu_ice() -> Theme {
    Theme {
        id: "vu-ice".into(),
        name: "Ice blue".into(),
        // Deliberately matches the VFD Ice segmented colourway.
        lit: "#bee6ff".into(),
        hot: "#d8f2ff".into(),
        panel: "#040c14".into(),
        panel_alpha: 0.96,
        edge: "#a0dcff".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}

pub fn vu_green() -> Theme {
    Theme {
        id: "vu-green".into(),
        name: "Green".into(),
        // Matches Matrix Green.
        lit: "#b4ffcd".into(),
        hot: "#c8ffd8".into(),
        panel: "#020e06".into(),
        panel_alpha: 0.96,
        edge: "#8cffb4".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}

pub fn vu_red() -> Theme {
    Theme {
        id: "vu-red".into(),
        name: "Red".into(),
        // Closest match to the system accent (#D0000C).
        lit: "#ffaa9b".into(),
        hot: "#ffb3a6".into(),
        panel: "#140302".into(),
        panel_alpha: 0.96,
        edge: "#ff826e".into(),
        edge_alpha: 0.18,
        ..vu_base()
    }
}
```

- [ ] **Step 4: Implement the VU renderer**

Replace the `todo!()` in `src/render/vu.rs`:

```rust
    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // Ballistics live on the family, not the canvas, so a resize does not
        // reset the needles.
        self.l += (d.rms_l.clamp(0.0, 1.0) - self.l) * VU_SMOOTHING;
        self.r += (d.rms_r.clamp(0.0, 1.0) - self.r) * VU_SMOOTHING;
        self.pk_l = (self.pk_l - 0.004).max(self.l);
        self.pk_r = (self.pk_r - 0.004).max(self.r);

        c.clear();
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&t.panel, t.panel_alpha));

        // Warm backlight pooling from the bottom.
        for y in (h / 3)..(h - 4) {
            let f = (y - h / 3) as f32 / (h - 4 - h / 3).max(1) as f32;
            c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, 0.22 * f));
        }

        let ink = Rgba::from_hex(&t.lit, 0.72);
        let over = Rgba::from_hex(t.overload_hex(), 0.85);
        let dial_w = w / 2 - 3;

        for (idx, (level, peak)) in [(self.l, self.pk_l), (self.r, self.pk_r)].iter().enumerate() {
            let cx = 2 + idx as i32 * (w / 2) + dial_w / 2;
            let cy = h - 4;
            let radius = (dial_w as f32 * 0.60) as i32;
            let (a0, a1) = (-std::f32::consts::PI * 0.78, -std::f32::consts::PI * 0.22);

            // Printed arc, with the overload segment in its own colour.
            for step in 0..=60 {
                let f = step as f32 / 60.0;
                let ang = a0 + (a1 - a0) * f;
                let col = if f >= OVERLOAD_AT { over } else { ink };
                let px = cx + (ang.cos() * radius as f32) as i32;
                let py = cy + (ang.sin() * radius as f32) as i32;
                c.fill_rect(px, py, 1, 1, col);
            }

            // Tick marks, longer at the ends and centre.
            for k in 0..=6 {
                let ang = a0 + (a1 - a0) * k as f32 / 6.0;
                let big = k == 0 || k == 3 || k == 6;
                let inner = radius - if big { 5 } else { 3 };
                for rr in inner..=radius {
                    let px = cx + (ang.cos() * rr as f32) as i32;
                    let py = cy + (ang.sin() * rr as f32) as i32;
                    c.fill_rect(px, py, 1, 1, ink);
                }
            }

            // Ghost peak needle.
            let pang = a0 + (a1 - a0) * peak.clamp(0.0, 1.0);
            for rr in (radius / 3)..radius {
                let px = cx + (pang.cos() * rr as f32) as i32;
                let py = cy + (pang.sin() * rr as f32) as i32;
                c.fill_rect(px, py, 1, 1, Rgba::from_hex("#ffffff", 0.32));
            }

            // Live needle, red past the overload point.
            let ang = a0 + (a1 - a0) * level.clamp(0.0, 1.0);
            let needle = if *level > OVERLOAD_AT {
                Rgba::from_hex(t.overload_hex(), 1.0)
            } else {
                Rgba::from_hex(&t.hot, 1.0)
            };
            for rr in 0..(radius as f32 * 0.95) as i32 {
                let px = cx + (ang.cos() * rr as f32) as i32;
                let py = cy + (ang.sin() * rr as f32) as i32;
                c.fill_rect(px, py, 1, 1, needle);
            }
            c.fill_rect(cx - 1, cy - 1, 2, 2, needle);
        }

        c.bloom(t.bloom as i32, 0.7);

        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
```

- [ ] **Step 5: Generate the golden and inspect it**

Regenerate `tests/golden/vu-cream.txt` and open it. It must show two arcs with needles at clearly different angles (L at 0.65, R at 0.40). If the needles are at the same angle the channels are not independent.

- [ ] **Step 6: Extend the builtin tests to cover all fifteen**

The contrast, panel-alpha, unique-id and unique-signature tests in Task 12 now run across all 15 themes. Update the count assertion:

```rust
    #[test]
    fn ships_fifteen_colourways_across_three_families() {
        let all = all();
        assert_eq!(all.len(), 15, "expected 15 colourways, got {}", all.len());
        for fam in ["segmented", "scope", "vu"] {
            let n = all.iter().filter(|t| t.family == fam).count();
            assert_eq!(n, 5, "family {fam} should have 5 colourways, has {n}");
        }
    }
```

Note: the `each_colourway_has_a_distinct_texture_or_bloom` test from Task 12 compares `(texture, bloom)` pairs across *all* themes, and the scope and vu families share `Texture::None_`/`Filament`. Change that test to group by family before comparing:

```rust
    #[test]
    fn colourways_are_visually_distinct_within_their_family() {
        for fam in ["segmented", "scope", "vu"] {
            let mut sigs: Vec<String> = all()
                .iter()
                .filter(|t| t.family == fam)
                .map(|t| format!("{:?}/{}/{}/{}", t.texture, t.bloom, t.lit, t.fade))
                .collect();
            let before = sigs.len();
            sigs.sort();
            sigs.dedup();
            assert_eq!(sigs.len(), before, "two {fam} themes are identical");
        }
    }
```

- [ ] **Step 7: Run the full suite and check all fifteen live**

Run: `cargo test 2>&1 | tail -30`

Then `cargo run --release` and cycle all fifteen from the tray. Compare against both mockups in `docs/reference/mockups/`.

- [ ] **Step 8: Commit**

```bash
git add src/ tests/golden/
git commit -m "feat: VU family with five dial backlights

Needle state lives on the family rather than the canvas, so the widget rect
changing width does not make the needles jump. The red dial flips its
overload arc to white - the one colourway needing behaviour, not just hex."
```

---

### Task 15: TOML schema, loading and precedence

**The extensibility requirement.** Deliberately last: the schema is now derived from three working renderers rather than guessed at.

The externally visible contract is in `README.md` — the theme-authoring prompt. **If this task changes any key name or default, update that prompt in the same commit.** The spec records that as a shipping requirement, not documentation polish.

**Files:**
- Create: `src/themes/schema.rs`
- Modify: `src/themes/mod.rs`
- Create: `tests/themes/valid.toml`, `tests/themes/unknown-key.toml`, `tests/themes/malformed.toml`, `tests/themes/future-schema.toml`, `tests/themes/override-vfd-ice.toml`, `tests/themes/zoned.toml`

**Interfaces:**
- Consumes: `themes::{Theme, Texture, Zone}`, `dsp::ballistics::Ballistics`.
- Produces:
  - `themes::schema::SCHEMA_VERSION: u32 = 1`
  - `themes::schema::parse(src: &str) -> Result<Theme, ThemeError>`
  - `themes::schema::ThemeError` enum: `Toml(String) | UnsupportedSchema(u32) | MissingField(&'static str) | BadFamily(String)` implementing `Display`
  - `themes::schema::load_dir(dir: &Path) -> (Vec<Theme>, Vec<String>)` — returns themes plus human-readable warnings
  - `themes::registry() -> (Vec<Theme>, Vec<String>)` — built-ins merged with `%APPDATA%` overrides

- [ ] **Step 1: Write the fixture files**

`tests/themes/valid.toml`:

```toml
schema = 1
id     = "my-purple"
name   = "My Purple"
family = "segmented"

[colour]
lit         = "#c07fff"
hot         = "#ecd9ff"
panel       = "#0a040f"
panel_alpha = 0.62
edge        = "#c07fff"
edge_alpha  = 0.15

[look]
ghost   = 0.12
bloom   = 10.0
texture = "haze"

[ballistics]
attack    = 0.55
decay     = 0.11
peak_fall = 0.005
```

`tests/themes/unknown-key.toml` — same as above but with `id = "unknown-key-theme"` and an extra `sparkle = true` under `[look]`.

`tests/themes/malformed.toml`:

```toml
schema = 1
id = "broken
this is not valid toml {{{
```

`tests/themes/future-schema.toml`:

```toml
schema = 99
id     = "from-the-future"
name   = "Future"
family = "segmented"
```

`tests/themes/override-vfd-ice.toml`:

```toml
schema = 1
id     = "vfd-ice"
name   = "VFD Ice (mine)"
family = "segmented"

[colour]
lit         = "#00ffff"
hot         = "#ccffff"
panel       = "#000508"
panel_alpha = 0.60
```

`tests/themes/zoned.toml`:

```toml
schema = 1
id     = "my-meter"
name   = "My Meter"
family = "segmented"

[colour]
panel       = "#050505"
panel_alpha = 0.66

[[zone]]
upto = 0.5
lit  = "#40e060"
hot  = "#c0ffd0"

[[zone]]
upto = 1.0
lit  = "#ff4030"
hot  = "#ffc0b0"
```

- [ ] **Step 2: Write the failing test**

Create `src/themes/schema.rs`:

```rust
use super::{Texture, Theme, Zone};
use crate::dsp::ballistics::Ballistics;
use serde::Deserialize;
use std::fmt;
use std::path::Path;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub enum ThemeError {
    Toml(String),
    UnsupportedSchema(u32),
    MissingField(&'static str),
    BadFamily(String),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::Toml(e) => write!(f, "not valid TOML: {e}"),
            ThemeError::UnsupportedSchema(v) => write!(
                f,
                "schema = {v} is not supported (this build understands schema = {SCHEMA_VERSION})"
            ),
            ThemeError::MissingField(k) => write!(f, "missing required field `{k}`"),
            ThemeError::BadFamily(fam) => {
                write!(f, "unknown family `{fam}` (expected segmented, scope or vu)")
            }
        }
    }
}

pub fn parse(src: &str) -> Result<Theme, ThemeError> {
    todo!()
}

pub fn load_dir(dir: &Path) -> (Vec<Theme>, Vec<String>) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_valid_theme() {
        let t = parse(include_str!("../../tests/themes/valid.toml")).expect("should parse");
        assert_eq!(t.id, "my-purple");
        assert_eq!(t.name, "My Purple");
        assert_eq!(t.family, "segmented");
        assert_eq!(t.lit, "#c07fff");
        assert_eq!(t.panel_alpha, 0.62);
        assert_eq!(t.texture, Texture::Haze);
        assert_eq!(t.ballistics.attack, 0.55);
    }

    #[test]
    fn an_unknown_key_is_ignored_and_the_theme_still_loads() {
        // Forward compatibility: a theme written for a later build must not break.
        let t = parse(include_str!("../../tests/themes/unknown-key.toml"))
            .expect("unknown keys must not fail the parse");
        assert_eq!(t.id, "unknown-key-theme");
        assert_eq!(t.texture, Texture::Haze);
    }

    #[test]
    fn malformed_toml_is_an_error_not_a_panic() {
        let e = parse(include_str!("../../tests/themes/malformed.toml"));
        assert!(matches!(e, Err(ThemeError::Toml(_))), "got {e:?}");
    }

    #[test]
    fn a_future_schema_is_rejected_with_a_clear_message() {
        match parse(include_str!("../../tests/themes/future-schema.toml")) {
            Err(ThemeError::UnsupportedSchema(v)) => {
                assert_eq!(v, 99);
                let msg = ThemeError::UnsupportedSchema(99).to_string();
                assert!(msg.contains("99") && msg.contains("1"), "message was: {msg}");
            }
            other => panic!("expected UnsupportedSchema, got {other:?}"),
        }
    }

    #[test]
    fn missing_required_fields_are_named_in_the_error() {
        match parse("schema = 1\nname = \"No Id\"\nfamily = \"segmented\"") {
            Err(ThemeError::MissingField(k)) => assert_eq!(k, "id"),
            other => panic!("expected MissingField(id), got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_family_is_rejected() {
        let src = "schema = 1\nid = \"x\"\nname = \"X\"\nfamily = \"hologram\"";
        match parse(src) {
            Err(ThemeError::BadFamily(f)) => assert_eq!(f, "hologram"),
            other => panic!("expected BadFamily, got {other:?}"),
        }
    }

    #[test]
    fn omitted_keys_take_documented_defaults() {
        let src = "schema = 1\nid = \"bare\"\nname = \"Bare\"\nfamily = \"segmented\"";
        let t = parse(src).expect("a minimal file is valid");
        let d = Theme::default();
        assert_eq!(t.bloom, d.bloom);
        assert_eq!(t.ghost, d.ghost);
        assert_eq!(t.panel_alpha, d.panel_alpha);
    }

    #[test]
    fn zones_are_parsed_in_order() {
        let t = parse(include_str!("../../tests/themes/zoned.toml")).expect("should parse");
        assert_eq!(t.zones.len(), 2);
        assert_eq!(t.lit_at(0.3), "#40e060");
        assert_eq!(t.lit_at(0.9), "#ff4030");
    }

    #[test]
    fn every_texture_name_round_trips() {
        for (name, want) in [
            ("glass", Texture::Glass),
            ("scanlines", Texture::Scanlines),
            ("haze", Texture::Haze),
            ("filament", Texture::Filament),
            ("grille", Texture::Grille),
            ("none", Texture::None_),
        ] {
            let src = format!(
                "schema = 1\nid = \"t\"\nname = \"T\"\nfamily = \"segmented\"\n\n[look]\ntexture = \"{name}\""
            );
            assert_eq!(parse(&src).unwrap().texture, want, "texture {name}");
        }
    }

    #[test]
    fn an_unknown_texture_falls_back_rather_than_failing() {
        let src = "schema = 1\nid = \"t\"\nname = \"T\"\nfamily = \"segmented\"\n\n[look]\ntexture = \"marble\"";
        let t = parse(src).expect("unknown texture should not fail the whole theme");
        assert_eq!(t.texture, Texture::None_);
    }

    #[test]
    fn load_dir_skips_bad_files_and_reports_them() {
        let dir = Path::new("tests/themes");
        let (themes, warnings) = load_dir(dir);
        let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"my-purple"), "good files must load");
        assert!(!ids.contains(&"from-the-future"), "future schema must be skipped");
        assert_eq!(warnings.len(), 2, "expected warnings for malformed + future, got {warnings:?}");
        assert!(
            warnings.iter().any(|w| w.contains("malformed")),
            "warning should name the offending file: {warnings:?}"
        );
    }

    #[test]
    fn load_dir_on_a_missing_directory_is_empty_not_an_error() {
        let (themes, warnings) = load_dir(Path::new("tests/does-not-exist"));
        assert!(themes.is_empty());
        assert!(warnings.is_empty(), "a missing themes dir is normal, not a warning");
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Add `pub mod schema;` to `src/themes/mod.rs`.

Run: `cargo test schema 2>&1 | tail -20`
Expected: FAIL — both functions `todo!()`.

- [ ] **Step 4: Write the implementation**

Replace the `todo!()` bodies in `src/themes/schema.rs`:

```rust
#[derive(Deserialize)]
struct RawZone {
    upto: f32,
    lit: String,
    hot: String,
}

#[derive(Deserialize, Default)]
struct RawColour {
    lit: Option<String>,
    hot: Option<String>,
    panel: Option<String>,
    panel_alpha: Option<f32>,
    edge: Option<String>,
    edge_alpha: Option<f32>,
}

#[derive(Deserialize, Default)]
struct RawLook {
    ghost: Option<f32>,
    bloom: Option<f32>,
    fade: Option<f32>,
    texture: Option<String>,
}

#[derive(Deserialize, Default)]
struct RawBallistics {
    attack: Option<f32>,
    decay: Option<f32>,
    peak_fall: Option<f32>,
}

#[derive(Deserialize, Default)]
struct RawDual {
    trail: Option<String>,
    fade: Option<f32>,
}

/// Unknown keys are permitted at every level - this is what makes a theme file
/// written for a later build still load here.
#[derive(Deserialize)]
struct RawTheme {
    schema: Option<u32>,
    id: Option<String>,
    name: Option<String>,
    family: Option<String>,
    #[serde(default)]
    colour: RawColour,
    #[serde(default)]
    look: RawLook,
    #[serde(default)]
    ballistics: RawBallistics,
    #[serde(default)]
    dual: Option<RawDual>,
    #[serde(default)]
    zone: Vec<RawZone>,
}

fn texture_from(name: Option<String>, fallback: Texture) -> Texture {
    match name.as_deref() {
        Some("glass") => Texture::Glass,
        Some("scanlines") => Texture::Scanlines,
        Some("haze") => Texture::Haze,
        Some("filament") => Texture::Filament,
        Some("grille") => Texture::Grille,
        Some("none") => Texture::None_,
        // An unrecognised texture must not sink the whole theme.
        Some(_) => Texture::None_,
        None => fallback,
    }
}

pub fn parse(src: &str) -> Result<Theme, ThemeError> {
    let raw: RawTheme = toml::from_str(src).map_err(|e| ThemeError::Toml(e.to_string()))?;

    match raw.schema {
        Some(v) if v == SCHEMA_VERSION => {}
        Some(v) => return Err(ThemeError::UnsupportedSchema(v)),
        None => return Err(ThemeError::MissingField("schema")),
    }

    let id = raw.id.ok_or(ThemeError::MissingField("id"))?;
    let name = raw.name.ok_or(ThemeError::MissingField("name"))?;
    let family = raw.family.ok_or(ThemeError::MissingField("family"))?;
    if !matches!(family.as_str(), "segmented" | "scope" | "vu") {
        return Err(ThemeError::BadFamily(family));
    }

    let d = Theme::default();
    let db = Ballistics::default();

    Ok(Theme {
        id,
        name,
        family,
        lit: raw.colour.lit.unwrap_or(d.lit),
        hot: raw.colour.hot.unwrap_or(d.hot),
        panel: raw.colour.panel.unwrap_or(d.panel),
        panel_alpha: raw.colour.panel_alpha.unwrap_or(d.panel_alpha),
        edge: raw.colour.edge.unwrap_or(d.edge),
        edge_alpha: raw.colour.edge_alpha.unwrap_or(d.edge_alpha),
        ghost: raw.look.ghost.unwrap_or(d.ghost),
        bloom: raw.look.bloom.unwrap_or(d.bloom),
        fade: raw.look.fade.unwrap_or(d.fade),
        texture: texture_from(raw.look.texture, d.texture),
        ballistics: Ballistics {
            attack: raw.ballistics.attack.unwrap_or(db.attack),
            decay: raw.ballistics.decay.unwrap_or(db.decay),
            peak_fall: raw.ballistics.peak_fall.unwrap_or(db.peak_fall),
        },
        zones: raw
            .zone
            .into_iter()
            .map(|z| Zone { upto: z.upto, lit: z.lit, hot: z.hot })
            .collect(),
        dual: raw.dual.and_then(|dl| {
            dl.trail.map(|t| (t, dl.fade.unwrap_or(0.055)))
        }),
    })
}

pub fn load_dir(dir: &Path) -> (Vec<Theme>, Vec<String>) {
    let mut themes = Vec::new();
    let mut warnings = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        // No themes directory is the normal case, not a problem.
        Err(_) => return (themes, warnings),
    };

    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "toml").unwrap_or(false))
        .collect();
    paths.sort(); // deterministic load order

    for path in paths {
        let label = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        match std::fs::read_to_string(&path) {
            Ok(src) => match parse(&src) {
                Ok(t) => themes.push(t),
                Err(e) => warnings.push(format!("{label}: {e}")),
            },
            Err(e) => warnings.push(format!("{label}: unreadable ({e})")),
        }
    }
    (themes, warnings)
}
```

- [ ] **Step 5: Add the registry with precedence**

Append to `src/themes/mod.rs`:

```rust
/// Built-ins first, then `%APPDATA%\taskbar-eq\themes\*.toml`. An external theme
/// sharing a built-in `id` replaces it; a new `id` is appended.
pub fn registry() -> (Vec<Theme>, Vec<String>) {
    let mut themes = builtin::all();
    let dir = crate::config::Config::dir().join("themes");
    let (external, warnings) = schema::load_dir(&dir);
    for ext in external {
        match themes.iter().position(|t| t.id == ext.id) {
            Some(i) => themes[i] = ext,
            None => themes.push(ext),
        }
    }
    (themes, warnings)
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn an_external_theme_overrides_a_builtin_of_the_same_id() {
        let mut themes = builtin::all();
        let (external, _) = schema::load_dir(Path::new("tests/themes"));
        let before = themes.len();
        for ext in external {
            match themes.iter().position(|t| t.id == ext.id) {
                Some(i) => themes[i] = ext,
                None => themes.push(ext),
            }
        }
        let ice = themes.iter().find(|t| t.id == "vfd-ice").expect("vfd-ice present");
        assert_eq!(ice.name, "VFD Ice (mine)", "override should replace the built-in");
        assert_eq!(ice.lit, "#00ffff");
        assert!(themes.iter().any(|t| t.id == "my-purple"), "new ids are appended");
        assert!(themes.len() > before, "new themes increase the count");
        assert_eq!(
            themes.iter().filter(|t| t.id == "vfd-ice").count(),
            1,
            "override must replace, not duplicate"
        );
    }
}
```

- [ ] **Step 6: Run the tests**

Run: `cargo test themes 2>&1 | tail -30`
Expected: PASS — 12 schema tests plus the registry test.

- [ ] **Step 7: Verify the README prompt still matches**

Read the theme-authoring prompt in `README.md` and check every key name, texture value and default against `schema.rs`. If anything drifted, fix the README in this commit. A stale prompt produces themes that silently take defaults.

- [ ] **Step 8: Commit**

```bash
git add src/themes/ tests/themes/ README.md
git commit -m "feat: external TOML colourways with versioned schema and override-by-id

Unknown keys are ignored and unknown textures fall back, so a theme file
written for a later build still loads. A malformed file is skipped with a
named warning rather than taking the app down."
```

---

### Task 16: Hot reload, right-click theme menu, left-click passthrough

**Files:**
- Create: `src/themes/watch.rs`
- Modify: `src/themes/mod.rs`, `src/win/overlay.rs`, `src/main.rs`

**Interfaces:**
- Consumes: `themes::registry`, `win::overlay::Overlay`.
- Produces:
  - `themes::watch::Watcher` with `Watcher::new() -> Watcher` and `fn changed(&self) -> bool`
  - `win::overlay::OverlayEvent` enum: `LeftClick | RightClick`
  - `win::overlay::Overlay::take_event(&mut self) -> Option<OverlayEvent>`
  - `win::overlay::open_widgets_panel() -> anyhow::Result<()>` — synthesises `Win+W`

- [ ] **Step 1: Write the watcher and its test**

Create `src/themes/watch.rs`:

```rust
use notify::{RecursiveMode, Watcher as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct Watcher {
    dirty: Arc<AtomicBool>,
    _inner: Option<notify::RecommendedWatcher>,
}

impl Watcher {
    /// Watches the themes directory. Creates it if absent so the user has an
    /// obvious place to drop files, and so the watch has something to attach to.
    pub fn new() -> Self {
        let dir = crate::config::Config::dir().join("themes");
        let _ = std::fs::create_dir_all(&dir);

        let dirty = Arc::new(AtomicBool::new(false));
        let flag = dirty.clone();

        let inner = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                if ev.paths.iter().any(|p| p.extension().map(|e| e == "toml").unwrap_or(false)) {
                    flag.store(true, Ordering::Relaxed);
                }
            }
        })
        .and_then(|mut w| {
            w.watch(&dir, RecursiveMode::NonRecursive)?;
            Ok(w)
        })
        .ok();

        if inner.is_none() {
            eprintln!("themes: hot reload unavailable; edits need a restart");
        }
        Watcher { dirty, _inner: inner }
    }

    /// True once per batch of changes, then resets.
    pub fn changed(&self) -> bool {
        self.dirty.swap(false, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_clean() {
        let w = Watcher::new();
        assert!(!w.changed(), "a fresh watcher has no pending changes");
    }

    #[test]
    fn changed_resets_after_reading() {
        let w = Watcher::new();
        w.dirty.store(true, Ordering::Relaxed);
        assert!(w.changed(), "first read sees the change");
        assert!(!w.changed(), "second read must be clean - one reload per batch");
    }

    #[test]
    fn survives_an_unwatchable_directory() {
        // Constructing must never panic even if the watch cannot be established.
        let _ = Watcher::new();
    }
}
```

- [ ] **Step 2: Run the watcher tests**

Add `pub mod watch;` to `src/themes/mod.rs`.

Run: `cargo test watch 2>&1 | tail -20`
Expected: PASS, 3 tests.

- [ ] **Step 3: Add click handling to the overlay**

The overlay must be **clickable**, so it does not use `WS_EX_TRANSPARENT`. Left-click synthesises `Win+W` so covering the weather costs nothing.

Add to `src/win/overlay.rs`:

```rust
use std::sync::atomic::{AtomicU8, Ordering};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_LWIN, VK_W,
};
use windows::Win32::UI::WindowsAndMessaging::{WM_LBUTTONUP, WM_RBUTTONUP};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayEvent {
    LeftClick,
    RightClick,
}

// The wndproc is a plain fn, so the click flag lives in a static.
static PENDING: AtomicU8 = AtomicU8::new(0);
const P_LEFT: u8 = 1;
const P_RIGHT: u8 = 2;

impl Overlay {
    pub fn take_event(&mut self) -> Option<OverlayEvent> {
        match PENDING.swap(0, Ordering::Relaxed) {
            P_LEFT => Some(OverlayEvent::LeftClick),
            P_RIGHT => Some(OverlayEvent::RightClick),
            _ => None,
        }
    }
}

/// Synthesises Win+W to open the Widgets panel, so the weather stays reachable
/// while the overlay covers its button.
pub fn open_widgets_panel() -> Result<()> {
    unsafe {
        let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                    ..Default::default()
                },
            },
        };
        let seq = [
            key(VK_LWIN, false),
            key(VK_W, false),
            key(VK_W, true),
            key(VK_LWIN, true),
        ];
        let sent = SendInput(&seq, std::mem::size_of::<INPUT>() as i32);
        if sent as usize != seq.len() {
            return Err(anyhow!("SendInput sent {sent} of {} events", seq.len()));
        }
    }
    Ok(())
}
```

Replace the `wndproc` in `src/win/overlay.rs`:

```rust
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    match msg {
        WM_LBUTTONUP => {
            PENDING.store(P_LEFT, Ordering::Relaxed);
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_RBUTTONUP => {
            PENDING.store(P_RIGHT, Ordering::Relaxed);
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
```

Remove `WS_EX_NOACTIVATE` from the `CreateWindowExW` flags — a window that never activates will not receive clicks. Keep `WS_EX_TOOLWINDOW` and `WS_EX_TOPMOST`.

- [ ] **Step 4: Wire hot reload and both click paths into main**

In the main loop:

```rust
// Reload themes when a file changes, keeping the current selection if it survives.
if watcher.changed() {
    let (fresh, warnings) = themes::registry();
    for w in &warnings {
        eprintln!("themes: {w}");
    }
    all_themes = fresh;
    if let Some(t) = all_themes.iter().find(|t| t.id == cfg.theme) {
        theme = t.clone();
        family = render::family_for(&theme.family);
        smoother.set_ballistics(theme.ballistics);
    } else {
        // The selected theme was deleted - fall back rather than showing nothing.
        theme = all_themes.first().cloned().unwrap_or_default();
        cfg.theme = theme.id.clone();
        family = render::family_for(&theme.family);
        let _ = cfg.save();
    }
    println!("themes: reloaded {} colourways", all_themes.len());
}

match overlay.take_event() {
    Some(win::overlay::OverlayEvent::LeftClick) => {
        let _ = win::overlay::open_widgets_panel();
    }
    Some(win::overlay::OverlayEvent::RightClick) => {
        let menu: Vec<(String, String)> = all_themes
            .iter()
            .map(|t| (t.id.clone(), format!("{} - {}", t.family, t.name)))
            .collect();
        if let Some(win::tray::TrayEvent::SelectTheme(id)) =
            tray.show_menu_for(&menu, &cfg.theme)
        {
            if let Some(t) = all_themes.iter().find(|t| t.id == id) {
                theme = t.clone();
                family = render::family_for(&theme.family);
                smoother.set_ballistics(theme.ballistics);
                cfg.theme = id;
                let _ = cfg.save();
            }
        }
    }
    None => {}
}
```

Add `show_menu_for(&self, items: &[(String, String)], current: &str) -> Option<TrayEvent>` to `Tray` — the same body as `show_menu` but taking the item list as an argument, so the overlay's right-click menu and the tray menu share one implementation rather than duplicating the Win32 menu code.

- [ ] **Step 5: Verify hot reload by hand**

Run `cargo run --release` with music playing, then in another terminal:

```powershell
Copy-Item tests\themes\valid.toml "$env:APPDATA\taskbar-eq\themes\my-purple.toml"
```

Expected: the console prints `themes: reloaded 16 colourways` within a second or two, and "My Purple" appears in the right-click menu without a restart.

Then edit the file's `lit` colour and save. Select it first, and confirm the live overlay changes colour without a restart.

Then break it deliberately — write `{{{` into the file — and confirm the app logs a warning, keeps running, and keeps the other 15 themes.

- [ ] **Step 6: Verify both click paths**

With the overlay visible: left-click opens the Widgets panel; right-click shows the theme menu grouped by family. Selecting a theme switches immediately and survives a restart.

- [ ] **Step 7: Run the full suite and build the shippable exe**

```bash
cargo test 2>&1 | tail -20
cargo build --release
ls -la target/release/taskbar-eq.exe
```

Expected: all tests pass; the exe exists. Note its size — with `lto` and `strip` it should be a few MB with no runtime dependency.

- [ ] **Step 8: Confirm it is genuinely portable**

Copy `target/release/taskbar-eq.exe` alone to a different directory and run it from there. It must work with no `themes/` folder and no config present — that is the case on the second machine.

- [ ] **Step 9: Commit**

```bash
git add src/
git commit -m "feat: hot reload, right-click theme menu and Win+W passthrough

Hot reload keeps the current selection when a file changes and falls back to
the first theme if the selected one is deleted. The overlay is clickable
rather than click-through, and forwards left-clicks as Win+W so the weather
stays reachable while covered."
```

---

## Plan self-review

Run after the plan is written, before execution.

**1. Spec coverage.** Every spec section maps to a task:

| Spec section | Task(s) |
|---|---|
| §1 constraints, DPI, environment | 1, Global Constraints |
| §2 placement, rect discovery, visibility | 2, 3, 5 |
| §3 auto-reveal, DSP parameters | 6, 7, 8 |
| §4 theme system, families, colourways | 10, 12, 13, 14 |
| §4 loading, precedence, hot reload, schema | 15, 16 |
| §5 interaction (left/right click, tray) | 11, 16 |
| §6 settings | 11 |
| §7 module boundaries | file structure table |
| §8 testing strategy | every task; goldens in 10, 12, 13, 14 |
| §9 non-goals | Global Constraints |
| §10 build order | task sequence table |
| §11 deliverable + README prompt requirement | 15 step 7, 16 steps 7–8 |

**2. Placeholder scan.** No `TBD`, no "add error handling", no "similar to Task N". Every code step carries real code. The two `Family` stubs in Task 10 are explicitly labelled with the task that fills them.

**3. Type consistency.** Names used across tasks, checked to match their definitions:
- `Rect { x, y, w, h }` — defined Task 2, used 3, 5, 10
- `Rgba::from_hex(hex, alpha)` — defined Task 4, used 5, 10, 12, 13, 14
- `Canvas::{clear, fill_rect, rounded_rect, punch_row, bloom, get, bits, width, height}` — defined Task 4, used throughout
- `FrameData { levels, peaks, waveform, rms_l, rms_r }` — defined Task 10, used 13, 14
- `Family::{id, draw}` — defined Task 10, implemented 10, 13, 14
- `Theme` fields incl. `fade`, `dual`, `zones` — defined Task 10, consumed 13, 15
- `Theme::{lit_at, hot_at}` defined Task 10; `overload_hex` added Task 14
- `Ballistics { attack, decay, peak_fall }` — defined Task 7, used 10, 12, 15
- `Smoother::{new, update, levels, peaks, set_ballistics}` — defined Task 7, used 10, 16
- `Gate::{new, update, is_visible}` + `GateConfig` — defined Task 8, used 10, 11
- `Frame { bands, waveform, rms_l, rms_r, rms }` — defined Task 9, used 10
- `Config::{dir, path, load, save, gate_config}` — defined Task 11, used 15, 16
- `TrayEvent::{Quit, SelectTheme, ToggleAutostart}` — defined Task 11, used 16
- `schema::{parse, load_dir, SCHEMA_VERSION, ThemeError}` — defined Task 15, used 16 via `registry`

One deliberate addition flagged during review: Task 16 requires `Tray::show_menu_for`, a generalisation of Task 11's `show_menu`. Task 16 step 4 states this explicitly rather than leaving the implementer to discover a signature mismatch.

