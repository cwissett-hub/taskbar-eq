//! Every song ever identified, one JSON object per line in `songs.jsonl` beside `config.toml`.
//!
//! Append-only JSON lines rather than a database: a crash mid-write can only damage the last line,
//! there is no schema to migrate, and a few hundred rows is nothing. Repeat finds are separate
//! lines so the history can say first heard / last heard / how many times.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Find {
    /// Unix seconds.
    pub when: i64,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub cover_url: Option<String>,
    pub shazam_url: Option<String>,
    pub apple_music_url: Option<String>,
    /// `spotify:track:<id>` only; Shazam's `spotify:search:` deeplinks are not track links.
    pub spotify_uri: Option<String>,
    pub isrc: Option<String>,
    /// Shazam's track key. Identity for "same song".
    pub shazam_key: String,
    pub app_version: String,
}


pub fn path() -> PathBuf {
    crate::config::Config::dir().join("songs.jsonl")
}

pub fn append(f: &Find) -> std::io::Result<()> {
    std::fs::create_dir_all(crate::config::Config::dir())?;
    append_to(&path(), f)
}

pub fn append_to(path: &Path, f: &Find) -> std::io::Result<()> {
    let line = serde_json::to_string(f).map_err(std::io::Error::other)?;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")
}

/// Never fails: a missing file is an empty history and a bad line is logged and skipped.
pub fn load() -> Vec<Find> {
    let (v, bad) = load_from(&path());
    if bad > 0 {
        crate::log::write(&format!("songs.jsonl: skipped {bad} unreadable line(s)"));
    }
    v
}

pub fn load_from(path: &Path) -> (Vec<Find>, usize) {
    let Ok(file) = std::fs::File::open(path) else {
        return (Vec::new(), 0);
    };
    let mut out = Vec::new();
    let mut bad = 0;
    for line in std::io::BufReader::new(file).lines() {
        let Ok(line) = line else {
            bad += 1;
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Find>(&line) {
            Ok(f) if !f.shazam_key.is_empty() => out.push(f),
            _ => bad += 1,
        }
    }
    (out, bad)
}

/// Newest first, one entry per song (its newest occurrence).
pub fn recent_distinct(finds: &[Find], n: usize) -> Vec<Find> {
    let mut seen = std::collections::HashSet::new();
    let mut sorted: Vec<&Find> = finds.iter().collect();
    sorted.sort_by_key(|f| std::cmp::Reverse(f.when));
    sorted
        .into_iter()
        .filter(|f| seen.insert(f.shazam_key.clone()))
        .take(n)
        .cloned()
        .collect()
}

pub struct Grouped {
    /// The newest occurrence, whose links and cover are shown.
    pub find: Find,
    pub first: i64,
    pub last: i64,
    pub count: u32,
}

/// One row per song, last-heard descending.
pub fn grouped(finds: &[Find]) -> Vec<Grouped> {
    let mut map: std::collections::HashMap<&str, Grouped> = std::collections::HashMap::new();
    for f in finds {
        match map.get_mut(f.shazam_key.as_str()) {
            Some(g) => {
                g.first = g.first.min(f.when);
                if f.when >= g.last {
                    g.last = f.when;
                    g.find = f.clone();
                }
                g.count += 1;
            }
            None => {
                map.insert(
                    &f.shazam_key,
                    Grouped { find: f.clone(), first: f.when, last: f.when, count: 1 },
                );
            }
        }
    }
    let mut v: Vec<Grouped> = map.into_values().collect();
    v.sort_by_key(|g| std::cmp::Reverse(g.last));
    v
}

/// RFC 3986 unreserved characters pass; everything else, byte-wise, becomes %XX.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn query(f: &Find) -> String {
    percent_encode(&format!("{} {}", f.title, f.artist))
}

pub fn spotify_url(f: &Find) -> String {
    match f.spotify_uri.as_deref().and_then(|u| u.strip_prefix("spotify:track:")) {
        Some(id) => format!("https://open.spotify.com/track/{id}"),
        None => format!("https://open.spotify.com/search/{}", query(f)),
    }
}

