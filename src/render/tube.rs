//! The vacuum-tube row family: a rank of valves on a chassis, each glowing with its band.
//!
//! A Soviet-lab counterpart to `segmented` - same job, opposite century. Where the VFD
//! family reads as a modern instrument (crisp lit marks on a dark panel), this one reads as
//! hardware: a milled chassis with a row of glass envelopes bolted through it, each one
//! lighting as its band drives it.
//!
//! Three things do the heavy lifting, and each is a deliberate departure from `segmented`:
//!
//! - **The heater never goes fully out.** Real valves idle warm. A tube that goes black at
//!   silence looks broken rather than quiet, so every tube keeps a floor glow and the band
//!   level drives it upward from there. This is the single detail that makes the row read as
//!   valves instead of as circles.
//! - **The glow is inside the glass.** A radial gradient centred on the plate, clipped to
//!   the envelope, rather than a halo around the outside - light from a valve comes from the
//!   cathode and is *contained* by the glass, and the containment is what sells it.
//! - **Glass is drawn over the glow, not under it.** The envelope's highlight and rim go on
//!   last at low alpha, so the tube reads as a lit thing seen *through* glass.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// Floor alpha the heater hairlines keep before any audio. See the heater note in the module
/// docs.
///
/// Applied ONCE, and only to the two 1px filaments - not to the cathode glow. It used to be
/// folded into a shared `drive` term that the filament then floored again on top of
/// (`0.30 + 0.70 * drive` where `drive` was already `0.17 + 0.83 * level`), which expands to
/// `0.419 + 0.581 * level`: 42% of a near-white alpha spent before a single sample arrived, and
/// only 58% of the range left for the music. The valves still never go fully out, because these
/// hairlines still glow at silence - but the big cathode glow now starts from zero and gets the
/// whole range.
const HEATER_FLOOR: f32 = 0.10;

/// Band level at which a valve starts to light, and the span it lights over.
///
/// The mapping was `HEATER_FLOOR + (1 - HEATER_FLOOR) * level` - linear over the whole 0..1 range
/// - but the DSP only ever delivers about 0.15..0.65 for active bands, so a valve used barely a
/// third of its range on real music. Measured on the shipped code: across that window the cathode
/// core moved from 0.289 to 0.695 alpha, worth 3.70 dL* on the composited pixels, and the
/// filaments moved only 1.60x. Remapping the window the DSP actually produces onto the full alpha
/// range is worth 3.70 -> 5.56 dL* on its own.
///
/// A FIXED window, deliberately, not a peak follower. The scope and the vaporwave grid both
/// auto-range because they show the SHAPE of a signal and absolute level is the VU family's job -
/// but a valve row is a level meter, like the segmented VFD. A follower here would present the
/// same band level at very different brightnesses depending on what came before it, i.e. it would
/// make a quiet passage look like a loud one, which is the opposite of what this family is for.
const RESP_FLOOR: f32 = 0.10;
const RESP_SPAN: f32 = 0.52;

/// Weight given to a group's LOUDEST band rather than its mean.
///
/// Each valve covers about 6.4 of the 64 log bands, and averaging them flattens exactly the
/// single-band peaks that make one valve differ from its neighbour. Measured on the case that
/// matches the complaint - one band peaking inside an otherwise quiet group - a plain mean gave
/// 1.46 dL* between the driven valve and its neighbour, which is BELOW the ~2.3 dL* threshold at
/// which a difference is visible at all. That is why the row looked static. Biasing toward the
/// max takes the same case to 9.47 dL*.
///
/// Invisible to any test that drives every band to the same level, because mean == max there -
/// which is precisely what every test in this file used to do.
const GROUP_MAX_BIAS: f32 = 0.65;

/// Per-frame fall of a valve's peak marker.
///
/// Sourced from the displayed response with its own fast fall, NOT from `FrameData.peaks`. The
/// shared peak-hold falls at 0.0055 per frame (see dsp::ballistics), which needs about 1.8s to
/// drop from 0.65 to this marker's 0.05 visibility threshold - so under continuous music the bar
/// was drawn every frame at a near-constant alpha on every valve, adding a uniform bright feature
/// worth ~16 dL* that made the row read MORE uniform, not less.
const MARKER_FALL: f32 = 0.035;

/// Pitch one valve wants, in pixels.
///
/// 19 is the 190px reference panel divided by the ten tubes that were tuned and approved, so
/// the narrow case is unchanged. Chosen because it leaves an envelope wide enough to have a
/// visible plate inside it, which is the point of the family: narrower collapses the glass to a
/// stripe with no interior, wider reads as an arched window rather than a valve.
const TUBE_PITCH: i32 = 19;

