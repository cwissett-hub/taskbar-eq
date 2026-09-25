# Song identification (Shazam) - design

**Date:** 2026-09-25
**Status:** approved in conversation, awaiting written review

## Intent

Press a hotkey (or click a tray menu entry) while any audio plays on the PC - a video in a
browser, a Discord call, a game - and have the taskbar banner name the song. Every find is
kept, and a history page lists them all with one-click links to Spotify, Apple Music, YouTube
and Shazam.

Audience: the user and a couple of friends. Not a company project - no yellow-room theme.

**What the user said:** on-demand, hotkey-triggered; the sources are browser videos and
Discord, so fingerprinting is the point (media-session metadata is not enough); ShazamIO-style
Shazam recognition; a database of all finds; a robust history with easy links; HTML for the
history now, Win32 maybe later; dark mode, cyberpunk or vaporwave styling.

**Assumptions:** the banner should say "listening..." and "no match" as well as the result; a
second press while an identification is running is ignored; repeat finds of the same song are
kept as separate events so the history can show a count.

## Decisions already taken

| Decision | Choice | Why |
|---|---|---|
| Recogniser | Rust-native port of SongRec's Shazam signature + the amp.shazam.com endpoint | Keeps the single exe; sub-second results; no Python sidecar (30 MB, second process, second build system) |
| Licence | Repo becomes GPL-3 | SongRec is GPL-3; the user accepted this |
| Result surface | Existing taskbar banner + tray submenu + HTML history page | Toasts need a start-menu shortcut registration the portable exe avoids |
| History surface | Self-contained HTML page opened in the default browser | A Win32 popup menu cannot carry search, sort or several links per row; a native ListView is days of GDI for a worse result |
| Storage | JSON-lines file `songs.jsonl` beside `config.toml` | Append-only survives crashes; no C build step; a few hundred rows is trivial |
| Styling | Dark base, neon magenta/cyan accents, vaporwave sunset gradient where it fits | User's call: "cyberpunk or vaporwave, whatever fits" |

## Architecture

```
hotkey / menu ──► identify::request()
                       │  (ignored if one is already running)
                       ▼
             identify thread
                       │ 1. banner "listening..."
                       │ 2. take up to 12 s of 48 kHz mono from capture (see Audio hand-off)
                       │ 3. dsp::shazam_sig  -> binary signature
                       │ 4. net::shazam      -> HTTP POST -> Match | NoMatch | Error
                       │ 5. songs::append(find)      (on Match)
                       │ 6. banner "Title, Artist" / "no match"
                       ▼
             tray menu  Songs ▸ last 10 distinct finds, "Song history", "Open songs folder"
                       │
                       ▼
             history::write_html(songs) -> %APPDATA%\taskbar-eq\songs.html -> ShellExecute open
```

### New modules

| Module | Purpose | Depends on |
|---|---|---|
| `dsp::shazam_sig` | 48 kHz mono f32 -> 16 kHz s16 -> Shazam binary signature (base64 + sample count). Port of SongRec's `fingerprinting` with attribution. | `rustfft` (already a dependency) |
| `net::shazam` | Builds the request body, POSTs to the discovery endpoint, parses the reply into `Find`. | one blocking HTTP client crate with rustls (`ureq`), `serde_json` |
| `songs` | `Find` type; append to / read from `songs.jsonl`; hidden flags; distinct-recent list. | `serde`, `serde_json` |
| `history` | Renders the HTML page from `Vec<Find>`; builds the per-service URLs. | none |
| `identify` | Orchestrates the thread, the busy flag, banner text and logging. | all of the above, `win::capture`, `win::media` banner |

Existing code touched: `win::hotkeys` (one new `Slot::IdentifySong`, `SLOTS` 7 -> 8),
`config::Hotkeys` (one new key field), `win::tray` (Songs submenu, new events),
`win::capture` (audio hand-off, below), `main.rs` (wire events), `Cargo.toml`, `README.md`,
`LICENSE`.

