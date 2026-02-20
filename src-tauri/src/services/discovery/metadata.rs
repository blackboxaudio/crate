use serde::Serialize;

use crate::error::{CrateError, Result};

use super::detect_source_type;

#[derive(Debug, Clone, Serialize)]
pub struct FetchedMetadata {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub label: Option<String>,
    pub release_date: Option<String>,
    pub artwork_url: Option<String>,
    pub tracks: Vec<FetchedTrack>,
    pub source_type: String,
    pub parent_url: Option<String>,
    pub parent_album_title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchedTrack {
    pub name: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
}

pub(super) fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (compatible; CrateApp/0.1)")
        .build()
        .map_err(|e| CrateError::Discovery(format!("Failed to create HTTP client: {e}")))
}

pub async fn fetch_metadata(url: &str) -> Result<FetchedMetadata> {
    let client = build_client()?;
    let source_type = detect_source_type(url);

    let mut metadata = match source_type.as_str() {
        "bandcamp" => fetch_bandcamp(&client, url).await,
        "soundcloud" => fetch_soundcloud(&client, url).await,
        "youtube" => fetch_youtube(&client, url).await,
        "discogs" => fetch_discogs(&client, url).await,
        _ => fetch_generic(&client, url).await,
    }?;

    metadata.source_type = source_type;
    metadata.release_date = metadata.release_date.map(|d| normalize_date(&d));
    Ok(metadata)
}

/// Normalize a date string to `YYYY-MM-DD` format.
///
/// Handles:
/// - Already ISO format (`2024-06-19...`) → returns first 10 chars
/// - Bandcamp's `"DD Mon YYYY HH:MM:SS GMT"` format → converts to ISO
/// - Falls back to returning the string as-is
fn normalize_date(date_str: &str) -> String {
    let trimmed = date_str.trim();

    // Already ISO format (starts with YYYY-MM-DD)
    if trimmed.len() >= 10 {
        let bytes = trimmed.as_bytes();
        if bytes[4] == b'-' && bytes[7] == b'-' && bytes[..4].iter().all(|b| b.is_ascii_digit()) {
            return trimmed[..10].to_string();
        }
    }

    // Bandcamp format: "19 Jun 2024 00:00:00 GMT"
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() >= 3 {
        if let (Ok(day), Some(month_num), Ok(year)) = (
            parts[0].parse::<u32>(),
            month_abbrev_to_num(parts[1]),
            parts[2].parse::<u32>(),
        ) {
            if (1..=31).contains(&day) && (1000..=9999).contains(&year) {
                return format!("{year:04}-{month_num:02}-{day:02}");
            }
        }
    }

    trimmed.to_string()
}