/// Valves to draw at a given panel width.
///
/// Scaled rather than fixed, because a fixed count stretches: measured at 380px the ten tubes
/// grew to a 37px pitch with 20px-wide glass, which read as a row of arched windows. Adding
/// valves instead keeps every one the size it was tuned at - and it also narrows each valve's
/// share of the spectrum, so neighbouring valves differ more, which is the thing the row was
/// short of.
fn tube_count(w: i32) -> usize {
    ((w / TUBE_PITCH).max(4) as usize).min(40)
}

#[derive(Default)]
pub struct Tube {
    /// Fast-falling peak hold per valve, in displayed-response units.
    ///
    /// A `Vec` rather than an array because `tube_count` scales to 40 and `#[derive(Default)]`
    /// does not exist for `[f32; 40]` - the std impls stop at 32.
    marker: Vec<f32>,
}

impl Tube {
    /// Mean of the bands feeding one tube.
    ///
    /// Averaged rather than sampled, because at ten tubes across 64 bands a point sample
    /// throws away five sixths of the spectrum and makes neighbouring tubes jump
    /// independently of the music.
    fn level_for(d: &FrameData, i: usize, tubes: usize) -> f32 {
        let n = d.levels.len();
        let tubes = tubes.max(1);
        let lo = i * n / tubes;
        let hi = (((i + 1) * n / tubes).max(lo + 1)).min(n);
        let mut acc = 0.0;
        let mut cnt = 0.0;
        let mut peak = 0.0f32;
        for v in &d.levels[lo..hi] {
            if v.is_finite() {
                acc += *v;
                cnt += 1.0;
                peak = peak.max(*v);
            }
        }
        if cnt <= 0.0 {
            return 0.0;
        }
        let mean = acc / cnt;
        // Blended toward the peak - see GROUP_MAX_BIAS. A pure mean hides single-band peaks; a
        // pure max makes neighbouring valves jump independently of anything musical.
        (mean * (1.0 - GROUP_MAX_BIAS) + peak * GROUP_MAX_BIAS).clamp(0.0, 1.0)
    }

    /// Maps a group level onto the valve's usable alpha range.
    fn response(level: f32, sensitivity: f32) -> f32 {
        if !level.is_finite() {
            return 0.0;
        }
        (((level - RESP_FLOOR) / RESP_SPAN) * sensitivity.max(0.0)).clamp(0.0, 1.0)
    }

}

impl Family for Tube {
    fn id(&self) -> &'static str {
        "tube"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        if w < 24 || h < 20 {
            // Too small to hold an envelope with an interior; fill the chassis and stop
            // rather than drawing a row of unrecognisable smudges.
            c.clear();
            c.rounded_rect(1, 2, (w - 2).max(1), (h - 4).max(1), 3, Rgba::from_hex(&t.panel, t.panel_alpha));
            return;
        }

        c.clear();
        // Chassis: a plate with a subtle top-lit gradient, so it reads as milled metal
        // rather than as a flat background.
        c.rounded_rect(1, 2, w - 2, h - 4, 3, Rgba::from_hex(&t.panel, t.panel_alpha));
        c.vertical_gradient(
            2,
            3,
            w - 4,
            h - 6,
            &[
                (0.0, Rgba::from_hex(&t.tube.chassis_top, 0.55)),
                (1.0, Rgba::from_hex(&t.tube.chassis_bottom, 0.55)),
            ],
            true,
        );

        // Geometry. A valve is a narrow capsule: a domed envelope standing in a socket, with
        // chassis visible either side of it. The first version made the glass `pitch - 5`
        // wide, which at a 19px pitch left no chassis between tubes and rendered the row as
        // ten touching slabs rather than as separate valves.
        let margin = 4;
        let tubes = tube_count(w);
        let pitch = ((w - margin * 2) as f32 / tubes as f32).max(6.0);
        let glass_w = ((pitch * 0.55) as i32).max(5) | 1; // odd, so it has a true centre column
        let base_h = 6;
        let top = 5;
        let base_y = h - 4 - base_h;
        let glass_h = (base_y - top).max(8);
        // Dome over the top third, so the silhouette curves for long enough to read as glass.
        let dome = (glass_w / 2).max(2) + glass_h / 6;

