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
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchedTrack {
    pub name: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
}

fn build_client() -> Result<reqwest::Client> {
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
        _ => fetch_generic(&client, url).await,
    }?;

    metadata.source_type = source_type;
    Ok(metadata)
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
    })
}

fn parse_bandcamp_json_ld(html: &str) -> Option<FetchedMetadata> {
    // Find <script type="application/ld+json"> blocks
    let mut search_from = 0;
    while let Some(start) = html[search_from..].find("<script type=\"application/ld+json\">") {
        let abs_start = search_from + start + "<script type=\"application/ld+json\">".len();
        if let Some(end) = html[abs_start..].find("</script>") {
            let json_str = &html[abs_start..abs_start + end];
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                let schema_type = value.get("@type").and_then(|v| v.as_str()).unwrap_or("");
                if schema_type == "MusicRelease"
                    || schema_type == "MusicAlbum"
                    || schema_type == "MusicRecording"
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
                    let tracks = parse_bandcamp_tracks(&value);

                    return Some(FetchedMetadata {
                        artist,
                        title,
                        label,
                        release_date,
                        artwork_url,
                        tracks,
                        source_type: String::new(),
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

    let duration_ms = sound_data
        .get("duration")
        .and_then(|d| d.as_i64());

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
    })
}

/// Parse ISO 8601 duration (e.g., "PT3M45S") to milliseconds.
fn parse_iso_duration(s: &str) -> Option<i64> {
    let s = s.strip_prefix("PT")?;
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
    fn test_parse_iso_duration() {
        assert_eq!(parse_iso_duration("PT3M45S"), Some(225_000));
        assert_eq!(parse_iso_duration("PT1H2M3S"), Some(3_723_000));
        assert_eq!(parse_iso_duration("PT30S"), Some(30_000));
        assert_eq!(parse_iso_duration("PT5M"), Some(300_000));
        assert_eq!(parse_iso_duration("invalid"), None);
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
        let html = make_sc_hydration_html(
            r#"[{"hydratable": "user", "data": {}}]"#,
        );
        assert!(parse_sc_hydration(&html).is_none());
    }
}
