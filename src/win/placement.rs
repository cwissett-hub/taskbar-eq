use crate::geom::Rect;
use anyhow::Result;
use windows::core::{w, BSTR};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants};
use windows::Win32::UI::Shell::SHQueryUserNotificationState;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect, IsWindowVisible};

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

    /// The real taskbar of the development machine, captured with
    /// `tools/probe/Probe-Taskbar.ps1`. Physical pixels, 125% scale, taskbar row at y=1140.
    ///
    /// Real coordinates rather than invented ones, because the numbers are the whole point:
    /// the last pinned app ends at 1064 and the widget starts at 1416, so there are 352px of
    /// dead taskbar to claim - and only 15px on the other side before "Show Hidden Icons",
    /// which is why widening goes left and not right.
    fn observed_taskbar() -> Vec<(String, Rect)> {
        let row = |x: i32, w: i32| Rect { x, y: 1140, w, h: 60 };
        vec![
            ("Start".into(), row(0, 69)),
            ("Search".into(), Rect { x: 72, y: 1150, w: 275, h: 40 }),
            ("Task View".into(), row(349, 55)),
            ("Microsoft Teams pinned".into(), row(404, 55)),
            ("File Explorer pinned".into(), row(459, 55)),
            ("Microsoft Edge pinned".into(), row(514, 55)),
            ("Microsoft Store pinned".into(), row(569, 55)),
            ("Google Chrome pinned".into(), row(624, 55)),
            ("Lens Studio pinned".into(), row(679, 55)),
            ("Cursor pinned".into(), row(734, 55)),
            ("Claude pinned".into(), row(789, 55)),
            ("Visual Studio Code - 1 running window pinned".into(), row(844, 55)),
            ("Spotify - 1 running window".into(), row(899, 55)),
            ("Slack - 1 running window".into(), row(954, 55)),
            ("Corin (Snap Inc.) - Chrome - 1 running window".into(), row(1009, 55)),
            // The container under-reports: it stops at 899 while its buttons reach 1064.
            ("Running applications".into(), Rect { x: 69, y: 1140, w: 830, h: 60 }),
            ("Widgets 22\u{b0}C Partly sunny".into(), row(1416, 190)),
            ("Show Hidden Icons".into(), row(1621, 40)),
            ("Network Lenses - Primary".into(), row(1661, 35)),
            ("Volume Speakers".into(), row(1696, 30)),
            ("Power Battery status: fully charged 100%".into(), row(1726, 82)),
            ("Clock 17:22".into(), row(1808, 97)),
            ("Show Desktop".into(), row(1905, 15)),
        ]
    }

    fn observed_bar() -> Rect {
        Rect { x: 0, y: 1140, w: 1920, h: 60 }
    }

    #[test]
    fn left_limit_finds_the_last_app_button_not_the_container() {
        let els = observed_taskbar();
        let widget = widget_rect_in(&els).expect("the observed layout has a widget");
        assert_eq!(widget, Rect { x: 1416, y: 1140, w: 190, h: 60 });
        // 1064 is the right edge of the last pinned app. Trusting the "Running applications"
        // pane instead would give 899 and let the overlay cover three real buttons.
        assert_eq!(left_limit(&els, widget), Some(1064));
    }

    #[test]
    fn left_limit_ignores_elements_on_a_different_row() {
        let mut els = observed_taskbar();
        let widget = widget_rect_in(&els).unwrap();
        // A flyout or a secondary-monitor element well above the taskbar row must not count as
        // being "in the way", or the overlay would refuse to widen for no reason.
        els.push(("Some flyout".into(), Rect { x: 1300, y: 400, w: 200, h: 300 }));
        assert_eq!(left_limit(&els, widget), Some(1064));
    }

    #[test]
    fn doubling_the_width_fits_on_the_real_taskbar() {
        let els = observed_taskbar();
        let widget = widget_rect_in(&els).unwrap();
        let r = widened(widget, observed_bar(), left_limit(&els, widget), 380, 8);
        assert_eq!(r.w, 380, "380 fits in the 352px gap plus the widget's own 190");
        // Right edge stays glued to the widget's right edge.
        assert_eq!(r.x + r.w, widget.x + widget.w);
        assert!(r.x >= 1064 + 8, "must not touch the last app button: x={}", r.x);
    }

    #[test]
    fn a_crowded_taskbar_shrinks_the_display_instead_of_covering_buttons() {
        let els = observed_taskbar();
        let widget = widget_rect_in(&els).unwrap();
        // Six more windows open, at 55px each, pushing the last button's edge to 1394.
        let r = widened(widget, observed_bar(), Some(1394), 380, 8);
        assert!(r.w < 380, "it must give up width rather than overlap: w={}", r.w);
        assert_eq!(r.x, 1402, "left edge sits exactly `margin` clear of the last button");
        assert!(r.x >= 1394 + 8);
    }

    #[test]
    fn a_full_taskbar_degrades_to_exactly_the_widget() {
        let els = observed_taskbar();
        let widget = widget_rect_in(&els).unwrap();
        // Apps right up against the widget - there is no room at all.
        let r = widened(widget, observed_bar(), Some(1416), 380, 8);
        assert_eq!(r, widget, "with no clearance it must fall back to the widget's own rect");
    }

    #[test]
    fn never_narrower_than_the_widget_even_if_asked() {
        let els = observed_taskbar();
        let widget = widget_rect_in(&els).unwrap();
        // Covering the weather is the point, so a silly config value cannot expose it.
        for desired in [0, 1, 40, 100, 189] {
            let r = widened(widget, observed_bar(), left_limit(&els, widget), desired, 8);
            assert_eq!(r.w, widget.w, "desired {desired} must not shrink below the widget");
        }
    }

    #[test]
    fn stays_inside_the_taskbar_when_the_widget_is_near_the_left_edge() {
        // A left-aligned taskbar puts the widget close to x=0, so an ambitious width would
        // otherwise place the window at a negative x and draw off-screen.
        let widget = Rect { x: 40, y: 1140, w: 190, h: 60 };
        let r = widened(widget, observed_bar(), None, 900, 8);
        assert!(r.x >= 0, "x must not go negative: {}", r.x);
        assert!(r.x + r.w <= widget.x + widget.w);
    }

    #[test]
    fn the_width_is_stable_while_the_weather_text_changes_length() {
        // The regression this guards: the overlay used the widget rect verbatim, so its WIDTH
        // moved with the forecast wording - and the scope family clears its persistence buffers
        // on any canvas resize, so the phosphor trail was wiped when the weather changed.
        let bar = observed_bar();
        let mut widths = std::collections::HashSet::new();
        for (x, w) in [(1416, 190), (1385, 221), (1425, 181), (1440, 166)] {
            let widget = Rect { x, y: 1140, w, h: 60 };
            widths.insert(widened(widget, bar, Some(1064), 380, 8).w);
        }
        assert_eq!(widths.len(), 1, "width must not vary with the weather text: {widths:?}");
    }
}

