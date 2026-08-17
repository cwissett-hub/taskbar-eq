pub mod pick;
pub mod builtin;
pub mod schema;
pub mod watch;

use crate::dsp::ballistics::Ballistics;

// Only `builtin::vfd_ice` exists so far, and it uses `Glass`. The other
// panel-texture variants are exercised by the remaining four segmented
// colourways from the reference mockup (Matrix Green/scanlines, Neon
// Pink/haze, Vac Tube Orange/filament, Classic Three-Colour/grille) - not
// yet ported into `builtin::all`.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Texture {
    Glass,
    Scanlines,
    Haze,
    Filament,
    Grille,
    None_,
}

/// Saturation the rainbow runs at, and it is a measured ceiling rather than taste.
///
/// This project requires every lit colour to clear 3:1 contrast against its own panel. Swept over
/// all 360 hues against a near-black panel: at full saturation pure blue (hue 240) manages only
/// 2.31:1 and FAILS that rule; 0.9 gives 2.48 and 0.8 gives 2.88, still failing. 0.70 is the first
/// value that passes at every hue, at 3.59:1. 0.68 is used for a little margin.
///
/// So a rainbow cannot be fully saturated here. Blue is simply too dark against black at any
/// brightness, and no amount of value fixes it - only pulling it toward white does.
pub const RAINBOW_SAT: f32 = 0.68;

/// How often a family's flourish fires, by default.
///
/// A flourish is the rare whole-display event each family does on an exceptional hit - the vaporwave
/// lightning generalised. 0 disables it; larger fires more often. See `dsp::flourish` for what
/// "exceptional" means and how it is measured, and
/// `dsp::flourish::tests::the_default_setting_is_rare_on_every_kind_of_music` for what this value
/// actually produces on three recordings of real music.
///
/// Measured, not guessed. Swept over a 119-second capture of NINE varied tracks - the calibration
/// that matters, because looping one 13-second recording makes the same exceptional moment recur once
/// per loop and the count jumps in steps of the loop count instead of forming a curve:
///
/// ```text
///   strength   0.10  0.20  0.30  0.35  0.45  0.50  0.60  0.70  0.85  1.00
///   one per     60s   40s   40s   30s   30s   24s   20s   15s    9s    7s
/// ```
///
/// 0.45 is one every thirty seconds, which is "fairly rare" for an event that takes over the whole
/// display, and it errs on the rare side because raising it is trivial and because the failure this
/// project keeps hitting is the opposite one - an event nobody ever sees.
///
/// A constant rather than a bare literal in `Theme::default` so the calibration test asserts against
/// the shipped value rather than a copy of it that could drift.
pub const DEFAULT_FLOURISH: f32 = 0.45;

/// Human-readable name for a family, for the theme menu's submenu titles.
///
/// Falls back to a title-cased version of the raw family id rather than skipping or
/// panicking on an unknown one, so a family added by a TOML file - or a new built-in whose
/// label nobody remembered to add here - still appears in the menu with a readable name.
/// The standing requirement is that themes stay expandable; a lookup that drops unknown
/// families would quietly break that.
pub fn family_label(family: &str) -> String {
    match family {
        "segmented" => "Segmented VFD".into(),
        "scope" => "Oscilloscope".into(),
        "vu" => "VU dials".into(),
        "vapor" => "Vaporwave grid".into(),
        "tube" => "Valve row".into(),
        "nixie" => "Nixie tubes".into(),
        "waterfall" => "Spectrogram".into(),
        "reel" => "Reel-to-reel".into(),
        "patchbay" => "Patchbay".into(),
        "radar" => "Radar".into(),
        "fluid" => "Fluid".into(),
        "pantone" => "Pantone".into(),
        "chroma" => "Chroma field".into(),
        "flame" => "Flame organ".into(),
        other => {
            let mut c = other.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => "Other".into(),
            }
        }
    }
}

/// Material colours for the vacuum-tube row family.
///
/// Separate from the `lit`/`hot`/`panel` trio because a valve is made of several materials
/// that are not variations of one accent colour: glass, bakelite, brass and the dark metal
/// of the plate all have to be independently settable or every colourway ends up looking
/// like the same tube under a different gel.
#[derive(Debug, Clone)]
pub struct TubeParams {
    /// Top of the chassis gradient - the lit edge of the metal.
    pub chassis_top: String,
    pub chassis_bottom: String,
    /// Plate and grid metal, silhouetted against the glow. Dark on purpose.
    pub internals: String,
    /// Bakelite socket the envelope sits in.
    pub socket: String,
    /// Brass collar and pins.
    pub collar: String,
    /// Specular highlight on the envelope.
    pub glass: String,
}

impl Default for TubeParams {
    fn default() -> Self {
        TubeParams {
            chassis_top: "#3c4436".into(),
            chassis_bottom: "#161a12".into(),
            internals: "#0b0d08".into(),
            socket: "#241a10".into(),
            collar: "#8a6a2a".into(),
            glass: "#cfe0d8".into(),
        }
    }
}

