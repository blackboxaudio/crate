use std::sync::Arc;

use crate::error::{CrateError, Result};

use super::{is_compilation, FetchedMetadata, FetchedTrack, CHROME_USER_AGENT, YT_CONSENT_COOKIE};

// =============================================================================
// YouTube
// =============================================================================

#[derive(Debug)]
pub(crate) struct YouTubeUrl {
    pub video_id: Option<String>,
    pub playlist_id: Option<String>,
}

pub(crate) fn extract_query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    let prefix = format!("{key}=");
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix(&prefix) {
            let value = value.split('#').next().unwrap_or(value);
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub(crate) fn parse_youtube_url(url: &str) -> YouTubeUrl {
    let video_id = extract_query_param(url, "v");
    let playlist_id = extract_query_param(url, "list");

    // Handle youtu.be short URLs: youtu.be/VIDEO_ID
    let video_id = video_id.or_else(|| {
        let rest = url
            .strip_prefix("https://youtu.be/")
            .or_else(|| url.strip_prefix("http://youtu.be/"))?;
        let path = rest.split('?').next().unwrap_or(rest);
        let path = path.split('/').next().unwrap_or(path);
        if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        }
    });

    YouTubeUrl {
        video_id,
        playlist_id,
    }
}

/// YouTube innertube client configuration for the player API.
pub(crate) struct YtClientConfig {
    pub client_name: &'static str,
    pub client_id: &'static str,
    pub client_version: &'static str,
    pub user_agent: &'static str,
    /// Whether stream URLs from this client work in a browser/WebView Audio element
    /// without requiring the matching user-agent on the CDN request.
    pub browser_compatible: bool,
    /// Extra context fields for native app clients (device info, OS version, etc.).
    pub extra_context: Option<&'static [(&'static str, &'static str)]>,
}

/// Fallback chain of YouTube innertube clients, ordered by preference.
///
/// Browser-compatible clients are tried first because their stream URLs can be played directly
/// by the HTML5 Audio element without the localhost proxy, avoiding seeking/pause issues.
///
/// - WEB_EMBEDDED: handles most non-restricted videos.
/// - WEB: handles embedded-restricted videos that WEB_EMBEDDED returns UNKNOWN for, since
///   embedded-restricted videos ARE playable on youtube.com itself (just not in iframes).
/// - ANDROID_VR: fallback for videos that only work via native app innertube (e.g. embedded-
///   restricted). Uses the Oculus Quest client, which doesn't require PO tokens and returns
///   direct stream URLs with `n` parameters suitable for transformation. The IOS client was
///   removed because YouTube now requires PO tokens for IOS CDN access (403 without one).
pub(crate) const YT_CLIENTS: &[YtClientConfig] = &[
    YtClientConfig {
        client_name: "WEB_EMBEDDED",
        client_id: "56",
        client_version: "1.20250120.00.00",
        user_agent: CHROME_USER_AGENT,
        browser_compatible: true,
        extra_context: None,
    },
    YtClientConfig {
        client_name: "WEB",
        client_id: "1",
        client_version: "2.20250120.01.00",
        user_agent: CHROME_USER_AGENT,
        browser_compatible: true,
        extra_context: None,
    },
    YtClientConfig {
        client_name: "ANDROID_VR",
        client_id: "28",
        client_version: "1.71.26",
        user_agent: "com.google.android.apps.youtube.vr.oculus/1.71.26 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
        browser_compatible: false,
        extra_context: Some(&[
            ("deviceMake", "Oculus"),
            ("deviceModel", "Quest 3"),
            ("osName", "Android"),
            ("osVersion", "12L"),
            ("androidSdkVersion", "32"),
        ]),
    },
];

/// Build a reqwest client with a specific YouTube client config's user-agent
/// and optional persistent cookie jar for session continuity.
pub(crate) fn build_yt_client_with_config(
    config: &YtClientConfig,
    jar: Option<Arc<reqwest::cookie::Jar>>,
) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(config.user_agent);
    if let Some(jar) = jar {
        builder = builder.cookie_provider(jar);
    }
    builder
        .build()
        .map_err(|e| CrateError::Discovery(format!("Failed to create YouTube client: {e}")))
}

/// Build a reqwest client using the primary YouTube client config (no cookie jar).
fn build_yt_client() -> Result<reqwest::Client> {
    build_yt_client_with_config(&YT_CLIENTS[0], None)
}