fn tray_hwnd() -> Result<HWND> {
    Ok(unsafe { FindWindowW(w!("Shell_TrayWnd"), None)? })
}

/// Every named element on the taskbar with its rect, in physical pixels, from ONE UIA walk.
///
/// One walk rather than three. Finding the widget, the chevron and the left-hand clearance each
/// used to instantiate a COM automation object and enumerate the whole descendant tree - three
/// times a second, for data that is consistent only if it comes from a single snapshot. Reading
/// the clearance from a different enumeration than the widget rect would let the two disagree
/// about where things are mid-move.
pub fn taskbar_elements() -> Result<Vec<(String, Rect)>> {
    let tray = tray_hwnd()?;
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let root = unsafe { automation.ElementFromHandle(tray)? };
    let cond = unsafe { automation.CreateTrueCondition()? };
    let all = unsafe { root.FindAll(TreeScope_Descendants, &cond)? };

    let mut out = Vec::new();
    for i in 0..unsafe { all.Length()? } {
        let el = unsafe { all.GetElement(i)? };
        let name: BSTR = unsafe { el.CurrentName().unwrap_or_default() };
        let n = name.to_string();
        if n.is_empty() {
            continue;
        }
        if let Ok(r) = unsafe { el.CurrentBoundingRectangle() } {
            out.push((
                n,
                Rect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top },
            ));
        }
    }
    Ok(out)
}

/// Right edge of the nearest thing to the LEFT of the widget on the same taskbar row.
///
/// This is the hard limit on how far the display may be widened, and the limit is FUNCTIONAL,
/// not cosmetic: the overlay deliberately does not set WS_EX_TRANSPARENT, because it has to
/// receive its own right-click and left-click (see win::overlay). So any pixel it covers is a
/// pixel the taskbar underneath can no longer be clicked on. Overrunning this would eat a
/// pinned app button.
///
/// Returns None when nothing is to the left, in which case the taskbar's own left edge is the
/// only limit.
///
/// Containers are harmless here because this takes the MAXIMUM right edge: on a real taskbar the
/// "Running applications" pane reported a right edge of 899 while the app buttons it contains
/// actually reached 1064, so trusting the container alone would have allowed the overlay to
/// cover five pinned buttons.
pub fn left_limit(elements: &[(String, Rect)], widget: Rect) -> Option<i32> {
    let (band_top, band_bottom) = (widget.y, widget.y + widget.h);
    elements
        .iter()
        .filter(|(_, r)| r.w > 0 && r.h > 0)
        // same taskbar row - a vertically disjoint element is not in the way
        .filter(|(_, r)| r.y < band_bottom && r.y + r.h > band_top)
        // entirely to the left of the widget
        .filter(|(_, r)| r.x + r.w <= widget.x)
        .map(|(_, r)| r.x + r.w)
        .max()
}