        // Half-width of the envelope at a given row: a semi-elliptical dome over straight
        // sides. A closure so the clip and the fills cannot disagree about the silhouette -
        // they did in the first version, where the peak rim was drawn outside the clip as a
        // full-width bar and flattened the dome it was supposed to cap.
        let half_at = |y: i32| -> i32 {
            let from_top = y - top;
            if from_top < 0 || y >= base_y {
                return 0;
            }
            let hw = glass_w / 2;
            if from_top < dome {
                let k = from_top as f32 / dome as f32;
                (((hw as f32) * (k * (2.0 - k)).sqrt()).round() as i32).max(1)
            } else {
                hw
            }
        };

        // Everything that emits light goes on its own layer so it can be bloomed once and
        // composited over the opaque chassis. Blooming the chassis directly leaves the halo
        // invisible - every destination pixel is already fully opaque, the same trap already
        // documented in segmented/scope/vu.
        let mut lit = Canvas::new(w, h);

        for i in 0..tubes {
            let cx = margin + (pitch * (i as f32 + 0.5)) as i32;
            let level = Self::level_for(d, i, tubes);
            let resp = Self::response(level, t.sensitivity);
            // Marker hold, from the DISPLAYED response rather than the shared slow peak-hold.
            if self.marker.len() != tubes {
                self.marker.resize(tubes, 0.0);
            }
            self.marker[i] = (self.marker[i] - MARKER_FALL).max(resp);
            let peak = self.marker[i];

            // Cathode glow, centred on the plate. The radius is tied to the ENVELOPE, not to
            // the larger of width/height - at `glass_w.max(glass_h/2)` it exceeded the glass
            // entirely, so every tube was uniformly flooded and the falloff that makes it
            // look like a contained light source was invisible.
            let plate_top = top + dome + glass_h / 12;
            let plate_h = (glass_h - dome - glass_h / 6).max(4);
            let plate_mid = plate_top + plate_h / 2;
            let reach = (glass_w as f32 * 1.15) as i32;
            lit.radial_gradient(
                cx,
                plate_mid,
                1,
                reach.max(3),
                &[
                    // Floorless: the glow starts from nothing and gets the whole range. The
                    // "never fully out" property is carried by the filaments below instead.
                    (0.0, Rgba::from_hex(&t.hot, (resp * 0.98).clamp(0.0, 1.0))),
                    (0.40, Rgba::from_hex(&t.lit, (resp * 0.72).clamp(0.0, 1.0))),
                    (1.0, Rgba::from_hex(&t.lit, 0.0)),
                ],
            );

            // Heater: two bright hairlines flanking the plate, which is where the light
            // actually escapes on a real valve. A single line up the centre with the dark
            // plate either side of it read as a domino, not a filament.
            // HEATER_FLOOR applied ONCE, here and nowhere else - see its own doc comment.
            let fil = Rgba::from_hex(
                &t.hot,
                (HEATER_FLOOR + (1.0 - HEATER_FLOOR) * resp).clamp(0.0, 1.0),
            );
            let pw = (glass_w / 2).max(1);
            for dx in [-(pw / 2 + 1), pw / 2 + 1] {
                lit.fill_rect(cx + dx, plate_top + 1, 1, plate_h - 2, fil);
            }

            // Anode plate: dark metal in front of the light. Drawn lit-layer-side so the clip
            // below applies to it too, and dark because a plate is a silhouette.
            lit.fill_rect(
                cx - pw / 2,
                plate_top,
                pw.max(1),
                plate_h,
                Rgba::from_hex(&t.tube.internals, 0.80),
            );

            // Getter flash: the silvery mirror deposited inside the dome of a real valve.
            lit.fill_rect(cx - glass_w / 4, top + 1, (glass_w / 2).max(1), 1, Rgba::from_hex(&t.tube.glass, 0.34));

            // Peak marker. INSIDE the clip region, as a short bar no wider than the envelope
            // is at that row - see the note on `half_at`.
            if peak > 0.05 {
                let y = plate_top - 2;
                let hw = half_at(y);
                if hw > 1 {
                    lit.fill_rect(cx - hw + 1, y, (hw * 2 - 1).max(1), 1, Rgba::from_hex(&t.hot, (peak * 0.9).clamp(0.0, 1.0)));
                }
            }

        }

