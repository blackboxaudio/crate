use crate::error::{CrateError, Result};

use super::metadata::{self, build_client};

#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub track_position: i32,
    pub stream_url: String,
    pub expires_at: String,
    /// When set, the stream URL requires this user-agent to avoid 403 from YouTube's CDN.
    /// The command layer uses this to return a localhost proxy URL instead of the raw stream URL.
    pub proxy_ua: Option<String>,
}

/// Extract stream URLs from a Bandcamp release page.
///
/// Parses the `data-tralbum` attribute or `var TralbumData = {...}` JS variable
/// to find `trackinfo[].file["mp3-128"]` URLs.
pub async fn extract_bandcamp_streams(url: &str) -> Result<Vec<StreamInfo>> {
    let client = build_client()?;
    let html = client
        .get(url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch Bandcamp page: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read Bandcamp response: {e}")))?;

    // Try data-tralbum attribute first, then TralbumData JS variable
    let tralbum_json = extract_tralbum_data(&html)
        .ok_or_else(|| CrateError::Discovery("Could not find TralbumData on page".to_string()))?;

    let tralbum: serde_json::Value = serde_json::from_str(&tralbum_json)
        .map_err(|e| CrateError::Discovery(format!("Failed to parse TralbumData: {e}")))?;

    let track_info = tralbum
        .get("trackinfo")
        .and_then(|t| t.as_array())
        .ok_or_else(|| CrateError::Discovery("No trackinfo in TralbumData".to_string()))?;

    let mut streams = Vec::new();
    for (idx, track) in track_info.iter().enumerate() {
        let position = track
            .get("track_num")
            .and_then(|n| n.as_i64())
            .unwrap_or(idx as i64 + 1) as i32;

        let stream_url = track
            .get("file")
            .and_then(|f| f.get("mp3-128"))
            .and_then(|u| u.as_str());

        if let Some(url) = stream_url {
            let expires_at = parse_bandcamp_expiry(url);
            streams.push(StreamInfo {
                track_position: position,
                stream_url: url.to_string(),
                expires_at,
                proxy_ua: None,
            });
        }
    }

    if streams.is_empty() {
        return Err(CrateError::Discovery(
            "No streamable tracks found".to_string(),
        ));
    }

    Ok(streams)
}

/// Extract stream URLs from a SoundCloud release page.
///
/// Returns `(streams, client_id)` so the caller can cache the client_id.
pub async fn extract_soundcloud_streams(
    url: &str,
    cached_client_id: Option<String>,
) -> Result<(Vec<StreamInfo>, String)> {
    let client = build_client()?;
    let html = client
        .get(url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch SoundCloud page: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read SoundCloud response: {e}")))?;

    // Extract track data (progressive transcoding URLs + auth tokens) from hydration
    let track_data = extract_sc_track_data(&html)?;
    if track_data.is_empty() {
        return Err(CrateError::Discovery(
            "No tracks found in SoundCloud page".to_string(),
        ));
    }
    log::debug!(
        "Extracted {} tracks from SoundCloud hydration",
        track_data.len()
    );

    // Resolve client_id
    let mut client_id = match cached_client_id {
        Some(cid) => cid,
        None => resolve_sc_client_id(&client).await?,
    };

    // Try to resolve streams; on auth failure, re-resolve client_id and retry once
    match resolve_sc_streams(&client, &track_data, &client_id).await {
        Ok(streams) => Ok((streams, client_id)),
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("401") || err_str.contains("403") {
                log::warn!("SoundCloud client_id rejected, re-resolving...");
                client_id = resolve_sc_client_id(&client).await?;
                let streams = resolve_sc_streams(&client, &track_data, &client_id).await?;
                Ok((streams, client_id))
            } else {
                Err(e)
            }
        }
    }
}

/// Resolve the SoundCloud client_id by fetching JS bundles from soundcloud.com.
pub async fn resolve_sc_client_id(client: &reqwest::Client) -> Result<String> {
    let html = client
        .get("https://soundcloud.com")
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch SoundCloud homepage: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read SoundCloud homepage: {e}")))?;

    // Find <script crossorigin src="..."> tags pointing to JS bundles
    let script_re = regex::Regex::new(r#"<script[^>]+src="(https://[^"]*\.js)"[^>]*>"#)
        .map_err(|e| CrateError::Discovery(format!("Regex error: {e}")))?;

    let script_urls: Vec<String> = script_re
        .captures_iter(&html)
        .map(|cap| cap[1].to_string())
        .collect();

    let client_id_re = regex::Regex::new(r#"client_id[=:]["']?([a-zA-Z0-9]{20,})["']?"#)
        .map_err(|e| CrateError::Discovery(format!("Regex error: {e}")))?;

    // Check bundles in reverse order (client_id is usually in the last few bundles)
    for script_url in script_urls.iter().rev() {
        let js = match client.get(script_url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(text) => text,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        if let Some(cap) = client_id_re.captures(&js) {
            let cid = cap[1].to_string();
            log::info!("Resolved SoundCloud client_id: {cid}");
            return Ok(cid);
        }
    }

    Err(CrateError::Discovery(
        "Could not resolve SoundCloud client_id from JS bundles".to_string(),
    ))
}

// =============================================================================
// Internal Helpers
// =============================================================================

/// Extract TralbumData JSON from Bandcamp page HTML.
fn extract_tralbum_data(html: &str) -> Option<String> {
    // Try data-tralbum attribute first
    if let Some(start) = html.find("data-tralbum=\"") {
        let json_start = start + "data-tralbum=\"".len();
        if let Some(end) = html[json_start..].find('"') {
            let escaped = &html[json_start..json_start + end];
            // data-tralbum uses HTML entity encoding
            let decoded = escaped
                .replace("&quot;", "\"")
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&#39;", "'");
            return Some(decoded);
        }
    }

    // Fall back to var TralbumData = {...}
    let marker = "var TralbumData = ";
    let start = html.find(marker)? + marker.len();
    // Find the end of the JSON object — it ends with `;\n` or `};`
    let rest = &html[start..];

    // Find matching closing brace by counting nesting depth
    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;
    for (i, ch) in rest.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(rest[..=i].to_string());
                }
            }
            _ => {}
        }
    }

    None
}