/// Scene parameters for the vaporwave grid family.
///
/// Every default here was tuned by the user in a live browser tuner, not chosen by me - see
/// `docs/superpowers/specs/2026-07-31-vaporwave-grid-family-design.md` §1 for the raw
/// output and §7 for which of my own guesses it overrode. The tuner reported percentages
/// and hundredths; these are the same numbers as fractions, so `amp = 101` there is 1.01
/// here.
///
/// These names are a published schema the moment a theme file sets one, so they match the
/// spec exactly and must not be renamed without bumping `schema`.
#[derive(Debug, Clone)]
pub struct VaporParams {
    /// Horizon height as a fraction of panel height.
    pub horizon: f32,
    /// Displacement scale for the terrain.
    pub amp: f32,
    /// Receding horizontal grid lines.
    pub lines: i32,
    /// Converging vertical lines.
    pub verts: i32,
    /// Scroll speed.
    pub scroll: f32,
    /// Depth-spacing exponent. Higher bunches lines toward the horizon.
    pub persp: f32,
    /// Width spread of the near edge.
    pub spread: f32,
    /// Peak-glow brightness.
    pub glow: f32,
    /// Spectral smoothing, 0..1. Higher gives rolling hills over spikes.
    pub smoothing: f32,
    /// Sun radius, as a fraction of the base radius.
    pub sun: f32,
    /// Horizontal slots cut in the sun.
    pub slots: i32,
    /// Slot widening toward the horizon. The user chose ZERO - uniform 1px slots.
    pub slot_bias: f32,
    /// How far down the sun the first slot sits.
    pub slot_top: f32,
    /// Halo strength.
    pub halo: f32,
    /// Gradient warmth; higher is pinker at the horizon.
    pub warmth: f32,
    /// Rise in bass needed to fire a bolt.
    pub bolt_sens: f32,
    pub bolt_bright: f32,
    pub sky_flash: f32,
    pub grid_flash: f32,
    pub bolt_decay: f32,
    /// Hidden-line removal. Off looks like overlapping spaghetti; see the family docs.
    pub occlusion: bool,
    pub crisp: bool,
    pub sun_rim: bool,
    /// True: the grid recedes, and the newest audio lands on the NEAREST lines.
    ///
    /// This is the difference between a terrain that reacts and one that does not, and the two
    /// properties are inseparable. Displacement scales with depth, so the nearest lines move most -
    /// and a line only keeps the spectrum from when it was born. With the grid flowing toward the
    /// viewer, new audio is born at the horizon, where it is rendered at the SMALLEST displacement,
    /// and it then needs a full scroll cycle - about 1.3s - to reach the front where it would be
    /// biggest. Both penalties at once, which reads as a calm grid that lags the music.
    ///
    /// Receding puts the newest spectrum on the nearest, largest lines immediately. The cost is the
    /// direction of travel: set this false for the classic flying-forward look, and accept that the
    /// front of the grid shows what the music did a second ago.
    pub recede: bool,
    pub sky_top: String,
    pub sky_horizon: String,
    pub ground: String,
    pub sun_crown: String,
    pub sun_upper: String,
    pub sun_lower: String,
    pub sun_base: String,
}

impl Default for VaporParams {
    fn default() -> Self {
        VaporParams {
            horizon: 0.48,
            // amp, lines and persp are the three values re-tuned from the tuner's output.
            //
            // The tuner ran in a browser canvas far taller than 60px, and all three crush
            // at this size. Measured at the tuner's amp=1.01/lines=16/persp=2.07: SEVEN of
            // the sixteen lines collapsed onto just two pixel rows (28 and 29), so the far
            // half of the grid was a solid band rather than a receding grid - and that is
            // also why hidden-line removal had no measurable effect, since lines sharing an
            // integer row cannot occlude one another.
            //
            // persp 1.40 is the highest value at which the STATIC grid keeps one row per
            // line at every scroll phase (2.07 manages 88%). lines 12 keeps the smallest
            // static gap at ~0.9px. amp 0.55 puts the peak displacement near 7.6px - about
            // two to three line gaps - where 1.01 gave 13.9px, half the entire 29px ground.
            //
            // The FORMULAS are left exactly as the spec published them, so `amp = 1.01`
            // still means what the spec says it means; only these defaults are adapted, and
            // they remain overridable per colourway from TOML.
            // 0.55 was chosen when band levels were fed in RAW, and real music only spans
            // about 0.15-0.65 of the 0..1 range - so the nearest ridge moved 1.1px to 4.9px on
            // a 29px ground while the static gap between grid lines is already 0.9-3.2px. On a
            // quiet passage the terrain moved less than one line gap, which is a flat grid.
            // Now that the terrain auto-ranges, the loudest band reliably reaches the top of
            // its displacement range, so this sets how much relief that buys: ~14.5px, half the
            // visible ground. 1.50 was measurably denser without reading as more responsive.
            amp: 1.15,
            // 12 lines made the audio unreadable, and the arithmetic says why: at 12 the near-field
            // gap between lines is 3.4px while the peak displacement is 15.8px, so a ridge crosses
            // about five lines and stops being distinguishable from the grid's own density. The
            // terrain became a hatch that jiggles rather than hills that rise. At 9 the ratio is
            // 3.5x and a ridge reads as a ridge.
            //
            // 9 lines / scroll 2.6 / amp 1.15 is the combination the user picked by eye, from four
            // variants compared live in the running app (kept in docs/reference/theme-variants).
            // Do not change these three without offering that comparison again - it is a taste
            // call on a real trade, not a value that can be derived.
            //
            // This costs resolution in the DEPTH axis only. The 64 log frequency bands across each
            // line are untouched, so the spectrum's fidelity is unchanged - it is the number of
            // historical snapshots on screen that drops.
            lines: 9,
            verts: 18,
            // 1.24 gave a near-field flow of 0.50 px/frame, which is below the rate at which motion
            // reads as motion - the grid crawled. It also meant a line was born only every 6.7
            // frames, so the terrain sampled audio at 8.9Hz and roughly 93% of what was on screen
            // was frozen history at any instant. This is the fourth value from the browser tuner
            // that did not survive 60px, after amp, persp and lines; the tuner's canvas was far
            // larger, so the same phase rate looked far faster there.
            scroll: 2.6,
            persp: 1.40,
            spread: 1.50,
            glow: 0.98,
            // 0.65 was a radius-3 moving average over 64 bands, which is a lot of averaging: it
            // was chosen to stop the ridges reading as spaghetti, but it flattens exactly the peaks
            // that make the terrain look like it is reacting. Lowering it SHARPENS the ridges and
            // raises fidelity at the same time, which is the direction asked for.
            smoothing: 0.40,
            sun: 0.83,
            slots: 6,
            slot_bias: 0.0,
            slot_top: 0.18,
            halo: 0.84,
            warmth: 0.63,
            bolt_sens: 0.55,
            bolt_bright: 0.90,
            sky_flash: 0.35,
            grid_flash: 0.60,
            bolt_decay: 0.55,
            occlusion: true,
            crisp: true,
            sun_rim: true,
            recede: true,
            sky_top: "#1a0b2e".into(),
            sky_horizon: "#ff5f93".into(),
            ground: "#12061f".into(),
            sun_crown: "#fff6d0".into(),
            sun_upper: "#ffd76e".into(),
            sun_lower: "#ff9c4a".into(),
            sun_base: "#ff5f93".into(),
        }
    }
}