### Audio hand-off

The capture thread already downmixes to mono into a ring that is trimmed to `FFT_SIZE * 2`.
It gains a second, optional sink: an `Arc<Mutex<Option<Recorder>>>`. When `identify` starts it
installs a `Recorder { rate, buf: Vec<f32> }`; the capture loop appends every mono chunk to it
while installed; `identify` polls the buffer and removes the recorder when it has 12 s or has
given up. The capture loop's own behaviour is unchanged when no recorder is installed, and a
poisoned mutex is treated as "no recorder".

The capture thread's sample rate is whatever `GetMixFormat` returns (48 kHz here). The
resampler in `dsp::shazam_sig` handles any integer or non-integer ratio by linear interpolation
after a simple low-pass; Shazam is tolerant of this and SongRec does the same.

### Shazam signature (port of SongRec)

- Input 16 kHz mono s16, any length; signature covers everything provided.
- Sliding window of 2048 samples, hop 128, Hann window, real FFT, power spectrum.
- Peaks picked in SongRec's four bands (250-520, 520-1450, 1450-3500, 3500-5500 Hz) with
  its neighbourhood and magnitude tests, stored as `FrequencyPeak { fft_pass_number, peak_magnitude, corrected_peak_frequency_bin, sample_rate_hz }`.
- Encoded into SongRec's binary layout (magic `0xcafe2580`, CRC32, header, per-band
  TLV blocks), then base64. `samplems` = samples / 16.
- Attribution: module header names SongRec (marin-m, GPL-3) as the source.

### Shazam request

`POST https://amp.shazam.com/discovery/v5/en/US/android/-/tag/{uuid4}/{uuid4}?sync=true&webv3=true&sampling=true&connected=&shazamapiversion=v3&sharehub=true&video=v3`
with SongRec's headers (User-Agent, Content-Language) and JSON body
`{ geolocation: {altitude, latitude, longitude}, signature: { uri: "data:audio/vnd.shazam.sig;base64,...", samplems }, timestamp, timezone }`.
Timeout 10 s. The UUIDs are random per request (a tiny local generator, no crate).

Reply parsing takes only what the page needs:

```rust
pub struct Find {
    pub when: i64,              // unix seconds
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub cover_url: Option<String>,
    pub shazam_url: Option<String>,   // share.href
    pub apple_music_url: Option<String>,
    pub spotify_uri: Option<String>,  // "spotify:track:..." from hub.providers, if present
    pub isrc: Option<String>,
    pub shazam_key: String,           // track.key - identity for "distinct" and "hidden"
    pub app_version: String,
}
```

`{"matches": []}` is `NoMatch`. Anything else non-2xx or unparseable is `Error(String)`.

### Retry policy