/// Parse the expiry timestamp from a Bandcamp stream URL's `e=` query parameter.
/// Falls back to now + 6 hours if parsing fails.
fn parse_bandcamp_expiry(stream_url: &str) -> String {
    if let Some(e_param) = stream_url
        .split('?')
        .nth(1)
        .and_then(|q| q.split('&').find(|p| p.starts_with("e=")))
        .and_then(|p| p.strip_prefix("e="))
    {
        if let Ok(ts) = e_param.parse::<i64>() {
            if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
                return dt.to_rfc3339();
            }
        }
    }

    // Fallback: 6 hours from now
    (chrono::Utc::now() + chrono::Duration::hours(6)).to_rfc3339()
}

/// Track data extracted from SoundCloud's `__sc_hydration`.
struct ScTrackData {
    position: i32,
    /// Progressive transcoding URL from `media.transcodings`.
    progressive_url: Option<String>,
    /// Per-track JWT for stream access.
    track_authorization: Option<String>,
}

/// Extract SoundCloud track data from `__sc_hydration`.
///
/// Parses progressive transcoding URLs and `track_authorization` JWTs
/// from both single-track ("sound") and playlist ("playlist") pages.
fn extract_sc_track_data(html: &str) -> Result<Vec<ScTrackData>> {
    let marker = "window.__sc_hydration = ";
    let start = html
        .find(marker)
        .ok_or_else(|| CrateError::Discovery("No __sc_hydration found".to_string()))?
        + marker.len();
    let end = start
        + html[start..]
            .find(";</script>")
            .ok_or_else(|| CrateError::Discovery("Malformed __sc_hydration".to_string()))?;

    let hydration: serde_json::Value = serde_json::from_str(&html[start..end])
        .map_err(|e| CrateError::Discovery(format!("Failed to parse __sc_hydration: {e}")))?;

    let arr = hydration
        .as_array()
        .ok_or_else(|| CrateError::Discovery("__sc_hydration is not an array".to_string()))?;

    // Check for playlist (set) first
    if let Some(playlist_data) = arr.iter().find_map(|entry| {
        if entry.get("hydratable")?.as_str()? == "playlist" {
            entry.get("data")
        } else {
            None
        }
    }) {
        if let Some(tracks) = playlist_data.get("tracks").and_then(|t| t.as_array()) {
            let data: Vec<ScTrackData> = tracks
                .iter()
                .enumerate()
                .filter_map(|(idx, t)| {
                    // Skip entries without an id (minimal stubs)
                    t.get("id")?.as_i64()?;
                    Some(ScTrackData {
                        position: (idx + 1) as i32,
                        progressive_url: extract_progressive_url(t),
                        track_authorization: t
                            .get("track_authorization")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                    })
                })
                .collect();
            if !data.is_empty() {
                return Ok(data);
            }
        }
    }

    // Single track (sound)
    if let Some(sound_data) = arr.iter().find_map(|entry| {
        if entry.get("hydratable")?.as_str()? == "sound" {
            entry.get("data")
        } else {
            None
        }
    }) {
        if sound_data.get("id").and_then(|i| i.as_i64()).is_some() {
            return Ok(vec![ScTrackData {
                position: 1,
                progressive_url: extract_progressive_url(sound_data),
                track_authorization: sound_data
                    .get("track_authorization")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            }]);
        }
    }

    Ok(vec![])
}