/// Create a new cookie jar pre-seeded with the YouTube consent cookie.
pub(crate) fn new_yt_cookie_jar() -> Arc<reqwest::cookie::Jar> {
    let jar = reqwest::cookie::Jar::default();
    let yt_url = "https://www.youtube.com".parse::<reqwest::Url>().unwrap();
    jar.add_cookie_str(YT_CONSENT_COOKIE, &yt_url);
    Arc::new(jar)
}

/// Generate a randomized delay with jitter around a base duration.
/// Returns a `Duration` of `base_ms ± (0..base_ms/2)`, clamped to a minimum of 500ms.
pub(crate) fn jittered_delay(base_ms: u64) -> std::time::Duration {
    use rand::Rng;
    let jitter = rand::rng().random_range(0..=(base_ms / 2));
    let delay = if rand::rng().random_bool(0.5) {
        base_ms.saturating_add(jitter)
    } else {
        base_ms.saturating_sub(jitter)
    };
    std::time::Duration::from_millis(delay.max(500))
}

/// Encode a u64 as a protobuf-style LEB128 varint.
fn encode_varint(buf: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            buf.push(byte);
            break;
        }
        buf.push(byte | 0x80);
    }
}

/// Generate synthetic visitorData as base64-encoded protobuf, matching yt-dlp's format.
///
/// The protobuf structure is:
/// - Field 1 (string): 11 random alphanumeric characters (visitor ID)
/// - Field 5 (varint): current Unix timestamp in seconds
fn generate_visitor_data() -> String {
    use base64::Engine;

    // Generate 11 random alphanumeric chars from UUID bytes
    let uuid_bytes = uuid::Uuid::new_v4().into_bytes();
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let visitor_id: String = uuid_bytes
        .iter()
        .take(11)
        .map(|b| CHARSET[(*b as usize) % CHARSET.len()] as char)
        .collect();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Encode as protobuf: field 1 (string) = tag 0x0A, field 5 (varint) = tag 0x28
    let mut buf = Vec::new();
    buf.push(0x0A); // field 1, wire type 2 (length-delimited)
    encode_varint(&mut buf, visitor_id.len() as u64);
    buf.extend_from_slice(visitor_id.as_bytes());
    buf.push(0x28); // field 5, wire type 0 (varint)
    encode_varint(&mut buf, timestamp);

    base64::engine::general_purpose::URL_SAFE.encode(&buf)
}

/// Call YouTube's internal player API with a specific client configuration.
pub(crate) async fn fetch_yt_player_response_with_config(
    client: &reqwest::Client,
    video_id: &str,
    config: &YtClientConfig,
) -> Result<serde_json::Value> {
    let mut client_ctx = serde_json::json!({
        "clientName": config.client_name,
        "clientVersion": config.client_version,
        "hl": "en",
        "timeZone": "UTC",
        "utcOffsetMinutes": 0,
    });

    // Add extra context fields for native app clients (device info, OS version, etc.)
    if let Some(extras) = config.extra_context {
        let obj = client_ctx.as_object_mut().unwrap();
        for (key, value) in extras {
            // androidSdkVersion is an integer, not a string
            if *key == "androidSdkVersion" {
                if let Ok(v) = value.parse::<u32>() {
                    obj.insert(key.to_string(), serde_json::json!(v));
                    continue;
                }
            }
            obj.insert(key.to_string(), serde_json::json!(value));
        }
        // Include the user-agent in the JSON body for native app clients (matches yt-dlp)
        obj.insert(
            "userAgent".to_string(),
            serde_json::json!(config.user_agent),
        );
    }

    // Add visitorData for all clients to help avoid bot detection.
    // yt-dlp extracts this from prior responses; we generate a synthetic one since we don't
    // have a prior web session. Real browsers always send visitor data.
    let visitor_data = generate_visitor_data();
    client_ctx
        .as_object_mut()
        .unwrap()
        .insert("visitorData".to_string(), serde_json::json!(&visitor_data));

    let body = serde_json::json!({
        "videoId": video_id,
        "contentCheckOk": true,
        "racyCheckOk": true,
        "context": {
            "client": client_ctx
        },
        "playbackContext": {
            "contentPlaybackContext": {
                "html5Preference": "HTML5_PREF_WANTS"
            }
        }
    });

    client
        .post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false")
        .header("Origin", "https://www.youtube.com")
        .header("X-YouTube-Client-Name", config.client_id)
        .header("X-YouTube-Client-Version", config.client_version)
        .header("X-Goog-Visitor-Id", &visitor_data)
        .header("Cookie", YT_CONSENT_COOKIE)
        .json(&body)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch YouTube player data: {e}")))?
        .json()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to parse YouTube player response: {e}")))
}