/// A rainbow colour for an element at horizontal fraction `x01`, or None when the colourway is not
/// a rainbow one.
///
/// Returned as (hue, saturation, value) rather than a colour so `render` can build an `Rgba` with
/// whatever alpha it needs - `themes` deliberately knows nothing about the canvas.
/// How far toward white a plate change pulls at its midpoint.
///
/// 0.65 was chosen as the smallest value that keeps every intermediate colour of every shipped
/// Pantone colourway above the 3:1 contrast floor, verified by
/// `every_hue_of_a_rainbow_colourway_clears_three_to_one_against_its_panel` which sweeps all 360
/// hues. Larger washes the palette out; smaller lets the cyan-to-magenta crossing dip into blue.
const MORPH_DESAT: f32 = 0.65;

pub fn rainbow_hsv(t: &Theme, x01: f32, time_s: f32, hot: bool) -> Option<(f32, f32, f32)> {
    if t.rainbow <= 0.0 {
        return None;
    }
    let x = if x01.is_finite() { x01.clamp(0.0, 1.0) } else { 0.0 };
    let time = if time_s.is_finite() { time_s } else { 0.0 };
    let mut hue = time * t.rainbow + x * t.rainbow_spread;
    // Set by the ink quantisation below when it is mid-plate-change; see `MORPH_DESAT`.
    let mut morph_cross = 0.0f32;

    // INK QUANTISATION. Snaps the hue to one of `inks` evenly spaced steps, so the palette becomes a
    // set of process inks rather than a continuous wheel.
    //
    // This lived nowhere until now. The Pantone family set `inks` on four of its five colourways and
    // its own comment said "`tint` resolves the rainbow (and its ink quantisation)" - but `tint` calls
    // this function, which was written before that family existed and honoured neither `inks` nor
    // `ink_chroma`. Both fields were completely inert, which is exactly why `pantone-process` at three
    // inks was indistinguishable from the continuous `pantone-spectrum`: they were rendering the same
    // colours. Same class of fault as the vaporwave auto-ranger - a documented feature that no code
    // read.
    //
    // Quantising is also what makes full chroma legal: the 3:1 contrast rule is only binding on a
    // CONTINUOUS wheel, because it is the dark blues that fail it. Snap to three inks and the wheel
    // lands on yellow/cyan/magenta, none of which is dark, so `ink_chroma` can go to 1.0.
    if t.inks > 0 {
        let n = t.inks as f32;
        // The HALF-STEP OFFSET is not cosmetic. Without it, N inks land ON the primaries: three gives
        // hues 0/120/240, which is red/green/blue - the ADDITIVE primaries - and pure blue fails the
        // 3:1 contrast rule at 2.34:1. Offset by half a step they straddle instead, and three lands on
        // 60/180/300: yellow, cyan and magenta, the subtractive process set, worst 6.41:1.
        //
        // That is what the Pantone family's own module docs describe, and its stated measurements -
        // 6.41:1 at three inks, 3.22:1 at two, 3.47:1 at six - reproduce exactly with this offset and
        // not at all without it. The formula was correct in the design and simply never reached the
        // code, because the quantisation belongs to this function and this function predates that
        // family.
        let x = hue.rem_euclid(1.0) * n;
        let step = x.floor();
        // `ink_morph` widens the plate change from a single-frame jump into an eased crossfade over
        // the tail of the dwell. At 0 this is exactly the old `(step + 0.5) / n` snap, which is why
        // every non-Pantone colourway is byte-identical.
        let w = if t.ink_morph.is_finite() { t.ink_morph.clamp(0.0, 0.5) } else { 0.0 };
        let mut centre = step + 0.5;
        // How far through a plate change we are, 0 at both ends and 1 in the middle. Drives the
        // desaturation below.
        let mut crossing = 0.0f32;
        if w > 0.0 {
            let frac = x - step;
            let raw = ((frac - (1.0 - w)) / w).clamp(0.0, 1.0);
            // Smoothstep, so the morph leaves and arrives at zero rate. A linear blend still reads
            // as a jolt at each end, which is the thing being fixed.
            centre += raw * raw * (3.0 - 2.0 * raw);
            crossing = 1.0 - (2.0 * raw - 1.0).abs();
        }
        hue = centre / n;
        morph_cross = crossing.clamp(0.0, 1.0);
    }

    // `ink_chroma` rather than RAINBOW_SAT, defaulting to it, so a quantised palette can be fully
    // saturated while a continuous one stays inside the contrast rule.
    let mut sat = if t.ink_chroma.is_finite() { t.ink_chroma.clamp(0.0, 1.0) } else { RAINBOW_SAT };

    // THE MORPH CROSSES THROUGH WHITE, NOT ROUND THE WHEEL, and this is a correctness fix rather
    // than a stylistic one.
    //
    // Easing the HUE between two inks sweeps every hue in between, and between cyan and magenta that
    // means passing through blue. Measured, that is exactly what broke: fluid-pantone at hue 219
    // reached 2.34:1 against its panel, failing the 3:1 rule - which is the very failure the
    // half-step offset exists to avoid, walked straight back in by interpolating between the offsets.
    //
    // Pulling the saturation toward white across the crossing fixes it by construction: the palest
    // point of the transition is also the brightest, so the worst contrast in a morph now occurs at
    // its ENDS, which are the flat inks that were already measured safe. It also happens to be what
    // a plate change looks like in print - the ink lifts off the paper rather than rotating hue.
    if morph_cross > 0.0 {
        sat *= 1.0 - MORPH_DESAT * morph_cross;
    }
    if hot {
        // The hot core keeps the hue but pulls hard toward white, exactly as the fixed colourways do
        // with their own `hot` - otherwise a rainbow loses the sense of a bright centre entirely.
        Some((hue, sat * 0.30, 1.0))
    } else {
        Some((hue, sat, 1.0))
    }
}

