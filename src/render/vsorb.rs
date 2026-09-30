//! The Virtual Self "orb" family: a low-poly chrome sphere the bass inflates, ringed by a spectrum meter.
//!
//! The third sibling of the Virtual Self set - the same early-2000s chrome-and-cherub world as `vswings`,
//! but its emblem rather than its plumage: a wireframe icosphere spinning at the panel centre, shaded like
//! a chrome ball (a bright sky reflected in its top, a darker ground in its bottom - the classic two-band
//! chrome map). The RADIUS is the bass, so a kick swells the whole orb; twenty-four SPIKES stand around
//! the rim, one per pair of bands, and THOSE are the readable meter - a ring of needles whose lengths are
//! the spectrum. The orb answers the low end, the spikes answer everything.
//!
//! # Why the spikes are the meter and the orb is the bass
//!
//! A spike is a bar drawn radially: reading the ring of spikes is reading a bar row bent into a circle,
//! low bands at one angle running round to high bands at the next, so a glance still resolves "where is
//! the energy". The orb itself carries only ONE number - the bass energy, as its size - which is why it
//! can afford to be the ornament: it is a single fat bar the eye reads as a pulse, and the fine reading
//! lives in the needles around it. Bass at the orb means the loudest, most rhythmic part of most music
//! drives the biggest, most central shape - the one the eye lands on first.
//!
//! # The panel is opaque, and the background is a Y2K gradient
//!
//! Like every other family this one paints an OPAQUE panel that covers the Windows weather widget while
//! music plays - a see-through family would show the forecast through the orb. The background is a
//! vertical gradient from `panel` to `edge` (an ice gradient for chrome, black for mono) with three soft
//! lens-flare discs drifting across it on sine paths, drawn in `hot` at `ghost` alpha. The orb, its
//! spikes and the shatter are all drawn over that, and a final clip keeps everything off the rounded
//! corners.
//!
//! # The chrome, and where the bloom goes
//!
//! Each edge is shaded by its screen-space vertical normal: an edge whose midpoint faces UP takes the
//! sky colour (`hot` shading to `lit` at the horizon), one facing DOWN the ground colour (`lit` shading
//! to `edge`). Front-facing edges are near-opaque and back-facing ones faint, so the sphere reads as a
//! sphere without a depth sort - alpha carries the depth. The orb, spikes and shards are drawn on their
//! own transparent layer, bloomed there and composited over the opaque panel, which is the exact idiom
//! `Canvas::bloom`/`draw_over` document for a halo over an opaque field - bloom applied to the panel
//! itself would show nothing, since the halo blends UNDER the opaque fill.
//!
//! # The flourish: the orb shatters
//!
//! On a rare, exceptional hit the orb bursts into its triangles, which fly outward and fade over 700ms
//! while a fresh orb grows from zero at the centre. The shatter is seeded from a constant so it is
//! deterministic frame to frame, and the shard buffer is preallocated to the triangle count so the
//! flourish allocates nothing.
//!
//! # Colour goes through linear light, and no rainbow
//!
//! The chrome map and the shard fade are mixed through the shared linear-light blend, never by averaging
//! sRGB bytes. There is no rainbow colourway: the identity is chrome, cobalt/electric, a pink angel and a
//! mono wireframe - a restrained Y2K palette a hue wheel would fight.

use std::collections::{HashMap, HashSet};

use crate::dsp::bands::NUM_BANDS;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Vertices after one subdivision of an icosahedron: 12 + 30 edge midpoints.
const N_VERTS: usize = 42;

/// Spikes around the rim, one per pair-and-a-bit of the 64 bands.
const SPIKES: usize = 24;

/// The fixed tilt of the orb toward the viewer, in degrees about the screen X axis - enough to open the
/// top of the sphere so the chrome sky/ground split reads, without flattening it into a disc.
const TILT_DEG: f32 = 20.0;

/// How long the shatter takes to fade, in milliseconds - the spec's 700ms.
const SHATTER_MS: f32 = 700.0;

/// The onset net that speeds the spin. The permissive flux net, so the orb spins up on the beat without
/// needing its own calibration; the flourish's own trigger is separate and far rarer.
const SPIN_ONSET_RATIO: f32 = 2.8;
const SPIN_ONSET_REFRACTORY_MS: f32 = 120.0;

/// How fast the spin-onset excitement decays, in milliseconds.
const ONSET_DECAY_MS: f32 = 250.0;

/// Radius as a fraction of panel height: `MIN` at silence (40% of height as diameter) rising by `GAIN`
/// times the bass (to 95% as diameter at full bass). See the spec.
const RADIUS_MIN: f32 = 0.20;
const RADIUS_GAIN: f32 = 0.275;

/// Spike length: a stub plus level times this fraction of `min(w, h*3)`, so a spike grows with the panel
/// and never runs the full height at 48px.
const SPIKE_BASE: f32 = 3.0;
const SPIKE_SPAN_FRAC: f32 = 0.12;

/// The panel is at least this large before the family draws - smaller, and it sheds rather than smudging,
/// exactly as the other families do.
const MIN_W: i32 = 60;
const MIN_H: i32 = 18;