/// Call YouTube's internal player API using the primary client config.
async fn fetch_yt_player_response(
    client: &reqwest::Client,
    video_id: &str,
) -> Result<serde_json::Value> {
    fetch_yt_player_response_with_config(client, video_id, &YT_CLIENTS[0]).await
}

/// Extract `var ytInitialData = {...}` from YouTube page HTML.
pub(crate) fn parse_yt_initial_data(html: &str) -> Option<serde_json::Value> {
    let marker = "var ytInitialData = ";
    let start = html.find(marker)? + marker.len();
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
                    let json_str = &rest[..=i];
                    return serde_json::from_str(json_str).ok();
                }
            }
            _ => {}
        }
    }

    None
}

#[derive(Debug, Clone)]
pub(crate) struct YouTubeVideo {
    pub video_id: String,
    pub title: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
}

/// Extract video entries from ytInitialData's playlistVideoListRenderer.
pub(crate) fn extract_playlist_videos(yt_data: &serde_json::Value) -> Vec<YouTubeVideo> {
    let videos = yt_data
        .get("contents")
        .and_then(|c| c.get("twoColumnBrowseResultsRenderer"))
        .and_then(|r| r.get("tabs"))
        .and_then(|t| t.as_array())
        .and_then(|tabs| tabs.first())
        .and_then(|tab| tab.get("tabRenderer"))
        .and_then(|tr| tr.get("content"))
        .and_then(|c| c.get("sectionListRenderer"))
        .and_then(|slr| slr.get("contents"))
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|s| s.get("itemSectionRenderer"))
        .and_then(|isr| isr.get("contents"))
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|p| p.get("playlistVideoListRenderer"))
        .and_then(|pvlr| pvlr.get("contents"))
        .and_then(|c| c.as_array());

    let Some(video_items) = videos else {
        return Vec::new();
    };

    video_items
        .iter()
        .filter_map(|item| {
            let renderer = item.get("playlistVideoRenderer")?;
            let video_id = renderer
                .get("videoId")
                .and_then(|v| v.as_str())?
                .to_string();
            let title = renderer
                .get("title")
                .and_then(|t| t.get("runs"))
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.first())
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("Untitled")
                .to_string();
            let position = renderer
                .get("index")
                .and_then(|i| i.get("simpleText"))
                .and_then(|s| s.as_str())
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
            let duration_ms = renderer
                .get("lengthSeconds")
                .and_then(|l| l.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .map(|s| s * 1000);

            Some(YouTubeVideo {
                video_id,
                title,
                position,
                duration_ms,
            })
        })
        .collect()
}

pub(super) async fn fetch_youtube(client: &reqwest::Client, url: &str) -> Result<FetchedMetadata> {
    let parsed = parse_youtube_url(url);

    // Playlist path (including video+playlist URLs — treat as playlist)
    if let Some(ref playlist_id) = parsed.playlist_id {
        return fetch_youtube_playlist(client, playlist_id).await;
    }

    // Single video path
    if let Some(ref video_id) = parsed.video_id {
        return fetch_youtube_single(client, url, video_id).await;
    }

    Err(CrateError::Discovery(
        "Could not parse YouTube URL: no video or playlist ID found".into(),
    ))
}