fn month_abbrev_to_num(s: &str) -> Option<u32> {
    match s.to_ascii_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

async fn fetch_bandcamp(client: &reqwest::Client, url: &str) -> Result<FetchedMetadata> {
    let html = client
        .get(url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch Bandcamp page: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read Bandcamp response: {e}")))?;

    // Try JSON-LD first (structured data)
    if let Some(metadata) = parse_bandcamp_json_ld(&html) {
        return Ok(metadata);
    }

    // Fall back to OpenGraph meta tags
    Ok(FetchedMetadata {
        artist: extract_meta_content(&html, "og:site_name"),
        title: extract_meta_content(&html, "og:title"),
        label: None,
        release_date: None,
        artwork_url: extract_meta_content(&html, "og:image"),
        tracks: Vec::new(),
        source_type: String::new(),
        parent_url: None,
        parent_album_title: None,
    })
}

/// Extract the recognized schema.org `@type` from a JSON-LD value.
/// Handles both a plain string (`"@type": "MusicAlbum"`) and an array
/// (`"@type": ["MusicAlbum", "MusicRelease"]`).
fn extract_schema_type(value: &serde_json::Value) -> Option<&'static str> {
    const RECOGNIZED: &[&str] = &["MusicRelease", "MusicAlbum", "MusicRecording"];

    match value.get("@type") {
        Some(serde_json::Value::String(s)) => RECOGNIZED.iter().find(|&&r| r == s).copied(),
        Some(serde_json::Value::Array(arr)) => arr.iter().find_map(|v| {
            let s = v.as_str()?;
            RECOGNIZED.iter().find(|&&r| r == s).copied()
        }),
        _ => None,
    }
}

fn parse_bandcamp_json_ld(html: &str) -> Option<FetchedMetadata> {
    // Find <script type="application/ld+json"> blocks
    let mut search_from = 0;
    while let Some(start) = html[search_from..].find("<script type=\"application/ld+json\">") {
        let abs_start = search_from + start + "<script type=\"application/ld+json\">".len();
        if let Some(end) = html[abs_start..].find("</script>") {
            let json_str = &html[abs_start..abs_start + end];
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                let schema_type = match extract_schema_type(&value) {
                    Some(t) => t,
                    None => {
                        search_from = abs_start + end;
                        continue;
                    }
                };
                {
                    // For MusicRecording (individual track pages), Bandcamp puts the
                    // page owner in byArtist and the actual track artist in inAlbum.byArtist.
                    // Prefer inAlbum.byArtist.name, falling back to byArtist.name.
                    let artist = value
                        .get("inAlbum")
                        .and_then(|a| a.get("byArtist"))
                        .and_then(|a| a.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| {
                            value
                                .get("byArtist")
                                .and_then(|a| a.get("name"))
                                .and_then(|n| n.as_str())
                                .map(|s| s.to_string())
                        });

                    let title = value
                        .get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string());

                    let artwork_url = value
                        .get("image")
                        .and_then(|i| i.as_str())
                        .map(|s| s.to_string());

                    let release_date = value
                        .get("datePublished")
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string());

                    let label = value
                        .get("recordLabel")
                        .or_else(|| value.get("publisher"))
                        .and_then(|l| l.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                        .filter(|label_name| {
                            // Skip if publisher name matches artist (self-released)
                            artist.as_ref().is_none_or(|a| {
                                if a.eq_ignore_ascii_case(label_name) {
                                    return false;
                                }
                                // Also check if publisher matches any comma-separated
                                // artist part (e.g. "Apellum, Gansi" with publisher "Apellum")
                                !a.split(", ")
                                    .any(|part| part.eq_ignore_ascii_case(label_name))
                            })
                        });

                    // Parse tracks from albumRelease or track.itemListElement
                    let mut tracks = parse_bandcamp_tracks(&value);

                    // For MusicRecording (individual track pages), create a single
                    // track from the root-level name/duration when no track list exists
                    if tracks.is_empty() && schema_type == "MusicRecording" {
                        if let Some(name) = title.clone() {
                            let duration_ms = value
                                .get("duration")
                                .and_then(|d| d.as_str())
                                .and_then(parse_iso_duration);
                            tracks.push(FetchedTrack {
                                name,
                                position: 1,
                                duration_ms,
                            });
                        }
                    }

                    // For MusicRecording (individual track pages), extract parent album info
                    let (parent_url, parent_album_title) = if schema_type == "MusicRecording" {
                        let p_url = value
                            .get("inAlbum")
                            .and_then(|a| {
                                a.get("@id")
                                    .or_else(|| a.get("url"))
                                    .and_then(|u| u.as_str())
                            })
                            .map(|s| s.to_string());
                        let p_title = value
                            .get("inAlbum")
                            .and_then(|a| a.get("name"))
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string());
                        (p_url, p_title)
                    } else {
                        (None, None)
                    };

                    return Some(FetchedMetadata {
                        artist,
                        title,
                        label,
                        release_date,
                        artwork_url,
                        tracks,
                        source_type: String::new(),
                        parent_url,
                        parent_album_title,
                    });
                }
            }
            search_from = abs_start + end;
        } else {
            break;
        }
    }
    None
}

fn parse_bandcamp_tracks(value: &serde_json::Value) -> Vec<FetchedTrack> {
    // Try track.itemListElement (common in MusicAlbum)
    let items = value
        .get("track")
        .and_then(|t| t.get("itemListElement"))
        .and_then(|i| i.as_array());

    if let Some(items) = items {
        return items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                let track_item = item.get("item").unwrap_or(item);
                let name = track_item
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())?;

                let position = item
                    .get("position")
                    .and_then(|p| p.as_i64())
                    .unwrap_or(idx as i64 + 1) as i32;

                let duration_ms = track_item
                    .get("duration")
                    .and_then(|d| d.as_str())
                    .and_then(parse_iso_duration);

                Some(FetchedTrack {
                    name,
                    position,
                    duration_ms,
                })
            })
            .collect();
    }

    Vec::new()
}

