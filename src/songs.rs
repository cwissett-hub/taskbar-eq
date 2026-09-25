//! Every song ever identified, one JSON object per line in `songs.jsonl` beside `config.toml`.
//! The store functions land in Task 4; the record type is here now because the Shazam parser
//! produces it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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

impl Default for Find {
    fn default() -> Self {
        Find {
            when: 0,
            title: String::new(),
            artist: String::new(),
            album: None,
            cover_url: None,
            shazam_url: None,
            apple_music_url: None,
            spotify_uri: None,
            isrc: None,
            shazam_key: String::new(),
            app_version: String::new(),
        }
    }
}
