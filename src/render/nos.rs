//! The NOS family: a 2 Fast 2 Furious street race at night, from the driver's seat - and the
//! "DANGER TO MANIFOLD" nitrous-warning screen as its flourish.
//!
//! # The view
//!
//! First person through the windshield, down a neon city street, using `drift`'s perspective camera
//! and car model. Every ground row below the horizon is a depth (`EYE_H * f / (row - horizon)`), the
//! street's gentle curves are integrated forward from the camera, and everything beside the road is
//! projected through the same camera.
//!
//! - **Sky (baked).** A night gradient with the city's glow at the horizon, a far skyline and stars.
//! - **The street.** Four lanes in alternating asphalt bands, dashed white lane lines and a double
//!   yellow centre line, kerbs and sidewalks, all streaming toward you and fogged to the horizon.
//! - **The city.** Building facades on both sides, block by block (heights hashed; every fifth block
//!   a cross street), each with a grid of lit windows, a glowing shopfront strip at street level and
//!   on many a neon blade sign standing out from the wall in the colourway's neon or `hot` - some
//!   flickering. Sodium street lamps pass overhead every `LAMP_GAP`, each throwing a pool on the road.
//! - **The rival.** A car (the `drift` model, in the neon) in the next lane ahead, its tail lamps
//!   glowing. You close on it when the music pushes and it pulls away when it eases off.
//! - **The cockpit.** A-pillars and the headliner frame the windshield; a rear-view mirror catches
//!   the headlights of a car behind (they flash on a kick); our headlights light the road ahead. The
//!   dashboard carries the **meter - a wide LED strip**, 48 segments (bands folded 64 -> 48), each lit
//!   when its band passes a threshold rising left to right, `lit` to 70 %, amber, red. Right-hand
//!   drive: behind the steering wheel, right of centre (it turns with the street's curves), the
//!   cluster - 10 shift lights and a rev bar; left of the wheel the speed (`MPH`, 2x `font3x5`) and
//!   the gear in a box, then the NOS bottle's pressure, which the hit drains and which then refills.
//!   The A-pillars lean in toward the roof, with the side windows dimmed beyond them, a door mirror
//!   at the foot of each, and the headliner deepening toward the sides.
//!
//! # The engine
//!
//! Nothing is read straight off rms each frame - that is what made the old dash twitch. rms (eased,
//! `THROTTLE_EASE_MS`) is the throttle. Revs climb at a rate that falls with each gear, and fall off
//! the throttle; at `UPSHIFT` the car shifts up and the revs drop by the gear ratio (the shift lights
//! fill toward it and flash blue just before), and a bass kick with the revs past `EARLY_SHIFT` shifts
//! early, on the beat. Revs under `DOWNSHIFT` shift down. Speed is the gear's top speed times the
//! revs, and it drives the street. A shift jolts the camera; at most one shift per `SHIFT_GAP_MS`.
//! And it races: from a launch the car runs up through the gears, cruises at the top for a couple
//! of seconds, then brakes hard with rev-matched downshifts to somewhere between 35 and 75 mph, and
//! goes again - so the speed keeps rising and falling on a steady loud track.
//!
//! # The flourish - the NOS hit
//!
//! 900 ms, fired only on a bass hit. For the first 300 ms the panel is the green-on-black warning
//! screen: a 1 px frame, `DANGER TO MANIFOLD` in 2x green blinking at 4 Hz (on two lines when one
//! would not leave `WARN_MARGIN` either side), `NOS` and a bar gauge draining to empty. Then the
//! purge: the speed doubles, the view widens, warp streaks fly out from the vanishing point, the
//! windshield washes blue, the revs pin at redline with the shift lights flashing blue, and the rival
//! drops behind as you blow past it.
//!
//! No car logos, badges or film wordmarks: the words are our own font, the shapes our own.
//!
//! The panel is opaque (painted first) and the clip to the rounded rect runs last on every path.
//! `draw` allocates nothing after the first frame at a size: the sky canvas and the layout are rebuilt
//! only on a size (or colour) change; the road and row tables, the warp pool and the readouts are
//! fixed arrays.

use std::f32::consts::TAU;

use crate::dsp::bands::NUM_BANDS;
use crate::dsp::flourish::{Envelope, Trigger};
use crate::dsp::onset::Flux;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::drift::{self, Cam, Paint, Pose};
use crate::render::font3x5;
use crate::render::{Family, FrameData};
use crate::themes::Theme;

// ---- the camera ----

/// The horizon's share of the interior from the top, and the dashboard's from the bottom.
const HZ_FRAC: f32 = 0.36;
const DASH_FRAC: f32 = 0.27;
/// Focal length in px on a full-height panel; the driver's eye height and lane, world units.
const FOCAL: f32 = 120.0;
const EYE_H: f32 = 4.2;
/// The driver sits on the right (a Japanese car), a little right of the lane's centre.
const EYE_X: f32 = 5.5;

// ---- the street ----

/// The road's half width, a lane's width and the sidewalk's, world units.
const HALF_W: f32 = 16.0;
const LANE: f32 = 8.0;
const WALK: f32 = 5.0;
const KERB_W: f32 = 0.6;
/// Asphalt band and lane-dash lengths.
const SEG: f32 = 8.0;
const DASH_LEN: f32 = 6.0;
/// The road table: depth `Z_FAR` in `DZ` steps.
const Z_FAR: f32 = 360.0;
const DZ: f32 = 3.0;
const ROAD_N: usize = 121;
/// The street repeats every `COURSE` units, a multiple of every period here; its curves reach
/// `C_MAX` per unit.
const COURSE: f32 = 8640.0;
const C_MAX: f32 = 0.0035;
/// Building blocks, the cross street (the first `CROSS` units of every fifth block), window pitch.
const BLOCK: f32 = 36.0;
const CROSS: f32 = 14.0;
const FLOOR: f32 = 3.2;
/// Windows are drawn out to this depth.
const WINDOW_Z: f32 = 230.0;
/// Street lamps: the gap, the pole's height and the arm's reach over the road.
const LAMP_GAP: f32 = 40.0;
const LAMP_H: f32 = 9.0;
const LAMP_ARM: f32 = 3.0;
/// Fog: full haze at this depth.
const FOG_Z: f32 = 420.0;
const ROWS: usize = 256;

// ---- the engine ----

/// rms (eased) is the throttle.
const THROTTLE_EASE_MS: f32 = 400.0;
/// Each gear's top speed (mph) and how fast the revs climb in it at full throttle (per second).
const TOP_MPH: [f32; 7] = [0.0, 38.0, 62.0, 88.0, 116.0, 148.0, 186.0];
const REV_RATE: [f32; 7] = [0.0, 1.5, 1.15, 0.9, 0.72, 0.58, 0.45];
/// Revs fall this fast off the throttle (per second), never below idle.
const REV_FALL: f32 = 0.35;
const IDLE: f32 = 0.12;
/// Shift points, and the least time between shifts.
const UPSHIFT: f32 = 0.93;
const EARLY_SHIFT: f32 = 0.72;
/// An early shift on a kick also waits this long after the last shift.
const EARLY_GAP_MS: f32 = 500.0;
const DOWNSHIFT: f32 = 0.3;
const SHIFT_GAP_MS: f32 = 300.0;
/// At the shift point the engine bounces on the limiter waiting for a kick to shift on, at most
/// this long.
const LIMITER_MS: f32 = 450.0;
/// A shift's camera jolt (eye height, units) and how fast it settles.
const JOLT: f32 = 0.35;
const JOLT_MS: f32 = 180.0;
/// World units per second per mph.
const UNITS_PER_MPH: f32 = 0.9;
/// Below this throttle there is no early shift on a kick.
const KICK_THROTTLE: f32 = 0.08;
/// A kick: an onset with the low bands over this.
const KICK_BASS: f32 = 0.45;
const ONSET_RATIO: f32 = 2.8;
const ONSET_REFRACTORY_MS: f32 = 200.0;
/// The rival: how far ahead at no throttle and how much closer at full; how fast the gap changes.
const RIVAL_FAR: f32 = 150.0;
const RIVAL_CLOSE: f32 = 72.0;
const RIVAL_EASE_MS: f32 = 1500.0;
/// The race cycle: from a launch the car runs up through the gears, cruises at the top for
/// `CRUISE_MS` (plus up to `CRUISE_VAR_MS`), then brakes hard - revs falling at `BRAKE_RATE`, rev-matched
/// downshifts under `BRAKE_DOWNSHIFT` - to between `BRAKE_TO` and `BRAKE_TO + BRAKE_VAR` mph, and goes
/// again. A run gives up waiting for top gear after `RUN_MAX_MS`.
const CRUISE_MS: f32 = 2200.0;
const CRUISE_VAR_MS: f32 = 1800.0;
const BRAKE_RATE: f32 = 0.9;
const BRAKE_DOWNSHIFT: f32 = 0.5;
const BRAKE_TO: f32 = 35.0;
const BRAKE_VAR: f32 = 40.0;
const BRAKE_MAX_MS: f32 = 3500.0;
const RUN_MAX_MS: f32 = 14000.0;
/// NOS bottle: refill time.
const BOTTLE_REFILL_MS: f32 = 6000.0;

// ---- the dash ----

/// LED strip segments (bands folded 64 -> 48), their thresholds left to right, colour bands.
const SEGS: usize = 48;
const SEG_THR_LO: f32 = 0.10;
const SEG_THR_HI: f32 = 0.35;
const AMBER_FROM: f32 = 0.7;
const RED_FROM: f32 = 0.9;
/// Shift lights; at `REDLINE` revs all flash blue at `REDLINE_HZ`.
const SHIFT_LEDS: usize = 10;
const REDLINE: f32 = 0.9;
const REDLINE_HZ: f32 = 8.0;
/// Below this width the speed readout is dropped.
const SPEED_MIN_W: i32 = 200;

// ---- the flourish ----

const NOS_MS: f32 = 900.0;
const WARN_MS: f32 = 300.0;
const WARN_BLINK_HZ: f32 = 4.0;
/// Minimum clear space either side of `DANGER TO MANIFOLD` on one line.
const WARN_MARGIN: i32 = 24;
const FLOURISH_BASS_MIN: f32 = 0.6;
/// Warp streaks in the purge.
const WARPS: usize = 28;

