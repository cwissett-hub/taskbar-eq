//! The Shazam call, as SongRec and ShazamIO make it: POST a signature to the undocumented
//! discovery endpoint and read the track back. Unofficial, so every field read is optional except
//! the three that define a match (key, title, subtitle), and the raw body is saved to disk so a
//! format change can be diagnosed from `last_shazam.json` rather than guessed at.

use crate::dsp::shazam_sig::Signature;
use crate::songs::Find;
use serde_json::Value;

pub enum Outcome {
    Match(Find),
    NoMatch,
    Error(String),
}

const USER_AGENT: &str = "Dalvik/2.1.0 (Linux; U; Android 10; Pixel 4 Build/QQ3A.200805.001)";

/// Random v4-shaped UUID from the OS clock and a splitmix64 step. Not cryptographic; Shazam only
/// wants two distinct-looking ids per request.
pub fn random_uuid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEED: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    let mut x = SEED.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed) ^ t;
    let mut next = || {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let (a, b) = (next(), next());
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_le_bytes());
    bytes[8..].copy_from_slice(&b.to_le_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h = |r: std::ops::Range<usize>| {
        bytes[r].iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    format!("{}-{}-{}-{}-{}", h(0..4), h(4..6), h(6..8), h(8..10), h(10..16))
}

pub fn request_body(sig: &Signature, now_ms: u64) -> String {
    serde_json::json!({
        "geolocation": { "altitude": 300, "latitude": 45, "longitude": 2 },
        "signature": {
            "samplems": sig.sample_ms(),
            "timestamp": now_ms as u32,
            "uri": sig.encode_to_uri()
        },
        "timestamp": now_ms as u32,
        "timezone": "Europe/Paris"
    })
    .to_string()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Where the last raw reply is kept, for diagnosis and as a parser fixture.
pub fn last_reply_path() -> std::path::PathBuf {
    crate::config::Config::dir().join("last_shazam.json")
}

/// Blocking. Call from the identify thread only.
pub fn recognize(sig: &Signature) -> Outcome {
    let url = format!(
        "https://amp.shazam.com/discovery/v5/en/US/android/-/tag/{}/{}?sync=true&webv3=true&sampling=true&connected=&shazamapiversion=v3&sharehub=true&video=v3",
        random_uuid().to_uppercase(),
        random_uuid()
    );
    let body = request_body(sig, now_ms());
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .http_status_as_error(false)
        .build()
        .new_agent();
    let resp = agent
        .post(&url)
        .header("User-Agent", USER_AGENT)
        .header("Content-Language", "en_US")
        .header("Content-Type", "application/json")
        .send(body.as_bytes());
    let mut resp = match resp {
        Ok(r) => r,
        Err(e) => return Outcome::Error(format!("request failed: {e}")),
    };
    let status = resp.status().as_u16();
    let text = match resp.body_mut().read_to_string() {
        Ok(t) => t,
        Err(e) => return Outcome::Error(format!("HTTP {status}, body unreadable: {e}")),
    };
    let _ = std::fs::create_dir_all(crate::config::Config::dir());
    let _ = std::fs::write(last_reply_path(), &text);
    if status == 429 {
        return Outcome::Error("HTTP 429: rate limited by Shazam".into());
    }
    if status != 200 {
        let head: String = text.chars().take(200).collect();
        return Outcome::Error(format!("HTTP {status}: {head}"));
    }
    let now = (now_ms() / 1000) as i64;
    parse_reply(&text, now)
}

fn s(v: &Value) -> Option<String> {
    v.as_str().map(|x| x.to_string())
}

pub fn parse_reply(body: &str, now_unix: i64) -> Outcome {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            let head: String = body.chars().take(200).collect();
            return Outcome::Error(format!("reply is not JSON ({e}): {head}"));
        }
    };
    let matches = v["matches"].as_array();
    if matches.map(|m| m.is_empty()).unwrap_or(false) {
        return Outcome::NoMatch;
    }
    let track = &v["track"];
    let (Some(key), Some(title), Some(artist)) =
        (s(&track["key"]), s(&track["title"]), s(&track["subtitle"]))
    else {
        return Outcome::Error(
            "reply has matches but no track key/title/subtitle - format changed?".into(),
        );
    };
    let mut album = None;
    if let Some(sections) = track["sections"].as_array() {
        for sec in sections {
            if sec["type"] == "SONG" {
                if let Some(meta) = sec["metadata"].as_array() {
                    for m in meta {
                        if m["title"] == "Album" {
                            album = s(&m["text"]);
                        }
                    }
                }
            }
        }
    }
    let mut apple = None;
    let mut spotify = None;
    if let Some(opts) = track["hub"]["options"].as_array() {
        for o in opts {
            if let Some(acts) = o["actions"].as_array() {
                for a in acts {
                    if a["type"] == "applemusicopen" {
                        apple = apple.or_else(|| s(&a["uri"]));
                    }
                }
            }
        }
    }
    if let Some(provs) = track["hub"]["providers"].as_array() {
        for p in provs {
            if p["type"] == "SPOTIFY" {
                if let Some(acts) = p["actions"].as_array() {
                    for a in acts {
                        if let Some(u) = s(&a["uri"]) {
                            if u.starts_with("spotify:track:") {
                                spotify = Some(u);
                            }
                        }
                    }
                }
            }
        }
    }
    Outcome::Match(Find {
        when: now_unix,
        title,
        artist,
        album,
        cover_url: s(&track["images"]["coverarthq"]).or_else(|| s(&track["images"]["coverart"])),
        shazam_url: s(&track["share"]["href"]).or_else(|| s(&track["url"])),
        apple_music_url: apple,
        spotify_uri: spotify,
        isrc: s(&track["isrc"]),
        shazam_key: key,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::shazam_sig::{FrequencyPeak, Signature};

    fn fixture(name: &str) -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/identify")
            .join(name);
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn parses_a_match_into_a_find() {
        let Outcome::Match(f) = parse_reply(&fixture("shazam_match.json"), 1_700_000_000) else {
            panic!("expected a match");
        };
        assert_eq!(f.title, "Resonance");
        assert_eq!(f.artist, "HOME");
        assert_eq!(f.album.as_deref(), Some("Odyssey"));
        assert_eq!(f.shazam_key, "5933917");
        assert_eq!(f.isrc.as_deref(), Some("QZDA61474185"));
        assert_eq!(f.when, 1_700_000_000);
        assert!(f.cover_url.as_deref().unwrap().starts_with("https://is1-ssl"));
        assert_eq!(f.shazam_url.as_deref(), Some("https://www.shazam.com/track/5933917/resonance"));
        assert!(f.apple_music_url.as_deref().unwrap().starts_with("https://music.apple.com/"));
        // A search deeplink is not a track link; it must not be reported as one.
        assert_eq!(f.spotify_uri, None);
        assert_eq!(f.app_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn spotify_track_uri_is_kept_when_present() {
        let body = fixture("shazam_match.json")
            .replace("spotify:search:Resonance%20HOME", "spotify:track:0Zxu4C6zA6Qw6Xy");
        let Outcome::Match(f) = parse_reply(&body, 0) else { panic!() };
        assert_eq!(f.spotify_uri.as_deref(), Some("spotify:track:0Zxu4C6zA6Qw6Xy"));
    }

    #[test]
    fn empty_matches_is_no_match() {
        assert!(matches!(parse_reply(&fixture("shazam_nomatch.json"), 0), Outcome::NoMatch));
    }

    #[test]
    fn garbage_and_shape_changes_are_errors_not_no_match() {
        assert!(matches!(parse_reply("<html>rate limited</html>", 0), Outcome::Error(_)));
        // Valid JSON, matches present, but no track object: an API change, not silence.
        assert!(matches!(parse_reply(r#"{"matches":[{"id":"1"}]}"#, 0), Outcome::Error(_)));
    }

    #[test]
    fn request_body_has_the_fields_shazam_requires() {
        let sig = Signature {
            sample_rate_hz: 16_000,
            number_samples: 64_000,
            bands: [
                vec![FrequencyPeak { fft_pass_number: 1, peak_magnitude: 7000, corrected_peak_frequency_bin: 3000 }],
                vec![],
                vec![],
                vec![],
            ],
        };
        let body: serde_json::Value =
            serde_json::from_str(&request_body(&sig, 1_700_000_000_123)).unwrap();
        assert_eq!(body["signature"]["samplems"], 4000);
        assert!(body["signature"]["uri"]
            .as_str()
            .unwrap()
            .starts_with("data:audio/vnd.shazam.sig;base64,"));
        assert!(body["timestamp"].is_number());
        assert!(body["geolocation"]["latitude"].is_number());
        assert_eq!(body["timezone"], "Europe/Paris");
    }

    #[test]
    fn uuid_has_the_v4_shape_and_varies() {
        let a = random_uuid();
        let b = random_uuid();
        assert_eq!(a.len(), 36);
        assert_eq!(a.as_bytes()[14], b'4');
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    }
}
