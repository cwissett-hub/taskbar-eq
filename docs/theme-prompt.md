# Prompt: generate more themes

Paste the block below into any coding agent to have it author new colourways. It is
self-contained — the agent needs no other context about this project.

````text
I need you to author new colourways for Taskbar EQ, a Windows 11 taskbar audio
visualiser. A colourway is data, not code.

THE CANVAS - READ THIS BEFORE CHOOSING COLOURS
  The display is 190 x 60 physical pixels, on the Windows 11 taskbar over the weather
  widget. Three constraints follow, and they matter more than taste:

  1. DARK MODE ONLY. Light mode is not supported. Do not design for it.
  2. THE PANEL IS FULLY OPAQUE - panel_alpha 1.0. Anything less transmits the weather
     text behind it: at 0.96, 4% of white text is ~10 luminance, invisible against a
     lit bar but clearly visible against the dark segment gaps. Do not lower it.
  3. COLOURS ARE EMISSIVE - glowing phosphors and LEDs, not chart fills. They belong
     near maximum lightness. A colourway that "reads gray" is correct here if it is
     meant to look white-hot.

  Hard requirement: every lit colour must reach at least 3:1 contrast against its own
  theme's panel. Compute it (WCAG relative luminance); do not eyeball it. A test will
  fail you otherwise.

  Avoid a hue ramp that merely tracks bar height. Green -> amber -> red IS allowed and
  ships as a built-in, because it encodes headroom (safe / loud / peaking), not
  magnitude.

PICK A FAMILY - a renderer with fixed geometry. You cannot invent one in data.
  segmented  discrete stacked segments on a glass panel, dormant grid, peak-hold caps
  scope      a triggered oscilloscope trace with a graticule and phosphor persistence
  vu         two analogue needle dials with a printed arc and a red overload zone
  vapor      a sunset over a scrolling perspective grid; the grid carries the audio
  tube       a row of vacuum tubes, each glowing with its band inside the glass

  An unknown family name is NOT an error - it falls back to `segmented` and logs a
  warning. So a typo does not fail loudly; check the family name spelling.