const AMBER: Rgba = Rgba { r: 0xff, g: 0xb0, b: 0x00, a: 255 };
const RED: Rgba = Rgba { r: 0xff, g: 0x2a, b: 0x2a, a: 255 };
const SHIFT_GREEN: Rgba = Rgba { r: 0x2c, g: 0xe0, b: 0x48, a: 255 };
const SHIFT_BLUE: Rgba = Rgba { r: 0x2a, g: 0x5c, b: 0xff, a: 255 };
const WARN_GREEN: Rgba = Rgba { r: 0x39, g: 0xff, b: 0x5a, a: 255 };
const NOS_BLUE: Rgba = Rgba { r: 0x3a, g: 0x8c, b: 0xff, a: 255 };
const SODIUM: Rgba = Rgba { r: 0xff, g: 0xb2, b: 0x4a, a: 255 };
const WARM: Rgba = Rgba { r: 0xff, g: 0xd2, b: 0x8a, a: 255 };
const YELLOW: Rgba = Rgba { r: 0xe8, g: 0xb8, b: 0x2a, a: 255 };
const ASPHALT: Rgba = Rgba { r: 0x40, g: 0x40, b: 0x48, a: 255 };
const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 255 };
const BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 255 };

/// The warning screen's layout.
#[derive(Clone, Copy, Default, PartialEq)]
struct Warn {
    two_lines: bool,
    /// Top-left of each text line (line 2 unused on one line).
    l1: (i32, i32),
    l2: (i32, i32),
    nos_scale: i32,
    nos: (i32, i32),
    /// The bar gauge: x, y, w, h (outline included).
    bar: (i32, i32, i32, i32),
}

/// Everything positional, for a panel size. Shared by `draw` and the tests.
#[derive(Clone, Copy, Default)]
struct Layout {
    /// World scale (1 on a full-height panel), focal length, horizon line and first ground row.
    s: f32,
    f: f32,
    hz: f32,
    hz_row: i32,
    /// The dashboard's top row (the windshield is above it).
    dash_top: i32,
    /// The LED strip: left x, pitch, segment width, top y, height.
    strip_x: f32,
    strip_pitch: f32,
    strip_w: i32,
    strip_y: i32,
    strip_h: i32,
    /// The steering wheel: centre, outer radius, rim thickness.
    wheel: (f32, f32, f32, f32),
    /// The cluster behind the wheel: the shift lights (left x, y, width, gap), the rev bar.
    led_x: i32,
    led_y: i32,
    led_w: i32,
    led_gap: i32,
    rev_bar: (i32, i32, i32, i32),
    show_speed: bool,
    /// The gear box (x, y, w, h), the speed digits' and `MPH`'s top-left.
    gear_box: (i32, i32, i32, i32),
    speed: (i32, i32),
    mph: (i32, i32),
    /// The NOS bottle gauge: `NOS`'s top-left and the bar (x, y, w, h).
    nos_label: (i32, i32),
    nos_bar: (i32, i32, i32, i32),
    /// The rear-view mirror: x, y, w, h.
    mirror: (i32, i32, i32, i32),
    warn: Warn,
}

/// The width of `text` in `font3x5` at `scale`.
fn text_w(text: &str, scale: i32) -> i32 {
    (text.chars().count() as i32 * 4 * scale - scale).max(0)
}

fn layout(w: i32, h: i32) -> Layout {
    let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
    let (iw, ih) = (ix1 - ix0, iy1 - iy0);
    let mut l = Layout { s: (ih as f32 / 52.0).clamp(0.6, 1.0), ..Layout::default() };
    l.f = FOCAL * l.s;
    l.hz_row = iy0 + (ih as f32 * HZ_FRAC).round() as i32;
    l.hz = l.hz_row as f32;
    l.dash_top = iy1 - (ih as f32 * DASH_FRAC).round() as i32;
    let cx = ix0 + iw / 2;

    // The LED strip runs along the dash top, clear of the A-pillars.
    let margin = (34.0 * l.s) as i32;
    let span_w = (iw - 2 * margin) as f32;
    l.strip_pitch = span_w / SEGS as f32;
    l.strip_x = (ix0 + margin) as f32;
    l.strip_w = (l.strip_pitch - 1.0).floor().max(1.0) as i32;
    l.strip_y = l.dash_top + 1;
    l.strip_h = if ih >= 44 { 2 } else { 1 };

    // The wheel - right-hand drive, so right of centre - tops out a little under the strip.
    let r = 30.0 * l.s;
    let top = (l.strip_y + l.strip_h + 2) as f32;
    let wx = ix0 as f32 + iw as f32 * 0.70;
    l.wheel = (wx, top + r, r, (4.0 * l.s).max(2.0));

    // The cluster, in the wheel's opening: shift lights, then the rev bar.
    l.led_w = if iw >= 200 { 3 } else { 2 };
    l.led_gap = 1;
    let leds_w = SHIFT_LEDS as i32 * (l.led_w + l.led_gap) - l.led_gap;
    l.led_x = wx as i32 - leds_w / 2;
    l.led_y = top as i32 + (l.wheel.3 as i32) + 1;
    l.rev_bar = (l.led_x, l.led_y + 2, leds_w, 2);

    // Left of the wheel, toward the centre console: speed and gear, then the NOS bottle.
    let edge = (wx - r) as i32 - 5;
    let gear_w = 3 * 2 + 4;
    let dy = l.dash_top + (iy1 - l.dash_top - 10).max(0) / 2 + 1;
    l.show_speed = w >= SPEED_MIN_W && iy1 - l.dash_top >= 12;
    let readouts_left = if l.show_speed {
        let (sw, mw) = (text_w("000", 2), text_w("MPH", 1));
        let x0 = edge - (sw + 3 + mw + 5 + gear_w);
        l.speed = (x0, dy);
        l.mph = (x0 + sw + 3, dy + 5);
        l.gear_box = (l.mph.0 + mw + 5, l.dash_top + 1, gear_w, (iy1 - l.dash_top - 1).min(14));
        x0
    } else {
        l.gear_box = (edge - gear_w, l.dash_top + 2, gear_w, (iy1 - l.dash_top - 2).min(14));
        edge - gear_w
    };
    // Clear of the left A-pillar's foot; on a narrow panel the label goes and the bar shortens.
    let left = readouts_left - 6;
    let room = left - (ix0 + (18.0 * l.s) as i32);
    let label_w = text_w("NOS", 1) + 3;
    let bar_w = ((44.0 * l.s) as i32).min(room - label_w);
    if bar_w >= 14 {
        l.nos_label = (left - bar_w - label_w, dy + 2);
        l.nos_bar = (left - bar_w, dy + 2, bar_w, 5);
    } else {
        l.nos_label = (-100, -100);
        let bw = room.max(6);
        l.nos_bar = (left - bw, dy + 2, bw, 5);
    }

    // The mirror, top centre.
    let mw = (46.0 * l.s) as i32;
    let mh = (7.0 * l.s).round().max(4.0) as i32;
    l.mirror = (cx - mw / 2, iy0 + 1, mw, mh);

    // ---- the warning screen ----
    let one = "DANGER TO MANIFOLD";
    let (a, b) = ("DANGER TO", "MANIFOLD");
    let mut wv = Warn { two_lines: text_w(one, 2) + 2 * WARN_MARGIN > iw, ..Warn::default() };
    let text_h = if wv.two_lines { 10 + 2 + 10 } else { 10 };
    wv.nos_scale = if text_h + 3 + 10 <= ih - 6 { 2 } else { 1 };
    let nos_h = 5 * wv.nos_scale;
    let gap = if wv.two_lines { 3 } else { 5 };
    let block = text_h + gap + nos_h;
    let ty = iy0 + (ih - block) / 2;
    if wv.two_lines {
        wv.l1 = (ix0 + (iw - text_w(a, 2)) / 2, ty);
        wv.l2 = (ix0 + (iw - text_w(b, 2)) / 2, ty + 12);
    } else {
        wv.l1 = (ix0 + (iw - text_w(one, 2)) / 2, ty);
    }
    let ny = ty + text_h + gap;
    let nos_w = text_w("NOS", wv.nos_scale);
    let wbar_w = (iw / 3).clamp(16, 72);
    let row_w = nos_w + 4 + wbar_w;
    let nx = ix0 + (iw - row_w) / 2;
    wv.nos = (nx, ny);
    wv.bar = (nx + nos_w + 4, ny, wbar_w, nos_h);
    l.warn = wv;
    l
}

/// The street's curvature at distance `s` (wrapped): long gentle bends. Positive bends right.
fn curve(s: f32) -> f32 {
    let p = TAU * s.rem_euclid(COURSE) / COURSE;
    C_MAX * (1.2 * (6.0 * p).sin() + 0.5 * (13.0 * p + 1.0).sin()).clamp(-1.0, 1.0)
}

/// A block's slot on the course and whether it starts with a cross street.
fn block_of(s: f32) -> (i64, bool, f32) {
    let u = s.rem_euclid(COURSE) / BLOCK;
    let k = u.floor() as i64;
    let into = (u - k as f32) * BLOCK;
    (k, k.rem_euclid(5) == 0 && into < CROSS, into)
}

/// A building's height on `side` (-1 left, +1 right) at distance `s`; 0 across a cross street.
fn building_h(s: f32, side: f32) -> f32 {
    let (k, gap, _) = block_of(s);
    if gap {
        return 0.0;
    }
    let salt = if side < 0.0 { 3 } else { 11 };
    10.0 + 30.0 * drift::hash01(k * 17 + salt)
}

/// A finite level, clamped 0..1.
fn lvl(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn with_alpha(c: Rgba, a: f32) -> Rgba {
    Rgba { a: (a.clamp(0.0, 1.0) * 255.0).round() as u8, ..c }
}

/// The colourway's neon: its one zone's `lit`, falling back to `lit`.
fn neon_of(t: &Theme) -> Rgba {
    let hex = t.zones.first().map(|z| z.lit.as_str()).unwrap_or(t.lit.as_str());
    Rgba::from_hex(hex, 1.0)
}

/// FNV-1a over the strings/values the baked sky depends on. No allocation.
fn colour_key(t: &Theme, w: i32, h: i32) -> u64 {
    let mut k: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        k ^= b as u64;
        k = k.wrapping_mul(0x0100_0000_01b3);
    };
    let neon = t.zones.first().map(|z| z.lit.as_str()).unwrap_or("");
    for s in [t.panel.as_str(), t.lit.as_str(), t.hot.as_str(), t.edge.as_str(), neon] {
        for b in s.bytes() {
            eat(b);
        }
        eat(0);
    }
    for b in w.to_le_bytes().into_iter().chain(h.to_le_bytes()) {
        eat(b);
    }
    k | 1
}