pub fn apple_url(f: &Find) -> String {
    f.apple_music_url
        .clone()
        .unwrap_or_else(|| format!("https://music.apple.com/search?term={}", query(f)))
}

pub fn youtube_url(f: &Find) -> String {
    format!("https://www.youtube.com/results?search_query={}", query(f))
}

pub fn shazam_url(f: &Find) -> String {
    f.shazam_url
        .clone()
        .unwrap_or_else(|| format!("https://www.shazam.com/search?q={}", query(f)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(key: &str, when: i64) -> Find {
        Find {
            when,
            title: format!("Song {key}"),
            artist: "Artist & Co".into(),
            shazam_key: key.into(),
            app_version: "t".into(),
            ..Default::default()
        }
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir()
            .join(format!("taskbar-eq-songs-test-{name}-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn append_then_load_round_trips_in_order() {
        let p = temp("rt");
        append_to(&p, &find("a", 1)).unwrap();
        append_to(&p, &find("b", 2)).unwrap();
        let (v, bad) = load_from(&p);
        assert_eq!(bad, 0);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].shazam_key, "a");
        assert_eq!(v[1].when, 2);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn a_bad_line_is_skipped_not_fatal() {
        let p = temp("bad");
        append_to(&p, &find("a", 1)).unwrap();
        std::fs::write(&p, format!("{}{{not json\n", std::fs::read_to_string(&p).unwrap())).unwrap();
        append_to(&p, &find("b", 2)).unwrap();
        let (v, bad) = load_from(&p);
        assert_eq!(bad, 1);
        assert_eq!(v.len(), 2);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn missing_file_is_empty() {
        let (v, bad) = load_from(&temp("missing"));
        assert!(v.is_empty());
        assert_eq!(bad, 0);
    }

    #[test]
    fn recent_distinct_is_newest_first_and_deduped() {
        let v = vec![find("a", 1), find("b", 2), find("a", 3), find("c", 4)];
        let r = recent_distinct(&v, 10);
        let keys: Vec<&str> = r.iter().map(|f| f.shazam_key.as_str()).collect();
        assert_eq!(keys, ["c", "a", "b"]);
        assert_eq!(r[1].when, 3, "the NEWEST occurrence is kept");
        assert_eq!(recent_distinct(&v, 2).len(), 2);
    }

    #[test]
    fn grouped_counts_and_spans() {
        let v = vec![find("a", 1), find("b", 2), find("a", 3)];
        let g = grouped(&v);
        assert_eq!(g[0].find.shazam_key, "a");
        assert_eq!((g[0].first, g[0].last, g[0].count), (1, 3, 2));
        assert_eq!((g[1].first, g[1].last, g[1].count), (2, 2, 1));
    }

    #[test]
    fn urls_prefer_real_links_and_fall_back_to_encoded_searches() {
        let mut f = find("a", 1);
        assert_eq!(spotify_url(&f), "https://open.spotify.com/search/Song%20a%20Artist%20%26%20Co");
        assert_eq!(
            youtube_url(&f),
            "https://www.youtube.com/results?search_query=Song%20a%20Artist%20%26%20Co"
        );
        assert_eq!(apple_url(&f), "https://music.apple.com/search?term=Song%20a%20Artist%20%26%20Co");
        assert_eq!(shazam_url(&f), "https://www.shazam.com/search?q=Song%20a%20Artist%20%26%20Co");
        f.spotify_uri = Some("spotify:track:XYZ".into());
        f.apple_music_url = Some("https://music.apple.com/x".into());
        f.shazam_url = Some("https://www.shazam.com/track/1".into());
        assert_eq!(spotify_url(&f), "https://open.spotify.com/track/XYZ");
        assert_eq!(apple_url(&f), "https://music.apple.com/x");
        assert_eq!(shazam_url(&f), "https://www.shazam.com/track/1");
    }

    #[test]
    fn percent_encoding_covers_reserved_and_unicode() {
        assert_eq!(percent_encode("a b&c/d?e#f"), "a%20b%26c%2Fd%3Fe%23f");
        assert_eq!(percent_encode("Zoë"), "Zo%C3%AB");
        assert_eq!(percent_encode("safe-._~09Az"), "safe-._~09Az");
    }
}
