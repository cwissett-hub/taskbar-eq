# Known gaps

- **Nothing has been tested on Windows 10 or on a second machine.** The chevron fallback
  exists precisely because Win10 has no Widgets button, but that path is reasoning from code,
  not evidence. If nothing appears, run `tools/probe/Probe-Taskbar.ps1` there and send me the
  output — the element names will say exactly what to match.
- **The dark menu uses two undocumented uxtheme calls** (ordinals 135/136). There is no
  documented way to dark-mode a Win32 menu. If either ordinal is missing on your build the
  menu silently stays light — deliberately, since a light menu is a cosmetic flaw and a crash
  is not an acceptable price for avoiding it.
- **Hidden-line removal in the vaporwave family is inert at the shipped settings.** It only
  bites when the perspective term packs grid lines tighter than the audio lift can separate
  them; the `persp` needed for legibility at 60 px removes that condition. Measured: `persp`
  1.4 changes 0 pixels, 2.07 changes 83. Kept for the wider variant and for theme files that
  raise `persp`.
- Until 2026-08-05 the TOML parser **rejected `family = "tube"` and `family = "vapor"`** even
  though this README documented both, so neither of the newest families could be authored or
  tuned from a file. Fixed, and the parser now reads the renderer's own family list so it cannot
  fall behind again. If you wrote a theme file for either family before then, it was being
  skipped with a warning.
- **Six families are new and only lightly reviewed.** An adversarial pass on each found real defects
  and **both are now fixed**, though neither turned out to be what the note above claimed. The
  **Patchbay** was said to flatten on real music; measured over three real-music captures the spread
  across cables is healthy (0.70–0.80 of the range, and 0% of frames flat) — the real fault was that
  its response window was placed on *band* levels while the family feeds it a *peak-biased group*
  level, so the bass cable sat pinned at full deflection on 64%, 96% and 100% of frames. And the
  **Spectrogram**'s vacuous fold test was not a ramp problem: the pitch-track marker lands on the same
  row a lone loud band folds into, so the test was measuring the marker. Fixed already: the Reel's peak lamp needed an RMS of 0.508 to light, against a real
  ceiling of about 0.12, so it was dead code that could never fire on music.
- **Every family leaks a few bezel pixels outside the rounded panel.** The bezel is drawn after
  `clip_to_rounded_rect`, and it is square while the panel's corners are not, so ~4 pixels per corner
  land on the bare taskbar. Measured and pre-existing, and invisible in practice against a dark
  taskbar - but it is a real leak, and it is the panel's own rounded corners that it escapes through.
- The width is **clamped by what UI Automation reports**, so an element it cannot see is an
  element the overlay may cover. Every named taskbar element on the test machine was accounted
  for, but this has not been tried on a taskbar with third-party shell extensions.
- Theme *aesthetics* at 190×60 and 380×60 are not verified by anything automated. Every family has an
  `#[ignore]`d dump harness (`cargo test --release dump_ -- --ignored`) that writes raw RGBA
  for eyeballing, because "does this look like a smear" is not a question a golden can answer.
  See [docs/lessons.md](lessons.md) for the full measured-vs-assumed split.