/// Structure parameters for the Pantone family; inert for the other families.
///
/// Separate from `lit`/`hot`/`panel` because none of these are colours: they are how much of the
/// panel each of Pantone's recognisable devices gets. The five shipped colourways differ mostly by
/// these four numbers rather than by hue, which is deliberate - the identity of that look is the
/// STRUCTURE (misregistration, halftone, barcode, glitch), not the palette.
#[derive(Debug, Clone)]
pub struct PantoneParams {
    /// Height of the barcode stripe band, as a fraction of the panel interior. 0 removes it.
    pub barcode: f32,
    /// Halftone dot-field strength, 0..1. Above ~0.5 the dots also screen the lit bars, not just
    /// the dormant part of the field.
    pub halftone: f32,
    /// Peak sideways displacement of the glitch slice, in PIXELS at full drive. 0 removes it.
    pub glitch: f32,
    /// The hard diagonal splitting solid ink from screened ink across the bar field.
    pub split: bool,
}

impl Default for PantoneParams {
    fn default() -> Self {
        PantoneParams { barcode: 0.16, halftone: 0.55, glitch: 4.0, split: true }
    }
}

/// Print parameters for the chroma-field family.
///
/// Separate from the `lit`/`hot`/`panel` trio for the same reason `TubeParams` is: this family
/// is not one accent colour on a panel, it is a printed surface, and the surface has parts -
/// a stripe palette, a key plate, a channel misregistration and a halftone screen - that are
/// not variations of each other.
///
/// These names are a published schema the moment a theme file sets one, so they must not be
/// renamed without bumping `schema`.
#[derive(Debug, Clone)]
pub struct ChromaParams {
    /// Target stripe pitch in pixels; the stripe COUNT is derived from it so a wider panel
    /// gets more stripes rather than fatter ones. See `render::chroma::stripe_count` for why
    /// 18 is the measured choice at the 190px reference.
    pub stripe_px: f32,
    /// Extra weight a fully driven stripe carries over an idle one, so the width ratio
    /// between them is `1 + swell`.
    ///
    /// Measured at 4.0, ten stripes, 186px of interior, against an 18.6px rest width: a
    /// shaped spectrum gives 13..25px, alternating loud and quiet groups gives 7..30px, and a
    /// single group driven alone against silence reaches 14..59px. Lower values compress that
    /// - at 1.24 the single-driven case only reaches 37px and the realistic case collapses to
    /// 12..25px, which is a field that moves without the pinch reading as a pinch.
    pub swell: f32,
    /// Stripe chroma, as a fraction of the most sRGB holds at this `lightness` and hue.
    ///
    /// 1.0 - full chroma - is the point of the family. Note what it now means: the MAXIMUM AVAILABLE
    /// at a fixed perceptual lightness, which is a different number for every hue. That is the honest
    /// reading of "full chroma" once the ramp is perceptual, and it is much less than pure #0000ff for
    /// the blues - deliberately, because pure blue is what made the field flicker in brightness and
    /// what forced this family out of the 3:1 contrast rule. See `lightness`.
    pub sat: f32,
    /// Perceptual lightness (OKLab L) the whole hue ramp is held at, 0..1.
    ///
    /// **Holding this constant is the fix for "the colours are not the most pleasing".** The ramp used
    /// to be an HSV sweep at full saturation and value, which is uneven in two measurable ways: HSV hue
    /// steps are not perceptually even, and lightness swings from L* 97 at yellow to L* 32 at blue, so
    /// the field visibly flickered in brightness across its width. A constant-lightness ramp in OKLab
    /// looks deliberate because every stripe carries the same weight and every hue step is the same
    /// size.
    ///
    /// It also earns the contrast rule back. Because luminance no longer depends on hue, the worst case
    /// is the same as the best case, and the family stops needing its `contrast_floor` opt-in - see
    /// `render::chroma::tests::the_perceptual_ramp_holds_its_lightness_and_clears_the_contrast_rule`.
    ///
    /// Lower is deeper and more saturated (more chroma is available low down); higher is lighter and
    /// necessarily softer. This is the main per-colourway character knob.
    pub lightness: f32,
    /// How much of each hue's NATURAL lightness to give back, 0..1.
    ///
    /// A perfectly flat ramp fixes the flicker and introduces one problem of its own: yellow's natural
    /// lightness is near 0.97, so holding it down to a mid value is precisely what makes olive, and the
    /// middle of the ramp went muddy. Visible on the first eyeball sheet at every flat lightness tried.
    ///
    /// 0 is dead flat. 1 restores the full natural variation, which is the HSV flicker back again. A
    /// third or so keeps the field even enough to read as one weight while letting the yellows be
    /// yellow - the same compress-don't-remove trick as any tone curve.
    pub lightness_tilt: f32,
    /// Hue turns spanned left to right across the field, and the hue the left edge starts at.
    /// 0.85 from 0.0 runs red through to violet without wrapping back to red.
    pub hue_span: f32,
    pub hue_offset: f32,
    /// A fixed process palette. Empty means the spectrum hue ramp; a list replaces it, which
    /// is how the CMYK and barcode colourways restrict themselves to real plate colours.
    pub inks: Vec<String>,
    /// Pick each stripe's ink by hash rather than in order. A print's plates cycle; a
    /// barcode's runs do not, and a cycling palette against varying widths reads as a
    /// repeating pattern rather than as a barcode.
    pub scramble: bool,
    /// Draw the loudest stripe in full chroma even when `inks` would have made it grey.
    /// This is how a colourway can withhold chroma almost entirely and still say WHERE the
    /// energy is.
    pub accent: bool,
    /// Horizontal displacement of the red and blue planes from green, in pixels. Opposite
    /// signs give the classic mis-printed page, with a warm fringe on one side of a keyline
    /// and a cool one on the other.
    pub shift_r: i32,
    pub shift_b: i32,
    /// Fraction of the field height the halftone screen covers, anchored at the bottom.
    pub halftone: f32,
    /// Lattice pitch of the halftone dots, in pixels.
    pub halftone_pitch: i32,
    /// Ink strength of the halftone at the deepest end of its ramp.
    pub halftone_strength: f32,
    /// The key plate: keylines and halftone dots. Black in the reference work, and the
    /// keyline is load-bearing rather than decorative - see `contrast_floor`.
    pub ink: String,
    /// Bass rise needed to fire the glitch, 1.0 being the most sensitive. 0.0 disables it.
    pub glitch_sens: f32,
    /// How far the glitched slice is displaced, in pixels.
    pub glitch_px: i32,
}