/// Find the progressive MP3 transcoding URL from a track's `media.transcodings` array.
fn extract_progressive_url(track: &serde_json::Value) -> Option<String> {
    let transcodings = track
        .get("media")
        .and_then(|m| m.get("transcodings"))
        .and_then(|t| t.as_array())?;

    transcodings
        .iter()
        .find(|t| {
            let format = t.get("format");
            let protocol = format
                .and_then(|f| f.get("protocol"))
                .and_then(|p| p.as_str());
            let mime = format
                .and_then(|f| f.get("mime_type"))
                .and_then(|m| m.as_str());
            protocol == Some("progressive") && mime.is_some_and(|m| m.starts_with("audio/mpeg"))
        })
        .and_then(|t| t.get("url"))
        .and_then(|u| u.as_str())
        .map(|s| s.to_string())
}

/// Parse the expiry timestamp from a SoundCloud/CloudFront stream URL's `Policy` parameter.
/// CloudFront signed URLs contain a base64-encoded JSON policy with the expiry epoch time.
/// Falls back to now + 1 hour if parsing fails.
fn parse_soundcloud_expiry(stream_url: &str) -> String {
    use base64::Engine;

    let result = (|| -> Option<String> {
        // Extract the Policy query parameter
        let policy_encoded = stream_url
            .split('?')
            .nth(1)?
            .split('&')
            .find(|p| p.starts_with("Policy="))?
            .strip_prefix("Policy=")?;

        // CloudFront uses URL-safe base64 with custom replacements: - → +, _ → /, ~ → =
        let standard_b64: String = policy_encoded
            .chars()
            .map(|c| match c {
                '-' => '+',
                '_' => '/',
                '~' => '=',
                other => other,
            })
            .collect();

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&standard_b64)
            .ok()?;
        let policy_json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;

        let epoch = policy_json
            .get("Statement")?
            .as_array()?
            .first()?
            .get("Condition")?
            .get("DateLessThan")?
            .get("AWS:EpochTime")?
            .as_i64()?;

        // Subtract 60s safety buffer
        let expires_at = epoch - 60;
        let dt = chrono::DateTime::from_timestamp(expires_at, 0)?;
        Some(dt.to_rfc3339())
    })();

    result.unwrap_or_else(|| {
        log::warn!("Failed to parse CloudFront Policy expiry, falling back to 1 hour");
        (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339()
    })
}