/// One drifting lens-flare disc on the background: a base position (as a fraction of the interior), a
/// sine drift amplitude/phase/rate, and a radius (as a fraction of height). Fixed at construction so the
/// background is deterministic for a given `time_s`, which is what the test hook depends on.
#[derive(Clone, Copy)]
struct Flare {
    bx: f32,
    by: f32,
    amp: f32,
    speed: f32,
    phase: f32,
    r: f32,
}

/// One shattered triangle: its three projected screen vertices at the moment of the burst, and the unit
/// direction it flies. Preallocated to the triangle count; filled in place on a flourish.
#[derive(Clone, Copy, Default)]
struct Shard {
    pts: [[f32; 2]; 3],
    dir: [f32; 2],
}

/// The `(w, h, panel, edge, hot, panel_alpha, ghost, disc positions)` the cached fully-composed
/// background (`Vsorb::bg_final`) was built for - see `draw_background`.
type BgFinalKey = (i32, i32, String, String, String, f32, f32, [(i32, i32); 3]);

pub struct Vsorb {
    /// Unit-sphere vertices, built once at construction.
    verts: Vec<[f32; 3]>,
    /// Wireframe edges as vertex-index pairs, deduped at construction.
    edges: Vec<(u16, u16)>,
    /// Triangles, for the shatter, built at construction.
    tris: Vec<[u16; 3]>,
    /// Current spin angle about Y, in radians.
    rot: f32,
    /// Smoothed radius in pixels - the bass reading. Regrows from zero after a shatter.
    radius: f32,
    /// Last frame's panel centre, so the test accessors agree with what was drawn.
    cx: f32,
    cy: f32,
    /// Smoothed spike lengths in pixels, one per spike.
    spike_len: [f32; SPIKES],
    /// The spin onset - a local detector, because the orb spins up far more often than it shatters.
    onset: crate::dsp::onset::Flux,
    /// Decaying 0..1 excitement from the last onset, driving the spin speed.
    onset_strength: f32,
    /// Fires the shatter on a rare, exceptional hit - see `dsp::flourish`.
    flourish: crate::dsp::flourish::Trigger,
    /// The shatter's one-shot decay envelope.
    shatter: crate::dsp::flourish::Envelope,
    /// The shard buffer, preallocated to `tris.len()`.
    shards: Vec<Shard>,
    /// Whether a shatter is currently flying.
    shards_live: bool,
    /// The three drifting background discs.
    flares: [Flare; 3],
    /// The transparent layer the orb is drawn and bloomed on before compositing over the opaque panel,
    /// sized once per panel size and reused - so `draw` allocates nothing after the first frame.
    scratch: Option<Canvas>,
    /// The cached opaque panel fill + vertical gradient - everything about the background except the
    /// drifting discs, which move every frame and are drawn fresh. Constant for a given (size, theme),
    /// so recomputing it every frame was pure waste: rebuilt only when `bg_key` no longer matches.
    bg_cache: Option<Canvas>,
    /// The `(w, h, panel, edge, panel_alpha)` the current `bg_cache` was built for.
    bg_key: Option<(i32, i32, String, String, f32)>,
    /// The fully composed background - `bg_cache` plus the three discs drawn at their
    /// last-rebuilt-for integer pixel positions. The discs drift by a small fraction of a pixel per
    /// frame (see `ensure_bg_final`), so this is a cache hit almost every frame; when it misses, it is
    /// rebuilt from `bg_cache` rather than redrawing the gradient too.
    bg_final: Option<Canvas>,
    /// The `(w, h, panel, edge, hot, panel_alpha, ghost, disc positions)` `bg_final` was built for.
    bg_final_key: Option<BgFinalKey>,
    /// Scratch canvas sized to the orb/spikes' worst-case (bass = 1.0) bounding box padded by two
    /// bloom radii, so `bloom` can run its box blur over that box instead of the whole (much wider)
    /// panel. Fixed size for a given panel size - built from the RADIUS formula's maximum, not the
    /// current `self.radius`, which drifts every frame with bass - so `Canvas::new` fires only on an
    /// actual resize, never mid-playback.
    bloom_scratch: Option<Canvas>,
    /// splitmix64 state, seeded constant, for the shatter's jitter.
    rng: u64,
}

impl Default for Vsorb {
    fn default() -> Self {
        let (verts, edges, tris) = icosphere();
        let shards = vec![Shard::default(); tris.len()];
        Vsorb {
            verts,
            edges,
            tris,
            rot: 0.0,
            radius: 0.0,
            cx: 0.0,
            cy: 0.0,
            spike_len: [SPIKE_BASE; SPIKES],
            onset: Default::default(),
            onset_strength: 0.0,
            flourish: Default::default(),
            shatter: Default::default(),
            shards,
            shards_live: false,
            flares: [
                Flare { bx: 0.26, by: 0.34, amp: 0.06, speed: 0.5, phase: 0.0, r: 0.55 },
                Flare { bx: 0.72, by: 0.62, amp: 0.08, speed: 0.35, phase: 2.1, r: 0.72 },
                Flare { bx: 0.52, by: 0.24, amp: 0.05, speed: 0.7, phase: 4.2, r: 0.42 },
            ],
            scratch: None,
            bg_cache: None,
            bg_key: None,
            bg_final: None,
            bg_final_key: None,
            bloom_scratch: None,
            rng: 0x243f_6a88_85a3_08d3,
        }
    }
}