impl Default for ChromaParams {
    fn default() -> Self {
        ChromaParams {
            stripe_px: 18.0,
            swell: 4.0,
            sat: 1.0,
            lightness: 0.72,
            lightness_tilt: 0.35,
            hue_span: 0.85,
            hue_offset: 0.0,
            inks: Vec::new(),
            scramble: false,
            accent: false,
            // Opposite directions at 2px: 1px is present but reads as anti-aliasing at this
            // size, and beyond 3px on an 18px stripe the fringe starts to outweigh the core.
            shift_r: 2,
            shift_b: -2,
            halftone: 0.45,
            halftone_pitch: 3,
            halftone_strength: 0.85,
            ink: "#000000".into(),
            glitch_sens: 0.55,
            glitch_px: 9,
        }
    }
}

/// Tank, liquid and driver parameters for the fluid family.
///
/// Separate from the `lit`/`hot`/`panel` trio for the reason `TubeParams` is: this family is not
/// one accent colour on a panel, it is a tank of liquid with two drivers in it, and the parts -
/// the body's depth ramp, the meniscus, the specular, the cone materials - are not variations of
/// each other.
///
/// Several of these are deliberately PHYSICAL rather than cosmetic, and that is what makes the
/// five shipped colourways differ structurally instead of by hue: `damping` decides whether a
/// wave reaches the far wall at all (mercury rings, ink dies at the cone), `wave_speed` decides
/// whether the interference pattern is coarse or fine, `surface` decides whether the scene is a
/// deep tank or a shallow film, and `droplets`/`caustics`/`emissive`/`iridescence`/`sheen` each
/// add or remove a whole visual element rather than recolouring one.
///
/// These names are a published schema the moment a theme file sets one, so they must not be
/// renamed without bumping `schema`.
#[derive(Debug, Clone)]
pub struct FluidParams {
    /// Rest height of the liquid surface as a fraction of the panel interior, measured from the
    /// top. Low values give a deep tank with little headroom; high values a shallow film with
    /// room for big crests and long droplet arcs.
    pub surface: f32,
    /// Liquid colour immediately under the surface, and at the tank floor. The ramp between them
    /// is anchored to the FLOOR, not to the moving surface - see the family docs.
    pub body_top: String,
    pub body_deep: String,
    /// The second thin-film interference colour. Only visible where `iridescence` is non-zero, and
    /// the meniscus is mixed toward it by the local surface SLOPE - which is what an oil film
    /// actually does, its colour depending on the angle it is seen at.
    ///
    /// There is deliberately no `meniscus` or `glint` field beside it: the surface line is the
    /// shared `Theme::lit` and the specular is the shared `Theme::hot`, because those are exactly
    /// what those two fields mean everywhere else. Keeping them shared is also what puts this
    /// family's real drawn colours inside
    /// `builtin::tests::every_lit_colour_clears_three_to_one_against_its_own_panel` instead of
    /// leaving them unmeasured.
    pub film: String,
    /// Cone diaphragm, and the fixed basket/motor behind it.
    pub cone: String,
    pub cone_dark: String,
    /// Wave speed multiplier. 1.0 is 120 px/s, i.e. about 1.6s end to end across the 190px tank.
    /// Implemented as the number of fixed sub-steps per second, so it CANNOT affect stability -
    /// see `render::fluid`'s module docs.
    pub wave_speed: f32,
    /// Per-sub-step velocity retention, 0.80..0.9999. This is the viscosity, and it is the single
    /// most structural parameter here: at 0.9990 a wave crosses the tank almost intact and the
    /// interference pattern dominates, at 0.9850 it is down to a fifth by mid-tank, and at 0.90 the
    /// liquid barely moves outside the cone mouths.
    pub damping: f32,
    /// Pixels of surface displacement per unit of cone excursion, at the 56px reference interior
    /// height and scaled with the panel.
    pub surface_gain: f32,
    /// Cone travel at full excursion, as a fraction of the liquid depth.
    pub cone_travel: f32,
    /// How hard the cone forces the columns over its mouth, per sub-step. Below 1 the mouth is
    /// partly transparent to returning waves rather than acting as a second wall.
    pub coupling: f32,
    /// Droplets thrown per transient. 0 for a liquid too viscous to throw any.
    pub droplets: i32,
    /// Droplet launch speed in px/s at the reference height.
    pub droplet_v: f32,
    /// Sub-surface caustic band under a crest.
    pub caustics: bool,
    /// Thin-film colour shift with surface slope, 0..1. See `film`.
    pub iridescence: f32,
    /// Hard specular band immediately below the surface, 0..1 - the liquid-metal horizon.
    pub sheen: f32,
    /// How much the liquid itself EMITS, 0..1. Non-zero puts the top rows of the body onto the
    /// bloomed light layer, so the liquid glows outward instead of merely being brightly coloured.
    pub emissive: f32,
    /// Strength of the underglow that fires from the tank floor on a bass transient, 0 = off.
    ///
    /// Distinct from `emissive`, which is a CONSTANT glow the liquid always has. This is an event:
    /// fast attack on an onset, slow release, so it reads as the tank being lit from beneath on the
    /// hit. Asked for as "flashing underglow on the bass hits, like the flashes in the vaporwave but
    /// more glow than flash" - hence a release measured in most of a second rather than the
    /// vaporwave lightning's few frames.
    pub underglow: f32,
}