/// Widens the display leftward from the widget, stopping short of whatever is already there.
///
/// Anchored to the widget's RIGHT edge, which also fixes something that was already wrong: the
/// overlay used the widget's rect verbatim, and that rect's WIDTH changes with the weather text
/// ("22C Partly sunny" is wider than "5C Fog"). Every such change resized the canvas, and the
/// scope family reallocates - and therefore CLEARS - its persistence buffers whenever the canvas
/// size changes, so the phosphor trail was being wiped whenever the forecast wording changed.
/// Pinning the width makes only `x` track the widget.
///
/// Never returns narrower than the widget itself: covering the weather is the entire point, so a
/// crowded taskbar degrades to exactly the old behaviour rather than to a sliver.
pub fn widened(widget: Rect, taskbar: Rect, left_limit: Option<i32>, desired_w: i32, margin: i32) -> Rect {
    let right = widget.x + widget.w;
    let floor_w = widget.w.max(1);
    let limit = left_limit.unwrap_or(taskbar.x) + margin.max(0);
    let room = (right - limit).max(floor_w);
    let w = desired_w.max(floor_w).min(room).min(taskbar.w.max(floor_w));
    let x = (right - w).max(taskbar.x);
    Rect { x, y: widget.y, w, h: widget.h }
}

/// The widget's rect from an already-captured element list.
pub fn widget_rect_in(elements: &[(String, Rect)]) -> Option<Rect> {
    elements.iter().find(|(n, _)| is_widget_name(n)).map(|(_, r)| *r)
}

/// The chevron's rect from an already-captured element list.
pub fn chevron_rect_in(elements: &[(String, Rect)]) -> Option<Rect> {
    elements.iter().find(|(n, _)| is_chevron_name(n)).map(|(_, r)| *r)
}

pub fn taskbar_rect() -> Option<Rect> {
    let h = tray_hwnd().ok()?;
    let mut r = RECT::default();
    if unsafe { GetWindowRect(h, &mut r) }.is_err() {
        return None;
    }
    Some(Rect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top })
}

/// True for the taskbar's overflow chevron - "Show Hidden Icons" on Windows 11,
/// "Show hidden icons" on Windows 10. Matched case-insensitively on the stable part
/// of the phrase precisely because those differ.
pub fn is_chevron_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("hidden icons")
}