/// Normalises a 3-vector onto the unit sphere in place; a zero vector is left alone.
fn normalize(v: &mut [f32; 3]) {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 1.0e-6 {
        v[0] /= len;
        v[1] /= len;
        v[2] /= len;
    }
}

/// The built mesh: unit-sphere vertices, deduped edges as index pairs, and triangles as index triples.
type Mesh = (Vec<[f32; 3]>, Vec<(u16, u16)>, Vec<[u16; 3]>);

/// Builds the icosphere: an icosahedron (12 verts, 30 edges, 20 tris) subdivided once into 42 verts, 120
/// edges and 80 tris, with midpoints deduped so a shared edge yields ONE new vertex rather than two. All
/// vertices are on the unit sphere. Built once at construction - the `HashMap`/`HashSet` here never run
/// in `draw`.
fn icosphere() -> Mesh {
    let t = (1.0 + 5.0f32.sqrt()) / 2.0;
    let mut verts: Vec<[f32; 3]> = vec![
        [-1.0, t, 0.0], [1.0, t, 0.0], [-1.0, -t, 0.0], [1.0, -t, 0.0],
        [0.0, -1.0, t], [0.0, 1.0, t], [0.0, -1.0, -t], [0.0, 1.0, -t],
        [t, 0.0, -1.0], [t, 0.0, 1.0], [-t, 0.0, -1.0], [-t, 0.0, 1.0],
    ];
    for v in &mut verts {
        normalize(v);
    }
    let faces: [[u16; 3]; 20] = [
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];

    /// The midpoint of an edge, normalised, created once and cached by its (low, high) endpoint pair.
    fn midpoint(
        verts: &mut Vec<[f32; 3]>,
        cache: &mut HashMap<(u16, u16), u16>,
        a: u16,
        b: u16,
    ) -> u16 {
        let key = if a < b { (a, b) } else { (b, a) };
        if let Some(&m) = cache.get(&key) {
            return m;
        }
        let (va, vb) = (verts[a as usize], verts[b as usize]);
        let mut m = [(va[0] + vb[0]) * 0.5, (va[1] + vb[1]) * 0.5, (va[2] + vb[2]) * 0.5];
        normalize(&mut m);
        let idx = verts.len() as u16;
        verts.push(m);
        cache.insert(key, idx);
        idx
    }

    let mut cache: HashMap<(u16, u16), u16> = HashMap::new();
    let mut tris: Vec<[u16; 3]> = Vec::with_capacity(80);
    for f in faces {
        let a = midpoint(&mut verts, &mut cache, f[0], f[1]);
        let b = midpoint(&mut verts, &mut cache, f[1], f[2]);
        let c = midpoint(&mut verts, &mut cache, f[2], f[0]);
        tris.push([f[0], a, c]);
        tris.push([f[1], b, a]);
        tris.push([f[2], c, b]);
        tris.push([a, b, c]);
    }

    let mut set: HashSet<(u16, u16)> = HashSet::new();
    for tr in &tris {
        for &(x, y) in &[(tr[0], tr[1]), (tr[1], tr[2]), (tr[2], tr[0])] {
            set.insert(if x < y { (x, y) } else { (y, x) });
        }
    }
    let mut edges: Vec<(u16, u16)> = set.into_iter().collect();
    edges.sort_unstable(); // deterministic order, so dumps reproduce frame to frame
    (verts, edges, tris)
}

/// Rotates a vertex about Y by `rot` then tilts it about X by `tilt`, all in radians. Returns the rotated
/// `(x, y, z)`; `+y` is up and `+z` is toward the viewer.
fn rotate(v: [f32; 3], rot: f32, tilt: f32) -> (f32, f32, f32) {
    let (s, c) = rot.sin_cos();
    let x1 = v[0] * c + v[2] * s;
    let z1 = -v[0] * s + v[2] * c;
    let y1 = v[1];
    let (st, ct) = tilt.sin_cos();
    let y2 = y1 * ct - z1 * st;
    let z2 = y1 * st + z1 * ct;
    (x1, y2, z2)
}