impl Default for FluidParams {
    fn default() -> Self {
        FluidParams {
            // 0.42 leaves 23 rows of headroom over 33 rows of liquid at the 190x60 reference:
            // Reviewed by eye at 0.42 and reported as "almost too much fluid": the body filled
            // nearly 60% of the panel, which reads as a full tank rather than a liquid with a
            // surface, and left the crests crowded against the top. 0.50 splits the panel evenly -
            // still enough body to read as a volume, and now enough air that an 8px crest plus a
            // droplet arc has somewhere to go.
            surface: 0.58,
            body_top: "#1d6fa8".into(),
            body_deep: "#04121f".into(),
            film: "#7f5bd6".into(),
            cone: "#22303a".into(),
            cone_dark: "#0a1016".into(),
            wave_speed: 1.0,
            // 0.997 per sub-step is 0.988 per nominal frame. A wave needs ~106 sub-steps to reach
            // mid-tank from a cone at 190px wide, which leaves it at 0.997^106 = 0.73 of its
            // amplitude - so the interference in the middle is genuinely visible. At the 0.985
            // that looked reasonable by eye it arrives at 0.20 and there is nothing to see.
            damping: 0.997,
            // Deep water sits second-highest on the family's amplitude ladder. The six colourways
            // are spaced ~1.5x apart in median surface relief on purpose, because
            // `every_fluid_colourway_renders_and_they_differ_structurally_not_just_in_hue` requires
            // every PAIR to differ by 25% and six values on one bounded axis do not spread
            // themselves. Lowering the water line gave every colourway more headroom and so more
            // relief, which collapsed deep and mercury into each other until the ladder was placed
            // deliberately. After the splash rework the measured ladder is ink 2.01, oil 3.66,
            // pantone 5.24, mercury 7.94, deep 12.46, coolant 16.36px, with steps of 1.82x, 1.43x,
            // 1.52x, 1.57x and 1.31x - every one clear of the 1.25x the pair assertion requires.
            //
            // Every rung is HIGHER than the ladder it replaced (ink 2.0, oil 3.0, pantone 4.6,
            // mercury 6.8, deep 10.2, coolant 15.9), so the surface moves more everywhere. The rework
            // first pushed it much further than that - deep reached 13.9 and coolant flat-topped on
            // 3.2% of column-frames - because each landing droplet punches `SPLASH` into the surface,
            // so more droplets meant bigger waves meant more droplets. `surface_gain` came down here
            // and on oil to absorb that feedback rather than to make the tank calmer. Mercury is
            // deliberately NOT the flattest: a dedicated assertion requires the lossless ringing
            // liquid and the dead viscous one to look different, and putting mercury at the bottom
            // of the ladder broke exactly that.
            surface_gain: 17.5,
            cone_travel: 0.16,
            coupling: 0.22,
            // Raised from 5 on the report that "the small splashes are cool" and there should be
            // more of them. The per-frame cost is bounded elsewhere by MAX_DROPS, so this only
            // changes how many a single crest throws.
            droplets: 18,
            droplet_v: 155.0,
            caustics: true,
            iridescence: 0.0,
            sheen: 0.0,
            emissive: 0.0,
            underglow: 0.85,
        }
    }
}

/// Radar-family structure parameters; inert for the other families.
#[derive(Debug, Clone)]
pub struct RadarParams {
    /// Draw the radar warning receiver - the round scope at the left of the panel.
    ///
    /// On by default, because the scope was asked for and this is how it gets seen. It is a
    /// per-colourway flag rather than a constant so the plain sweep field remains reachable: turning
    /// it off in a `[radar]` table gives back the eight columns of spectrum resolution the scope
    /// costs at 190px, which is a real trade and belongs to whoever is looking at it.
    pub rwr: bool,
    /// Transient strength, 0..1, at which a contact becomes a launch warning.
    ///
    /// The knob for how RARE the launch flash is - the requirement was "fairly rare, just for big
    /// hits, but allow it to be tunable per theme so I can tune later to my taste".
    ///
    /// 0.55 is measured across four tracks captured live from a real Spotify session, not chosen:
    ///
    /// ```text
    ///   track                        @0.30    @0.55    @0.70    @0.85
    ///   Sub Focus - Desire          every 1.0s  1.9s     2.6s    13.2s
    ///   Campbell - Would You          never    never    never    never
    ///   Ely Oaks - Running Around   every 0.9s  1.9s     2.6s     6.6s
    ///   Skepsis - Been Here Before  every 4.4s  6.6s     never    never
    /// ```
    ///
    /// 0.55 is the HIGHEST setting at which three of the four show the effect at all, and "never seen
    /// it" was the actual reported failure - so it is the right side to err on. Raise it toward 0.85 for
    /// only the biggest moments of the most dynamic material; drop it to 0.30 for roughly twice as
    /// often. Above about 0.85 expect flat-mastered tracks never to fire at all, which is legitimate:
    /// they genuinely have no big hits.
    pub launch: f32,
    /// Threat designators the scope annotates ordinary contacts with, low band to high.
    ///
    /// A real US/NATO warning receiver labels each contact with an alphanumeric identifying the
    /// emitter: numerals for the SA-series surface-to-air systems (a `6` is an SA-6, a `10` an
    /// SA-10), letters for the named ones. That convention is what this follows, and the default
    /// below is a plausible mixed threat environment rather than any one aircraft's real table.
    ///
    /// A LIST rather than a constant because it is exactly the kind of thing worth substituting -
    /// swap in a set from a particular airframe, or shorten it so codes repeat less. Index is chosen
    /// from where the transient sat in the low band, so a given emitter always reports the same
    /// designator, which is the entire point of a designator.
    ///
    /// Only characters the 3x5 font has will draw; anything else leaves a gap rather than failing, so
    /// a hand-edited list cannot break the render. `S`, `T` and `N` are not available - see the note
    /// in `canvas::glyph_3x5`.
    pub codes: Vec<String>,
}