/// Resolve stream URLs by fetching each track's progressive transcoding endpoint.
async fn resolve_sc_streams(
    client: &reqwest::Client,
    tracks: &[ScTrackData],
    client_id: &str,
) -> Result<Vec<StreamInfo>> {
    let mut streams = Vec::new();

    for track in tracks {
        let progressive_url = match &track.progressive_url {
            Some(url) => url,
            None => {
                log::warn!(
                    "No progressive transcoding for SC track at position {}",
                    track.position
                );
                continue;
            }
        };

        let track_auth = track.track_authorization.as_deref().unwrap_or_default();
        let api_url =
            format!("{progressive_url}?client_id={client_id}&track_authorization={track_auth}");

        let resp = client
            .get(&api_url)
            .send()
            .await
            .map_err(|e| CrateError::Discovery(format!("Failed to resolve SC stream: {e}")))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(CrateError::Discovery(format!(
                "SoundCloud API returned {status}"
            )));
        }

        let data: serde_json::Value = resp.json().await.map_err(|e| {
            CrateError::Discovery(format!("Failed to parse SC stream response: {e}"))
        })?;

        if let Some(url) = data.get("url").and_then(|u| u.as_str()) {
            let expires_at = parse_soundcloud_expiry(url);
            streams.push(StreamInfo {
                track_position: track.position,
                stream_url: url.to_string(),
                expires_at,
                proxy_ua: None,
            });
        } else {
            log::warn!(
                "SC transcoding response missing 'url' field for position {}",
                track.position
            );
        }
    }

    if streams.is_empty() {
        return Err(CrateError::Discovery(
            "No streamable tracks found on SoundCloud".to_string(),
        ));
    }

    Ok(streams)
}

// =============================================================================
// YouTube
// =============================================================================

/// Extract audio stream URLs from YouTube video(s).
///
/// For playlists, extracts streams for all videos. For single videos, extracts
/// a single stream. Prefers itag 140 (audio/mp4 AAC 128kbps) for best HTML5 Audio
/// compatibility, falls back to other audio-only formats.
pub async fn extract_youtube_streams(url: &str) -> Result<Vec<StreamInfo>> {
    let parsed = metadata::parse_youtube_url(url);

    // Build list of (video_id, position) pairs
    let video_list = if let Some(ref playlist_id) = parsed.playlist_id {
        let client = build_client()?;
        let playlist_url = format!("https://www.youtube.com/playlist?list={playlist_id}");
        let html = client
            .get(&playlist_url)
            .send()
            .await
            .map_err(|e| CrateError::Discovery(format!("Failed to fetch YouTube playlist: {e}")))?
            .text()
            .await
            .map_err(|e| CrateError::Discovery(format!("Failed to read YouTube playlist: {e}")))?;

        let yt_data = metadata::parse_yt_initial_data(&html).ok_or_else(|| {
            CrateError::Discovery("Could not find ytInitialData on playlist page".into())
        })?;

        let videos = metadata::extract_playlist_videos(&yt_data);
        if videos.is_empty() {
            return Err(CrateError::Discovery(
                "No videos found in YouTube playlist".into(),
            ));
        }

        videos
            .into_iter()
            .enumerate()
            .map(|(idx, v)| {
                let pos = if v.position > 0 {
                    v.position
                } else {
                    (idx + 1) as i32
                };
                (v.video_id, pos)
            })
            .collect::<Vec<_>>()
    } else if let Some(video_id) = parsed.video_id {
        vec![(video_id, 1)]
    } else {
        return Err(CrateError::Discovery(
            "Could not parse YouTube URL: no video or playlist ID found".into(),
        ));
    };

    let mut streams = Vec::new();
    for (idx, (video_id, position)) in video_list.iter().enumerate() {
        // Small delay between requests for playlists to avoid rate limiting
        if idx > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        match extract_single_youtube_stream(video_id, *position).await {
            Ok(stream) => streams.push(stream),
            Err(e) => {
                log::warn!("Failed to extract stream for YouTube video {video_id}: {e}");
            }
        }
    }

    if streams.is_empty() {
        return Err(CrateError::Discovery(
            "No streamable YouTube videos found".into(),
        ));
    }

    Ok(streams)
}

