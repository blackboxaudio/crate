use crate::error::{CrateError, Result};

use super::youtube::parse_youtube_url;
use super::{is_compilation, FetchedMetadata, FetchedTrack};

// =============================================================================
// Discogs
// =============================================================================

#[derive(Debug, PartialEq)]
pub(super) enum DiscogsUrlKind {
    Release(u64),
    Master(u64),
}

/// Parse a Discogs URL into a typed identifier.
///
/// Handles formats:
/// - `/release/12345`, `/release/12345-Slug`
/// - `/master/67890`, `/master/67890-Slug`
/// - `/{artist-slug}/release/12345` (older format)
pub(super) fn parse_discogs_url(url: &str) -> Option<DiscogsUrlKind> {
    let url_without_query = url.split('?').next().unwrap_or(url);
    let path = url_without_query
        .strip_prefix("https://")
        .or_else(|| url_without_query.strip_prefix("http://"))
        .and_then(|s| s.find('/').map(|i| &s[i..]))
        .unwrap_or(url_without_query);

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    for (i, seg) in segments.iter().enumerate() {
        let kind = match seg.to_lowercase().as_str() {
            "release" => Some("release"),
            "master" => Some("master"),
            _ => None,
        };
        if let Some(kind_str) = kind {
            if let Some(id_seg) = segments.get(i + 1) {
                // ID may be "12345" or "12345-Some-Slug"
                let id_part = id_seg.split('-').next().unwrap_or(id_seg);
                if let Ok(id) = id_part.parse::<u64>() {
                    return match kind_str {
                        "release" => Some(DiscogsUrlKind::Release(id)),
                        "master" => Some(DiscogsUrlKind::Master(id)),
                        _ => None,
                    };
                }
            }
        }
    }

    None
}

/// Parse a Discogs duration string (e.g. `"4:30"`, `"1:02:15"`) to milliseconds.
pub(super) fn parse_discogs_duration(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let parts: Vec<&str> = s.split(':').collect();
    let total_secs = match parts.len() {
        2 => {
            let mins: u64 = parts[0].parse().ok()?;
            let secs: u64 = parts[1].parse().ok()?;
            mins * 60 + secs
        }
        3 => {
            let hours: u64 = parts[0].parse().ok()?;
            let mins: u64 = parts[1].parse().ok()?;
            let secs: u64 = parts[2].parse().ok()?;
            hours * 3600 + mins * 60 + secs
        }
        _ => return None,
    };

    if total_secs > 0 {
        Some(total_secs as i64 * 1000)
    } else {
        None
    }
}

/// Strip Discogs disambiguation suffixes like `" (2)"` from artist names.
/// Preserves non-numeric suffixes like `" (UK)"`.
pub(super) fn strip_discogs_suffix(name: &str) -> &str {
    if let Some(open) = name.rfind(" (") {
        if name.ends_with(')') {
            let inner = &name[open + 2..name.len() - 1];
            if inner.chars().all(|c| c.is_ascii_digit()) {
                return &name[..open];
            }
        }
    }
    name
}