FILE FORMAT - one `.toml` file per theme, saved under `%APPDATA%\taskbar-eq\themes\`
(filename does not matter; `id` inside is the identity, and the override key - a file
whose `id` matches one of the 150 built-ins REPLACES it, any other `id` is added):

  schema = 1
  id     = "my-theme"
  name   = "My Theme"
  family = "segmented"

  [colour]
  lit         = "#..."
  hot         = "#..."
  panel       = "#..."
  panel_alpha = 1.0
  edge        = "#..."
  edge_alpha  = 0.15

  [look]
  ghost         = 0.11
  bloom         = 5.0
  glow_strength = 0.35
  edge_glow     = 4.0
  fade          = 0.30
  sensitivity   = 1.0
  texture       = "glass"

  [ballistics]
  attack    = 0.55
  decay     = 0.11
  peak_fall = 0.005

  # optional, repeatable - see "zones" below
  [[zone]]
  upto = 0.5
  lit  = "#..."
  hot  = "#..."

  # optional, scope family only - see "dual" below
  [dual]
  trail = "#..."
  fade  = 0.20

  # optional, vapor family only. Every key optional; these are the shipped defaults,
  # which are NOT the browser-tuner values - see "the 60px problem" below.
  [vaporwave]
  horizon      = 0.48   # fraction of panel height
  amp          = 0.55   # terrain displacement scale
  lines        = 12     # receding horizontal grid lines
  verts        = 18     # converging verticals
  scroll       = 1.24
  persp        = 1.40   # depth-spacing exponent
  spread       = 1.50   # width spread of the near edge
  glow         = 0.98   # peak-glow brightness
  smoothing    = 0.65   # spectral smoothing; higher = rolling hills, not spikes
  sun          = 0.83
  slots        = 6      # horizontal gaps cut in the sun
  slot_bias    = 0.0    # slot widening toward the horizon
  slot_top     = 0.18
  halo         = 0.84
  warmth       = 0.63
  bolt_sens    = 0.55   # rise in bass needed to fire lightning
  bolt_bright  = 0.90   # set 0.0 to disable lightning entirely
  sky_flash    = 0.35
  grid_flash   = 0.60
  bolt_decay   = 0.55
  occlusion    = true
  crisp        = true
  sun_rim      = true
  sky_top      = "#1a0b2e"
  sky_horizon  = "#ff5f93"
  ground       = "#12061f"
  sun_crown    = "#fff6d0"
  sun_upper    = "#ffd76e"
  sun_lower    = "#ff9c4a"
  sun_base     = "#ff5f93"

  # optional, tube family only. A valve is several materials, none of which is a
  # variation of the accent colour, so they are set independently.
  [tube]
  chassis_top    = "#3c4436"
  chassis_bottom = "#161a12"
  internals      = "#0b0d08"   # plate metal, silhouetted against the glow - keep it DARK
  socket         = "#241a10"   # bakelite
  collar         = "#8a6a2a"   # brass
  glass          = "#cfe0d8"   # specular highlight

Every field below is optional except `schema`/`id`/`name`/`family` - anything you omit
takes the documented default, so a minimal file is valid. Unknown keys and unknown
`texture` values are ignored rather than rejected, so a file written for a later
version of this schema still loads.

FIELDS
  schema        always `1` - the version of this file format. A newer number than the
                app understands is rejected outright (with a message naming both
                numbers), not silently reinterpreted.
  id            kebab-case, stable, unique. Also the override key - see FILE FORMAT.
  name          shown in a context menu, so keep it short.
  family        segmented | scope | vu | vapor | tube

  [colour]
  lit           the main emissive colour
  hot           the brighter core. Usually `lit` pushed toward white, not a new hue.
  panel         the display panel. Near-black, tinted toward `lit`'s hue - this is
                what makes each theme feel like its own device rather than a recolour.
  panel_alpha   1.0. See constraint 2.
  edge          1px bezel line;  edge_alpha  0.10-0.25

  [look]
  ghost         alpha of the unlit dormant grid. 0 hides it; 0.17 is clearly visible.
  bloom         halo RADIUS in px, NOT brightness. Must stay small relative to the 7px
                bar pitch - at 16 the halos of adjacent bars merged into one wash
                sitting behind the segments. 3-8 is the usable range.
  glow_strength halo brightness. THIS is the knob for "more glow", not bloom. ~0.35
                gives a tight visible halo; above ~0.7 the bars merge together.
  edge_glow     a dim halo masked to the display's edge ring, as a multiple of
                glow_strength. ~4.0 reads as the bezel catching light. 0.3 measured
                DARKER than the panel it sat on, so do not go low.
  fade          cross-fade duration in seconds when switching to this theme at
                runtime. 0.30 is the shipped default; unrelated to `[dual].fade` below.
  texture       glass | scanlines | haze | filament | grille | none
                glass=lit top edge, scanlines=CRT lines, haze=neon radial glow,
                filament=warm pool along the bottom, grille=fine vertical lines

  [ballistics]
  attack/decay  0-1 per frame. decay MUST be lower than attack - fast attack with slow
                decay is what makes a meter feel right. Never set them equal.
  peak_fall     how fast peak-hold marks sink. Small = slow.

  [[zone]]      optional, repeatable table array, for meters that change colour by
                height (see the classic three-colour built-in). Each has upto/lit/hot;
                upto values must ascend and the last one must be >= 1.0.

  [dual]        optional, scope family only - a second, slower-fading phosphor layer
                behind the trace (real P7 tubes work this way: a blue-white flash over
                a lingering yellow-green afterglow).
  trail         hex colour of the afterglow layer.
  fade          0-1 per frame, how fast the afterglow decays. Small = slow. Distinct
                from `[look].fade` (the theme cross-fade) above.

MEASURED REFERENCE - the five shipped colourways, so you can anchor your numbers
rather than guess. "ratio" is the luminance of a lit segment divided by the gap
BETWEEN adjacent bars, measured at 75% level. Below about 2.2 the bars stop reading
as separate; above about 9 there is no visible halo at all. Aim for 4-8.

  theme                  bloom  glow_str  edge_glow  ghost  texture     ratio
  vfd-ice                  4.0      0.35        4.0   0.11  glass        5.91
  matrix-green             5.0      0.35        4.0   0.17  scanlines    5.97
  neon-pink                6.0      0.35        4.0   0.09  haze         7.88
  vac-tube-orange         12.0      0.35        4.0   0.13  filament     4.21
  classic-three-colour     7.0      0.35        4.0   0.13  grille       5.66

  Two things that table teaches, both of which are counter-intuitive and were both
  learned the hard way:

  * A BIGGER bloom radius makes the halo FAINTER, not stronger. The blur normalises
    by kernel size, so a wider radius spreads the same energy across far more pixels.
    neon-pink at radius 14 measured as the faintest of the five; dropping it to 6 made
    it brighter. If you want more glow, raise glow_strength, never bloom.
  * The texture affects the measurement. vac-tube-orange reads as the strongest halo
    partly because `filament` brightens the lower panel, which lifts the gap reading.
    Do not chase the ratio number alone.

WHAT I WANT
  <describe: how many, what mood, which families, any reference hardware>

FOR EACH THEME, TELL ME
  1. The finished `.toml` file in the FILE FORMAT above, ready to drop into
     `%APPDATA%\taskbar-eq\themes\`.
  2. The computed contrast ratio of `lit` against `panel`, as a number.
  3. Which shipped colourway in the table above yours sits closest to, and why you
     departed from it.
  4. One sentence on what real device it imitates. If you cannot name one, the theme is
     probably an arbitrary hue shift - I would rather have fewer, more deliberate themes
     than more generic ones.
````