impl Default for RadarParams {
    fn default() -> Self {
        RadarParams {
            rwr: true,
            launch: 0.55,
            codes: ["6", "8", "H", "10", "11", "P", "13", "A", "15", "M", "19", "R", "2", "U", "3"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub upto: f32,
    pub lit: String,
    pub hot: String,
}

#[derive(Debug, Clone)]
pub struct Theme {
    // `id`/`name` identify a theme for a future theme-picker UI; nothing
    // reads them yet since only one theme (`vfd_ice`) is ever selected.
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
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
    /// Halo intensity, independent of `bloom` (which is the radius). Kept separate
    /// because the two are not interchangeable: the radius must stay small relative
    /// to the bar pitch or adjacent halos merge at ANY strength, so brightness has
    /// to come from here.
    pub glow_strength: f32,
    /// Strength of the dim halo confined to the display's edge ring, as a fraction
    /// of `glow_strength`.
    pub edge_glow: f32,
    /// Display gain applied to the audio level before it is mapped to the meter.
    ///
    /// Exists because the raw values are far smaller than they look: typical music sits
    /// at an RMS of 0.02-0.12, so feeding that straight to a needle put it at 2-12% of
    /// arc travel and the meter barely moved. Each family applies this on top of its own
    /// sane default mapping, so 1.0 is already usable and this is the knob to reach for
    /// if a meter feels dead.
    pub sensitivity: f32,
    /// Hue cycles per second. 0 disables the rainbow entirely and the fixed `lit`/`hot` are used.
    ///
    /// A rainbow cannot be expressed as a hex string, because it changes every frame and varies
    /// across the display, so it is the one visual property that has to be computed rather than
    /// declared. This is still data - a colourway turns it on - but the colour itself comes from
    /// `Theme::rainbow_at`.
    /// Pixels the red plate is offset (blue goes the other way), i.e. RGB misregistration.
    ///
    /// The single most identifiable element of the Pantone look and what separates it from a
    /// generic rainbow, so it is a shared `[look]` field rather than a family-private one: the
    /// segmented VFD, the oscilloscope and the VU dials all apply it as their last step. 0 is off,
    /// which is what every pre-existing colourway keeps.
    /// Pantone-only structure parameters; inert for the other families.
    pub pantone: PantoneParams,
    /// Chroma-field-only print parameters; inert for the other families.
    pub chroma: ChromaParams,
    /// Radar-only structure parameters; inert for the other families.
    pub radar: RadarParams,
    /// Hue quantisation: 0 for a continuous wheel, N for N evenly spaced ink plates. See
    /// `quantise_hue`.
    pub inks: u32,
    /// Saturation the computed rainbow colour runs at. Defaults to `RAINBOW_SAT`.
    ///
    /// Exists because that 0.68 ceiling is only binding on a CONTINUOUS hue wheel: raise `inks` and
    /// the dark hues that force it are simply not in the palette any more, so full chroma becomes
    /// legal. That is the honest resolution of "Pantone wants maximum chroma but every lit colour
    /// must clear 3:1" - a print process has a handful of inks, not a wheel.
    ///
    /// Raising this on a CONTINUOUS colourway will fail
    /// `every_hue_of_a_rainbow_colourway_clears_three_to_one_against_its_panel`, which is the
    /// intended outcome - see RAINBOW_SAT for the measurements.
    ///
    /// Named `ink_chroma` rather than `chroma`: two families built in parallel both chose a Theme
    /// field called `chroma` for different things, and a bare one beside `chroma: ChromaParams`
    /// would be a trap for the next reader.
    pub ink_chroma: f32,
    /// Fraction of each ink's dwell spent MORPHING into the next one, 0.0..0.5.
    ///
    /// 0.0 snaps, which is what ink quantisation did when it was first wired up, and it was reviewed
    /// as "random switching" and "hard jolting": a plate change was a single-frame jump between two
    /// fully saturated process colours with nothing leading into it, so it read as a glitch rather
    /// than as a decision. 0.35 keeps roughly two thirds of the dwell as a flat ink - which is the
    /// whole point of a process palette - and eases through the remainder.
    pub ink_morph: f32,
    pub aberration: f32,
    /// Minimum contrast ratio this colourway's lit colours must clear against its own panel.
    ///
    /// 3.0 is the project rule and the default, so no existing colourway changes. It is a
    /// per-colourway field rather than a constant because ONE family cannot meet it honestly:
    /// the chroma field runs at maximum chroma by design, and a full-chroma rainbow provably
    /// cannot clear 3:1 at every hue against any flat panel - measured, pure blue reaches only
    /// 2.36:1 on a near-black panel, and on a light panel yellow drops to 1.00:1. That family
    /// carries its legibility in 1px black keylines around every stripe instead, so its
    /// colourways declare the floor they actually measure at.
    ///
    /// Lowering this is therefore an explicit, recorded decision per colourway, not a way out
    /// of the rule: `builtin::tests::only_the_recorded_colourways_lower_the_contrast_floor`
    /// names the ones allowed to, and
    /// `render::chroma::tests::every_stripe_colour_clears_its_own_colourways_declared_contrast_floor`
    /// additionally requires the declared value to be TIGHT against what is measured - so a
    /// deliberate 2.3:1 passes and is recorded while an accidental 1.2:1 still fails.
    pub contrast_floor: f32,
    /// How often this colourway's flourish fires. 0 is off. See `DEFAULT_FLOURISH`.
    ///
    /// A shared `[look]` field rather than a per-family one because every family has exactly one
    /// flourish and the knob means the same thing for all of them: how rare the rare thing is.
    pub flourish: f32,
    pub rainbow: f32,
    /// Hue turns spanned across the width of the display, so the rainbow is a WAVE and not one
    /// flat colour shifting.
    ///
    /// 0 gives the "spectrum cycle" a keyboard does when the whole board changes together; ~0.8
    /// gives the "rainbow wave", where hue also varies by position. On a spectrum analyser that
    /// second one doubles as a frequency legend, which is why it is the default.
    pub rainbow_spread: f32,
    /// Vaporwave-only scene parameters; inert for the other families.
    pub vapor: VaporParams,
    /// Tube-row-only material colours; inert for the other families.
    /// Fluid-tank-only parameters; inert for the other families.
    pub fluid: FluidParams,
    pub tube: TubeParams,
    // Cross-fade duration for switching themes at runtime (Task 11+); the
    // segmented renderer draws every frame from scratch and has no
    // transition state to feed it yet.
    #[allow(dead_code)]
    pub fade: f32,
    pub texture: Texture,
    pub ballistics: Ballistics,
    pub zones: Vec<Zone>,
    /// (trail colour, trail fade) - scope family only, models a dual-layer phosphor.
    #[allow(dead_code)]
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
            // FULLY opaque. Not a preference: the panel must occlude the Widgets
            // button's own icon and text. 0.55 left the weather plainly legible; even
            // 0.96 transmitted 4% of it, which is invisible against a lit bar but
            // clearly visible against the dark segment gaps and the dormant grid.
            panel_alpha: 1.0,
            edge: "#96e1ff".into(),
            edge_alpha: 0.13,
            ghost: 0.11,
            // Radius, NOT intensity. Must stay small relative to the bar pitch or
            // adjacent halos merge into one wash behind the segments.
            bloom: 4.0,
            glow_strength: 0.35,
            // 0.30 was not merely too dim, it was NEGATIVE - the ring measured darker
            // than the panel it sat on. 4.0 puts it ~60 luminance above.
            edge_glow: 4.0,
            sensitivity: 1.0,
            pantone: PantoneParams::default(),
            chroma: ChromaParams::default(),
            radar: RadarParams::default(),
inks: 0,
            ink_chroma: RAINBOW_SAT,
            // 0.0 by default so every existing colourway keeps its exact colours; the Pantone ones
            // opt in.
            ink_morph: 0.0,
            aberration: 0.0,
            contrast_floor: 3.0,
            flourish: DEFAULT_FLOURISH,
            rainbow: 0.0,
            rainbow_spread: 0.8,
            vapor: VaporParams::default(),
            fluid: FluidParams::default(),
            tube: TubeParams::default(),
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

    /// The printed overload arc. Red on every dial except the red one, where it
    /// goes white because red-on-red is illegible.
    pub fn overload_hex(&self) -> &str {
        if self.id == "vu-red" {
            "#ffffff"
        } else {
            "#ff5a46"
        }
    }
}

/// After a hot reload, decide which theme should now be selected. If `wanted`
/// still exists in `themes`, keep it - a hot reload of the theme currently on
/// screen must not silently switch anything. Otherwise fall back to the first
/// available theme rather than leaving the app pointing at an id that no
/// longer exists, which happens when the user deletes the file for the theme
/// that was selected.
pub fn reconcile_reload(themes: &[Theme], wanted: &str) -> Theme {
    themes
        .iter()
        .find(|t| t.id == wanted)
        .or_else(|| themes.first())
        .cloned()
        .unwrap_or_default()
}

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
mod reconcile_reload_tests {
    use super::*;

    fn theme(id: &str) -> Theme {
        Theme { id: id.into(), name: id.into(), ..Theme::default() }
    }

    #[test]
    fn keeps_the_current_theme_when_it_still_exists() {
        // `wanted` is deliberately NOT the first entry, so a stub that always
        // returns `themes.first()` cannot pass this by accident.
        let themes = vec![theme("a"), theme("b"), theme("c")];
        let picked = reconcile_reload(&themes, "b");
        assert_eq!(picked.id, "b", "the surviving theme must be kept, not swapped");
    }

    #[test]
    fn falls_back_to_the_first_theme_when_the_selected_one_is_gone() {
        let themes = vec![theme("a"), theme("b")];
        let picked = reconcile_reload(&themes, "deleted-by-the-user");
        assert_eq!(picked.id, "a", "a vanished theme must fall back rather than pointing at nothing");
    }

    #[test]
    fn an_empty_registry_falls_back_to_the_default_theme_rather_than_panicking() {
        // Defensive only: `registry()` always includes the built-ins, so this
        // is never hit in practice, but reconcile_reload must not assume it.
        let picked = reconcile_reload(&[], "anything");
        assert_eq!(picked.id, Theme::default().id);
    }
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

    /// The test above drives the merge logic directly against `tests/themes`,
    /// bypassing `registry()`'s own `Config::dir().join("themes")` lookup entirely.
    /// This one exercises `registry()` itself against the real, environment-derived
    /// directory - self-restoring, like `config::tests`' real-filesystem cases.
    #[test]
    fn registry_reads_the_real_appdata_themes_directory() {
        let dir = crate::config::Config::dir().join("themes");
        std::fs::create_dir_all(&dir).expect("themes dir should be creatable");
        let marker = dir.join("__registry_test_marker.toml");
        std::fs::write(
            &marker,
            "schema = 1\nid = \"__registry-test-marker\"\nname = \"Marker\"\nfamily = \"segmented\"",
        )
        .expect("writing the marker file should succeed");

        let (themes, warnings) = registry();

        std::fs::remove_file(&marker).ok();

        assert!(warnings.is_empty(), "a valid marker file should not warn: {warnings:?}");
        assert!(
            themes.iter().any(|t| t.id == "__registry-test-marker"),
            "registry() must pick up a real file from %APPDATA%\\taskbar-eq\\themes"
        );
    }
}