fn parse_sc_hydration(html: &str) -> Option<FetchedMetadata> {
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
fn is_soundcloud_set(url: &str) -> bool {
    url.to_lowercase().contains("/sets/")
}

fn parse_sc_playlist_hydration(html: &str) -> Option<FetchedMetadata> {
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
                    let name = artist
                        .as_ref()
                        .and_then(|a| {
                            let prefix = format!("{a} - ");
                            raw_name.strip_prefix(&prefix).map(|s| s.to_string())
                        })
                        .unwrap_or(raw_name);
                    let duration_ms = track.get("duration").and_then(|d| d.as_i64());
                    Some(FetchedTrack {
                        name,
                        position: (idx + 1) as i32,
                        duration_ms,
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

async fn fetch_soundcloud(client: &reqwest::Client, url: &str) -> Result<FetchedMetadata> {
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

async fn fetch_youtube(client: &reqwest::Client, url: &str) -> Result<FetchedMetadata> {
    let oembed_url = format!("https://www.youtube.com/oembed?url={url}&format=json");

    let resp: serde_json::Value = client
        .get(&oembed_url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch YouTube oEmbed: {e}")))?
        .json()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to parse YouTube oEmbed: {e}")))?;

    let title = resp
        .get("title")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    let artist = resp
        .get("author_name")
        .and_then(|a| a.as_str())
        .map(|s| s.to_string());

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

// =============================================================================
// Discogs
// =============================================================================

#[derive(Debug, PartialEq)]
enum DiscogsUrlKind {
    Release(u64),
    Master(u64),
}

/// Parse a Discogs URL into a typed identifier.
///
/// Handles formats:
/// - `/release/12345`, `/release/12345-Slug`
/// - `/master/67890`, `/master/67890-Slug`
/// - `/{artist-slug}/release/12345` (older format)
fn parse_discogs_url(url: &str) -> Option<DiscogsUrlKind> {
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
fn parse_discogs_duration(s: &str) -> Option<i64> {
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
fn strip_discogs_suffix(name: &str) -> &str {
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
fn join_discogs_artists(artists: &[serde_json::Value]) -> Option<String> {
    if artists.is_empty() {
        return None;
    }

    let mut result = String::new();
    for (i, artist) in artists.iter().enumerate() {
        let name = artist.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let cleaned = strip_discogs_suffix(name);
        result.push_str(cleaned);

        if i < artists.len() - 1 {
            let join = artist
                .get("join")
                .and_then(|j| j.as_str())
                .unwrap_or(", ");
            // Ensure spacing around join separators
            if !join.starts_with(' ') {
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

async fn fetch_discogs(client: &reqwest::Client, url: &str) -> Result<FetchedMetadata> {
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
                .map_err(|e| {
                    CrateError::Discovery(format!("Failed to fetch Discogs master: {e}"))
                })?
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
                .ok_or_else(|| {
                    CrateError::Discovery("Discogs master has no main_release".into())
                })?
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

    // Extract tracks from tracklist, filtering to actual tracks (skip headings)
    let mut tracks = resp
        .get("tracklist")
        .and_then(|t| t.as_array())
        .map(|tracklist| {
            tracklist
                .iter()
                .filter(|t| {
                    t.get("type_")
                        .and_then(|ty| ty.as_str())
                        .unwrap_or("track")
                        == "track"
                })
                .enumerate()
                .filter_map(|(idx, track)| {
                    let name = track
                        .get("title")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())?;
                    let duration_ms = track
                        .get("duration")
                        .and_then(|d| d.as_str())
                        .and_then(parse_discogs_duration);
                    Some(FetchedTrack {
                        name,
                        position: (idx + 1) as i32,
                        duration_ms,
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

async fn fetch_generic(client: &reqwest::Client, url: &str) -> Result<FetchedMetadata> {
    let html = client
        .get(url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch page: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read response: {e}")))?;

    Ok(FetchedMetadata {
        artist: extract_meta_content(&html, "og:site_name"),
        title: extract_meta_content(&html, "og:title"),
        label: None,
        release_date: None,
        artwork_url: extract_meta_content(&html, "og:image"),
        tracks: Vec::new(),
        source_type: String::new(),
        parent_url: None,
        parent_album_title: None,
    })
}

/// Parse ISO 8601 duration (e.g., "PT3M45S" or "P00H03M45S") to milliseconds.
fn parse_iso_duration(s: &str) -> Option<i64> {
    // Try standard "PT..." first, then fall back to "P..." (Bandcamp uses P00H06M12S)
    let s = s.strip_prefix("PT").or_else(|| s.strip_prefix("P"))?;
    let mut total_ms: i64 = 0;
    let mut num_buf = String::new();

    for ch in s.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num_buf.push(ch);
        } else {
            let val: f64 = num_buf.parse().ok()?;
            num_buf.clear();
            match ch {
                'H' => total_ms += (val * 3_600_000.0) as i64,
                'M' => total_ms += (val * 60_000.0) as i64,
                'S' => total_ms += (val * 1_000.0) as i64,
                _ => {}
            }
        }
    }

    if total_ms > 0 {
        Some(total_ms)
    } else {
        None
    }
}

/// Extract content from an OpenGraph meta tag.
fn extract_meta_content(html: &str, property: &str) -> Option<String> {
    // Match both property="..." and name="..." patterns
    for attr in ["property", "name"] {
        let pattern = format!("{attr}=\"{property}\"");
        if let Some(pos) = html.find(&pattern) {
            // Look for content="..." nearby (within the same tag)
            let tag_start = html[..pos].rfind('<')?;
            let tag_end = html[pos..].find('>')? + pos;
            let tag = &html[tag_start..=tag_end];

            if let Some(content_start) = tag.find("content=\"") {
                let value_start = content_start + "content=\"".len();
                if let Some(value_end) = tag[value_start..].find('"') {
                    let value = &tag[value_start..value_start + value_end];
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_date_iso_format() {
        assert_eq!(normalize_date("2024-06-19"), "2024-06-19");
        assert_eq!(normalize_date("2025-05-08T00:00:00Z"), "2025-05-08");
        assert_eq!(normalize_date("2024-06-27T00:00:00Z"), "2024-06-27");
    }

    #[test]
    fn test_normalize_date_bandcamp_format() {
        assert_eq!(normalize_date("19 Jun 2024 00:00:00 GMT"), "2024-06-19");
        assert_eq!(normalize_date("01 Jan 2025 00:00:00 GMT"), "2025-01-01");
        assert_eq!(normalize_date("31 Dec 2023 00:00:00 GMT"), "2023-12-31");
    }

    #[test]
    fn test_normalize_date_passthrough() {
        assert_eq!(normalize_date("unknown"), "unknown");
        assert_eq!(normalize_date(""), "");
    }

    #[test]
    fn test_parse_iso_duration() {
        assert_eq!(parse_iso_duration("PT3M45S"), Some(225_000));
        assert_eq!(parse_iso_duration("PT1H2M3S"), Some(3_723_000));
        assert_eq!(parse_iso_duration("PT30S"), Some(30_000));
        assert_eq!(parse_iso_duration("PT5M"), Some(300_000));
        assert_eq!(parse_iso_duration("invalid"), None);
        // Bandcamp format: P prefix without T separator
        assert_eq!(parse_iso_duration("P00H03M45S"), Some(225_000));
        assert_eq!(parse_iso_duration("P00H06M12S"), Some(372_000));
        assert_eq!(parse_iso_duration("P01H02M03S"), Some(3_723_000));
    }

    #[test]
    fn test_extract_meta_content() {
        let html = r#"<html><head><meta property="og:title" content="Test Title"><meta property="og:image" content="https://example.com/img.jpg"></head></html>"#;
        assert_eq!(
            extract_meta_content(html, "og:title"),
            Some("Test Title".to_string())
        );
        assert_eq!(
            extract_meta_content(html, "og:image"),
            Some("https://example.com/img.jpg".to_string())
        );
        assert_eq!(extract_meta_content(html, "og:description"), None);
    }

    fn make_sc_hydration_html(hydration_json: &str) -> String {
        format!(
            "<html><head></head><body><script>window.__sc_hydration = {hydration_json};</script></body></html>"
        )
    }

    #[test]
    fn test_parse_sc_hydration() {
        let html = make_sc_hydration_html(
            r#"[
                {"hydratable": "user", "data": {}},
                {"hydratable": "sound", "data": {
                    "title": "Mind Fog",
                    "duration": 345000,
                    "artwork_url": "https://i1.sndcdn.com/artworks-abc-large.jpg",
                    "release_date": "2025-05-08T00:00:00Z",
                    "user": {"username": "Apellum"},
                    "publisher_metadata": {
                        "artist": "Apellum, Gansi",
                        "publisher": "Some Label"
                    }
                }}
            ]"#,
        );
        let meta = parse_sc_hydration(&html).expect("should parse hydration");
        assert_eq!(meta.title.as_deref(), Some("Mind Fog"));
        assert_eq!(meta.artist.as_deref(), Some("Apellum, Gansi"));
        assert_eq!(meta.label.as_deref(), Some("Some Label"));
        assert_eq!(meta.release_date.as_deref(), Some("2025-05-08"));
        assert_eq!(
            meta.artwork_url.as_deref(),
            Some("https://i1.sndcdn.com/artworks-abc-t500x500.jpg")
        );
        assert_eq!(meta.tracks.len(), 1);
        assert_eq!(meta.tracks[0].duration_ms, Some(345000));
    }

    #[test]
    fn test_parse_sc_hydration_fallback_to_user() {
        let html = make_sc_hydration_html(
            r#"[
                {"hydratable": "sound", "data": {
                    "title": "Some Track",
                    "duration": 200000,
                    "user": {"username": "DJ Test"},
                    "publisher_metadata": {}
                }}
            ]"#,
        );
        let meta = parse_sc_hydration(&html).expect("should parse hydration");
        assert_eq!(meta.artist.as_deref(), Some("DJ Test"));
        assert_eq!(meta.label, None);
        assert_eq!(meta.release_date, None);
    }

    #[test]
    fn test_parse_sc_hydration_label_upload() {
        let html = make_sc_hydration_html(
            r#"[
                {"hydratable": "sound", "data": {
                    "title": "Apellum - Sunshower",
                    "duration": 324046,
                    "artwork_url": "https://i1.sndcdn.com/artworks-xyz-large.jpg",
                    "release_date": "2024-06-27T00:00:00Z",
                    "label_name": "Perfect Dark",
                    "user": {"username": "Perfect Dark"},
                    "publisher_metadata": {
                        "artist": "Apellum"
                    }
                }}
            ]"#,
        );
        let meta = parse_sc_hydration(&html).expect("should parse hydration");
        assert_eq!(meta.title.as_deref(), Some("Sunshower"));
        assert_eq!(meta.artist.as_deref(), Some("Apellum"));
        assert_eq!(meta.label.as_deref(), Some("Perfect Dark"));
        assert_eq!(meta.release_date.as_deref(), Some("2024-06-27"));
    }

    #[test]
    fn test_parse_sc_hydration_no_sound() {
        let html = make_sc_hydration_html(r#"[{"hydratable": "user", "data": {}}]"#);
        assert!(parse_sc_hydration(&html).is_none());
    }

    fn make_bandcamp_json_ld_html(json_ld: &str) -> String {
        format!(
            r#"<html><head><script type="application/ld+json">{json_ld}</script></head><body></body></html>"#
        )
    }

    #[test]
    fn test_bandcamp_music_recording_creates_single_track() {
        let html = make_bandcamp_json_ld_html(
            r#"{
                "@type": "MusicRecording",
                "name": "Echoes",
                "duration": "P00H04M30S",
                "byArtist": {"name": "Test Artist"},
                "image": "https://example.com/art.jpg"
            }"#,
        );
        let meta = parse_bandcamp_json_ld(&html).expect("should parse MusicRecording");
        assert_eq!(meta.title.as_deref(), Some("Echoes"));
        assert_eq!(meta.tracks.len(), 1);
        assert_eq!(meta.tracks[0].name, "Echoes");
        assert_eq!(meta.tracks[0].position, 1);
        assert_eq!(meta.tracks[0].duration_ms, Some(270_000));
    }

    #[test]
    fn test_bandcamp_music_recording_without_duration() {
        let html = make_bandcamp_json_ld_html(
            r#"{
                "@type": "MusicRecording",
                "name": "No Duration Track",
                "byArtist": {"name": "Test Artist"}
            }"#,
        );
        let meta = parse_bandcamp_json_ld(&html).expect("should parse MusicRecording");
        assert_eq!(meta.tracks.len(), 1);
        assert_eq!(meta.tracks[0].name, "No Duration Track");
        assert_eq!(meta.tracks[0].duration_ms, None);
    }

    #[test]
    fn test_bandcamp_type_as_array() {
        let html = make_bandcamp_json_ld_html(
            r#"{
                "@type": ["MusicAlbum", "MusicRelease"],
                "name": "Array Type Album",
                "byArtist": {"name": "Test Artist"},
                "track": {
                    "itemListElement": [
                        {"position": 1, "item": {"name": "Track One", "duration": "PT3M00S"}}
                    ]
                }
            }"#,
        );
        let meta = parse_bandcamp_json_ld(&html).expect("should parse array @type");
        assert_eq!(meta.title.as_deref(), Some("Array Type Album"));
        assert_eq!(meta.tracks.len(), 1);
        assert_eq!(meta.tracks[0].name, "Track One");
        assert_eq!(meta.tracks[0].duration_ms, Some(180_000));
    }

    // =========================================================================
    // Discogs helpers
    // =========================================================================

    #[test]
    fn test_parse_discogs_url() {
        assert_eq!(
            parse_discogs_url("https://www.discogs.com/release/12345-Some-Slug"),
            Some(DiscogsUrlKind::Release(12345))
        );
        assert_eq!(
            parse_discogs_url("https://www.discogs.com/master/67890-Some-Slug"),
            Some(DiscogsUrlKind::Master(67890))
        );
        // Old format with artist prefix
        assert_eq!(
            parse_discogs_url("https://www.discogs.com/Artist-Name/release/99999"),
            Some(DiscogsUrlKind::Release(99999))
        );
        // No slug
        assert_eq!(
            parse_discogs_url("https://www.discogs.com/release/42"),
            Some(DiscogsUrlKind::Release(42))
        );
        // With query params
        assert_eq!(
            parse_discogs_url("https://www.discogs.com/release/123?anv=foo"),
            Some(DiscogsUrlKind::Release(123))
        );
        // Invalid
        assert_eq!(
            parse_discogs_url("https://www.discogs.com/artist/12345"),
            None
        );
        assert_eq!(parse_discogs_url("https://example.com/release/abc"), None);
    }

    #[test]
    fn test_parse_discogs_duration() {
        assert_eq!(parse_discogs_duration("4:30"), Some(270_000));
        assert_eq!(parse_discogs_duration("0:45"), Some(45_000));
        assert_eq!(parse_discogs_duration("1:02:15"), Some(3_735_000));
        assert_eq!(parse_discogs_duration(""), None);
        assert_eq!(parse_discogs_duration("  "), None);
        assert_eq!(parse_discogs_duration("invalid"), None);
        assert_eq!(parse_discogs_duration("0:00"), None);
    }

    #[test]
    fn test_strip_discogs_suffix() {
        assert_eq!(strip_discogs_suffix("Artist (2)"), "Artist");
        assert_eq!(strip_discogs_suffix("Artist (15)"), "Artist");
        // Non-numeric suffixes are preserved
        assert_eq!(strip_discogs_suffix("Artist (UK)"), "Artist (UK)");
        // No suffix
        assert_eq!(strip_discogs_suffix("Regular Artist"), "Regular Artist");
    }

    #[test]
    fn test_join_discogs_artists() {
        // Single artist
        let single = vec![serde_json::json!({"name": "Aphex Twin"})];
        assert_eq!(
            join_discogs_artists(&single),
            Some("Aphex Twin".to_string())
        );

        // Multi with comma join
        let multi = vec![
            serde_json::json!({"name": "Artist A", "join": ","}),
            serde_json::json!({"name": "Artist B"}),
        ];
        assert_eq!(
            join_discogs_artists(&multi),
            Some("Artist A, Artist B".to_string())
        );

        // Multi with ampersand join
        let ampersand = vec![
            serde_json::json!({"name": "Artist A", "join": "&"}),
            serde_json::json!({"name": "Artist B"}),
        ];
        assert_eq!(
            join_discogs_artists(&ampersand),
            Some("Artist A & Artist B".to_string())
        );

        // Strips disambiguation suffixes
        let disambig = vec![serde_json::json!({"name": "Artist (2)"})];
        assert_eq!(
            join_discogs_artists(&disambig),
            Some("Artist".to_string())
        );

        // Empty
        let empty: Vec<serde_json::Value> = vec![];
        assert_eq!(join_discogs_artists(&empty), None);
    }
}