        // Clip the light to the envelopes - ONE pass over every tube, punching only the gaps
        // between them.
        //
        // This was per-tube, and each tube punched a fixed span either side of its own centre.
        // That span (the glow reach plus the glass, 23px) is wider than the tube pitch
        // (18.2px), so every tube erased the light of the one before it and the whole row
        // rendered dark except for a few stray pixels. Clipping against the union of the
        // envelopes is the only formulation that cannot depend on draw order.
        let centres: Vec<i32> = (0..tubes)
            .map(|i| margin + (pitch * (i as f32 + 0.5)) as i32)
            .collect();
        for y in 2..(h - 2) {
            let hw = half_at(y);
            if hw == 0 {
                lit.punch_rect(0, y, w, 1);
                continue;
            }
            let mut prev_end = 0;
            for &cx in &centres {
                let lo = cx - hw;
                if lo > prev_end {
                    lit.punch_rect(prev_end, y, lo - prev_end, 1);
                }
                prev_end = cx + hw + 1;
            }
            if prev_end < w {
                lit.punch_rect(prev_end, y, w - prev_end, 1);
            }
        }

        if t.bloom > 0.0 {
            let mut glow = lit.clone();
            glow.bloom(t.bloom as i32, t.glow_strength.clamp(0.0, 1.0));
            c.draw_over(&glow);
        }
        c.draw_over(&lit);

        // Sockets and glass, drawn AFTER the light so a tube reads as a lit thing seen
        // through glass rather than as a glowing rectangle with a lid.
        for i in 0..tubes {
            let cx = margin + (pitch * (i as f32 + 0.5)) as i32;
            let gx = cx - glass_w / 2;

            // Bakelite socket with a brass collar and pins.
            c.fill_rect(gx - 1, base_y, glass_w + 2, base_h, Rgba::from_hex(&t.tube.socket, 1.0));
            c.fill_rect(gx - 1, base_y, glass_w + 2, 1, Rgba::from_hex(&t.tube.collar, 0.85));
            for pn in 0..3 {
                let px = gx + 1 + pn * ((glass_w - 2).max(3) / 3);
                c.fill_rect(px, base_y + 2, 1, base_h - 3, Rgba::from_hex(&t.tube.collar, 0.5));
            }

            // Glass: a specular highlight down one side and a dimmer catch-light on the other,
            // both following the dome so they curve with the envelope.
            for y in (top + 1)..base_y {
                let hw = half_at(y);
                if hw <= 1 {
                    continue;
                }
                c.fill_rect(cx - hw, y, 1, 1, Rgba::from_hex(&t.tube.glass, 0.22));
                c.fill_rect(cx + hw, y, 1, 1, Rgba::from_hex(&t.tube.glass, 0.11));
            }
        }

        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn frame(level: f32) -> FrameData {
        let mut d = FrameData::default();
        for v in d.levels.iter_mut() {
            *v = level;
        }
        for v in d.peaks.iter_mut() {
            *v = level;
        }
        d
    }

    /// Total light emitted, as a stand-in for "how lit does this look".
    fn brightness(t: &Theme, d: &FrameData) -> u64 {
        let mut tube = Tube::default();
        let mut c = Canvas::new(190, 60);
        tube.draw(&mut c, t, d);
        let mut sum = 0u64;
        for y in 0..60 {
            for x in 0..190 {
                let p = c.get(x, y);
                sum += p.r as u64 + p.g as u64 + p.b as u64;
            }
        }
        sum
    }

    /// One band peaking inside an otherwise quiet group - the case that matches the complaint
    /// "the valves need to glow for intensity at different eq bands".
    ///
    /// Every other test in this file drives EVERY band to the same level, where a group's mean
    /// equals its max, so the group reducer is a mathematical no-op and none of them can see this
    /// at all. That blind spot is why the row shipped looking static.
    fn one_loud_band(tubes: usize, loud_tube: usize, loud: f32, quiet: f32) -> FrameData {
        let mut d = FrameData::default();
        let n = d.levels.len();
        for v in d.levels.iter_mut() {
            *v = quiet;
        }
        // a single band, in the middle of the target valve's group
        let lo = loud_tube * n / tubes;
        let hi = ((loud_tube + 1) * n / tubes).min(n);
        d.levels[(lo + hi) / 2] = loud;
        d.peaks = d.levels;
        d
    }