/// Extract a single video's audio stream URL via the youtubei player API.
///
/// Tries each YouTube client configuration in [`metadata::YT_CLIENTS`] until one
/// returns a playable response with audio streams.
pub async fn extract_single_youtube_stream(video_id: &str, position: i32) -> Result<StreamInfo> {
    let mut last_error =
        CrateError::Discovery(format!("All YouTube clients failed for video {video_id}"));

    for config in metadata::YT_CLIENTS {
        let client = match metadata::build_yt_client_with_config(config) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("YouTube {} client build failed: {e}", config.client_name,);
                last_error = e;
                continue;
            }
        };

        let player =
            match metadata::fetch_yt_player_response_with_config(&client, video_id, config).await {
                Ok(p) => p,
                Err(e) => {
                    log::warn!(
                        "YouTube {} client fetch failed for {video_id}: {e}",
                        config.client_name,
                    );
                    last_error = e;
                    continue;
                }
            };

        // Check playability
        let status = player
            .get("playabilityStatus")
            .and_then(|ps| ps.get("status"))
            .and_then(|s| s.as_str())
            .unwrap_or("UNKNOWN");

        if status != "OK" {
            let reason = player
                .get("playabilityStatus")
                .and_then(|ps| ps.get("reason"))
                .and_then(|r| r.as_str())
                .unwrap_or("Unknown reason");
            log::warn!(
                "YouTube {} client returned {status} for {video_id}: {reason}",
                config.client_name,
            );
            last_error =
                CrateError::Discovery(format!("YouTube video {video_id} not playable: {reason}"));
            continue;
        }

        // Extract audio stream
        let adaptive_formats = match player
            .get("streamingData")
            .and_then(|sd| sd.get("adaptiveFormats"))
            .and_then(|af| af.as_array())
        {
            Some(af) => af,
            None => {
                log::warn!(
                    "YouTube {} client returned OK but no adaptive formats for {video_id}",
                    config.client_name,
                );
                last_error = CrateError::Discovery(format!(
                    "No adaptive formats found for YouTube video {video_id}"
                ));
                continue;
            }
        };

        // Prefer itag 140 (audio/mp4 AAC 128kbps), fall back to any audio format with direct URL
        let stream = adaptive_formats
            .iter()
            .find(|f| f.get("itag").and_then(|i| i.as_u64()) == Some(140) && f.get("url").is_some())
            .or_else(|| {
                adaptive_formats.iter().find(|f| {
                    f.get("mimeType")
                        .and_then(|m| m.as_str())
                        .is_some_and(|m| m.starts_with("audio/"))
                        && f.get("url").is_some()
                })
            });

        let stream = match stream {
            Some(s) => s,
            None => {
                log::warn!(
                    "YouTube {} client returned no audio streams for {video_id}",
                    config.client_name,
                );
                last_error = CrateError::Discovery(format!(
                    "No audio stream found for YouTube video {video_id}"
                ));
                continue;
            }
        };

        let stream_url = match stream.get("url").and_then(|u| u.as_str()) {
            Some(url) => url,
            None => {
                log::warn!(
                    "YouTube {} client stream requires signature deciphering for {video_id}",
                    config.client_name,
                );
                last_error = CrateError::Discovery(format!(
                    "YouTube video {video_id} requires signature deciphering (not supported)"
                ));
                continue;
            }
        };

        log::info!(
            "YouTube {} client succeeded for {video_id}",
            config.client_name,
        );

        let expires_at = parse_youtube_expiry(stream_url);
        let proxy_ua = if config.browser_compatible {
            None
        } else {
            Some(config.user_agent.to_string())
        };

        return Ok(StreamInfo {
            track_position: position,
            stream_url: stream_url.to_string(),
            expires_at,
            proxy_ua,
        });
    }

    Err(last_error)
}

/// Parse expiry from a YouTube stream URL's `expire=` query parameter.
/// Falls back to 5 hours from now (YouTube streams typically last ~6 hours).
fn parse_youtube_expiry(stream_url: &str) -> String {
    if let Some(expire_str) = metadata::extract_query_param(stream_url, "expire") {
        if let Ok(ts) = expire_str.parse::<i64>() {
            // Subtract 60s safety buffer
            if let Some(dt) = chrono::DateTime::from_timestamp(ts - 60, 0) {
                return dt.to_rfc3339();
            }
        }
    }

    // Fallback: 5 hours from now
    (chrono::Utc::now() + chrono::Duration::hours(5)).to_rfc3339()
}