/// Draws `text` in `font3x5` at `scale` with its top-left at (x, y).
fn text(c: &mut Canvas, x: i32, y: i32, s: &str, scale: i32, col: Rgba) {
    let mut cx = x;
    for ch in s.chars() {
        if let Some(rows) = font3x5::glyph(ch) {
            for (dy, row) in rows.iter().enumerate() {
                for dx in 0..3 {
                    if row & (0b100 >> dx) != 0 {
                        c.fill_rect(cx + dx * scale, y + dy as i32 * scale, scale, scale, col);
                    }
                }
            }
        }
        cx += 4 * scale;
    }
}

/// A 1 px rectangle outline.
fn frame(c: &mut Canvas, (x, y, w, h): (i32, i32, i32, i32), col: Rgba) {
    c.fill_rect(x, y, w, 1, col);
    c.fill_rect(x, y + h - 1, w, 1, col);
    c.fill_rect(x, y, 1, h, col);
    c.fill_rect(x + w - 1, y, 1, h, col);
}

/// The LED strip's colour at segment `i`: `lit` to 70 %, amber to 90 %, red above.
fn seg_colour(i: usize, lit: Rgba) -> Rgba {
    let f = i as f32 / (SEGS - 1) as f32;
    if f < AMBER_FROM {
        lit
    } else if f < RED_FROM {
        AMBER
    } else {
        RED
    }
}

/// Shift light `i`'s colour: green x3, amber x3, red x3, blue.
fn shift_colour(i: usize) -> Rgba {
    match i {
        0..=2 => SHIFT_GREEN,
        3..=5 => AMBER,
        6..=8 => RED,
        _ => SHIFT_BLUE,
    }
}

/// Smoothstep.
fn ease(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * p * (3.0 - 2.0 * p)
}

/// A facade's edge at one road sample: its foot and top on screen, and its height.
type Edge = ((f32, f32), (f32, f32), f32);

/// One warp streak: an angle out of the vanishing point and how far along it is (0..1).
#[derive(Clone, Copy, Default)]
struct Warp {
    angle: f32,
    t: f32,
    speed: f32,
}

/// Where the car is in the race cycle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Run,
    Cruise,
    Brake,
}

/// Which layers draw - a test hook.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Only {
    All,
    /// The dash's readouts alone (no street, no cockpit), so they can be measured.
    #[cfg(test)]
    Dash,
}

pub struct Nos {
    /// Fires the NOS hit on a rare bass hit. `pub(crate)` so the tests can force it.
    pub(crate) flourish: Trigger,
    hit: Envelope,
    onset: Flux,
    elapsed_s: f32,
    /// The engine: throttle (eased rms), revs (0..1), gear (1..6), speed (mph), time since a shift.
    throttle: f32,
    rpm: f32,
    gear: u8,
    mph: f32,
    since_shift_ms: f32,
    /// How long the revs have sat at the limiter.
    limiter_ms: f32,
    /// The race cycle: the phase, how long it has run, how long this cruise lasts, how slow this
    /// braking goes.
    phase: Phase,
    phase_ms: f32,
    cruise_for: f32,
    brake_to: f32,
    /// The camera jolt of the last shift (0..1), and the mirror's flash on a kick.
    jolt: f32,
    flash: f32,
    /// Distance along the street (wrapped), the steering wheel's turn.
    s_cam: f32,
    steer: f32,
    /// The rival's distance ahead and the NOS bottle's pressure (0..1).
    rival_z: f32,
    bottle: f32,
    road: [f32; ROAD_N],
    row_z: [f32; ROWS],
    row_cx: [f32; ROWS],
    warps: [Warp; WARPS],
    layout: Layout,
    layout_dim: (i32, i32),
    /// The baked sky. Keyed on size and colours.
    bg: Canvas,
    bg_key: u64,
    rng: u64,
    only: Only,
    /// Shifts so far, and how many landed on a kick - for the tests.
    #[cfg(test)]
    shifts: u32,
    #[cfg(test)]
    kick_shifts: u32,
}

impl Default for Nos {
    fn default() -> Self {
        Nos {
            flourish: Default::default(),
            hit: Default::default(),
            onset: Default::default(),
            elapsed_s: 0.0,
            throttle: 0.0,
            rpm: IDLE,
            gear: 1,
            mph: 0.0,
            since_shift_ms: 1.0e6,
            limiter_ms: 0.0,
            phase: Phase::Run,
            phase_ms: 0.0,
            cruise_for: CRUISE_MS,
            brake_to: BRAKE_TO,
            jolt: 0.0,
            flash: 0.0,
            s_cam: 0.0,
            steer: 0.0,
            rival_z: RIVAL_FAR,
            bottle: 1.0,
            road: [0.0; ROAD_N],
            row_z: [0.0; ROWS],
            row_cx: [0.0; ROWS],
            warps: [Warp::default(); WARPS],
            layout: Layout::default(),
            layout_dim: (0, 0),
            bg: Canvas::new(1, 1),
            bg_key: 0,
            rng: 0x6a09_e667_f3bc_c908,
            only: Only::All,
            #[cfg(test)]
            shifts: 0,
            #[cfg(test)]
            kick_shifts: 0,
        }
    }
}

/// The scene's colours for a colourway.
struct Palette {
    panel: Rgba,
    lit: Rgba,
    hot: Rgba,
    neon: Rgba,
    haze: Rgba,
    road_a: Rgba,
    road_b: Rgba,
    lane: Rgba,
    walk: Rgba,
    kerb: Rgba,
    bld_a: Rgba,
    bld_b: Rgba,
    roofline: Rgba,
    trim: Rgba,
    trim_hi: Rgba,
    dash: Rgba,
    ghost: f32,
}

impl Palette {
    fn new(t: &Theme) -> Self {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let neon = neon_of(t);
        let road_a = Rgba::lerp_linear(edge, ASPHALT, 0.45);
        Palette {
            panel,
            lit,
            hot,
            neon,
            haze: Rgba::lerp_linear(Rgba::lerp_linear(panel, edge, 0.6), neon, 0.04),
            road_a,
            road_b: Rgba::lerp_linear(road_a, BLACK, 0.18),
            lane: Rgba::lerp_linear(edge, WHITE, 0.6),
            walk: Rgba::lerp_linear(edge, ASPHALT, 0.7),
            kerb: Rgba::lerp_linear(edge, WHITE, 0.35),
            bld_a: Rgba::lerp_linear(panel, edge, 0.55),
            bld_b: Rgba::lerp_linear(panel, edge, 0.25),
            roofline: Rgba::lerp_linear(edge, lit, 0.25),
            trim: Rgba::lerp_linear(panel, BLACK, 0.55),
            trim_hi: Rgba::lerp_linear(edge, lit, 0.2),
            dash: Rgba::lerp_linear(panel, edge, 0.18),
            ghost: if t.ghost.is_finite() { t.ghost.clamp(0.04, 0.4) } else { 0.1 },
        }
    }
}

impl Nos {
    #[cfg(test)]
    fn only_for_test(&mut self, only: Only) {
        self.only = only;
    }

    fn next_rng(&mut self) -> u64 {
        let mut s = self.rng;
        let mut x = || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        let v = x();
        self.rng = s;
        v
    }

    /// The road's centre at depth `z` (linear in the table).
    fn road_at(&self, z: f32) -> f32 {
        let f = (z / DZ).clamp(0.0, (ROAD_N - 1) as f32);
        let i = (f as usize).min(ROAD_N - 2);
        let t = f - i as f32;
        self.road[i] + (self.road[i + 1] - self.road[i]) * t
    }

    /// The camera this frame: the eye in our lane, jolted by a shift, wider in the purge.
    fn cam(&self, w: i32, widen: f32) -> Cam {
        Cam {
            cx: w as f32 * 0.5,
            hz: self.layout.hz,
            f: self.layout.f * (1.0 - 0.28 * widen),
            x: EYE_X,
            y: EYE_H - JOLT * self.jolt,
        }
    }