    #[test]
    fn a_single_peaking_band_visibly_lights_its_own_valve() {
        // Mean-only reduction gave 1.46 dL* between the driven valve and its neighbour, below the
        // ~2.3 dL* at which a difference is perceptible at all. Measured as a luminance ratio here
        // rather than dL*, but the point is the same: the driven valve must clearly out-glow the
        // one beside it.
        let t = builtin::tube_soviet();
        let d = one_loud_band(10, 4, 0.65, 0.20);
        let mut tube = Tube::default();
        let mut c = Canvas::new(190, 60);
        tube.draw(&mut c, &t, &d);

        let pitch = (190 - 8) as f32 / 10.0;
        let valve = |i: usize| -> f64 {
            let cx = 4 + (pitch * (i as f32 + 0.5)) as i32;
            let mut s = 0.0f64;
            for y in 6..50 {
                for x in (cx - 6)..(cx + 7) {
                    let p = c.get(x, y);
                    s += 0.2126 * p.r as f64 + 0.7152 * p.g as f64 + 0.0722 * p.b as f64;
                }
            }
            s
        };
        let driven = valve(4);
        let neighbour = valve(5);
        assert!(
            driven > neighbour * 1.35,
            "a peaking band must visibly light its own valve: driven {driven:.0} vs neighbour {neighbour:.0}"
        );
    }

    #[test]
    fn the_group_reducer_is_biased_toward_the_peak_not_the_mean() {
        // Guards GROUP_MAX_BIAS directly. A group of one loud band among six quiet ones has a mean
        // far below its max; the reducer must land nearer the max or single-band peaks vanish.
        let d = one_loud_band(10, 0, 0.9, 0.1);
        let n = d.levels.len();
        let hi = (n / 10).max(1);
        let mean = d.levels[..hi].iter().sum::<f32>() / hi as f32;
        let peak = d.levels[..hi].iter().copied().fold(0.0f32, f32::max);
        let got = Tube::level_for(&d, 0, 10);
        assert!(got > mean + (peak - mean) * 0.5, "must sit above the midpoint: {got} in [{mean}, {peak}]");
        assert!(got <= peak + 1e-6, "but never above the peak itself: {got} vs {peak}");
    }

    #[test]
    fn the_response_window_spends_its_range_on_levels_the_dsp_actually_produces() {
        // The mapping was linear over 0..1 while the DSP delivers roughly 0.15-0.65, so a valve
        // used about a third of its range. Across that window the response must now travel most
        // of 0..1.
        let lo = Tube::response(0.15, 1.0);
        let hi = Tube::response(0.65, 1.0);
        assert!(hi - lo > 0.75, "the music window must cover most of the range: {lo} -> {hi}");
        assert_eq!(Tube::response(0.0, 1.0), 0.0, "silence must map to zero, not a pedestal");
        assert_eq!(Tube::response(1.0, 1.0), 1.0, "full scale must reach the top");
        // sensitivity is the user-facing knob and must actually do something
        assert!(Tube::response(0.3, 2.0) > Tube::response(0.3, 1.0), "sensitivity must scale it");
    }

    #[test]
    fn tube_count_holds_at_ten_when_narrow_and_grows_with_width() {
        assert_eq!(tube_count(190), 10, "the reference panel must keep the ten tuned valves");
        assert_eq!(tube_count(380), 20, "double the width doubles the valves rather than the glass");
        assert!(tube_count(4000) <= 40, "capped");
        assert!(tube_count(20) >= 4, "and never fewer than a handful");
    }

    #[test]
    fn a_wide_panel_keeps_the_valves_the_size_they_were_tuned_at() {
        // A fixed count stretched: at 380px the ten valves grew to a 37px pitch with 20px glass,
        // which read as a row of arched windows rather than valves. Pitch is what must stay put.
        let pitch_at = |w: i32| (w - 8) as f32 / tube_count(w) as f32;
        let reference = pitch_at(190);
        for w in [190, 240, 380, 456, 600] {
            let p = pitch_at(w);
            assert!(
                (p - reference).abs() < 4.0,
                "at width {w} the valve pitch drifted to {p:.1} from the tuned {reference:.1}"
            );
        }
    }

    #[test]
    fn tubes_glow_brighter_as_the_band_drives_them() {
        let t = builtin::tube_soviet();
        let quiet = brightness(&t, &frame(0.05));
        let mid = brightness(&t, &frame(0.5));
        let loud = brightness(&t, &frame(0.95));
        assert!(mid > quiet, "mid {mid} should out-glow quiet {quiet}");
        assert!(loud > mid, "loud {loud} should out-glow mid {mid}");
    }

