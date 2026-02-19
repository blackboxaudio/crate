use crate::error::{CrateError, Result};

use super::metadata::build_client;

#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub track_position: i32,
    pub stream_url: String,
    pub expires_at: String,
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

    // Extract track IDs from hydration data
    let track_ids = extract_sc_track_ids(&html)?;
    if track_ids.is_empty() {
        return Err(CrateError::Discovery(
            "No tracks found in SoundCloud page".to_string(),
        ));
    }

    // Resolve client_id
    let mut client_id = match cached_client_id {
        Some(cid) => cid,
        None => resolve_sc_client_id(&client).await?,
    };

    // Try to resolve streams; on auth failure, re-resolve client_id and retry once
    match resolve_sc_streams(&client, &track_ids, &client_id).await {
        Ok(streams) => Ok((streams, client_id)),
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("401") || err_str.contains("403") {
                log::warn!("SoundCloud client_id rejected, re-resolving...");
                client_id = resolve_sc_client_id(&client).await?;
                let streams = resolve_sc_streams(&client, &track_ids, &client_id).await?;
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
        .map_err(|e| {
            CrateError::Discovery(format!("Failed to read SoundCloud homepage: {e}"))
        })?;

    // Find <script crossorigin src="..."> tags pointing to JS bundles
    let script_re = regex::Regex::new(r#"<script[^>]+src="(https://[^"]*\.js)"[^>]*>"#)
        .map_err(|e| CrateError::Discovery(format!("Regex error: {e}")))?;

    let script_urls: Vec<String> = script_re
        .captures_iter(&html)
        .map(|cap| cap[1].to_string())
        .collect();

    let client_id_re = regex::Regex::new(r#"client_id:"([a-zA-Z0-9]{20,})""#)
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

/// Extract SoundCloud track IDs from __sc_hydration data.
fn extract_sc_track_ids(html: &str) -> Result<Vec<(i32, i64)>> {
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
            let ids: Vec<(i32, i64)> = tracks
                .iter()
                .enumerate()
                .filter_map(|(idx, t)| {
                    let id = t.get("id")?.as_i64()?;
                    Some(((idx + 1) as i32, id))
                })
                .collect();
            if !ids.is_empty() {
                return Ok(ids);
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
        if let Some(id) = sound_data.get("id").and_then(|i| i.as_i64()) {
            return Ok(vec![(1, id)]);
        }
    }

    Ok(vec![])
}

/// Resolve stream URLs for a list of SoundCloud track IDs.
async fn resolve_sc_streams(
    client: &reqwest::Client,
    track_ids: &[(i32, i64)],
    client_id: &str,
) -> Result<Vec<StreamInfo>> {
    let mut streams = Vec::new();
    let default_expiry = (chrono::Utc::now() + chrono::Duration::hours(6)).to_rfc3339();

    for (position, track_id) in track_ids {
        let api_url = format!(
            "https://api-v2.soundcloud.com/tracks/{track_id}/streams?client_id={client_id}"
        );

        let resp = client
            .get(&api_url)
            .send()
            .await
            .map_err(|e| CrateError::Discovery(format!("Failed to fetch SC stream: {e}")))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(CrateError::Discovery(format!(
                "SoundCloud API returned {status}"
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CrateError::Discovery(format!("Failed to parse SC stream response: {e}")))?;

        if let Some(url) = data.get("http_mp3_128_url").and_then(|u| u.as_str()) {
            streams.push(StreamInfo {
                track_position: *position,
                stream_url: url.to_string(),
                expires_at: default_expiry.clone(),
            });
        }
    }

    if streams.is_empty() {
        return Err(CrateError::Discovery(
            "No streamable tracks found on SoundCloud".to_string(),
        ));
    }

    Ok(streams)
}