impl Vsorb {
    /// One draw of splitmix64, for the shatter's outward-velocity jitter.
    fn next_rng(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A jitter in -0.5..0.5 from the rng.
    fn jitter(&mut self) -> f32 {
        (self.next_rng() % 1000) as f32 / 1000.0 - 0.5
    }

    /// The smoothed radius in pixels - the bass reading. For the level test.
    #[cfg(test)]
    pub fn radius_px(&self) -> f32 {
        self.radius
    }

    /// The smoothed length of spike `k` in pixels. For the spectrum test, so it can assert the spike
    /// carrying a lit band is far longer than one carrying a silent band - which is what makes the test
    /// discriminate a wrong band mapping rather than merely that SOME spike was drawn.
    #[cfg(test)]
    pub fn spike_len_for_test(&self, k: usize) -> f32 {
        self.spike_len[k.min(SPIKES - 1)]
    }

    /// The clamped screen tip of spike `k`, computed from the same state `draw` drew it with, so the two
    /// agree. For the spectrum test.
    #[cfg(test)]
    pub fn spike_tip_for_test(&self, k: usize) -> (i32, i32) {
        let ang = k as f32 * (std::f32::consts::TAU / SPIKES as f32);
        let (si, co) = ang.sin_cos();
        let reach = self.radius + self.spike_len[k.min(SPIKES - 1)];
        let tx = self.cx + reach * co;
        let ty = self.cy + reach * si;
        (tx.round() as i32, ty.round() as i32)
    }

    /// Rebuilds `bg_cache` - the opaque rounded panel fill plus the vertical gradient, exactly what
    /// `draw` used to redraw from scratch every frame - only when the size or the theme's panel/edge
    /// colours have actually changed. Comparing against `bg_key` by reference first means a cache HIT
    /// (the overwhelming majority of frames) touches no allocation at all; only a rebuild clones the
    /// two colour strings into the new key.
    fn ensure_bg_cache(&mut self, t: &Theme, w: i32, h: i32) {
        let hit = matches!(
            &self.bg_key,
            Some((kw, kh, kp, ke, kpa))
                if *kw == w && *kh == h && kp == &t.panel && ke == &t.edge && *kpa == t.panel_alpha
        );
        if hit {
            return;
        }
        let mut bg = Canvas::new(w, h);
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        bg.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        let top = Rgba::from_hex(&t.panel, 1.0);
        let bot = Rgba::from_hex(&t.edge, 1.0);
        bg.vertical_gradient(2, 2, w - 4, h - 4, &[(0.0, top), (1.0, bot)], false);
        self.bg_cache = Some(bg);
        self.bg_key = Some((w, h, t.panel.clone(), t.edge.clone(), t.panel_alpha));
    }

    /// The background: the cached opaque panel + vertical gradient plus the three lens-flare discs
    /// drifting across it in `hot` at `ghost` alpha, blitted in as one cheap opaque copy - see
    /// `ensure_bg_cache` and the disc-position cache key below. Deterministic for a given `time_s`.
    /// Drawn straight onto the opaque panel `c` - not on the bloom layer - so the discs stay soft and
    /// un-haloed.
    fn draw_background(&mut self, c: &mut Canvas, t: &Theme, time_s: f32, bbox: (i32, i32, i32, i32)) {
        let (ix0, iy0, ix1, iy1) = bbox;
        let (iw, ih) = (ix1 - ix0, iy1 - iy0);
        let (w, h) = (c.width(), c.height());
        self.ensure_bg_cache(t, w, h);

        let ghost = t.ghost.clamp(0.0, 1.0);
        if ghost <= 0.004 {
            if let Some(bg) = &self.bg_cache {
                c.draw_over(bg);
            }
            return;
        }

        // The discs' true (sub-pixel) centres, then rounded to the pixel they'd actually be drawn
        // at. `radial_gradient` samples in units of whole pixels anyway, so nothing is lost rounding
        // BEFORE the cache-key comparison below - it just means the rebuild only fires once the true
        // position has drifted a full pixel from the last redraw, rather than every frame for a
        // sub-pixel wobble nothing would render differently for anyway.
        let time = if time_s.is_finite() { time_s } else { 0.0 };
        let mut pos = [(0i32, 0i32); 3];
        for (i, f) in self.flares.iter().enumerate() {
            let dx = ix0 as f32 + (f.bx + f.amp * (time * f.speed + f.phase).sin()) * iw as f32;
            let dy = iy0 as f32 + (f.by + f.amp * (time * f.speed * 0.8 + f.phase).cos()) * ih as f32;
            pos[i] = (dx.round() as i32, dy.round() as i32);
        }

        let hit = matches!(
            &self.bg_final_key,
            Some((kw, kh, kp, ke, khot, kpa, kg, kpos))
                if *kw == w && *kh == h && kp == &t.panel && ke == &t.edge && khot == &t.hot
                    && *kpa == t.panel_alpha && *kg == ghost && *kpos == pos
        );
        if !hit {
            let mut bg = self
                .bg_final
                .take()
                .filter(|b| b.width() == w && b.height() == h)
                .unwrap_or_else(|| Canvas::new(w, h));
            if let Some(base) = &self.bg_cache {
                bg.copy_region(base, (0, 0), (0, 0), w, h);
            }
            let disc = Rgba::from_hex(&t.hot, ghost);
            let disc0 = Rgba::from_hex(&t.hot, 0.0);
            for (i, f) in self.flares.iter().enumerate() {
                let r = (f.r * ih as f32).round() as i32;
                bg.radial_gradient(pos[i].0, pos[i].1, 0, r, &[(0.0, disc), (1.0, disc0)]);
            }
            self.bg_final = Some(bg);
            self.bg_final_key =
                Some((w, h, t.panel.clone(), t.edge.clone(), t.hot.clone(), t.panel_alpha, ghost, pos));
        }
        if let Some(bg) = &self.bg_final {
            c.draw_over(bg);
        }
    }
}

impl Family for Vsorb {
    fn id(&self) -> &'static str {
        "vsorb"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        if w < MIN_W || h < MIN_H {
            return; // shed rather than smudge
        }
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 250.0) } else { 16.7 };

        // ---- the opaque panel and the Y2K background - the cached blit plus fresh discs ----
        let bbox = (2i32, 2i32, w - 2, h - 2);
        self.draw_background(c, t, d.time_s, bbox);

        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        self.cx = cx;
        self.cy = cy;

        // ---- radius: the bass, smoothed ----
        let mut bass = 0.0f32;
        for &v in &d.levels[0..8] {
            if v.is_finite() {
                bass += v.clamp(0.0, 1.0);
            }
        }
        bass /= 8.0;
        let target = h as f32 * (RADIUS_MIN + RADIUS_GAIN * bass);
        // `d.levels` has already been through `Smoother::new(theme.ballistics)`
        // upstream in `Ticker::tick` - re-smoothing the same bands with the SAME ballistics here is
        // a second low-pass pass, not a second effect. The radius tracks the already-smoothed bass
        // directly.
        if target.is_finite() {
            self.radius = target;
        }

        // ---- spin: base rate plus a decaying kick on each onset ----
        if self.onset.update(&d.levels, dt, SPIN_ONSET_RATIO, SPIN_ONSET_REFRACTORY_MS) {
            self.onset_strength = 1.0;
        }
        self.onset_strength = (self.onset_strength - dt / ONSET_DECAY_MS).max(0.0);
        self.rot += (dt / 1000.0) * (0.3 + 3.0 * self.onset_strength);
        if !self.rot.is_finite() {
            self.rot = 0.0;
        }
        let rot = self.rot.rem_euclid(std::f32::consts::TAU);
        let tilt = TILT_DEG.to_radians();

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);

        // ---- project every vertex once ----
        let mut px = [0.0f32; N_VERTS];
        let mut py = [0.0f32; N_VERTS];
        let mut pz = [0.0f32; N_VERTS];
        let mut pny = [0.0f32; N_VERTS];
        for (i, v) in self.verts.iter().enumerate() {
            let (rx, ry, rz) = rotate(*v, rot, tilt);
            px[i] = cx + self.radius * rx;
            py[i] = cy - self.radius * ry;
            pz[i] = rz;
            pny[i] = ry;
        }

        // ---- the transparent bloom layer the orb, spikes and shards are drawn on ----
        let mut layer = self
            .scratch
            .take()
            .filter(|s| s.width() == w && s.height() == h)
            .unwrap_or_else(|| Canvas::new(w, h));
        layer.clear();

        // ---- the wireframe, chrome-shaded by each edge's screen-space vertical normal ----
        for &(a, b) in &self.edges {
            let (a, b) = (a as usize, b as usize);
            let front = (pz[a] + pz[b]) * 0.5 > 0.0;
            let ny = (pny[a] + pny[b]) * 0.5;
            let base = if ny > 0.0 {
                // Sky: hot at the top of the sphere shading to lit at the horizon.
                Rgba::lerp_linear(hot, lit, 1.0 - ny)
            } else {
                // Ground: lit at the horizon shading to edge at the bottom.
                Rgba::lerp_linear(lit, edge, -ny)
            };
            let alpha: f32 = if front { 0.9 } else { 0.35 };
            let col = Rgba::new(base.r, base.g, base.b, (alpha * 255.0).round() as u8);
            layer.line(px[a].round() as i32, py[a].round() as i32, px[b].round() as i32, py[b].round() as i32, col);
        }

        // ---- the spikes: the readable meter, a ring of needles round the rim ----
        let spike_span = (w.min(h * 3) as f32) * SPIKE_SPAN_FRAC;
        for k in 0..SPIKES {
            let start = ((k as f32 * NUM_BANDS as f32 / SPIKES as f32) as usize).min(NUM_BANDS - 1);
            let end = (start + 3).min(NUM_BANDS);
            let mut lvl = 0.0f32;
            let mut n = 0u32;
            for &v in &d.levels[start..end] {
                if v.is_finite() {
                    lvl += v.clamp(0.0, 1.0);
                    n += 1;
                }
            }
            lvl = if n > 0 { lvl / n as f32 } else { 0.0 };
            let tgt = SPIKE_BASE + lvl * spike_span;
            // same double-smoothing as the radius above - `d.levels` is already
            // smoothed with `t.ballistics` upstream.
            self.spike_len[k] = if tgt.is_finite() { tgt } else { SPIKE_BASE };

            let ang = k as f32 * (std::f32::consts::TAU / SPIKES as f32);
            let (si, co) = ang.sin_cos();
            let rim = (cx + self.radius * co, cy + self.radius * si);
            // Clamp the tip into the canvas so a long spike at a narrow size never runs off it.
            let tx = (cx + (self.radius + self.spike_len[k]) * co).clamp(0.0, (w - 1) as f32);
            let ty = (cy + (self.radius + self.spike_len[k]) * si).clamp(0.0, (h - 1) as f32);
            // Two parallel 1px lines make a 2px needle; the perpendicular offset gives it its width.
            let (perp_x, perp_y) = (-si.round() as i32, co.round() as i32);
            layer.line(rim.0.round() as i32, rim.1.round() as i32, tx.round() as i32, ty.round() as i32, hot);
            layer.line(
                rim.0.round() as i32 + perp_x,
                rim.1.round() as i32 + perp_y,
                tx.round() as i32 + perp_x,
                ty.round() as i32 + perp_y,
                hot,
            );
        }

        // ---- the flourish: shatter the orb, and regrow one from zero ----
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let env = self.shatter.update(fired, dt, SHATTER_MS);
        if fired {
            // Snapshot each triangle at its current projection, with an outward jittered velocity.
            for i in 0..self.tris.len() {
                let tr = self.tris[i];
                let p: [[f32; 2]; 3] = [
                    [px[tr[0] as usize], py[tr[0] as usize]],
                    [px[tr[1] as usize], py[tr[1] as usize]],
                    [px[tr[2] as usize], py[tr[2] as usize]],
                ];
                let cenx = (p[0][0] + p[1][0] + p[2][0]) / 3.0;
                let ceny = (p[0][1] + p[1][1] + p[2][1]) / 3.0;
                let (mut dx, mut dy) = (cenx - cx + self.jitter() * 4.0, ceny - cy + self.jitter() * 4.0);
                let len = (dx * dx + dy * dy).sqrt();
                if len > 1.0e-3 {
                    dx /= len;
                    dy /= len;
                } else {
                    dx = 0.0;
                    dy = -1.0;
                }
                self.shards[i] = Shard { pts: p, dir: [dx, dy] };
            }
            self.shards_live = true;
            self.radius = 0.0; // the fresh orb grows from nothing next frame
        }
        if self.shards_live && env > 0.0 {
            let spread = h as f32 * 1.4;
            // Square-root ease: the shards leap apart early and coast to a stop, so even a frame soon
            // after the burst reads as triangles FLYING rather than a cluster still sitting on the orb.
            let off = (1.0 - env).clamp(0.0, 1.0).sqrt() * spread;
            let a = (env.clamp(0.0, 1.0) * 255.0).round() as u8;
            let col = Rgba::new(lit.r, lit.g, lit.b, a);
            for shard in &self.shards {
                let pts = [
                    ((shard.pts[0][0] + shard.dir[0] * off).round() as i32, (shard.pts[0][1] + shard.dir[1] * off).round() as i32),
                    ((shard.pts[1][0] + shard.dir[0] * off).round() as i32, (shard.pts[1][1] + shard.dir[1] * off).round() as i32),
                    ((shard.pts[2][0] + shard.dir[0] * off).round() as i32, (shard.pts[2][1] + shard.dir[1] * off).round() as i32),
                ];
                layer.fill_poly(&pts, col);
            }
        } else {
            self.shards_live = false;
        }

        // ---- bloom the layer and lay it over the opaque panel ----
        //
        // `bloom` box-blurs every pixel of whatever canvas it is given, but the orb/spikes/shards
        // never fill more than a bounding box around the panel's centre - a wide taskbar panel around
        // a squat, centred orb is mostly untouched transparent background the blur would otherwise
        // churn through for nothing. So: bound an INNER box by the worst-case reach - not the current
        // `self.radius`, which drifts every frame with bass, but the formula's own maximum (see
        // `max_reach` below) - so the box, and the scratch canvas sized to it, are fixed for a given
        // panel size and never trigger a reallocation mid-playback, only on an actual resize.
        //
        // The inner box is padded by one bloom radius (any wireframe/spike/shard pixel is provably
        // within it - see the comment on `max_reach`), and the box actually bloomed is the inner box
        // padded by a SECOND bloom radius on top of that. Blooming the wider outer box and then
        // pasting back only the inner box is what makes this bit-exact against blooming the whole
        // canvas: a pixel on the inner box's own edge needs neighbours up to one radius further out
        // to match a full-canvas blur, and the outer pad supplies exactly those (real, in-canvas,
        // correctly-zero-there) neighbours - narrower padding (one radius, discarding none of it)
        // would get the *inner* pixels right but silently give the *outer* pixels a smaller sample
        // count than a full-canvas blur would (fewer real in-bounds-but-zero taps counted before
        // hitting the cropped canvas's own edge), which is only a problem if those outer pixels were
        // ever composited back - they are not, here. Shards fly out to `h * 1.4` on a shatter, well
        // past any box worth cropping for a rare 700ms flourish, so that path keeps blooming the
        // whole layer.
        let radius = t.bloom as i32;
        if radius > 0 && t.glow_strength > 0.0 {
            if self.shards_live {
                layer.bloom(radius, t.glow_strength);
            } else {
                // Upper bound on any wireframe/spike pixel's distance from centre, using the RADIUS
                // formula's own maximum (at bass = 1.0) rather than the current `self.radius`: the
                // exponential smoothing in both the radius and spike-length updates is a convex
                // combination of the current value and a target that never exceeds this maximum, so
                // by induction from their starting values (0 and `SPIKE_BASE`) neither ever
                // overshoots it.
                let max_radius = h as f32 * (RADIUS_MIN + RADIUS_GAIN);
                let reach = max_radius + SPIKE_BASE + spike_span;
                let ibx0 = (cx - reach).floor() as i32 - radius;
                let ibx1 = (cx + reach).ceil() as i32 + radius + 1;
                let iby0 = (cy - reach).floor() as i32 - radius;
                let iby1 = (cy + reach).ceil() as i32 + radius + 1;
                let (bx0, bx1) = (ibx0.max(0), ibx1.min(w));
                let (by0, by1) = (iby0.max(0), iby1.min(h));
                let ox0 = (bx0 - radius).max(0);
                let ox1 = (bx1 + radius).min(w);
                let oy0 = (by0 - radius).max(0);
                let oy1 = (by1 + radius).min(h);
                let (obw, obh) = (ox1 - ox0, oy1 - oy0);
                if bx1 > bx0 && by1 > by0 && obw > 0 && obh > 0 && (obw < w || obh < h) {
                    let mut sub = self
                        .bloom_scratch
                        .take()
                        .filter(|s| s.width() == obw && s.height() == obh)
                        .unwrap_or_else(|| Canvas::new(obw, obh));
                    sub.copy_region(&layer, (ox0, oy0), (0, 0), obw, obh);
                    sub.bloom(radius, t.glow_strength);
                    // Paste back only the inner box - the outer pad ring exists solely to feed the
                    // inner box's own edge pixels correct neighbours and is discarded, not composited.
                    layer.copy_region(&sub, (bx0 - ox0, by0 - oy0), (bx0, by0), bx1 - bx0, by1 - by0);
                    self.bloom_scratch = Some(sub);
                } else {
                    layer.bloom(radius, t.glow_strength);
                }
            }
        }
        c.draw_over(&layer);

        // Keep nothing on the rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
        self.scratch = Some(layer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }

    /// A pixel counts as PAINTED when it differs from the background-only render at the same spot by more
    /// than a rounding margin. The panel is opaque and the background is a gradient, so "lit" is not raw
    /// alpha (every pixel is opaque) and not "differs from a flat panel colour" (there is no flat colour)
    /// - it is paint that changed the pixel away from the bare background. This is the adaptation the
    /// opaque-panel + gradient background forces on the sibling families' measure.
    fn painted(px: Rgba, bg: Rgba) -> bool {
        let d = (px.r as i32 - bg.r as i32).abs()
            + (px.g as i32 - bg.g as i32).abs()
            + (px.b as i32 - bg.b as i32).abs();
        d > 24
    }

    /// The background-only render for a colourway at a fixed `time_s` - the gradient and the drifting
    /// discs, and nothing else - so a full frame can be compared against it pixel for pixel.
    fn background(t: &Theme, w: i32, h: i32, time_s: f32) -> Canvas {
        let mut fam = Vsorb::default();
        let mut c = Canvas::new(w, h);
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        fam.draw_background(&mut c, t, time_s, (2, 2, w - 2, h - 2));
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
        c
    }

    /// Painted-over-background pixel count of a full render against its own background.
    fn lit(c: &Canvas, bg: &Canvas) -> usize {
        let mut n = 0;
        for y in 0..c.height() {
            for x in 0..c.width() {
                if painted(c.get(x, y), bg.get(x, y)) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"vsorb"));
        assert_eq!(crate::render::family_for("vsorb").id(), "vsorb");
        assert_ne!(crate::themes::family_label("vsorb"), "vsorb");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "vsorb").count(), 4);
    }

    #[test]
    fn orb_radius_follows_bass() {
        let t = theme("vsorb-mono");
        // Bass only (bands 0..8) vs treble only (bands 40..64) at the same energy: the orb must be larger for bass.
        let run = |lo: std::ops::Range<usize>| {
            let mut fam = Vsorb::default();
            let mut c = Canvas::new(380, 48);
            let mut d = FrameData::default();
            for i in lo {
                d.levels[i] = 0.9;
            }
            d.peaks = d.levels;
            d.dt_ms = 16.7;
            for k in 0..40 {
                d.time_s = k as f32 * 0.0167;
                fam.draw(&mut c, &t, &d);
            }
            fam.radius_px()
        };
        assert!(run(0..8) > run(40..64) * 1.4, "bass {} treble {}", run(0..8), run(40..64));
    }

    #[test]
    fn spikes_follow_the_spectrum() {
        // With only band 63 lit, the spike carrying that band (k=23, bands 61..64) must be far longer
        // than the one carrying band 0 (k=0, bands 0..3), and only ITS tip lit - nowhere near where the
        // short spike 0 would reach. This discriminates a WRONG band mapping: if every spike read band 0
        // the two lengths would be equal and the ratio assertion below would fail.
        let t = theme("vsorb-mono");
        let mut fam = Vsorb::default();
        let mut c = Canvas::new(380, 48);
        let mut d = FrameData::default();
        d.levels[63] = 1.0;
        d.peaks = d.levels;
        d.dt_ms = 16.7;
        for k in 0..30 {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        let bg = background(&t, 380, 48, 29.0 * 0.0167);

        // 1. The lit-band spike is much longer than the silent-band spike. (Bounded by the 3px floor:
        // spike 0 rests at 3.0, spike 23 reaches ~8.8, so the ratio is ~2.9 - assert 2.5x, which a
        // mapping that fed every spike band 0 could not reach, since then both would be 3.0.)
        let l23 = fam.spike_len_for_test(23);
        let l0 = fam.spike_len_for_test(0);
        assert!(l23 >= 2.5 * l0, "spike 23 (band 63) len {l23} not >= 2.5x spike 0 (band 0) len {l0}");

        // 2. Its tip IS painted.
        let (sx, sy) = fam.spike_tip_for_test(23);
        let (sx, sy) = (sx.clamp(0, 379), sy.clamp(0, 47));
        assert!(painted(c.get(sx, sy), bg.get(sx, sy)), "spike 23 tip not lit at {sx},{sy}");

        // 3. The pixel spike 0 WOULD light if it were as long as spike 23 is NOT painted - the short
        // spike does not reach out there, and nothing else does either.
        let (si0, co0) = (0.0f32).sin_cos();
        let reach = fam.radius_px() + l23;
        let px = (190.0 + reach * co0).round() as i32;
        let py = (24.0 + reach * si0).round() as i32;
        let (px, py) = (px.clamp(0, 379), py.clamp(0, 47));
        assert!(!painted(c.get(px, py), bg.get(px, py)), "spike 0 reached as far as spike 23 at {px},{py}");
    }

    #[test]
    fn rest_frame_is_not_empty() {
        // At silence the orb still draws at its minimum radius over the visible gradient.
        let t = theme("vsorb-mono");
        let mut fam = Vsorb::default();
        let mut c = Canvas::new(380, 48);
        let mut d = FrameData::default();
        d.dt_ms = 16.7;
        for k in 0..10 {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        let bg = background(&t, 380, 48, 9.0 * 0.0167);
        let n = lit(&c, &bg);
        assert!(n > 30, "the orb wireframe should show at rest: {n}");
    }

    #[test]
    fn shatter_at_narrow_size_does_not_panic() {
        for id in ["vsorb-chrome", "vsorb-eon", "vsorb-angel", "vsorb-mono"] {
            let t = theme(id);
            let mut fam = Vsorb::default();
            let mut c = Canvas::new(190, 48);
            let mut d = FrameData::default();
            for v in d.levels.iter_mut() {
                *v = 0.5;
            }
            d.peaks = d.levels;
            d.dt_ms = 16.7;
            for k in 0..5 {
                d.time_s = k as f32 * 0.0167;
                fam.draw(&mut c, &t, &d);
            }
            fam.flourish.force_next();
            for k in 5..20 {
                d.time_s = k as f32 * 0.0167;
                fam.draw(&mut c, &t, &d);
            }
            let bg = background(&t, 190, 48, 19.0 * 0.0167);
            assert!(lit(&c, &bg) > 0, "{id}");
        }
    }

    /// The per-frame cost, well under the 2ms budget at 380x60.
    ///
    /// Run: cargo test --release probe_vsorb_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_vsorb_cost() {
        let t = theme("vsorb-chrome");
        let mut fam = Vsorb::default();
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
        println!("vsorb steady: {steady:.3} ms/frame at 380x60");

        // The shatter path draws up to `tris.len()` (80) fill_poly triangles - measure it separately,
        // since it never runs on a steady frame. Fire, then time the whole 700ms envelope's worth of
        // frames (~42 at 16.7ms) plus a margin.
        fam.flourish.force_next();
        let m = 40;
        let t1 = std::time::Instant::now();
        for k in n..(n + m) {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        let shatter = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
        println!("vsorb shatter: {shatter:.3} ms/frame at 380x60");
    }

    /// Dumps for the eye test - composited over `#202020` like every other family's dump.
    ///
    /// Run: cargo test --release dump_vsorb -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_vsorb() {
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
        // A shaped, beating spectrum - bass-heavy so the orb pulses.
        let frame = |level: f32, t_s: f32| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                let shape = (1.0 - f).powf(1.3) * 0.7 + 0.12;
                let wob = 1.0 + 0.3 * (t_s * 2.4 + f * 6.0).sin();
                let kick = 1.0 + 0.7 * (t_s * 3.1).sin().max(0.0) * (1.0 - f);
                *v = ((shape * wob * kick) * level).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d.rms_l = level * 0.6;
            d.rms_r = level * 0.6;
            d
        };
        for id in ["vsorb-chrome", "vsorb-eon", "vsorb-angel", "vsorb-mono"] {
            let t = theme(id);
            for (tag, level) in [("calm", 0.28f32), ("loud", 0.85)] {
                let mut fam = Vsorb::default();
                let mut c = Canvas::new(380, 60);
                for k in 0..120 {
                    c.clear();
                    fam.draw(&mut c, &t, &frame(level, k as f32 * 0.0167));
                }
                write(format!("vsorb-{}-{tag}", &t.id["vsorb-".len()..]), &c);
            }
            // Flourish: settle, fire, then capture a mid-shatter frame (triangles flying, fresh orb small).
            let mut fam = Vsorb::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..120 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            }
            fam.flourish.force_next();
            for k in 120..130 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.35, k as f32 * 0.0167));
            }
            write(format!("vsorb-{}-flourish", &t.id["vsorb-".len()..]), &c);
        }
        // One at the awkward mid size.
        let t = theme("vsorb-chrome");
        let mut fam = Vsorb::default();
        let mut c = Canvas::new(190, 48);
        for k in 0..120 {
            c.clear();
            fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
        }
        write("vsorb-chrome-190x48".into(), &c);
    }
}