    fn resize(&mut self, w: i32, h: i32, t: &Theme) {
        if self.layout_dim != (w, h) {
            self.layout = layout(w, h);
            self.layout_dim = (w, h);
        }
        let key = colour_key(t, w, h);
        if key == self.bg_key {
            return;
        }
        self.bg_key = key;
        if self.bg.width() != w || self.bg.height() != h {
            self.bg = Canvas::new(w, h);
        }
        let l = self.layout;
        let p = Palette::new(t);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let bg = &mut self.bg;
        bg.clear();
        let (ix0, iy0, ix1) = (3, 4, w - 3);
        let iw = ix1 - ix0;
        let sky_h = (l.hz_row - iy0).max(1) as f32;
        for y in iy0..l.hz_row {
            let f = (y - iy0) as f32 / sky_h;
            let col = Rgba::lerp_linear(p.panel, edge, 0.8 * f.powf(1.5));
            bg.fill_rect(ix0, y, iw, 1, Rgba::lerp_linear(col, p.neon, 0.2 * f.powi(3)));
        }
        // A far skyline at the end of the street, and a few stars.
        let mut s: u64 = 0x1f2e_3d4c_5b6a_7988;
        let mut rnd = || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
        };
        let far = Rgba::lerp_linear(p.panel, edge, 0.5);
        let mut x = ix0;
        while x < ix1 {
            let bw = 3 + (rnd() * 8.0) as i32;
            let bh = (sky_h * (0.15 + 0.5 * rnd())) as i32;
            bg.fill_rect(x, l.hz_row - bh, bw, bh, far);
            for wy in (l.hz_row - bh + 2..l.hz_row - 1).step_by(3) {
                for wx in (x + 1..x + bw - 1).step_by(2) {
                    if rnd() < 0.18 {
                        bg.fill_rect(wx, wy, 1, 1, Rgba::lerp_linear(far, WARM, 0.5));
                    }
                }
            }
            x += bw;
        }
        for _ in 0..(iw / 14) {
            let sx = ix0 + (rnd() * iw as f32) as i32;
            let sy = iy0 + (rnd() * sky_h * 0.5) as i32;
            bg.fill_rect(sx, sy, 1, 1, with_alpha(WHITE, 0.3 + 0.4 * rnd()));
        }
    }

    /// The engine and the race for one frame.
    fn advance(&mut self, dt: f32, rms_norm: f32, kick: bool, purge: f32) {
        let dts = dt / 1000.0;
        self.throttle += (rms_norm - self.throttle) * (dt / THROTTLE_EASE_MS).min(1.0);
        if !self.throttle.is_finite() {
            self.throttle = 0.0;
        }
        let music = self.throttle.clamp(0.0, 1.0);
        let g = self.gear.clamp(1, 6) as usize;
        self.since_shift_ms = (self.since_shift_ms + dt).min(1.0e6);

        // The race cycle: run up the gears, cruise, brake hard, go again. With no music there is no
        // race - the car just coasts.
        self.phase_ms = (self.phase_ms + dt).min(1.0e6);
        if music < KICK_THROTTLE || purge > 0.0 {
            self.phase = Phase::Run;
            self.phase_ms = 0.0;
        } else {
            match self.phase {
                Phase::Run if (g == 6 && self.rpm >= 0.8) || self.phase_ms >= RUN_MAX_MS => {
                    let r = self.next_rng();
                    self.phase = Phase::Cruise;
                    self.phase_ms = 0.0;
                    self.cruise_for = CRUISE_MS + CRUISE_VAR_MS * (r >> 40) as f32 / (1u64 << 24) as f32;
                }
                Phase::Cruise if self.phase_ms >= self.cruise_for => {
                    let r = self.next_rng();
                    self.phase = Phase::Brake;
                    self.phase_ms = 0.0;
                    self.brake_to = BRAKE_TO + BRAKE_VAR * (r >> 40) as f32 / (1u64 << 24) as f32;
                }
                Phase::Brake if self.mph <= self.brake_to || self.phase_ms >= BRAKE_MAX_MS => {
                    self.phase = Phase::Run;
                    self.phase_ms = 0.0;
                }
                _ => {}
            }
        }
        let braking = self.phase == Phase::Brake;
        let thr = match self.phase {
            Phase::Run => music,
            Phase::Cruise => music * 0.75,
            Phase::Brake => 0.0,
        };
        if purge > 0.0 {
            self.rpm += (1.0 - self.rpm) * (dt / 120.0).min(1.0);
        } else if braking {
            self.rpm = (self.rpm - BRAKE_RATE * dts).max(IDLE);
        } else {
            let climb = REV_RATE[g] * thr - REV_FALL * (1.0 - thr);
            self.rpm = (self.rpm + climb * dts).clamp(IDLE, 1.0);
        }
        // Shifts: up at the shift point or early on a kick, down when the revs sag.
        let settled = self.since_shift_ms >= SHIFT_GAP_MS;
        // At the shift point: bounce on the limiter, and shift on the next kick (or give up waiting).
        if self.rpm >= UPSHIFT && g < 6 {
            self.limiter_ms += dt;
            self.rpm = UPSHIFT + 0.012 * (1.0 - (self.limiter_ms / 1000.0 * TAU * 9.0).cos());
        } else {
            self.limiter_ms = 0.0;
        }
        let early = kick && thr > KICK_THROTTLE && self.rpm >= EARLY_SHIFT && self.since_shift_ms >= EARLY_GAP_MS;
        let at_limit = self.rpm >= UPSHIFT && (kick || self.limiter_ms >= LIMITER_MS);
        let up = !braking && g < 6 && (at_limit || early);
        let down_at = if braking { BRAKE_DOWNSHIFT } else { DOWNSHIFT };
        if settled && purge == 0.0 && up {
            self.rpm *= TOP_MPH[g] / TOP_MPH[g + 1];
            self.gear += 1;
            self.since_shift_ms = 0.0;
            self.limiter_ms = 0.0;
            self.jolt = 1.0;
            #[cfg(test)]
            {
                self.shifts += 1;
                if kick {
                    self.kick_shifts += 1;
                }
            }
        } else if settled && g > 1 && self.rpm < down_at {
            // Rev-matched: the revs jump by the ratio, and the camera kicks a little.
            self.rpm = (self.rpm * TOP_MPH[g] / TOP_MPH[g - 1]).min(0.88);
            self.gear -= 1;
            self.since_shift_ms = 0.0;
            self.jolt = 0.6;
            #[cfg(test)]
            {
                self.shifts += 1;
            }
        }
        if kick {
            self.flash = 1.0;
        }
        self.jolt *= (-dt / JOLT_MS).exp();
        self.flash *= (-dt / 160.0).exp();
        let target_mph = TOP_MPH[self.gear.clamp(1, 6) as usize] * self.rpm;
        self.mph += (target_mph - self.mph) * (dt / 150.0).min(1.0);
        let v = self.mph * UNITS_PER_MPH * (1.0 + 1.2 * purge);
        let c_cam = curve(self.s_cam);
        self.s_cam = (self.s_cam + v * dts).rem_euclid(COURSE);
        self.steer += (c_cam / C_MAX * 0.5 - self.steer) * (dt / 300.0).min(1.0);

        // The road ahead: curvature integrated twice from the camera.
        let (mut x, mut dx) = (0.0f32, 0.0f32);
        for i in 0..ROAD_N {
            self.road[i] = x;
            dx += curve(self.s_cam + i as f32 * DZ) * DZ;
            x += dx * DZ;
        }

        // The rival: closer when the throttle is open; in the purge we blow past; once it is behind
        // us it comes back in from far ahead.
        if purge > 0.0 {
            self.rival_z -= 400.0 * dts;
        } else {
            let target = RIVAL_FAR - (RIVAL_FAR - RIVAL_CLOSE) * thr;
            self.rival_z += (target - self.rival_z) * (dt / RIVAL_EASE_MS).min(1.0);
        }
        if purge == 0.0 && self.rival_z < 2.0 {
            self.rival_z = 260.0;
        }
        self.bottle = if purge > 0.0 {
            (self.bottle - dt / 400.0).max(0.0)
        } else {
            (self.bottle + dt / BOTTLE_REFILL_MS).min(1.0)
        };
        for v in [&mut self.rpm, &mut self.mph, &mut self.rival_z, &mut self.steer, &mut self.s_cam] {
            if !v.is_finite() {
                *v = 0.0;
            }
        }
        self.rpm = self.rpm.max(IDLE);
    }
}

impl Family for Nos {
    fn id(&self) -> &'static str {
        "nos"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        let panel = Rgba::from_hex(&t.panel, 1.0);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        let (ix0, iy0, ix1, iy1) = (3i32, 4i32, w - 3, h - 4);
        let (iw, ih) = (ix1 - ix0, iy1 - iy0);
        if iw < 40 || ih < 20 {
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return;
        }
        self.resize(w, h, t);

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        self.elapsed_s = (self.elapsed_s + dt / 1000.0).rem_euclid(3600.0);
        if !self.elapsed_s.is_finite() {
            self.elapsed_s = 0.0;
        }
        let e = self.elapsed_s;
        let rms = lvl((d.rms_l + d.rms_r) * 0.5);
        let rms_norm = (rms * 4.0).clamp(0.0, 1.0);
        let bass = d.levels[0..8].iter().map(|&v| lvl(v)).sum::<f32>() / 8.0;
        let treble = d.levels[40..64].iter().map(|&v| lvl(v)).sum::<f32>() / 24.0;

        // ---- the NOS hit ----
        let triggered = self.flourish.update(&d.levels, dt, t.flourish);
        let bass_gate = triggered && bass >= FLOURISH_BASS_MIN;
        #[cfg(test)]
        let fired = bass_gate || (triggered && self.flourish.was_forced());
        #[cfg(not(test))]
        let fired = bass_gate;
        let env = self.hit.update(fired, dt, NOS_MS);
        let since_ms = (1.0 - env) * NOS_MS;
        let warning = env > 0.0 && since_ms < WARN_MS;
        let purge = if env > 0.0 && !warning { (env / (1.0 - WARN_MS / NOS_MS)).clamp(0.0, 1.0) } else { 0.0 };
        if fired {
            for i in 0..WARPS {
                let r = self.next_rng();
                self.warps[i] = Warp {
                    angle: (r >> 40) as f32 / (1u64 << 24) as f32 * TAU,
                    t: ((r >> 8) & 0xff) as f32 / 255.0,
                    speed: 1.2 + ((r >> 16) & 0xff) as f32 / 255.0 * 1.4,
                };
            }
        }

        let kick = self.onset.update(&d.levels, dt, ONSET_RATIO, ONSET_REFRACTORY_MS) && bass >= KICK_BASS;
        self.advance(dt, rms_norm, kick, purge);
        let l = self.layout;
        let pal = Palette::new(t);

        if warning && self.only == Only::All {
            draw_warning(c, &l, since_ms, (ix0, iy0, iw, ih));
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return;
        }

        let cam = self.cam(w, ease(purge));
        let f = cam.f;
        let shake = if purge > 0.0 { (((e * 60.0) as i32 % 2) * 2 - 1) as f32 * purge } else { 0.0 };
        let view_bottom = l.dash_top;

