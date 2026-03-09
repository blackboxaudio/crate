use crate::error::{CrateError, Result};

use super::common::{decode_html_entities, extract_meta_content, parse_iso_duration};
use super::{is_compilation, FetchedMetadata, FetchedTrack};

/// Returns `true` for `*.bandcamp.com` URLs that are artist/label pages
/// (i.e. NOT individual album or track pages).
pub(super) fn is_bandcamp_page_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    if !lower.contains("bandcamp.com") {
        return false;
    }
    // If the path contains /album/ or /track/, it's a release page
    !lower.contains("/album/") && !lower.contains("/track/")
}

/// Scan a Bandcamp artist/label page for releases.
///
/// Extracts releases from the music grid and returns them along with the page name.
pub(super) async fn scan_bandcamp_page(
    client: &reqwest::Client,
    url: &str,
) -> Result<(Vec<crate::models::ScannedRelease>, Option<String>)> {
    let html = client
        .get(url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch Bandcamp page: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read Bandcamp response: {e}")))?;

    let page_name = extract_meta_content(&html, "og:site_name");

    // Determine base URL for constructing absolute URLs
    let base_url = {
        // Extract origin from the URL (e.g. "https://artist.bandcamp.com")
        if let Some(idx) = url.find("://") {
            let rest = &url[idx + 3..];
            match rest.find('/') {
                Some(slash) => &url[..idx + 3 + slash],
                None => url,
            }
        } else {
            url
        }
    };

    let mut releases = Vec::new();

    // Parse the music grid. Bandcamp uses <li> items inside the music grid.
    // Each item has an <a> link, artwork <img>, and title text.
    // We look for <li class="music-grid-item ..."> patterns.
    let mut search_pos = 0;
    while let Some(li_start) = html[search_pos..].find("<li") {
        let abs_li_start = search_pos + li_start;

        // Find the end of this <li> tag
        let li_end = match html[abs_li_start..].find("</li>") {
            Some(end) => abs_li_start + end + 5,
            None => break,
        };

        let li_html = &html[abs_li_start..li_end];

        // Only process music grid items
        if !li_html.contains("music-grid-item") {
            search_pos = li_end;
            continue;
        }

        // Extract href from the first <a> tag
        let href = extract_attr(li_html, "href");

        // Extract artwork from <img> tag (try data-original first for lazy-loaded, then src)
        let artwork = extract_attr(li_html, "data-original")
            .or_else(|| {
                // Find <img and extract src from it
                if let Some(img_start) = li_html.find("<img") {
                    let img_html = &li_html[img_start..];
                    extract_attr(img_html, "src")
                } else {
                    None
                }
            })
            .filter(|s| !s.is_empty() && !s.contains("transparent.gif"));

        // Extract title from <p class="title"> ... </p>
        let title = extract_inner_text(li_html, "title");

        if let Some(href) = href {
            // Construct absolute URL
            let absolute_url = if href.starts_with("http") {
                href.to_string()
            } else {
                format!("{}{}", base_url.trim_end_matches('/'), href)
            };

            // Only include album/track links
            if absolute_url.contains("/album/") || absolute_url.contains("/track/") {
                releases.push(crate::models::ScannedRelease {
                    url: absolute_url,
                    artist: page_name.clone(),
                    title,
                    artwork_url: artwork.map(|s| s.to_string()),
                    release_date: None,
                    already_exists: false,
                });
            }
        }

        search_pos = li_end;
    }

    // Bandcamp only server-renders ~16 items in the music grid. The full discography
    // is embedded as a JSON array in a `data-client-items` attribute on the grid's <ol>.
    if let Some(json) = extract_data_client_items(&html) {
        let overflow = parse_client_items(&json, base_url, &page_name);
        if !overflow.is_empty() {
            let existing: std::collections::HashSet<String> =
                releases.iter().map(|r| r.url.clone()).collect();
            for release in overflow {
                if !existing.contains(&release.url) {
                    releases.push(release);
                }
            }
        }
    }

    Ok((releases, page_name))
}

/// Extract the value of an HTML attribute from a tag string.
fn extract_attr<'a>(html: &'a str, attr: &str) -> Option<&'a str> {
    let pattern = format!("{attr}=\"");
    if let Some(start) = html.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = html[value_start..].find('"') {
            let value = &html[value_start..value_start + end];
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

/// Extract the inner text content from an element with a specific class.
fn extract_inner_text(html: &str, class_name: &str) -> Option<String> {
    let class_pattern = format!("class=\"{class_name}");
    if let Some(class_pos) = html.find(&class_pattern) {
        // Find the closing > of this tag
        if let Some(tag_end) = html[class_pos..].find('>') {
            let content_start = class_pos + tag_end + 1;
            // Find the next < which starts the closing tag
            if let Some(content_end) = html[content_start..].find('<') {
                let text = html[content_start..content_start + content_end].trim();
                if !text.is_empty() {
                    return Some(decode_html_entities(text));
                }
            }
        }
    }
    None
}

/// A single item from the `data-client-items` JSON array on Bandcamp label/artist pages.
#[derive(serde::Deserialize)]
struct ClientItem {
    page_url: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    art_id: Option<u64>,
    #[serde(rename = "type")]
    item_type: Option<String>,
}

/// Extract the JSON string from the `data-client-items="..."` attribute in the HTML.
///
/// The attribute value is HTML-entity encoded (e.g. `&quot;` for `"`), so we decode it
/// before returning.
pub(super) fn extract_data_client_items(html: &str) -> Option<String> {
    let marker = "data-client-items=\"";
    let start = html.find(marker)? + marker.len();
    // The value ends at the next unencoded `"`. Since internal quotes are encoded as `&quot;`,
    // the first raw `"` after the marker is the closing delimiter.
    let end = html[start..].find('"')? + start;
    let raw = &html[start..end];
    if raw.is_empty() {
        return None;
    }
    Some(decode_html_entities(raw))
}

/// Parse the decoded `data-client-items` JSON into `ScannedRelease` entries.
pub(super) fn parse_client_items(
    json: &str,
    base_url: &str,
    page_name: &Option<String>,
) -> Vec<crate::models::ScannedRelease> {
    let items: Vec<ClientItem> = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    items
        .into_iter()
        .filter(|item| {
            matches!(
                item.item_type.as_deref(),
                Some("album") | Some("track")
            )
        })
        .filter_map(|item| {
            let relative_url = item.page_url?;
            let absolute_url = if relative_url.starts_with("http") {
                relative_url
            } else {
                format!("{}{}", base_url.trim_end_matches('/'), relative_url)
            };

            // Only include album/track links
            if !absolute_url.contains("/album/") && !absolute_url.contains("/track/") {
                return None;
            }

            let artwork_url = item
                .art_id
                .map(|id| format!("https://f4.bcbits.com/img/a{id}_16.jpg"));

            let artist = item
                .artist
                .filter(|a| !a.is_empty())
                .or_else(|| page_name.clone());

            Some(crate::models::ScannedRelease {
                url: absolute_url,
                artist,
                title: item.title,
                artwork_url,
                release_date: None,
                already_exists: false,
            })
        })
        .collect()
}

pub(super) async fn fetch_bandcamp(client: &reqwest::Client, url: &str) -> Result<FetchedMetadata> {
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

pub(super) fn parse_bandcamp_json_ld(html: &str) -> Option<FetchedMetadata> {
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
                        .or_else(|| {
                            // Some Bandcamp pages nest recordLabel inside albumRelease items
                            value
                                .get("albumRelease")
                                .and_then(|r| r.as_array())
                                .and_then(|arr| arr.iter().find_map(|rel| rel.get("recordLabel")))
                        })
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
                    let mut tracks = parse_bandcamp_tracks(&value, &artist);

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
                                video_id: None,
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

fn parse_bandcamp_tracks(
    value: &serde_json::Value,
    release_artist: &Option<String>,
) -> Vec<FetchedTrack> {
    // Try track.itemListElement (common in MusicAlbum)
    let items = value
        .get("track")
        .and_then(|t| t.get("itemListElement"))
        .and_then(|i| i.as_array());

    let is_comp = is_compilation(release_artist);

    if let Some(items) = items {
        return items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                let track_item = item.get("item").unwrap_or(item);
                let raw_name = track_item
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())?;

                // For compilations, prepend the per-track artist to preserve it
                let name = if is_comp {
                    track_item
                        .get("byArtist")
                        .and_then(|a| a.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|artist| format!("{artist} - {raw_name}"))
                        .unwrap_or(raw_name)
                } else {
                    raw_name
                };

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
                    video_id: None,
                })
            })
            .collect();
    }

    Vec::new()
}