async fn fetch_youtube_playlist(
    client: &reqwest::Client,
    playlist_id: &str,
) -> Result<FetchedMetadata> {
    let playlist_url = format!("https://www.youtube.com/playlist?list={playlist_id}");
    let html = client
        .get(&playlist_url)
        .header("Cookie", YT_CONSENT_COOKIE)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch YouTube playlist page: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read YouTube playlist page: {e}")))?;

    let yt_data = parse_yt_initial_data(&html).ok_or_else(|| {
        CrateError::Discovery("Could not find ytInitialData on playlist page".into())
    })?;

    // Extract playlist metadata from header
    let header = yt_data
        .get("header")
        .and_then(|h| h.get("playlistHeaderRenderer"));

    let title = header
        .and_then(|h| h.get("title"))
        .and_then(|t| t.get("simpleText"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    let artist = header
        .and_then(|h| h.get("ownerText"))
        .and_then(|o| o.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first())
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    let videos = extract_playlist_videos(&yt_data);

    // Use first video's thumbnail as artwork
    let artwork_url = videos
        .first()
        .map(|v| format!("https://i.ytimg.com/vi/{}/mqdefault.jpg", v.video_id));

    let raw_tracks: Vec<FetchedTrack> = videos
        .into_iter()
        .enumerate()
        .map(|(idx, v)| FetchedTrack {
            name: v.title,
            position: if v.position > 0 {
                v.position
            } else {
                (idx + 1) as i32
            },
            duration_ms: v.duration_ms,
            video_id: Some(v.video_id),
        })
        .collect();

    let (tracks, inferred_artist) = strip_youtube_track_artist_prefix(raw_tracks, &artist);
    let artist = artist.or(inferred_artist);

    Ok(FetchedMetadata {
        artist,
        title,
        label: None,
        release_date: None,
        artwork_url,
        tracks,
        source_type: String::new(),
        parent_url: None,
        parent_album_title: None,
    })
}

/// Strip a consistent "Artist - " prefix from YouTube track names.
///
/// First tries to match the existing release artist (mirrors SoundCloud playlist logic).
/// If that doesn't produce a majority match, detects the most common prefix across all
/// tracks and uses that instead. Returns the stripped tracks and an inferred artist name
/// (only set when a new dominant prefix was found that differs from the existing artist).
fn strip_youtube_track_artist_prefix(
    tracks: Vec<FetchedTrack>,
    existing_artist: &Option<String>,
) -> (Vec<FetchedTrack>, Option<String>) {
    if tracks.is_empty() || is_compilation(existing_artist) {
        return (tracks, None);
    }

    // First try: strip the existing playlist artist prefix (mirrors SoundCloud playlist logic)
    if let Some(ref a) = existing_artist {
        let prefix = format!("{a} - ");
        let stripped: Vec<FetchedTrack> = tracks
            .iter()
            .map(|t| FetchedTrack {
                name: t
                    .name
                    .strip_prefix(&prefix)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| t.name.clone()),
                position: t.position,
                duration_ms: t.duration_ms,
                video_id: t.video_id.clone(),
            })
            .collect();
        let stripped_count = stripped
            .iter()
            .zip(tracks.iter())
            .filter(|(s, o)| s.name != o.name)
            .count();
        if stripped_count * 2 >= tracks.len() {
            return (stripped, None);
        }
    }

    // Second try: detect a dominant "Artist - " prefix across all tracks
    let mut prefix_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for track in &tracks {
        if let Some(idx) = track.name.find(" - ") {
            *prefix_counts
                .entry(track.name[..idx].to_string())
                .or_insert(0) += 1;
        }
    }

    if let Some((dominant, count)) = prefix_counts.into_iter().max_by_key(|(_, c)| *c) {
        if count * 2 >= tracks.len() {
            let prefix = format!("{dominant} - ");
            let stripped = tracks
                .into_iter()
                .map(|t| FetchedTrack {
                    name: t
                        .name
                        .strip_prefix(&prefix)
                        .map(|s| s.to_string())
                        .unwrap_or(t.name),
                    ..t
                })
                .collect();
            return (stripped, Some(dominant));
        }
    }

    (tracks, None)
}

async fn fetch_youtube_single(
    client: &reqwest::Client,
    url: &str,
    video_id: &str,
) -> Result<FetchedMetadata> {
    // Use oEmbed for basic metadata
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

    let artwork_url = Some(format!("https://i.ytimg.com/vi/{video_id}/mqdefault.jpg"));

    // Get duration from youtubei player API
    let duration_ms = match build_yt_client() {
        Ok(yt_client) => match fetch_yt_player_response(&yt_client, video_id).await {
            Ok(player) => player
                .get("videoDetails")
                .and_then(|vd| vd.get("lengthSeconds"))
                .and_then(|l| l.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .map(|s| s * 1000),
            Err(e) => {
                log::warn!("Failed to get YouTube video duration: {e}");
                None
            }
        },
        Err(e) => {
            log::warn!("Failed to build YouTube client for duration: {e}");
            None
        }
    };

    // Parse "Artist - Title" from the video title (mirrors SoundCloud single handling).
    // When found, the parsed artist takes precedence over the channel name.
    let (artist, track_name, title) =
        match title.as_deref().and_then(|t| t.find(" - ").map(|i| (t, i))) {
            Some((t, idx)) => (
                Some(t[..idx].to_string()),
                Some(t[idx + 3..].to_string()),
                Some(t[idx + 3..].to_string()),
            ),
            None => (artist, title.clone(), title),
        };

    let tracks = if let Some(name) = track_name {
        vec![FetchedTrack {
            name,
            position: 1,
            duration_ms,
            video_id: Some(video_id.to_string()),
        }]
    } else {
        Vec::new()
    };

    Ok(FetchedMetadata {
        artist,
        title,
        label: None,
        release_date: None,
        artwork_url,
        tracks,
        source_type: String::new(),
        parent_url: None,
        parent_album_title: None,
    })
}