        if self.only == Only::All {
            // ---- the sky ----
            c.copy_region(&self.bg, (ix0, iy0), (ix0, iy0), iw, l.hz_row - iy0);

            // ---- the street, row by row ----
            for y in l.hz_row.max(0)..view_bottom.min(ROWS as i32) {
                let yi = y as usize;
                let z = cam.y * f / (y as f32 + 0.5 - cam.hz);
                if !(z.is_finite() && z > 0.0) {
                    continue;
                }
                self.row_z[yi] = z;
                let cx = cam.cx + (self.road_at(z) - cam.x) * f / z + shake;
                self.row_cx[yi] = cx;
                let px = f / z;
                let fog = (z / FOG_Z).clamp(0.0, 1.0).powf(1.2);
                let fogged = |col: Rgba| Rgba::lerp_linear(col, pal.haze, fog);
                let dist = self.s_cam + z;
                let band = ((dist / SEG).floor() as i64) & 1 == 1;
                let (l0, r0) = (cx - HALF_W * px, cx + HALF_W * px);
                let walk = fogged(pal.walk);
                drift::span(c, ix0 as f32, l0 - KERB_W * px, y, walk);
                drift::span(c, l0 - KERB_W * px, l0, y, fogged(pal.kerb));
                drift::span(c, l0, r0, y, fogged(if band { pal.road_a } else { pal.road_b }));
                drift::span(c, r0, r0 + KERB_W * px, y, fogged(pal.kerb));
                drift::span(c, r0 + KERB_W * px, ix1 as f32, y, walk);
                // The double yellow, and dashed lane lines.
                let lw = (0.35 * px).max(0.5);
                for side in [-0.45f32, 0.45] {
                    drift::span(c, cx + side * px - lw * 0.5, cx + side * px + lw * 0.5, y, fogged(YELLOW));
                }
                if ((dist / DASH_LEN).floor() as i64) & 1 == 0 {
                    for lx in [-LANE, LANE] {
                        drift::span(c, cx + lx * px - lw * 0.5, cx + lx * px + lw * 0.5, y, fogged(pal.lane));
                    }
                }
            }

            // ---- the lamps' pools on the road ----
            let near_slot = (self.s_cam / LAMP_GAP).floor() as i64;
            for k in (near_slot..near_slot + 7).rev() {
                let z = k as f32 * LAMP_GAP - self.s_cam;
                if z < 6.0 {
                    continue;
                }
                for side in [-1.0f32, 1.0] {
                    let x = self.road_at(z) + side * (HALF_W - LAMP_ARM + 0.6);
                    let mut pts = [(0.0f32, 0.0f32); 8];
                    for (i, q) in pts.iter_mut().enumerate() {
                        let a = i as f32 * TAU / 8.0;
                        let p = cam.project(x + 6.0 * a.cos(), 0.0, z + 6.0 * a.sin());
                        *q = (p.0 + shake, p.1);
                    }
                    drift::fill_convex(c, &pts, (0.0, 0.0), l.hz_row, with_alpha(SODIUM, 0.09));
                }
            }

            // ---- our headlights on the road ahead ----
            for (z1, spread, a) in [(80.0f32, 9.0f32, 0.035f32), (45.0, 6.0, 0.045)] {
                let pts = [
                    cam.project(EYE_X - 2.5, 0.0, 9.0),
                    cam.project(EYE_X + 2.5, 0.0, 9.0),
                    cam.project(self.road_at(z1) + EYE_X + spread, 0.0, z1),
                    cam.project(self.road_at(z1) + EYE_X - spread, 0.0, z1),
                ];
                drift::fill_convex(c, &pts, (shake, 0.0), l.hz_row, with_alpha(WHITE, a));
            }

            // ---- the buildings, far to near, both sides ----
            let wall_d = HALF_W + WALK;
            let mut prev: [Option<Edge>; 2] = [None, None];
            for i in (1..ROAD_N).rev() {
                let z = i as f32 * DZ;
                if z < 3.0 {
                    break;
                }
                let s_here = self.s_cam + z;
                let (blk, _, into) = block_of(s_here);
                let fog = (z / FOG_Z).clamp(0.0, 1.0).powf(1.2);
                let px = f / z;
                for (si, side) in [-1.0f32, 1.0].into_iter().enumerate() {
                    let x = self.road[i] + side * wall_d;
                    let bh = building_h(s_here, side);
                    let b = cam.project(x, 0.0, z);
                    let tp = cam.project(x, bh, z);
                    let (b, tp) = ((b.0 + shake, b.1), (tp.0 + shake, tp.1));
                    if let Some((pb, pt, ph)) = prev[si] {
                        if bh > 0.0 && ph > 0.0 {
                            let shade = drift::hash01(blk * 5 + si as i64);
                            let col = Rgba::lerp_linear(Rgba::lerp_linear(pal.bld_b, pal.bld_a, shade), pal.haze, fog);
                            drift::fill_convex(c, &[pb, pt, tp, b], (0.0, 0.0), iy0, col);
                            if (pt.0 - tp.0).abs() < 80.0 {
                                let rim = Rgba::lerp_linear(pal.roofline, pal.haze, fog);
                                c.line(pt.0.round() as i32, pt.1.round() as i32, tp.0.round() as i32, tp.1.round() as i32, rim);
                            }
                            // The shopfront strip at street level.
                            if z > 24.0 && drift::hash01(blk * 7 + si as i64 + 40) < 0.6 {
                                let glow = if drift::hash01(blk * 3 + si as i64) < 0.5 { pal.neon } else { WARM };
                                let q = |p0: (f32, f32), p1: (f32, f32), y: f32| (p0.0 + (p1.0 - p0.0) * y, p0.1 + (p1.1 - p0.1) * y);
                                let (h0, h1) = (2.2 / bh, 2.8 / bh);
                                let strip = [q(pb, pt, h0), q(pb, pt, h1), q(b, tp, h1), q(b, tp, h0)];
                                drift::fill_convex(c, &strip, (0.0, 0.0), iy0, with_alpha(Rgba::lerp_linear(glow, pal.haze, fog), 0.6));
                            }
                            // Windows: a column per sample, a row per floor.
                            if z > 10.0 && z < WINDOW_Z {
                                let col_k = (s_here / DZ).floor() as i64;
                                let (ww, wh) = ((0.9 * px).max(1.0), (1.3 * px).max(1.0));
                                let mut fy = 4.5;
                                let mut floor = 0i64;
                                while fy < bh - 1.5 {
                                    let hsh = drift::hash01(col_k * 131 + floor * 7 + si as i64 * 3);
                                    if hsh < 0.42 {
                                        let wc = if hsh < 0.18 { WARM } else if hsh < 0.32 { Rgba::lerp_linear(pal.lit, WHITE, 0.4) } else { Rgba::lerp_linear(pal.neon, WHITE, 0.2) };
                                        let p = cam.project(x, fy, z);
                                        c.fill_rect((p.0 + shake).round() as i32, p.1.round() as i32, ww.round() as i32, wh.round() as i32, Rgba::lerp_linear(wc, pal.haze, fog * 0.8));
                                    }
                                    fy += FLOOR;
                                    floor += 1;
                                }
                            }
                        }
                        // A neon blade sign standing out from the wall, on many blocks.
                        let sign_at = 10.0 + 8.0 * drift::hash01(blk * 23 + si as i64);
                        let prev_into = into - DZ;
                        if bh > 8.0 && drift::hash01(blk * 19 + si as i64 + 9) < 0.55 && prev_into < sign_at && into >= sign_at {
                            let y0 = 5.0 + 3.0 * drift::hash01(blk * 29 + si as i64);
                            let y1 = (y0 + 5.0 + 4.0 * drift::hash01(blk * 31 + si as i64)).min(bh - 1.0);
                            let a = cam.project(x, y0, z);
                            let bq = cam.project(x - side * 2.6, y1, z);
                            let base = if drift::hash01(blk * 37 + si as i64) < 0.3 { pal.hot } else { pal.neon };
                            let flick = drift::hash01(blk * 41 + si as i64) < 0.3;
                            let on = !flick || ((e * (1.0 + drift::hash01(blk)) + drift::hash01(blk * 43)).fract() > 0.15);
                            if on {
                                let (x0, x1) = ((a.0.min(bq.0) + shake).round() as i32, (a.0.max(bq.0) + shake).round() as i32);
                                let (y0p, y1p) = (bq.1.round() as i32, a.1.round() as i32);
                                let sw = (x1 - x0).max(1);
                                let sh = (y1p - y0p).max(1);
                                let glint = (0.2 + 0.5 * treble).min(0.7);
                                c.fill_rect(x0 - 1, y0p - 1, sw + 2, sh + 2, with_alpha(Rgba::lerp_linear(base, pal.haze, fog), 0.3));
                                c.fill_rect(x0, y0p, sw, sh, Rgba::lerp_linear(Rgba::lerp_linear(base, WHITE, glint * 0.3), pal.haze, fog));
                            }
                        }
                    }
                    prev[si] = Some((b, tp, bh));
                }
            }

            // ---- the street lamps, far to near ----
            for k in (near_slot..near_slot + 9).rev() {
                let z = k as f32 * LAMP_GAP - self.s_cam;
                if z < 3.0 {
                    continue;
                }
                let fog = (z / FOG_Z).clamp(0.0, 1.0).powf(1.2);
                let pw = (0.3 * f / z).max(1.0);
                for side in [-1.0f32, 1.0] {
                    let x = self.road_at(z) + side * (HALF_W + 0.6);
                    let (b, tp) = (cam.project(x, 0.0, z), cam.project(x, LAMP_H, z));
                    let arm = cam.project(x - side * LAMP_ARM, LAMP_H, z);
                    let pole = Rgba::lerp_linear(pal.trim_hi, pal.haze, fog);
                    c.fill_rect((b.0 - pw * 0.5 + shake).round() as i32, tp.1.round() as i32, pw.round() as i32, (b.1 - tp.1).round().max(1.0) as i32, pole);
                    c.line((tp.0 + shake).round() as i32, tp.1.round() as i32, (arm.0 + shake).round() as i32, arm.1.round() as i32, pole);
                    // The head: a small hot point with a faint sodium halo.
                    let r = (0.45 * f / z).max(1.0);
                    let (hx, hy) = ((arm.0 + shake).round() as i32, arm.1.round() as i32);
                    c.fill_circle(hx, hy + 1, (r * 2.0).round() as i32, with_alpha(SODIUM, 0.1 * (1.0 - fog)));
                    let ri = r.round() as i32;
                    c.fill_rect(hx - ri, hy, (2 * ri).max(1), ri.max(1), Rgba::lerp_linear(Rgba::lerp_linear(SODIUM, WHITE, 0.6), pal.haze, fog));
                }
            }

            // ---- the rival ----
            if self.rival_z > if purge > 0.0 { 30.0 } else { 18.0 } {
                let z = self.rival_z;
                let rx = self.road_at(z) - EYE_X + 0.6 * (e * 0.7).sin();
                let heading = ((self.road_at(z + DZ) - self.road_at(z - DZ)) / (2.0 * DZ)).atan();
                let rcam = Cam { cx: cam.cx + shake, ..cam };
                let pose = Pose::new(rx, z, heading, rcam);
                let paint = Paint::with_body(t, Rgba::lerp_linear(pal.neon, pal.panel, 0.25));
                drift::draw_shapes(c, &drift::car_shapes(&pose, 0.0, &paint));
                if drift::faces_viewer(&pose, &drift::tail_face()) {
                    let r = (1.4 * f / z).max(1.0).round() as i32;
                    for vc in drift::tail_lamps() {
                        let (gx, gy) = pose.screen((drift::BODY[4].0 - 0.5, vc, drift::TAIL_Y));
                        c.fill_circle(gx.round() as i32, gy.round() as i32, r, with_alpha(pal.hot, 0.25));
                    }
                }
            }

            // ---- the purge: warp streaks and the blue rush ----
            if purge > 0.0 {
                let (vx, vy) = (cam.cx + (self.road_at(Z_FAR) - cam.x) * f / Z_FAR, cam.hz);
                let reach = iw as f32 * 0.6;
                for wp in self.warps.iter_mut() {
                    wp.t = (wp.t + wp.speed * dt / 1000.0).fract();
                    let (ca, sa) = (wp.angle.cos(), wp.angle.sin() * 0.45);
                    let (r0, r1) = (reach * wp.t * wp.t, reach * (wp.t * wp.t + 0.08 + 0.12 * wp.t));
                    let col = with_alpha(Rgba::lerp_linear(WHITE, pal.neon, 0.3), 0.7 * purge);
                    c.line((vx + ca * r0) as i32, (vy + sa * r0) as i32, (vx + ca * r1) as i32, (vy + sa * r1) as i32, col);
                }
                c.fill_rect(ix0, iy0, iw, view_bottom - iy0, with_alpha(NOS_BLUE, 0.14 * purge));
            }

            // ---- the cockpit: headliner, A-pillars, mirror ----
            let s = l.s;
            let (top, bot) = (iy0 as f32, view_bottom as f32 + 1.0);
            // Each A-pillar is a band leaning in toward the roof; beyond it the side window, dimmed,
            // with the door mirror at its foot.
            let (out_t, out_b, pw) = (36.0 * s, 9.0 * s, 8.0 * s);
            let seal = Rgba::lerp_linear(pal.trim, BLACK, 0.6);
            for (sign, edge) in [(1.0f32, ix0 as f32), (-1.0, ix1 as f32)] {
                let (ot, ob) = (edge + sign * out_t, edge + sign * out_b);
                let (it, ib) = (ot + sign * pw, ob + sign * pw * 0.85);
                drift::fill_convex(c, &[(edge, top), (ot, top), (ob, bot), (edge, bot)], (0.0, 0.0), 0, with_alpha(BLACK, 0.55));
                let mx0 = edge + sign * 1.0;
                let mx1 = edge + sign * (out_b + 3.0 * s);
                let my = bot - 7.0 * s;
                drift::fill_convex(c, &[(mx0, my), (mx1, my + 1.0), (mx1, my + 4.0 * s), (mx0, my + 5.0 * s)], (0.0, 0.0), 0, pal.trim);
                c.fill_rect(((mx0 + mx1) * 0.5) as i32, (my + 2.0 * s) as i32, 2, 1, Rgba::lerp_linear(pal.trim, pal.neon, 0.4));
                drift::fill_convex(c, &[(ot, top), (it, top), (ib, bot), (ob, bot)], (0.0, 0.0), 0, pal.trim);
                c.line(it.round() as i32, iy0, ib.round() as i32, view_bottom, pal.trim_hi);
                c.line(ot.round() as i32, iy0, ob.round() as i32, view_bottom, seal);
            }
            // The headliner: the windshield's top edge, deeper toward the sides.
            for x in ix0..ix1 {
                let u = (x - (ix0 + iw / 2)) as f32 / (iw as f32 * 0.5);
                let d = (2.0 * s + 3.0 * s * u * u).round().max(1.0) as i32;
                c.fill_rect(x, iy0, 1, d, pal.trim);
                c.fill_rect(x, iy0 + d, 1, 1, seal);
            }
            let (mx, my, mw, mh) = l.mirror;
            c.fill_rect(mx + mw / 2, iy0, 1, my - iy0 + 1, pal.trim);
            c.fill_rect(mx, my, mw, mh, pal.trim);
            let glass = Rgba::lerp_linear(pal.panel, pal.neon, 0.015);
            c.fill_rect(mx + 1, my + 1, mw - 2, mh - 2, glass);
            // The car behind: two headlights, flashing on a kick.
            let hx = mx + mw / 2 + ((e * 0.5).sin() * mw as f32 * 0.2) as i32;
            let hy = my + mh / 2;
            let ha = 0.5 + 0.5 * self.flash;
            for dx in [-3, 2] {
                c.fill_rect(hx + dx, hy, 2, 1, with_alpha(Rgba::lerp_linear(WARM, WHITE, 0.6), ha));
            }
            if self.flash > 0.1 {
                c.fill_rect(mx + 1, my + 1, mw - 2, mh - 2, with_alpha(WHITE, 0.25 * self.flash));
            }

            // ---- the dashboard ----
            let cx = (ix0 + iw / 2) as f32;
            for x in ix0..ix1 {
                let u = (x as f32 - cx) / (iw as f32 * 0.5);
                let top = view_bottom - (2.0 * (1.0 - u * u).max(0.0)).round() as i32;
                c.fill_rect(x, top, 1, iy1 - top, pal.dash);
                c.fill_rect(x, top, 1, 1, Rgba::lerp_linear(pal.dash, pal.trim_hi, 0.6));
            }
        }