/// Places the display to the LEFT of the overflow chevron.
///
/// This is the Windows 10 answer as well as the "put it somewhere sensible" answer.
/// Win10 has no Widgets button at all - it has News and interests, a differently
/// named element - so on that OS `widget_rect_in` always returns None and the
/// overlay would otherwise run and show nothing. The chevron exists on both, sits in
/// the same part of the tray on both, and moves with the tray as icons come and go,
/// so anchoring to it gives a consistent position without hardcoding coordinates.
pub fn rect_left_of(anchor: Rect, taskbar: Rect, gap: i32, width: i32) -> Rect {
    let width = width.clamp(40, taskbar.w.max(40));
    let gap = gap.clamp(0, taskbar.w);
    // right edge sits `gap` px left of the anchor, then clamp into the taskbar
    let mut x = anchor.x - gap - width;
    if x < taskbar.x {
        x = taskbar.x;
    }
    Rect { x, y: taskbar.y, w: width, h: taskbar.h }
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

#[cfg(test)]
mod cost_tests {
    /// What the once-a-second UIA rediscovery actually costs.
    ///
    /// Run: cargo test --release probe_uia_cost -- --ignored --nocapture
    ///
    /// Motivated by a report that the overlay caused "massive stuttering in full screen apps while it
    /// was in the background". The render loop skips DRAWING when a fullscreen app is up, but it kept
    /// doing this - a fresh `IUIAutomation` instance, a full descendant enumeration of the taskbar, and
    /// two cross-process property fetches per element - once a second regardless.
    #[test]
    #[ignore]
    fn probe_uia_cost() {
        // COM MUST be initialised, in the same apartment the app uses. Without it `CoCreateInstance`
        // fails, `taskbar_elements` returns Err, the caller's `unwrap_or_default()` hands back an empty
        // vector and the probe cheerfully times the FAILURE path - the first run of this reported
        // "0 elements, median 0.1ms", which is a measurement of nothing at all.
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            );
        }
        let mut times = Vec::new();
        for _ in 0..15 {
            let t0 = std::time::Instant::now();
            let els = super::taskbar_elements().expect("UIA must work, or this measures nothing");
            times.push((t0.elapsed().as_micros() as f64 / 1000.0, els.len()));
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        let mut ms: Vec<f64> = times.iter().map(|(m, _)| *m).collect();
        ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "taskbar_elements(): {} elements, min {:.1}ms  median {:.1}ms  max {:.1}ms",
            times[0].1,
            ms[0],
            ms[ms.len() / 2],
            ms[ms.len() - 1]
        );
        println!("  at once per second that is {:.2}% of one core", ms[ms.len() / 2] / 1000.0 * 100.0);

        // The two calls the shell-state poller makes, for comparison. If these are cheap the poller can
        // keep running while suspended - and it has to, because it is what notices the fullscreen app
        // going away.
        let bench = |name: &str, f: &dyn Fn()| {
            let mut v = Vec::new();
            for _ in 0..200 {
                let t0 = std::time::Instant::now();
                f();
                v.push(t0.elapsed().as_micros() as f64);
            }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!("  {name}: median {:.0}us  max {:.0}us", v[v.len() / 2], v[v.len() - 1]);
        };
        bench("SHQueryUserNotificationState", &|| {
            let _ = super::notification_state();
        });
        bench("taskbar_visible", &|| {
            let _ = super::taskbar_visible();
        });
        bench("taskbar_rect", &|| {
            let _ = super::taskbar_rect();
        });
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    fn bar() -> Rect {
        Rect { x: 0, y: 1140, w: 1920, h: 60 }
    }

    #[test]
    fn chevron_matches_both_windows_spellings() {
        assert!(is_chevron_name("Show Hidden Icons"), "Windows 11");
        assert!(is_chevron_name("Show hidden icons"), "Windows 10");
        assert!(is_chevron_name("SHOW HIDDEN ICONS"));
    }

    #[test]
    fn chevron_predicate_rejects_other_tray_items() {
        for other in ["Widgets 19C Partly cloudy", "Clock 21:08", "Start", "Volume", "Network"] {
            assert!(!is_chevron_name(other), "{other} must not match");
        }
    }

    #[test]
    fn sits_immediately_left_of_the_chevron() {
        // measured chevron on the reference machine
        let chev = Rect { x: 1590, y: 1140, w: 40, h: 60 };
        let r = rect_left_of(chev, bar(), 4, 190);
        assert_eq!(r.x + r.w, chev.x - 4, "right edge sits `gap` px left of the chevron");
        assert_eq!(r.w, 190);
        assert_eq!(r.y, 1140);
        assert_eq!(r.h, 60, "takes the taskbar's full height");
    }

    #[test]
    fn clamps_into_the_taskbar_rather_than_going_off_the_left_edge() {
        let chev = Rect { x: 60, y: 1140, w: 40, h: 60 };
        let r = rect_left_of(chev, bar(), 4, 190);
        assert_eq!(r.x, bar().x, "clamped flush left instead of negative");
        assert!(r.is_plausible_widget());
    }

    #[test]
    fn fallback_rect_is_plausible_so_should_show_accepts_it() {
        let chev = Rect { x: 1590, y: 1140, w: 40, h: 60 };
        assert!(rect_left_of(chev, bar(), 4, 190).is_plausible_widget());
    }
}


#[cfg(test)]
mod live {
    use super::*;

    /// Reports what the widening would do on THIS machine right now. Not an assertion - the
    /// answer depends on which apps happen to be open.
    /// Run: cargo test --release probe_live_widening -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_live_widening() {
        // UIA is COM; the test harness is not the app's main, so nothing has initialised it.
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );
        }
        let els = match taskbar_elements() {
            Ok(e) => e,
            Err(e) => {
                println!("UIA unavailable: {e}");
                return;
            }
        };
        let bar = taskbar_rect();
        println!("taskbar: {bar:?} elements: {}", els.len());
        let Some(widget) = widget_rect_in(&els) else {
            println!("no Widgets button on this machine");
            return;
        };
        let limit = left_limit(&els, widget);
        println!("widget: {widget:?}");
        if let Some(l) = limit {
            let who = els
                .iter()
                .filter(|(_, r)| r.x + r.w == l && r.y < widget.y + widget.h && r.y + r.h > widget.y)
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>();
            println!("left limit: {l}  (set by {who:?})  gap = {}px", widget.x - l);
        }
        if let Some(bar) = bar {
            for desired in [190, 260, 380, 456, 600] {
                let r = widened(widget, bar, limit, desired, 8);
                println!("  desired {desired:>4} -> x={} w={} (right edge {})", r.x, r.w, r.x + r.w);
            }
        }
    }
}
