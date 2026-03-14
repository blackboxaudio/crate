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
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch Bandcamp page: {e}")))?;

    // Check if we were redirected to an album/track page (common for single-release artists)
    let final_url = response.url().to_string();
    let final_lower = final_url.to_lowercase();
    let redirected_to_release = final_lower.contains("/album/") || final_lower.contains("/track/");

    let html = response
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read Bandcamp response: {e}")))?;

    log::info!(
        "Fetched Bandcamp page: {} bytes, final URL: {final_url}",
        html.len()
    );

    let page_name = extract_meta_content(&html, "og:site_name");

    if redirected_to_release {
        let title = extract_meta_content(&html, "og:title");
        let artwork_url = extract_meta_content(&html, "og:image");
        let release = crate::models::ScannedRelease {
            url: final_url,
            artist: page_name.clone(),
            title,
            artwork_url,
            release_date: None,
            already_exists: false,
        };
        return Ok((vec![release], page_name));
    }

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

    // Primary strategy: scan for <a> links to /album/ or /track/ paths.
    // This handles all Bandcamp layouts including non-standard ones (e.g.
    // Knockout.js rendered pages where release class names only appear in CSS).
    let mut releases = parse_bandcamp_release_links(&html, base_url, &page_name);

    // Fallback: try layout-specific parsers for pages where the link scan
    // finds nothing (e.g. pages that load releases dynamically via JS).
    if releases.is_empty() {
        releases = parse_music_grid(&html, base_url, &page_name);
        let index_releases = parse_index_page_cells(&html, base_url, &page_name);
        if !index_releases.is_empty() {
            let existing: std::collections::HashSet<String> =
                releases.iter().map(|r| r.url.clone()).collect();
            for release in index_releases {
                if !existing.contains(&release.url) {
                    releases.push(release);
                }
            }
        }
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

    log::info!(
        "Bandcamp page scan complete: {} total releases found",
        releases.len()
    );

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

/// Parse releases from the standard Bandcamp music grid layout.
///
/// Searches for `<li class="music-grid-item ...">` elements and extracts
/// the release href, artwork, and title from each.
fn parse_music_grid(
    html: &str,
    base_url: &str,
    page_name: &Option<String>,
) -> Vec<crate::models::ScannedRelease> {
    let mut releases = Vec::new();
    let mut search_pos = 0;

    while let Some(li_start) = html[search_pos..].find("<li") {
        let abs_li_start = search_pos + li_start;

        let li_end = match html[abs_li_start..].find("</li>") {
            Some(end) => abs_li_start + end + 5,
            None => break,
        };

        let li_html = &html[abs_li_start..li_end];

        if !li_html.contains("music-grid-item") {
            search_pos = li_end;
            continue;
        }

        let href = extract_attr(li_html, "href");

        let artwork = extract_attr(li_html, "data-original")
            .or_else(|| {
                if let Some(img_start) = li_html.find("<img") {
                    let img_html = &li_html[img_start..];
                    extract_attr(img_html, "src")
                } else {
                    None
                }
            })
            .filter(|s| !s.is_empty() && !s.contains("transparent.gif"));

        let title = extract_inner_text(li_html, "title");

        if let Some(href) = href {
            let absolute_url = if href.starts_with("http") {
                href.to_string()
            } else {
                format!("{}{}", base_url.trim_end_matches('/'), href)
            };

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

    releases
}

/// Extract the title text from the second `<a>` tag in an index page cell.
///
/// Index page cells have two `<a>` tags: the first wraps the artwork image,
/// the second contains the release title text.
fn extract_index_cell_title(cell_html: &str) -> Option<String> {
    // Skip past the first </a> (closes the image link)
    let first_close = cell_html.find("</a>")?;
    let after_first = &cell_html[first_close + 4..];

    // Find the second <a tag
    let second_a = after_first.find("<a")?;
    let a_tag = &after_first[second_a..];

    // Find the > that closes the opening <a ...> tag
    let tag_end = a_tag.find('>')?;
    let content_start = tag_end + 1;

    // Read text until the next < (start of </a>)
    let content_end = a_tag[content_start..].find('<')?;
    let text = a_tag[content_start..content_start + content_end].trim();

    if !text.is_empty() {
        Some(decode_html_entities(text))
    } else {
        None
    }
}

/// Parse releases from the Bandcamp "index page" layout.
///
/// Some Bandcamp label/artist pages use `<div class="indexpage_list_cell">` elements
/// instead of the standard `<li class="music-grid-item">` music grid. Each cell contains
/// two `<a>` tags (one wrapping artwork, one with the title text) linking to the release.
pub(super) fn parse_index_page_cells(
    html: &str,
    base_url: &str,
    page_name: &Option<String>,
) -> Vec<crate::models::ScannedRelease> {
    let mut releases = Vec::new();
    let marker = "indexpage_list_cell";
    let mut search_pos = 0;

    while let Some(marker_pos) = html[search_pos..].find(marker) {
        let abs_marker = search_pos + marker_pos;

        // Walk back to find the opening <div that contains this class
        let div_start = match html[..abs_marker].rfind("<div") {
            Some(pos) => pos,
            None => {
                search_pos = abs_marker + marker.len();
                continue;
            }
        };

        // Find the closing </div> for this cell
        let div_end = match html[abs_marker..].find("</div>") {
            Some(end) => abs_marker + end + 6,
            None => break,
        };

        let cell_html = &html[div_start..div_end];

        // Extract href from the first <a> tag
        let href = extract_attr(cell_html, "href");

        // Extract artwork from <img src="...">
        let artwork = if let Some(img_start) = cell_html.find("<img") {
            let img_html = &cell_html[img_start..];
            extract_attr(img_html, "src")
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        } else {
            None
        };

        // Extract title from the second <a> tag
        let mut title = extract_index_cell_title(cell_html);

        // Strip "Artist - " prefix from title (index pages embed "Artist - Title" format)
        if let (Some(ref t), Some(ref name)) = (&title, page_name) {
            if let Some(stripped) = t.strip_prefix(&format!("{name} - ")) {
                title = Some(stripped.to_string());
            }
        }

        if let Some(href) = href {
            let absolute_url = if href.starts_with("http") {
                href.to_string()
            } else {
                format!("{}{}", base_url.trim_end_matches('/'), href)
            };

            if absolute_url.contains("/album/") || absolute_url.contains("/track/") {
                releases.push(crate::models::ScannedRelease {
                    url: absolute_url,
                    artist: page_name.clone(),
                    title,
                    artwork_url: artwork,
                    release_date: None,
                    already_exists: false,
                });
            }
        }

        search_pos = div_end;
    }

    releases
}

/// Fallback parser for Bandcamp pages with non-standard layouts.
///
/// Scans the HTML for `<a href="...">` tags whose href points to an `/album/` or `/track/`
/// path. Extracts artwork from any `<img>` within the same `<a>` tag, and title text from
/// the link's inner text content.
///
/// Only used when the music grid, index page, and data-client-items parsers all return
/// zero results.
pub(super) fn parse_bandcamp_release_links(
    html: &str,
    base_url: &str,
    page_name: &Option<String>,
) -> Vec<crate::models::ScannedRelease> {
    // Track insertion order + allow merging data from duplicate links.
    // Bandcamp pages often have two <a> tags per release: one wrapping artwork,
    // one containing the title text — both pointing to the same URL.
    let mut url_to_index: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut releases = Vec::new();
    let mut search_pos = 0;

    while let Some(a_start) = html[search_pos..].find("<a ") {
        let abs_a_start = search_pos + a_start;

        let a_close = match html[abs_a_start..].find("</a>") {
            Some(end) => abs_a_start + end + 4,
            None => break,
        };

        let a_html = &html[abs_a_start..a_close];

        // Extract href
        let href = match extract_attr(a_html, "href") {
            Some(h) => h,
            None => {
                search_pos = a_close;
                continue;
            }
        };

        let absolute_url = if href.starts_with("http") {
            href.to_string()
        } else {
            format!("{}{}", base_url.trim_end_matches('/'), href)
        };

        // Only include album/track links
        if !absolute_url.contains("/album/") && !absolute_url.contains("/track/") {
            search_pos = a_close;
            continue;
        }

        // Extract artwork from <img> within this <a> tag
        let artwork = if let Some(img_start) = a_html.find("<img") {
            let img_html = &a_html[img_start..];
            extract_attr(img_html, "src")
                .filter(|s| !s.is_empty() && !s.contains("transparent.gif"))
                .map(|s| s.to_string())
        } else {
            None
        };

        // Extract title from text content (skip <img> children)
        let title = extract_link_text(a_html);

        // Merge data from duplicate links (e.g. image link + title link)
        if let Some(&idx) = url_to_index.get(&absolute_url) {
            let existing: &mut crate::models::ScannedRelease = &mut releases[idx];
            if existing.title.is_none() && title.is_some() {
                existing.title = title;
            }
            if existing.artwork_url.is_none() && artwork.is_some() {
                existing.artwork_url = artwork;
            }
        } else {
            url_to_index.insert(absolute_url.clone(), releases.len());
            releases.push(crate::models::ScannedRelease {
                url: absolute_url,
                artist: page_name.clone(),
                title,
                artwork_url: artwork,
                release_date: None,
                already_exists: false,
            });
        }

        search_pos = a_close;
    }

    releases
}

/// Extract visible text content from an `<a>` tag, skipping nested tags like `<img>`.
fn extract_link_text(a_html: &str) -> Option<String> {
    // Find the end of the opening <a ...> tag
    let tag_end = a_html.find('>')? + 1;
    let inner = &a_html[tag_end..];

    // Find the closing </a>
    let close = inner.find("</a>")?;
    let inner = &inner[..close];

    // Collect text outside of child tags
    let mut text = String::new();
    let mut pos = 0;
    while pos < inner.len() {
        if let Some(tag_start) = inner[pos..].find('<') {
            // Collect text before the tag
            text.push_str(&inner[pos..pos + tag_start]);
            // Skip past the closing >
            match inner[pos + tag_start..].find('>') {
                Some(end) => pos = pos + tag_start + end + 1,
                None => break,
            }
        } else {
            text.push_str(&inner[pos..]);
            break;
        }
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(decode_html_entities(trimmed))
    }
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
        .filter(|item| matches!(item.item_type.as_deref(), Some("album") | Some("track")))
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