        // ---- the LED strip: the meter ----
        for i in 0..SEGS {
            let band = (i * NUM_BANDS) / SEGS;
            let lv = lvl(d.levels[band]).max(lvl(d.levels[(band + 1).min(NUM_BANDS - 1)]));
            let thr = SEG_THR_LO + (SEG_THR_HI - SEG_THR_LO) * i as f32 / (SEGS - 1) as f32;
            let on = lv >= thr || purge > 0.0;
            let col = seg_colour(i, pal.lit);
            let x = (l.strip_x + i as f32 * l.strip_pitch).round() as i32;
            let col = if on { col } else { Rgba::lerp_linear(pal.dash, col, pal.ghost * 2.0) };
            c.fill_rect(x, l.strip_y, l.strip_w, l.strip_h, col);
        }

        // ---- the cluster: shift lights and the rev bar ----
        let redline = self.rpm >= REDLINE;
        let blink = redline && (e * REDLINE_HZ).fract() < 0.5;
        let lit_n = (((self.rpm - 0.45) / (REDLINE - 0.45)).clamp(0.0, 1.0) * SHIFT_LEDS as f32).round() as usize;
        for i in 0..SHIFT_LEDS {
            let x = l.led_x + i as i32 * (l.led_w + l.led_gap);
            let col = if blink {
                SHIFT_BLUE
            } else if i < lit_n {
                shift_colour(i)
            } else {
                Rgba::lerp_linear(pal.dash, shift_colour(i), 0.18)
            };
            c.fill_rect(x, l.led_y, l.led_w, 1, col);
        }
        let (bx, by, bw, bh) = l.rev_bar;
        c.fill_rect(bx, by, bw, bh, Rgba::lerp_linear(pal.dash, pal.lit, 0.12));
        let fill = (bw as f32 * self.rpm).round() as i32;
        let red_from = (bw as f32 * 0.85) as i32;
        c.fill_rect(bx, by, fill.min(red_from), bh, pal.lit);
        if fill > red_from {
            c.fill_rect(bx + red_from, by, fill - red_from, bh, RED);
        }

        // ---- the steering wheel, turning with the street ----
        if self.only == Only::All {
            let (wcx, wcy, r, th) = l.wheel;
            let rim = Rgba::lerp_linear(pal.trim, pal.trim_hi, 0.25);
            for y in (wcy - r).floor() as i32..iy1 {
                let dy = y as f32 + 0.5 - wcy;
                let xo = (r * r - dy * dy).max(0.0).sqrt();
                let ri = r - th;
                let xi = (ri * ri - dy * dy).max(0.0).sqrt();
                if xo <= 0.0 {
                    continue;
                }
                if dy.abs() < ri {
                    drift::span(c, wcx - xo, wcx - xi, y, rim);
                    drift::span(c, wcx + xi, wcx + xo, y, rim);
                } else {
                    drift::span(c, wcx - xo, wcx + xo, y, rim);
                }
            }
            // The top-centre marker, turned with the steering.
            let a = -std::f32::consts::FRAC_PI_2 + self.steer * 0.6;
            let (mx, my) = (wcx + (r - th * 0.5) * a.cos(), wcy + (r - th * 0.5) * a.sin());
            c.fill_rect(mx.round() as i32 - 1, my.round() as i32 - 1, 2, 2, pal.hot);
        }

