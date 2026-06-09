use crate::error::{CrateError, Result};
use crate::models::ScannedRelease;

use super::{is_compilation, FetchedMetadata, FetchedTrack};

pub(super) fn parse_sc_hydration(html: &str) -> Option<FetchedMetadata> {
    // Find window.__sc_hydration JSON blob
    let marker = "window.__sc_hydration = ";
    let start = html.find(marker)? + marker.len();
    // The JSON ends with ";</script>" (no newline before the closing tag)
    let end = start + html[start..].find(";</script>")?;
    let json_str = &html[start..end];

    let hydration: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let arr = hydration.as_array()?;

    // Find the "sound" hydratable entry
    let sound_data = arr.iter().find_map(|entry| {
        if entry.get("hydratable")?.as_str()? == "sound" {
            entry.get("data")
        } else {
            None
        }
    })?;

    let raw_title = sound_data
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or_default();

    // Label accounts often use "Artist - Title" in the title field
    let (title_artist, title) = if let Some(idx) = raw_title.find(" - ") {
        (
            Some(raw_title[..idx].to_string()),
            Some(raw_title[idx + 3..].to_string()),
        )
    } else {
        (None, Some(raw_title.to_string()))
    };

    let pub_meta = sound_data.get("publisher_metadata");

    let artist = pub_meta
        .and_then(|pm| pm.get("artist"))
        .and_then(|a| a.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or(title_artist)
        .or_else(|| {
            sound_data
                .get("user")
                .and_then(|u| u.get("username"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
        });

    let label = pub_meta
        .and_then(|pm| pm.get("publisher"))
        .and_then(|p| p.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            sound_data
                .get("label_name")
                .and_then(|l| l.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        });

    let release_date = sound_data
        .get("release_date")
        .and_then(|d| d.as_str())
        .and_then(|s| s.get(..10))
        .map(|s| s.to_string());

    let artwork_url = sound_data
        .get("artwork_url")
        .and_then(|a| a.as_str())
        .map(|s| s.replace("-large", "-t500x500"));

    let duration_ms = sound_data.get("duration").and_then(|d| d.as_i64());

    let tracks = if let Some(name) = title.clone() {
        vec![FetchedTrack {
            name,
            position: 1,
            duration_ms,
            video_id: None,
        }]
    } else {
        Vec::new()
    };

    Some(FetchedMetadata {
        artist,
        title,
        label,
        release_date,
        artwork_url,
        tracks,
        source_type: String::new(),
        parent_url: None,
        parent_album_title: None,
    })
}

/// Check if a SoundCloud URL is a set/playlist URL
pub(super) fn is_soundcloud_set(url: &str) -> bool {
    url.to_lowercase().contains("/sets/")
}

pub(super) fn parse_sc_playlist_hydration(html: &str) -> Option<FetchedMetadata> {
    let marker = "window.__sc_hydration = ";
    let start = html.find(marker)? + marker.len();
    let end = start + html[start..].find(";</script>")?;
    let json_str = &html[start..end];

    let hydration: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let arr = hydration.as_array()?;

    let playlist_data = arr.iter().find_map(|entry| {
        if entry.get("hydratable")?.as_str()? == "playlist" {
            entry.get("data")
        } else {
            None
        }
    })?;

    let raw_title = playlist_data
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or_default();

    // Label accounts often use "Artist - Title" in the title field
    let (title_artist, title) = if let Some(idx) = raw_title.find(" - ") {
        (
            Some(raw_title[..idx].to_string()),
            Some(raw_title[idx + 3..].to_string()),
        )
    } else {
        (None, Some(raw_title.to_string()))
    };

    let pub_meta = playlist_data.get("publisher_metadata");

    let artist = pub_meta
        .and_then(|pm| pm.get("artist"))
        .and_then(|a| a.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or(title_artist)
        .or_else(|| {
            playlist_data
                .get("user")
                .and_then(|u| u.get("username"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
        });

    let label = pub_meta
        .and_then(|pm| pm.get("publisher"))
        .and_then(|p| p.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            playlist_data
                .get("label_name")
                .and_then(|l| l.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        });

    let release_date = playlist_data
        .get("release_date")
        .and_then(|d| d.as_str())
        .and_then(|s| s.get(..10))
        .map(|s| s.to_string());

    let artwork_url = playlist_data
        .get("artwork_url")
        .and_then(|a| a.as_str())
        .map(|s| s.replace("-large", "-t500x500"));

    // Extract tracks from the playlist
    let tracks = playlist_data
        .get("tracks")
        .and_then(|t| t.as_array())
        .map(|track_arr| {
            track_arr
                .iter()
                .enumerate()
                .filter_map(|(idx, track)| {
                    let raw_name = track.get("title").and_then(|t| t.as_str())?.to_string();
                    let name = if !is_compilation(&artist) {
                        artist
                            .as_ref()
                            .and_then(|a| {
                                let prefix = format!("{a} - ");
                                raw_name.strip_prefix(&prefix).map(|s| s.to_string())
                            })
                            .unwrap_or(raw_name)
                    } else {
                        raw_name
                    };
                    let duration_ms = track.get("duration").and_then(|d| d.as_i64());
                    Some(FetchedTrack {
                        name,
                        position: (idx + 1) as i32,
                        duration_ms,
                        video_id: None,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(FetchedMetadata {
        artist,
        title,
        label,
        release_date,
        artwork_url,
        tracks,
        source_type: String::new(),
        parent_url: None,
        parent_album_title: None,
    })
}

pub(super) async fn fetch_soundcloud(
    client: &reqwest::Client,
    url: &str,
) -> Result<FetchedMetadata> {
    // HTML-first strategy: fetch the page and try hydration data
    let html = client
        .get(url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch SoundCloud page: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read SoundCloud response: {e}")))?;

    // Try playlist hydration first for set URLs
    if is_soundcloud_set(url) {
        if let Some(metadata) = parse_sc_playlist_hydration(&html) {
            return Ok(metadata);
        }
    }

    if let Some(metadata) = parse_sc_hydration(&html) {
        return Ok(metadata);
    }

    log::warn!("SoundCloud hydration parsing failed for {url}, falling back to oEmbed");
    fetch_soundcloud_oembed(client, url).await
}

async fn fetch_soundcloud_oembed(client: &reqwest::Client, url: &str) -> Result<FetchedMetadata> {
    let oembed_url = format!("https://soundcloud.com/oembed?url={url}&format=json");

    let resp: serde_json::Value = client
        .get(&oembed_url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch SoundCloud oEmbed: {e}")))?
        .json()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to parse SoundCloud oEmbed: {e}")))?;

    let full_title = resp
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or_default();

    let author_name = resp
        .get("author_name")
        .and_then(|a| a.as_str())
        .unwrap_or_default();

    // SoundCloud titles are often "Artist - Title" or "Title by Artist"
    let (artist, title) = if let Some(idx) = full_title.find(" - ") {
        (
            Some(full_title[..idx].to_string()),
            Some(full_title[idx + 3..].to_string()),
        )
    } else if !author_name.is_empty() {
        let by_suffix = format!(" by {author_name}");
        let cleaned_title = full_title
            .strip_suffix(&by_suffix)
            .unwrap_or(full_title)
            .to_string();
        (Some(author_name.to_string()), Some(cleaned_title))
    } else {
        (None, Some(full_title.to_string()))
    };

    let artwork_url = resp
        .get("thumbnail_url")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    Ok(FetchedMetadata {
        artist,
        title,
        label: None,
        release_date: None,
        artwork_url,
        tracks: Vec::new(),
        source_type: String::new(),
        parent_url: None,
        parent_album_title: None,
    })
}

/// SoundCloud paths that look like a one-segment profile but aren't user accounts.
fn is_reserved_sc_path(seg: &str) -> bool {
    matches!(
        seg,
        "discover"
            | "stream"
            | "you"
            | "search"
            | "upload"
            | "settings"
            | "notifications"
            | "messages"
            | "popular"
            | "charts"
            | "tags"
            | "people"
            | "pages"
            | "jobs"
            | "developers"
            | "mobile"
            | "terms-of-use"
            | "imprint"
    )
}

/// Whether a URL is a SoundCloud artist/label PROFILE page (one path segment), as
/// opposed to a track (`/user/track`), a set (`/user/sets/...`), or a reserved path.
pub(super) fn is_soundcloud_page_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    if !lower.contains("soundcloud.com/") || lower.contains("/sets/") {
        return false;
    }
    let Some((_, after_host)) = lower.split_once("soundcloud.com/") else {
        return false;
    };
    let path = after_host.split(['?', '#']).next().unwrap_or("");
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    segments.len() == 1 && !is_reserved_sc_path(segments[0])
}

/// Scan a SoundCloud profile for the artist/label's own uploads. Uses the api-v2
/// `/users/{id}/tracks` endpoint, which returns only the profile's own tracks — reposts
/// are excluded by construction, so the follow feed isn't polluted with reposts.
pub(super) async fn scan_soundcloud_page(
    client: &reqwest::Client,
    url: &str,
) -> Result<(Vec<ScannedRelease>, Option<String>, Option<String>)> {
    let client_id = crate::services::discovery::streams::resolve_sc_client_id(client).await?;

    // Resolve the profile URL to a user object.
    let user: serde_json::Value = client
        .get("https://api-v2.soundcloud.com/resolve")
        .query(&[("url", url), ("client_id", client_id.as_str())])
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to resolve SoundCloud profile: {e}")))?
        .json()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to parse SoundCloud profile: {e}")))?;

    if user.get("kind").and_then(|k| k.as_str()) != Some("user") {
        return Err(CrateError::Discovery(
            "URL is not a SoundCloud user profile".into(),
        ));
    }
    let user_id = user
        .get("id")
        .and_then(|i| i.as_i64())
        .ok_or_else(|| CrateError::Discovery("SoundCloud user has no id".into()))?;
    let page_name = user
        .get("username")
        .and_then(|u| u.as_str())
        .map(|s| s.to_string());
    // Profile avatar — upgrade the default "-large" (100px) variant to a larger one.
    let avatar_url = user
        .get("avatar_url")
        .and_then(|a| a.as_str())
        .map(|s| s.replace("-large", "-t500x500"));

    // Own uploads only (this endpoint excludes reposts).
    let resp: serde_json::Value = client
        .get(format!(
            "https://api-v2.soundcloud.com/users/{user_id}/tracks"
        ))
        .query(&[
            ("client_id", client_id.as_str()),
            ("limit", "50"),
            ("linked_partitioning", "1"),
        ])
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch SoundCloud tracks: {e}")))?
        .json()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to parse SoundCloud tracks: {e}")))?;

    let mut releases = Vec::new();
    if let Some(items) = resp.get("collection").and_then(|c| c.as_array()) {
        for item in items {
            let Some(permalink) = item.get("permalink_url").and_then(|u| u.as_str()) else {
                continue;
            };
            let title = item
                .get("title")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());
            let artist = item
                .get("user")
                .and_then(|u| u.get("username"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
                .or_else(|| page_name.clone());
            let artwork_url = item
                .get("artwork_url")
                .and_then(|a| a.as_str())
                .map(|s| s.replace("-large", "-t500x500"));
            let release_date = item
                .get("display_date")
                .or_else(|| item.get("created_at"))
                .and_then(|d| d.as_str())
                .and_then(|s| s.get(..10))
                .map(|s| s.to_string());
            releases.push(ScannedRelease {
                url: permalink.to_string(),
                artist,
                title,
                artwork_url,
                release_date,
                already_exists: false,
            });
        }
    }

    Ok((releases, page_name, avatar_url))
}