    #[test]
    fn the_heater_never_goes_fully_out() {
        // The detail that makes the row read as valves rather than as circles: a tube that
        // goes black at silence looks broken, not quiet. Compared against the chassis alone
        // so this cannot pass on the chassis gradient.
        let t = builtin::tube_soviet();
        let silent = brightness(&t, &frame(0.0));

        let mut bare = t.clone();
        bare.hot = t.panel.clone();
        bare.lit = t.panel.clone();
        let chassis_only = brightness(&bare, &frame(0.0));
        assert!(
            silent > chassis_only,
            "at silence the heaters must still emit: {silent} vs bare chassis {chassis_only}"
        );
    }

    #[test]
    fn driving_one_band_lights_its_own_tube_and_not_the_far_one() {
        // Guards the envelope clip: without it the radial glow bleeds across the chassis and
        // the whole row lights together, which is this family's most likely failure.
        //
        // Measures each tube's RISE from silence rather than its absolute brightness. An
        // absolute comparison conflates the tube with the chassis gradient behind it and the
        // deliberate heater floor in front of it - it scored 1.75x between a fully driven and
        // a fully idle tube, which says more about the chassis than about bleed.
        let t = builtin::tube_soviet();
        let region = |c: &Canvas, x0: i32, x1: i32| -> i64 {
            let mut s = 0i64;
            for y in 6..50 {
                for x in x0..x1 {
                    let p = c.get(x, y);
                    s += p.r as i64 + p.g as i64 + p.b as i64;
                }
            }
            s
        };
        let render = |d: &FrameData| {
            let mut tube = Tube::default();
            let mut c = Canvas::new(190, 60);
            tube.draw(&mut c, &t, d);
            c
        };

        let silent = render(&frame(0.0));
        let mut low_only = FrameData::default();
        for (i, v) in low_only.levels.iter_mut().enumerate() {
            *v = if i < 6 { 1.0 } else { 0.0 };
        }
        low_only.peaks = low_only.levels;
        let driven = render(&low_only);

        let first_rise = region(&driven, 4, 22) - region(&silent, 4, 22);
        let last_rise = region(&driven, 168, 186) - region(&silent, 168, 186);
        assert!(first_rise > 1000, "the driven tube must actually light: rise {first_rise}");
        assert!(
            first_rise > last_rise.abs() * 8,
            "light must stay in its own envelope: driven tube rose {first_rise},              the far idle tube moved {last_rise}"
        );
    }

    #[test]
    fn survives_nan_a_tiny_canvas_and_an_empty_spectrum() {
        let t = builtin::tube_soviet();
        let mut d = frame(0.5);
        d.levels[0] = f32::NAN;
        d.levels[9] = f32::INFINITY;
        d.peaks[3] = f32::NAN;
        let mut tube = Tube::default();
        let mut c = Canvas::new(190, 60);
        tube.draw(&mut c, &t, &d);
        let mut tiny = Canvas::new(10, 8);
        tube.draw(&mut tiny, &t, &d);
        let mut thin = Canvas::new(190, 12);
        tube.draw(&mut thin, &t, &d);
    }

    #[test]
    fn every_tube_colourway_renders_and_differs() {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        for t in builtin::all().into_iter().filter(|t| t.family == "tube") {
            let mut tube = Tube::default();
            let mut c = Canvas::new(190, 60);
            tube.draw(&mut c, &t, &frame(0.6));
            let bits = c.bits().to_vec();
            assert!(bits.iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for prior in &seen {
                assert_ne!(prior, &bits, "{} renders identically to another colourway", t.id);
            }
            seen.push(bits);
        }
        assert!(seen.len() >= 4, "expected several tube colourways, got {}", seen.len());
    }

    /// Run: cargo test --release dump_tube_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_tube_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let mut n = 0usize;
        for t in builtin::all().into_iter().filter(|t| t.family == "tube") {
            // an uneven spectrum, so the tubes are visibly at different levels
            let mut d = FrameData::default();
            for (i, v) in d.levels.iter_mut().enumerate() {
                let x = i as f32 / 63.0;
                *v = (0.15 + 0.85 * (x * 9.0).sin().abs()) * (1.0 - x * 0.45);
            }
            d.peaks = d.levels;
            let mut tube = Tube::default();
            let mut c = Canvas::new(190, 60);
            tube.draw(&mut c, &t, &d);
            let mut out = Vec::with_capacity(190 * 60 * 4);
            for y in 0..60 {
                for x in 0..190 {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for ch in [px.r, px.g, px.b] {
                        out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("tube-{}.rgba", t.id)), &out).unwrap();
            n += 1;
        }
        println!("wrote {} tube dumps to {}", n, dir.display());
    }
}