        // ---- speed, gear, the NOS bottle ----
        let gear = self.gear.clamp(1, 6);
        let gch = [b'0' + gear];
        let gs = std::str::from_utf8(&gch).unwrap_or("1");
        let (gx, gy, gw, gh) = l.gear_box;
        frame(c, (gx, gy, gw, gh), pal.lit);
        text(c, gx + 2, gy + (gh - 10) / 2, gs, 2, pal.hot);
        if l.show_speed {
            let mph = (self.mph.round() as i32).clamp(0, 999) as u32;
            let digits = [b'0' + (mph / 100) as u8, b'0' + (mph / 10 % 10) as u8, b'0' + (mph % 10) as u8];
            let ds = std::str::from_utf8(&digits).unwrap_or("000");
            text(c, l.speed.0, l.speed.1, ds, 2, pal.hot);
            text(c, l.mph.0, l.mph.1, "MPH", 1, pal.lit);
        }
        text(c, l.nos_label.0, l.nos_label.1, "NOS", 1, pal.neon);
        let (nx, ny, nw, nh) = l.nos_bar;
        frame(c, (nx, ny, nw, nh), Rgba::lerp_linear(pal.dash, pal.neon, 0.6));
        let fill = ((nw - 2) as f32 * self.bottle).round() as i32;
        if fill > 0 {
            c.fill_rect(nx + 1, ny + 1, fill, nh - 2, if purge > 0.0 { NOS_BLUE } else { pal.neon });
        }

        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

fn draw_warning(c: &mut Canvas, l: &Layout, since_ms: f32, interior: (i32, i32, i32, i32)) {
    let (ix0, iy0, iw, ih) = interior;
    c.fill_rect(ix0, iy0, iw, ih, BLACK);
    frame(c, (ix0 + 1, iy0 + 1, iw - 2, ih - 2), WARN_GREEN);
    let wv = &l.warn;
    let blink_on = (since_ms / 1000.0 * WARN_BLINK_HZ).fract() < 0.5;
    if blink_on {
        if wv.two_lines {
            text(c, wv.l1.0, wv.l1.1, "DANGER TO", 2, WARN_GREEN);
            text(c, wv.l2.0, wv.l2.1, "MANIFOLD", 2, WARN_GREEN);
        } else {
            text(c, wv.l1.0, wv.l1.1, "DANGER TO MANIFOLD", 2, WARN_GREEN);
        }
    }
    text(c, wv.nos.0, wv.nos.1, "NOS", wv.nos_scale, WARN_GREEN);
    let (bx, by, bw, bh) = wv.bar;
    frame(c, (bx, by, bw, bh), WARN_GREEN);
    let left = (1.0 - since_ms / WARN_MS).clamp(0.0, 1.0);
    let fill = ((bw - 4) as f32 * left).round() as i32;
    if fill > 0 && bh > 4 {
        c.fill_rect(bx + 2, by + 2, fill, bh - 4, WARN_GREEN);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    const IDS: [&str; 4] = ["nos-2fast", "nos-original", "nos-quarter", "nos-miami"];

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    fn calm_theme(id: &str) -> Theme {
        Theme { flourish: 0.0, ..theme(id) }
    }
    fn frames(fam: &mut Nos, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
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
            c.clear();
            fam.draw(&mut c, t, &d);
        }
        c
    }
    /// A music-like frame `k`: bass-heavy and wobbling, with a kick every `beat` frames held for three
    /// (0 = no kicks).
    fn music(level: f32, rms: f32, k: usize, beat: usize) -> FrameData {
        let t_s = k as f32 * 0.0167;
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        let kick = beat > 0 && k % beat < 3;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.2) * 0.7 + 0.12;
            let wob = 1.0 + 0.3 * (t_s * 2.4 + f * 6.0).sin();
            let kk = if kick { 1.0 + 0.9 * (1.0 - f) } else { 1.0 };
            *v = (shape * wob * kk * level).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = rms;
        d.rms_r = rms;
        d
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
    fn is_warn_green(px: Rgba) -> bool {
        px.g > 200 && px.r < 120 && px.b < 140
    }
    fn warn_px(c: &Canvas) -> usize {
        let mut n = 0;
        for y in 6..c.height() - 6 {
            for x in 6..c.width() - 6 {
                if is_warn_green(c.get(x, y)) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"nos"));
        assert_eq!(crate::render::family_for("nos").id(), "nos");
        assert_eq!(crate::themes::family_label("nos"), "Fast & Furious: NOS");
        let ids: Vec<String> =
            crate::themes::builtin::all().into_iter().filter(|t| t.family == "nos").map(|t| t.id).collect();
        assert_eq!(ids, IDS.map(String::from).to_vec());
    }

    /// Silence still shows the street, the city and the cockpit.
    #[test]
    fn rest_frame_is_not_empty() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (190, 48), (128, 44)] {
                let c = frames(&mut Nos::default(), &t, w, h, 0.0, 10);
                let n = lit(&c, &t);
                assert!(n as f32 >= 0.25 * (w * h) as f32, "{id} {w}x{h}: only {n} px at rest");
            }
        }
    }

    /// The meter: bass-only lights the strip's left end, treble-only its right end.
    #[test]
    fn the_led_strip_is_the_spectrum() {
        let t = calm_theme("nos-2fast");
        let lit_c = Rgba::from_hex(&t.lit, 1.0);
        let run = |bands: std::ops::Range<usize>| {
            let mut fam = Nos::default();
            fam.only_for_test(Only::Dash);
            let mut c = Canvas::new(380, 60);
            let mut d = FrameData::default();
            for v in d.levels[bands].iter_mut() {
                *v = 0.9;
            }
            for _ in 0..5 {
                fam.draw(&mut c, &t, &d);
            }
            let l = fam.layout;
            (0..SEGS)
                .map(|i| {
                    let x = (l.strip_x + i as f32 * l.strip_pitch).round() as i32;
                    let p = c.get(x, l.strip_y);
                    p == lit_c || p == AMBER || p == RED
                })
                .collect::<Vec<bool>>()
        };
        let bass = run(0..16);
        let treble = run(48..64);
        assert!(bass[..10].iter().all(|&b| b) && !bass[40..].iter().any(|&b| b), "bass strip {bass:?}");
        assert!(treble[38..].iter().all(|&b| b) && !treble[..10].iter().any(|&b| b), "treble strip {treble:?}");
    }

    /// Not schizo: on loud steady music the car works up through the gears one at a time, the revs
    /// dropping at each upshift, never chattering (at least `SHIFT_GAP_MS` between shifts) and moving
    /// smoothly between them; on silence it slows and shifts back down.
    #[test]
    fn the_engine_climbs_through_the_gears_smoothly() {
        let t = calm_theme("nos-2fast");
        let mut fam = Nos::default();
        let mut c = Canvas::new(380, 60);
        let (mut last_gear, mut last_rpm, mut last_shift_k) = (fam.gear, fam.rpm, -1000i64);
        let mut ups = 0;
        for k in 0..600 {
            c.clear();
            fam.draw(&mut c, &t, &music(0.8, 0.22, k, 0));
            if fam.gear != last_gear {
                assert!((fam.gear as i32 - last_gear as i32).abs() == 1, "frame {k}: jumped {last_gear} -> {}", fam.gear);
                assert!((k as i64 - last_shift_k) as f32 * 16.7 >= SHIFT_GAP_MS, "frame {k}: shifts {} frames apart", k as i64 - last_shift_k);
                last_shift_k = k as i64;
                if fam.gear > last_gear {
                    assert!(last_rpm >= UPSHIFT - 0.02, "frame {k}: shifted up at {last_rpm:.2} revs");
                    assert!(fam.rpm < last_rpm - 0.1, "frame {k}: the revs did not drop ({last_rpm:.2} -> {:.2})", fam.rpm);
                    ups += 1;
                }
            } else {
                assert!((fam.rpm - last_rpm).abs() <= 0.04, "frame {k}: revs jumped {last_rpm:.2} -> {:.2}", fam.rpm);
            }
            last_gear = fam.gear;
            last_rpm = fam.rpm;
        }
        assert!(ups >= 4, "only {ups} upshifts in 10 s");
        let top = fam.mph;
        let _ = frames(&mut fam, &t, 380, 60, 0.0, 480);
        assert!(fam.gear <= 2, "still in gear {} after 8 s of silence", fam.gear);
        assert!(fam.mph < top * 0.4, "speed only fell {top:.0} -> {:.0}", fam.mph);
    }

    /// It races, not cruises: on a steady loud track the speed keeps rising and falling - several
    /// runs up the gears and several hard stops, the speed swinging by 70 mph or more.
    #[test]
    fn it_speeds_up_and_slows_down() {
        let t = calm_theme("nos-2fast");
        let mut fam = Nos::default();
        let mut c = Canvas::new(380, 60);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let (mut ups, mut downs, mut last) = (0, 0, fam.gear);
        let mut brakes = 0;
        let mut was_braking = false;
        for k in 0..2400 {
            c.clear();
            fam.draw(&mut c, &t, &music(0.8, 0.22, k, 30));
            if k > 600 {
                lo = lo.min(fam.mph);
                hi = hi.max(fam.mph);
            }
            if fam.gear > last {
                ups += 1;
            } else if fam.gear < last {
                downs += 1;
            }
            last = fam.gear;
            let braking = fam.phase == Phase::Brake;
            if braking && !was_braking {
                brakes += 1;
            }
            was_braking = braking;
        }
        assert!(hi - lo >= 70.0, "speed only ranged {lo:.0}..{hi:.0} mph");
        assert!(brakes >= 2 && downs >= 4 && ups >= 8, "{brakes} stops, {downs} downshifts, {ups} upshifts in 40 s");
    }

    /// The cockpit is right-hand drive: the steering wheel's rim is right of centre.
    #[test]
    fn the_wheel_is_on_the_right() {
        for (w, h) in [(380, 60), (190, 48), (128, 44)] {
            let l = layout(w, h);
            assert!(l.wheel.0 > w as f32 * 0.6, "{w}x{h}: wheel at x {:.0}", l.wheel.0);
            let (gx, _, gw, _) = l.gear_box;
            assert!(gx + gw < (l.wheel.0 - l.wheel.2) as i32, "{w}x{h}: gear box under the wheel");
        }
    }

    /// Shifts land on the beat: at the shift point the engine waits on the limiter for a kick, and a
    /// kick with the revs high enough shifts early - so with a steady kick most shifts are on one.
    #[test]
    fn a_kick_shifts_early() {
        let t = calm_theme("nos-2fast");
        let mut fam = Nos::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..900 {
            c.clear();
            fam.draw(&mut c, &t, &music(0.8, 0.22, k, 30));
        }
        assert!(fam.shifts >= 4 && fam.kick_shifts * 10 >= fam.shifts * 6, "only {} of {} shifts on a kick", fam.kick_shifts, fam.shifts);
    }

    /// Shift lights: more of them lit as the revs climb, all flashing blue only at the redline.
    #[test]
    fn shift_lights_follow_the_revs() {
        let t = calm_theme("nos-2fast");
        let count = |rpm: f32, k: usize| {
            let mut fam = Nos::default();
            fam.only_for_test(Only::Dash);
            fam.rpm = rpm;
            fam.gear = 6;
            let mut c = Canvas::new(380, 60);
            let mut d = FrameData { dt_ms: 0.0, ..FrameData::default() };
            d.dt_ms = 0.0;
            fam.elapsed_s = k as f32 * 0.0625;
            fam.draw(&mut c, &t, &d);
            let l = fam.layout;
            let (mut on, mut blue) = (0, 0);
            for i in 0..SHIFT_LEDS {
                let p = c.get(l.led_x + i as i32 * (l.led_w + l.led_gap), l.led_y);
                if p == shift_colour(i) {
                    on += 1;
                }
                if p == SHIFT_BLUE {
                    blue += 1;
                }
            }
            (on, blue)
        };
        let (lo, _) = count(0.5, 0);
        let (mid, _) = count(0.7, 0);
        assert!(mid > lo, "{lo} lit at 0.5 revs, {mid} at 0.7");
        let (_, b0) = count(0.95, 0);
        let (_, b1) = count(0.95, 1);
        assert!(b0 == SHIFT_LEDS || b1 == SHIFT_LEDS, "no blue flash at the redline ({b0}, {b1})");
        let (_, none) = count(0.7, 0);
        assert_eq!(none, 0, "blue below the redline");
    }

    /// The NOS hit: the warning screen first, then the purge, with the rival dropping behind.
    #[test]
    fn nos_hit_warns_then_purges_past_the_rival() {
        for id in IDS {
            let t = theme(id);
            let mut fam = Nos::default();
            let _ = frames(&mut fam, &t, 380, 60, 0.15, 60);
            let before = fam.rival_z;
            assert!(before > 20.0, "{id}: rival at {before}");
            fam.flourish.force_next();
            // The warning's 1 px frame runs the whole width of row 5; nothing else is green there.
            let border = |c: &Canvas| (3..377).filter(|&x| is_warn_green(c.get(x, 5))).count();
            let early = frames(&mut fam, &t, 380, 60, 0.15, 6);
            assert!(warn_px(&early) > 150 && border(&early) > 340, "{id}: no warning screen");
            let late = frames(&mut fam, &t, 380, 60, 0.15, 24);
            assert!(border(&late) < 40, "{id}: still warning 500 ms in");
            let mut passed = false;
            for _ in 0..20 {
                let _ = frames(&mut fam, &t, 380, 60, 0.15, 1);
                passed |= fam.rival_z < 18.0;
            }
            assert!(passed, "{id}: never passed the rival (at {:.0})", fam.rival_z);
            assert!(fam.bottle < 0.6, "{id}: the bottle did not drain ({:.2})", fam.bottle);
        }
    }

    /// The warning falls back to two lines when one would crowd the edges.
    #[test]
    fn manifold_warning_falls_back_to_two_lines() {
        assert!(!layout(380, 60).warn.two_lines);
        assert!(layout(128, 44).warn.two_lines);
    }

    /// Speed shows at 380 and 190 wide and drops at 128; the gear always shows.
    #[test]
    fn speed_and_gear_fit_or_drop() {
        assert!(layout(380, 60).show_speed);
        assert!(!layout(128, 44).show_speed);
        for (w, h) in [(380, 60), (190, 48), (128, 44)] {
            let l = layout(w, h);
            let (gx, gy, gw, gh) = l.gear_box;
            assert!(gx >= 3 && gx + gw <= w - 3 && gy >= 4 && gy + gh <= h - 4, "{w}x{h}: gear box {:?}", l.gear_box);
            let (nx, _, nw, _) = l.nos_bar;
            assert!(nx >= 3 && nx + nw <= w - 3, "{w}x{h}: NOS bar {:?}", l.nos_bar);
            assert!(l.nos_label.0 < 0 || l.nos_label.0 >= 3, "{w}x{h}: NOS label at {:?}", l.nos_label);
        }
    }

    /// The street moves: consecutive frames differ in the windshield, even at silence.
    #[test]
    fn the_street_streams_past() {
        let t = calm_theme("nos-miami");
        let mut fam = Nos::default();
        let _ = frames(&mut fam, &t, 380, 60, 0.2, 120);
        let a = frames(&mut fam, &t, 380, 60, 0.2, 1);
        let b = frames(&mut fam, &t, 380, 60, 0.2, 1);
        let mut diff = 0;
        for y in 4..fam.layout.dash_top {
            for x in 3..377 {
                if a.get(x, y) != b.get(x, y) {
                    diff += 1;
                }
            }
        }
        assert!(diff >= 200, "only {diff} px of the street changed between frames");
    }

    /// 190x48 and 128x44 with a forced hit: no panic, something drawn, nothing outside the panel.
    #[test]
    fn fits_the_narrow_panel_and_flourish_does_not_panic() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(190, 48), (128, 44)] {
                let mut fam = Nos::default();
                let _ = frames(&mut fam, &t, w, h, 0.7, 5);
                fam.flourish.force_next();
                for n in [1usize, 10, 10, 40] {
                    let c = frames(&mut fam, &t, w, h, 0.7, n);
                    assert!(lit(&c, &t) > 0, "{id} {w}x{h}");
                    for x in 0..w {
                        for y in [0, 1, h - 2, h - 1] {
                            assert_eq!(c.get(x, y).a, 0, "{id} {w}x{h}: paint outside the panel at {x},{y}");
                        }
                    }
                    for y in 0..h {
                        assert_eq!(c.get(0, y).a, 0, "{id} {w}x{h}: paint left of the panel at 0,{y}");
                        assert_eq!(c.get(w - 1, y).a, 0, "{id} {w}x{h}: paint right of the panel at {y}");
                    }
                }
            }
        }
    }

    #[test]
    fn garbage_frames_do_not_break_the_engine() {
        let t = theme("nos-2fast");
        let mut fam = Nos::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData::default();
        for k in 0..120 {
            d.levels = [if k % 2 == 0 { f32::NAN } else { f32::INFINITY }; NUM_BANDS];
            d.rms_l = f32::NAN;
            d.rms_r = f32::INFINITY;
            d.dt_ms = if k % 3 == 0 { f32::NAN } else { 1.0e9 };
            fam.draw(&mut c, &t, &d);
        }
        for v in [fam.rpm, fam.mph, fam.s_cam, fam.rival_z, fam.throttle] {
            assert!(v.is_finite(), "state went non-finite: {v}");
        }
        assert!((1..=6).contains(&fam.gear));
    }

    #[test]
    fn the_four_colourways_are_visibly_different() {
        let (w, h) = (380i32, 60i32);
        let canvases: Vec<Canvas> = IDS.iter().map(|id| frames(&mut Nos::default(), &theme(id), w, h, 0.12, 30)).collect();
        let interior = ((w - 6) * (h - 8)) as f32;
        let mut too_similar = Vec::new();
        for a in 0..IDS.len() {
            for b in (a + 1)..IDS.len() {
                let mut diff = 0usize;
                for y in 4..h - 4 {
                    for x in 3..w - 3 {
                        let (pa, pb) = (canvases[a].get(x, y), canvases[b].get(x, y));
                        let d = (pa.r as i32 - pb.r as i32).abs() + (pa.g as i32 - pb.g as i32).abs() + (pa.b as i32 - pb.b as i32).abs();
                        if d > 24 {
                            diff += 1;
                        }
                    }
                }
                if (diff as f32 / interior) < 0.12 {
                    too_similar.push(format!("{} vs {}", IDS[a], IDS[b]));
                }
            }
        }
        assert!(too_similar.is_empty(), "colourway pairs too similar: {too_similar:?}");
    }

    #[test]
    fn every_label_char_has_a_glyph() {
        for s in ["DANGER TO MANIFOLD", "NOS", "MPH", "0123456789"] {
            for ch in s.chars() {
                assert!(ch == ' ' || font3x5::glyph(ch).is_some(), "{s:?} {ch:?}");
            }
        }
    }

    /// Dumps for the eye test, composited over `#202020`. Each file is `<name>.<w>x<h>.rgba`; the strip
    /// is twelve frames 1.5 s apart, a full race cycle.
    ///
    /// Run: cargo test --release dump_nos -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_nos() {
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
            std::fs::write(dir.join(format!("{name}.{}x{}.rgba", c.width(), c.height())), &out).unwrap();
        };
        let run = |id: &str, w: i32, h: i32, level: f32, rms: f32, n: usize| {
            let t = calm_theme(id);
            let mut fam = Nos::default();
            let mut c = Canvas::new(w, h);
            for k in 0..n {
                c.clear();
                fam.draw(&mut c, &t, &music(level, rms, k, 30));
            }
            (fam, c, t)
        };
        let hit = |id: &str, w: i32, h: i32, at: usize| {
            let (mut fam, mut c, t) = run(id, w, h, 0.7, 0.18, 300);
            fam.flourish.force_next();
            for k in 300..300 + at {
                c.clear();
                fam.draw(&mut c, &t, &music(0.7, 0.18, k, 0));
            }
            c
        };
        for id in IDS {
            let short = &id["nos-".len()..];
            for (tag, level, rms, n) in [("calm", 0.3f32, 0.07f32, 300), ("loud", 0.85, 0.22, 420)] {
                let (_, c, _) = run(id, 380, 60, level, rms, n);
                write(format!("nos-{short}-{tag}"), &c);
            }
            write(format!("nos-{short}-warning"), &hit(id, 380, 60, 6));
            write(format!("nos-{short}-purge"), &hit(id, 380, 60, 28));
        }
        // Eight frames half a second apart, loud, to show the gears and the rival.
        let t = calm_theme("nos-2fast");
        let mut fam = Nos::default();
        let mut c = Canvas::new(380, 60);
        let mut out = Canvas::new(380, 720);
        for k in 0..(60 + 12 * 90) {
            c.clear();
            fam.draw(&mut c, &t, &music(0.85, 0.22, k, 30));
            if k >= 60 && (k - 60) % 90 == 89 {
                out.copy_region(&c, (0, 0), (0, ((k - 60) / 90) as i32 * 60), 380, 60);
            }
        }
        write("nos-2fast-strip".into(), &out);
        for (w, h) in [(190, 48), (128, 44)] {
            let (_, c, _) = run("nos-2fast", w, h, 0.85, 0.22, 420);
            write(format!("nos-2fast-{w}x{h}"), &c);
            write(format!("nos-2fast-{w}x{h}-warning"), &hit("nos-2fast", w, h, 6));
        }
        println!("wrote nos dumps to {}", dir.display());
    }

    /// Per-frame cost at 380x60.
    ///
    /// Run: cargo test --release probe_nos_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_nos_cost() {
        let t = theme("nos-2fast");
        let mut fam = Nos::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = 0.5 + 0.4 * ((i as f32) * 0.3).sin();
        }
        for _ in 0..60 {
            fam.draw(&mut c, &t, &d);
        }
        let t0 = std::time::Instant::now();
        for _ in 0..600 {
            fam.draw(&mut c, &t, &d);
        }
        println!("nos: {:.3} ms/frame", t0.elapsed().as_secs_f64() * 1000.0 / 600.0);
    }
}
