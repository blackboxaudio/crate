use crate::error::{CrateError, Result};

use super::youtube::parse_youtube_url;
use super::{is_compilation, FetchedMetadata, FetchedTrack};

// =============================================================================
// Discogs
// =============================================================================

/// Strip Discogs `-00` placeholders for unknown month/day.
///
/// - `"2018-00-00"` → `"2018"`
/// - `"2018-06-00"` → `"2018-06"`
/// - `"2018-06-19"` → `"2018-06-19"` (unchanged)
fn normalize_discogs_date(s: &str) -> String {
    s.trim_end_matches("-00").to_string()
}

#[derive(Debug, PartialEq)]
pub(super) enum DiscogsUrlKind {
    Release(u64),
    Master(u64),
    Artist(u64),
    Label(u64),
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
            "artist" => Some("artist"),
            "label" => Some("label"),
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
                        "artist" => Some(DiscogsUrlKind::Artist(id)),
                        "label" => Some(DiscogsUrlKind::Label(id)),
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

/// Send a GET to a Discogs API endpoint with status-code validation.
///
/// Returns the parsed JSON body, or a descriptive error for rate-limiting (429),
/// other non-success status codes, or Discogs-level `message` errors.
async fn discogs_api_get(client: &reqwest::Client, url: &str) -> Result<serde_json::Value> {
    let response = client
        .get(url)
        .header("User-Agent", "CrateApp/0.1")
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Discogs API request failed: {e}")))?;

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(CrateError::Discovery(
            "Discogs API rate limit exceeded — please wait a moment and try again".into(),
        ));
    }
    if !status.is_success() {
        return Err(CrateError::Discovery(format!(
            "Discogs API returned status {status}"
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to parse Discogs API response: {e}")))?;

    if let Some(msg) = body.get("message").and_then(|m| m.as_str()) {
        return Err(CrateError::Discovery(format!("Discogs API error: {msg}")));
    }

    Ok(body)
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

/// Score a Discogs release item for version selection during bulk scanning.
///
/// Higher scores indicate a "better" version — official releases with artwork
/// are preferred over test pressings, promos, and versions without artwork.
pub(super) fn score_discogs_release(item: &serde_json::Value, id: u64) -> i32 {
    let mut score: i32 = 0;

    // Strongly prefer versions with artwork
    let has_thumb = item
        .get("thumb")
        .and_then(|t| t.as_str())
        .is_some_and(|s| !s.is_empty());
    if has_thumb {
        score += 10;
    }

    // Penalize test pressings, promos, and white labels
    let format_lower = item
        .get("format")
        .and_then(|f| f.as_str())
        .unwrap_or("")
        .to_lowercase();
    if format_lower.contains("test pressing")
        || format_lower.contains("promo")
        || format_lower.contains("white label")
    {
        score -= 5;
    }

    // Tiebreaker: prefer lower IDs (typically the original pressing)
    if id < 10_000_000 {
        score += 1;
    }

    score
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
        DiscogsUrlKind::Artist(_) | DiscogsUrlKind::Label(_) => {
            return Err(CrateError::Discovery(
                "Artist/label pages cannot be fetched as a single release".into(),
            ));
        }
        DiscogsUrlKind::Release(id) => id,
        DiscogsUrlKind::Master(id) => {
            let master_url = format!("https://api.discogs.com/masters/{id}");
            let resp = discogs_api_get(client, &master_url).await?;

            resp.get("main_release")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| CrateError::Discovery("Discogs master has no main_release".into()))?
        }
    };

    let release_url = format!("https://api.discogs.com/releases/{release_id}");
    let resp = discogs_api_get(client, &release_url).await?;

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
    // Discogs uses `-00` for unknown month/day (e.g. "2018-00-00"), strip those.
    let release_date = resp
        .get("released")
        .and_then(|d| d.as_str())
        .filter(|s| !s.is_empty())
        .map(normalize_discogs_date)
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

/// Scan a Discogs artist or label page, returning all releases found.
///
/// Paginates through the Discogs API, extracting release items and constructing
/// canonical URLs. Returns (releases, page_artist, page_label).
pub(super) async fn scan_discogs_page(
    client: &reqwest::Client,
    kind: &DiscogsUrlKind,
    cancel_flag: &std::sync::atomic::AtomicBool,
    app_handle: Option<&tauri::AppHandle>,
) -> Result<(
    Vec<crate::models::ScannedRelease>,
    Option<String>,
    Option<String>,
)> {
    use super::jittered_delay;
    use tauri::Emitter;

    const MAX_PAGES: u32 = 10;

    let (api_base, entity_url) = match kind {
        DiscogsUrlKind::Artist(id) => (
            format!("https://api.discogs.com/artists/{id}/releases"),
            format!("https://api.discogs.com/artists/{id}"),
        ),
        DiscogsUrlKind::Label(id) => (
            format!("https://api.discogs.com/labels/{id}/releases"),
            format!("https://api.discogs.com/labels/{id}"),
        ),
        _ => {
            return Err(CrateError::Discovery(
                "Expected artist or label URL for page scanning".into(),
            ))
        }
    };

    // Fetch entity info first to get name
    log::info!("Discogs scan: fetching entity info for {kind:?}");
    let entity_resp = discogs_api_get(client, &entity_url).await?;

    let entity_name = entity_resp
        .get("name")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());

    log::info!("Discogs scan: entity name = {:?}", entity_name);

    if let Some(app) = app_handle {
        let _ = app.emit(
            "scan-page-progress",
            crate::models::ScanPageProgress {
                current_page: 0,
                total_pages: None,
                releases_found: 0,
                entity_name: entity_name.clone(),
            },
        );
    }

    let (page_artist, page_label) = match kind {
        DiscogsUrlKind::Artist(_) => (entity_name.clone(), None),
        DiscogsUrlKind::Label(_) => (None, entity_name.clone()),
        _ => (None, None),
    };

    let mut all_releases = Vec::new();
    // Maps (artist, title) → (index in all_releases, quality score) for smart dedup
    let mut seen_titles: std::collections::HashMap<(String, String), (usize, i32)> =
        std::collections::HashMap::new();
    let mut page = 1u32;

    loop {
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            log::info!("Discogs scan: cancelled at page {page}");
            break;
        }

        if page > 1 {
            tokio::time::sleep(jittered_delay(1500)).await;
        }

        log::info!("Discogs scan: fetching page {page}");
        let url = format!("{api_base}?page={page}&per_page=100&sort=year&sort_order=desc");
        let resp = discogs_api_get(client, &url).await?;

        let releases = resp
            .get("releases")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();

        for item in &releases {
            let item_type = item.get("type").and_then(|t| t.as_str());
            let is_master = item_type == Some("master");

            // For masters on artist pages, resolve to the canonical main_release.
            // Label endpoint omits the type field — all items are individual releases.
            let (id, release_url) = if is_master {
                if let Some(main_release_id) = item.get("main_release").and_then(|v| v.as_u64()) {
                    (
                        main_release_id,
                        format!("https://www.discogs.com/release/{main_release_id}"),
                    )
                } else {
                    // Fallback: use master URL (fetch_discogs resolves master → main_release)
                    let master_id = match item.get("id").and_then(|i| i.as_u64()) {
                        Some(id) => id,
                        None => continue,
                    };
                    (
                        master_id,
                        format!("https://www.discogs.com/master/{master_id}"),
                    )
                }
            } else {
                let id = match item.get("id").and_then(|i| i.as_u64()) {
                    Some(id) => id,
                    None => continue,
                };
                (id, format!("https://www.discogs.com/release/{id}"))
            };

            let title = item
                .get("title")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());

            let artist = item
                .get("artist")
                .and_then(|a| a.as_str())
                .map(|s| strip_discogs_suffix(s).to_string());

            // Dedup by (artist, title) — when duplicates are found (e.g. vinyl + digital
            // + test pressing), keep the version with the highest quality score.
            let dedup_key = (
                artist.as_deref().unwrap_or("").to_lowercase(),
                title.as_deref().unwrap_or("").to_lowercase(),
            );

            let score = score_discogs_release(item, id);

            if let Some(&(existing_idx, existing_score)) = seen_titles.get(&dedup_key) {
                if score > existing_score {
                    log::debug!(
                        "Discogs scan: replacing version for {:?} (score {score} > {existing_score})",
                        &dedup_key,
                    );

                    let artwork_url = item
                        .get("thumb")
                        .and_then(|t| t.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());

                    let release_date = item
                        .get("year")
                        .and_then(|y| y.as_u64())
                        .map(|y| format!("{y}"));

                    all_releases[existing_idx] = crate::models::ScannedRelease {
                        url: release_url,
                        artist,
                        title,
                        artwork_url,
                        release_date,
                        already_exists: false,
                    };
                    seen_titles.insert(dedup_key, (existing_idx, score));
                }
                continue;
            }

            let artwork_url = item
                .get("thumb")
                .and_then(|t| t.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            let release_date = item
                .get("year")
                .and_then(|y| y.as_u64())
                .map(|y| format!("{y}"));

            let idx = all_releases.len();
            seen_titles.insert(dedup_key, (idx, score));

            all_releases.push(crate::models::ScannedRelease {
                url: release_url,
                artist,
                title,
                artwork_url,
                release_date,
                already_exists: false,
            });
        }

        // Check pagination
        let total_pages = resp
            .get("pagination")
            .and_then(|p| p.get("pages"))
            .and_then(|p| p.as_u64())
            .unwrap_or(1) as u32;

        log::info!(
            "Discogs scan: page {page}/{total_pages} — {} releases on page, {} total",
            releases.len(),
            all_releases.len()
        );

        if let Some(app) = app_handle {
            let _ = app.emit(
                "scan-page-progress",
                crate::models::ScanPageProgress {
                    current_page: page,
                    total_pages: Some(total_pages),
                    releases_found: all_releases.len(),
                    entity_name: entity_name.clone(),
                },
            );
        }

        if page >= total_pages {
            break;
        }
        if page >= MAX_PAGES {
            log::warn!(
                "Discogs scan: stopped at page limit ({MAX_PAGES}), {} pages remaining",
                total_pages - page
            );
            break;
        }
        page += 1;
    }

    log::info!(
        "Discogs scan: complete — {} releases found",
        all_releases.len()
    );

    Ok((all_releases, page_artist, page_label))
}
