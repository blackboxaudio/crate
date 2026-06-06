//! Models for the "follow artists & labels" feature (issue #126).
//!
//! A `FollowedSource` flattens the synced `followed_sources` row together with the
//! device-local `followed_source_state` and a couple of computed counts, so the
//! frontend gets everything it needs for the Following modal in one object.

use serde::{Deserialize, Serialize};

/// Whether a followed source is an artist or a label. Stored as a plain string in
/// the DB (`'artist'` / `'label'`), mirroring how `source_type` is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FollowType {
    #[default]
    Artist,
    Label,
}

impl std::fmt::Display for FollowType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FollowType::Artist => write!(f, "artist"),
            FollowType::Label => write!(f, "label"),
        }
    }
}

impl std::str::FromStr for FollowType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "artist" => Ok(FollowType::Artist),
            "label" => Ok(FollowType::Label),
            _ => Err(format!("Unknown follow type: {s}")),
        }
    }
}

/// Per-source health for the watch loop (device-local; never synced).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FollowHealth {
    #[default]
    Unknown,
    Ok,
    Error,
    RateLimited,
}

impl std::fmt::Display for FollowHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FollowHealth::Unknown => write!(f, "unknown"),
            FollowHealth::Ok => write!(f, "ok"),
            FollowHealth::Error => write!(f, "error"),
            FollowHealth::RateLimited => write!(f, "rate_limited"),
        }
    }
}

impl std::str::FromStr for FollowHealth {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "ok" => FollowHealth::Ok,
            "error" => FollowHealth::Error,
            "rate_limited" => FollowHealth::RateLimited,
            _ => FollowHealth::Unknown,
        })
    }
}

/// A followed artist or label, flattened for the frontend. The first block mirrors
/// the synced `followed_sources` row; the trailing fields are joined from the local
/// `followed_source_state` and computed counts (defaulted when absent).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowedSource {
    pub id: String,
    pub url: String,
    pub source_type: String,
    pub follow_type: String,
    pub name: Option<String>,
    pub artwork_url: Option<String>,
    pub artwork_path: Option<String>,
    pub enabled: bool,
    pub date_added: String,
    pub date_modified: String,
    #[serde(default)]
    pub last_checked_at: Option<String>,
    #[serde(default)]
    pub health: String,
    #[serde(default)]
    pub last_error: Option<String>,
    /// Releases surfaced by this source that are still flagged "new".
    #[serde(default)]
    pub new_count: i64,
    /// Most recent `release_date` among this source's surfaced releases (for the
    /// "Recently released" sort).
    #[serde(default)]
    pub last_release_at: Option<String>,
}

/// Input to follow a source. From a pasted URL only `url` is set (the service scans
/// to fill the rest); from a known release (the row popover) the caller supplies the
/// entity details so no fetch is needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowedSourceCreate {
    pub url: String,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub follow_type: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub artwork_url: Option<String>,
}