Record 12 s total, but post at 4 s first (SongRec's approach: most songs match within 4-6 s).
If `NoMatch`, post again at 8 s and at 12 s with the whole buffer each time. Stop at the first
`Match`. Total worst case ~13 s plus network.

### Storage - `songs.jsonl`

One JSON object per line, one line per `Find`. Append with `OpenOptions::append`. A separate
`songs_hidden.json` holds a list of hidden `shazam_key`s (small, rewritten whole). Reading
skips lines that fail to parse, with one log entry per bad line.

Derived views: `recent_distinct(n)` - newest first, deduplicated by `shazam_key`, hidden
removed; `grouped()` for the page - per key: first heard, last heard, count.

### Tray menu

Under the Hotkeys section: **Identify song** with its binding, bindable like the others, and
offered as an action. New top-level **Songs ▸** submenu:

```
Songs ▸  Title - Artist            (last 10 distinct, newest first; click = open Spotify link)
         ...
         ───────────
         Song history...           (write HTML, open in browser)
         Open songs folder         (explorer on %APPDATA%\taskbar-eq)
```

Empty history shows one disabled entry "no songs yet".

### Banner

Reuses the existing track-name banner mechanism (`win::media::publish` or a sibling that
takes priority over the track name for a bounded time): "listening..." while recording,
"Title, Artist" for the normal banner duration on a match, "no match" for the same duration
otherwise. If `show_track_name` is off the identify banner still shows: the user asked for
it explicitly.

### History page

`history::write_html` renders `songs.html` beside the data, then `ShellExecuteW open`.
Self-contained: inline CSS and JS, data embedded as a JSON array, no external assets except
cover art URLs. Works from `file://`.

Content:
- Header: "SONG HISTORY" with count, and a live search box (filters on title/artist/album).
- Table, sortable by clicking headers: cover, Title, Artist, Album, First heard, Last heard,
  Count. Default sort last heard, newest first.
- Per row four link buttons, always populated:
  - Spotify: `https://open.spotify.com/track/{id}` from `spotify_uri`, else
    `https://open.spotify.com/search/{title} {artist}`.
  - Apple Music: `apple_music_url`, else `https://music.apple.com/search?term=...`.
  - YouTube: `https://www.youtube.com/results?search_query=...`.
  - Shazam: `shazam_url`, else `https://www.shazam.com/search?q=...`.
- Per row a **hide** button. Hiding is client-side only in this version: the row disappears
  and the key is written to `localStorage`; the page honours `localStorage` on load. This
  avoids a browser -> exe channel. The tray submenu does not see `localStorage` hides; that is
  accepted for now and noted as a follow-up (a Win32 page later would fix it properly).
- Styling: near-black background (#0b0710), neon magenta (#ff2bd6) and cyan (#19e6ff) accents,
  a vaporwave sunset gradient behind the header (magenta -> orange -> deep purple), monospace
  display font, subtle horizontal scanline overlay, glow on hover. Cover art has a thin
  cyan border. Everything readable at 100% and 125% DPI.

## Failure handling

| Failure | Behaviour |
|---|---|
| No recorder audio (capture thread dead, silence flag) | "no match" after 12 s; log says why |
| Network error / timeout / non-2xx / parse error | "no match"; the real error in the log |
| Shazam changes its API | same as above; the log carries the status and first 200 bytes of body |
| Identify pressed while running | ignored; log entry |
| `songs.jsonl` missing | treated as empty |
| A bad line in `songs.jsonl` | skipped, logged, rest still loads |
| Cannot write `songs.html` | log; menu does nothing visible |

Nothing in this feature can prevent the app starting or the visualiser rendering.

## Testing

Per the user's standing preference, no automated GUI or browser tests.

- `dsp::shazam_sig`: resampler tests (rate, length, a pure tone stays a pure tone); peak picker
  on a synthetic two-tone signal finds both; encoder golden test - a fixture WAV
  (`tests/fixtures/identify/tone_mix.wav`, synthetic, 6 s) whose signature is captured ONCE from
  SongRec's implementation and stored as the expected base64. CRC32 test vector.
- `net::shazam`: parser against a saved real reply (`match.json`) and `{"matches":[]}`;
  request body has the required fields; no live network in tests.
- `songs`: append then read round-trip; bad line skipped; `recent_distinct` dedupes and
  respects hidden; `grouped` counts.
- `history`: every row gets four non-empty URLs; HTML-escaping of `<script>` in a title; the
  fallback search URLs are percent-encoded.
- `hotkeys`/`config`: `SLOTS` round-trip test already exists and must still pass with 8.
- Live check, judged by the user: play a song in a browser tab, press the hotkey, see the
  banner, see the row on the page, click each of the four links.

## Out of scope (for now)

- Continuous / automatic identification.
- Win32 native history window.
- Hidden-flag sync between page and tray submenu.
- Odesli / song.link cross-service resolution (search links are enough).
- Any recogniser other than Shazam.

## Follow-ups noted

- If Shazam's endpoint breaks, the fallback is ACRCloud or AudD behind the same `Find` type.
- A version bump to 0.2.0 in `Cargo.toml` when this ships.