/// Join a Discogs `artists` array, respecting the `join` field between entries.
pub(super) fn join_discogs_artists(artists: &[serde_json::Value]) -> Option<String> {
    if artists.is_empty() {
        return None;
    }

    let mut result = String::new();
    for (i, artist) in artists.iter().enumerate() {
        let name = artist.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let cleaned = strip_discogs_suffix(name);
        result.push_str(cleaned);

        if i < artists.len() - 1 {
            let join = artist.get("join").and_then(|j| j.as_str()).unwrap_or(", ");
            // Ensure spacing around join separators (no leading space for punctuation like commas)
            if !join.starts_with(' ') && !join.starts_with(',') && !join.starts_with(';') {
                result.push(' ');
            }
            result.push_str(join);
            if !join.ends_with(' ') {
                result.push(' ');
            }
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

pub(super) async fn fetch_discogs(client: &reqwest::Client, url: &str) -> Result<FetchedMetadata> {
    let kind = parse_discogs_url(url)
        .ok_or_else(|| CrateError::Discovery("Could not parse Discogs URL".into()))?;

    // For master releases, resolve to the main release first
    let release_id = match kind {
        DiscogsUrlKind::Release(id) => id,
        DiscogsUrlKind::Master(id) => {
            let master_url = format!("https://api.discogs.com/masters/{id}");
            let resp: serde_json::Value = client
                .get(&master_url)
                .header("User-Agent", "CrateApp/0.1")
                .send()
                .await
                .map_err(|e| CrateError::Discovery(format!("Failed to fetch Discogs master: {e}")))?
                .json()
                .await
                .map_err(|e| {
                    CrateError::Discovery(format!("Failed to parse Discogs master: {e}"))
                })?;

            if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
                return Err(CrateError::Discovery(format!("Discogs API error: {msg}")));
            }

            resp.get("main_release")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| CrateError::Discovery("Discogs master has no main_release".into()))?
        }
    };

    let release_url = format!("https://api.discogs.com/releases/{release_id}");
    let resp: serde_json::Value = client
        .get(&release_url)
        .header("User-Agent", "CrateApp/0.1")
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch Discogs release: {e}")))?
        .json()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to parse Discogs release: {e}")))?;

    if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
        return Err(CrateError::Discovery(format!("Discogs API error: {msg}")));
    }

    let artist = resp
        .get("artists")
        .and_then(|a| a.as_array())
        .and_then(|arr| join_discogs_artists(arr));

    let title = resp
        .get("title")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    let label = resp
        .get("labels")
        .and_then(|l| l.as_array())
        .and_then(|arr| arr.first())
        .and_then(|l| l.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());

    // Prefer full `released` date, fallback to `year`
    let release_date = resp
        .get("released")
        .and_then(|d| d.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            resp.get("year")
                .and_then(|y| y.as_u64())
                .map(|y| format!("{y}"))
        });

    // Prefer primary image, fallback to first image
    let artwork_url = resp
        .get("images")
        .and_then(|i| i.as_array())
        .and_then(|images| {
            images
                .iter()
                .find(|img| img.get("type").and_then(|t| t.as_str()) == Some("primary"))
                .or_else(|| images.first())
        })
        .and_then(|img| img.get("uri").and_then(|u| u.as_str()))
        .map(|s| s.to_string());

    let is_comp = is_compilation(&artist);

    // Extract tracks from tracklist, filtering to actual tracks (skip headings)
    let mut tracks = resp
        .get("tracklist")
        .and_then(|t| t.as_array())
        .map(|tracklist| {
            tracklist
                .iter()
                .filter(|t| t.get("type_").and_then(|ty| ty.as_str()).unwrap_or("track") == "track")
                .enumerate()
                .filter_map(|(idx, track)| {
                    let raw_name = track
                        .get("title")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())?;

                    // For compilations, prepend the per-track artist to preserve it
                    let name = if is_comp {
                        track
                            .get("artists")
                            .and_then(|a| a.as_array())
                            .and_then(|arr| join_discogs_artists(arr))
                            .map(|track_artist| format!("{track_artist} - {raw_name}"))
                            .unwrap_or(raw_name)
                    } else {
                        raw_name
                    };

                    let duration_ms = track
                        .get("duration")
                        .and_then(|d| d.as_str())
                        .and_then(parse_discogs_duration);
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

    // Fallback: fill missing track durations from video metadata
    if let Some(videos) = resp.get("videos").and_then(|v| v.as_array()) {
        for track in tracks.iter_mut().filter(|t| t.duration_ms.is_none()) {
            let track_name_lower = track.name.to_ascii_lowercase();

            if let Some(video_duration) = videos.iter().find_map(|video| {
                let video_title = video.get("title").and_then(|t| t.as_str())?;
                let video_title_lower = video_title.to_ascii_lowercase();

                if video_title_lower == track_name_lower
                    || video_title_lower.contains(&track_name_lower)
                {
                    video
                        .get("duration")
                        .and_then(|d| d.as_u64())
                        .filter(|&d| d > 0)
                } else {
                    None
                }
            }) {
                track.duration_ms = Some(video_duration as i64 * 1000);
            }
        }
    }

    // Extract YouTube video IDs from videos[] and assign to tracks
    if let Some(videos) = resp.get("videos").and_then(|v| v.as_array()) {
        let yt_videos: Vec<(String, String)> = videos
            .iter()
            .filter_map(|v| {
                let uri = v.get("uri").and_then(|u| u.as_str())?;
                let title = v.get("title").and_then(|t| t.as_str())?;
                let video_id = parse_youtube_url(uri).video_id?;
                Some((video_id, title.to_string()))
            })
            .collect();

        if !yt_videos.is_empty() {
            // Title-based matching: strip "Artist - " prefix from video title before comparing
            let artist_prefix = artist.as_deref().unwrap_or("");
            for track in tracks.iter_mut() {
                if track.video_id.is_some() {
                    continue;
                }
                let track_name_lower = track.name.to_ascii_lowercase();
                if let Some((vid_id, _)) = yt_videos.iter().find(|(_, vtitle)| {
                    let vtitle_lower = vtitle.to_ascii_lowercase();
                    let stripped = if !artist_prefix.is_empty() {
                        let prefix = format!("{} - ", artist_prefix.to_ascii_lowercase());
                        vtitle_lower
                            .strip_prefix(&prefix)
                            .unwrap_or(&vtitle_lower)
                            .to_string()
                    } else {
                        vtitle_lower.clone()
                    };
                    stripped == track_name_lower || stripped.contains(&track_name_lower)
                }) {
                    track.video_id = Some(vid_id.clone());
                }
            }

            // Positional fallback: if all tracks are unmatched and counts align
            let all_unmatched = tracks.iter().all(|t| t.video_id.is_none());
            if all_unmatched && tracks.len() == yt_videos.len() {
                for (track, (vid_id, _)) in tracks.iter_mut().zip(yt_videos.iter()) {
                    track.video_id = Some(vid_id.clone());
                }
            }
        }
    }

    Ok(FetchedMetadata {
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
