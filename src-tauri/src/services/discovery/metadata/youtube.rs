use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use crate::error::{CrateError, Result};

use super::{is_compilation, FetchedMetadata, FetchedTrack, YT_CONSENT_COOKIE};

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
    /// Whether the CDN serves this client's stream URLs to any user-agent. When false, the
    /// proxy must replay `user_agent` on every CDN request or the CDN answers 403.
    pub browser_compatible: bool,
    /// Extra context fields for native app clients (device info, OS version, etc.).
    pub extra_context: Option<&'static [(&'static str, &'static str)]>,
}

/// The visionOS YouTube app presents itself with a Safari user-agent.
const VISIONOS_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";

/// Fallback chain of YouTube innertube clients, ordered by preference.
///
/// Mirrors the unauthenticated defaults of yt-dlp's `INNERTUBE_CLIENTS` table
/// (`yt_dlp/extractor/youtube/_base.py`, release 2026.08.19). When YouTube breaks a client,
/// that table is the first place to look; `scripts/yt-probe.mjs` exercises this exact chain
/// outside the app.
///
/// - VISIONOS: yt-dlp's primary default since 2026.08. Needs no PO token and no JS player:
///   its stream URLs are direct (no `signatureCipher`, no throttling `n` parameter) and the
///   CDN serves them to any user-agent, so they work through the proxy with no transformation.
/// - ANDROID_VR: last-resort fallback. yt-dlp dropped it from its defaults in 2026.08 because
///   YouTube intermittently enforces PO tokens on it, but it still answers for some videos the
///   visionOS client refuses. Version must stay ≤ 1.65: newer versions only return SABR
///   (URL-less) formats.
///
/// Web clients (WEB, WEB_EMBEDDED_PLAYER, MWEB, TVHTML5) are deliberately absent: since mid
/// 2026 they return SABR-only streaming data without a PO token, which the app cannot mint.
pub(crate) const YT_CLIENTS: &[YtClientConfig] = &[
    YtClientConfig {
        client_name: "VISIONOS",
        client_id: "101",
        client_version: "1.02",
        user_agent: VISIONOS_USER_AGENT,
        browser_compatible: true,
        extra_context: Some(&[
            ("deviceMake", "Apple"),
            ("deviceModel", "RealityDevice17,1"),
            ("osName", "visionOS"),
            ("osVersion", "26.5.23O471"),
        ]),
    },
    YtClientConfig {
        client_name: "ANDROID_VR",
        client_id: "28",
        client_version: "1.65.10",
        user_agent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
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
        .timeout(Duration::from_secs(15))
        .user_agent(config.user_agent);
    if let Some(jar) = jar {
        builder = builder.cookie_provider(jar);
    }
    builder
        .build()
        .map_err(|e| CrateError::Discovery(format!("Failed to create YouTube client: {e}")))
}

/// Create a new cookie jar pre-seeded with the YouTube consent cookie.
fn new_yt_cookie_jar() -> Arc<reqwest::cookie::Jar> {
    let jar = reqwest::cookie::Jar::default();
    let yt_url = "https://www.youtube.com".parse::<reqwest::Url>().unwrap();
    jar.add_cookie_str(YT_CONSENT_COOKIE, &yt_url);
    Arc::new(jar)
}

// =============================================================================
// Session
// =============================================================================

/// Cookies and visitor identity shared by every YouTube player request in the process.
///
/// YouTube's bot check keys on the visitor identity: a player request that arrives with a
/// visitorData YouTube never issued (the synthetic kind the app used to generate per call)
/// is answered with `LOGIN_REQUIRED` "Sign in to confirm you're not a bot" for most videos,
/// while the same request carrying a visitorData minted by a real page load and the cookies
/// set alongside it succeeds. So the process keeps one browser-like session and reuses it.
#[derive(Clone)]
pub(crate) struct YtSession {
    pub jar: Arc<reqwest::cookie::Jar>,
    pub visitor_data: String,
    established_at: Instant,
}

static YT_SESSION: OnceLock<tokio::sync::Mutex<Option<YtSession>>> = OnceLock::new();

/// Visitor identities are long-lived, but refreshing occasionally keeps the session looking
/// like a browser that revisits the site rather than one frozen in time.
const YT_SESSION_TTL: Duration = Duration::from_secs(6 * 60 * 60);

fn session_slot() -> &'static tokio::sync::Mutex<Option<YtSession>> {
    YT_SESSION.get_or_init(|| tokio::sync::Mutex::new(None))
}

/// Return the shared session, establishing one on first use or after it expires.
pub(crate) async fn yt_session() -> YtSession {
    let mut slot = session_slot().lock().await;
    if let Some(session) = slot.as_ref() {
        if session.established_at.elapsed() < YT_SESSION_TTL {
            return session.clone();
        }
    }
    let session = establish_session().await;
    *slot = Some(session.clone());
    session
}

/// Replace the shared session after YouTube answered a player request with a bot check,
/// which marks the visitor identity as burned. `stale` is the session the caller used: if
/// another caller already replaced it, the replacement is returned as-is so concurrent
/// failures don't trigger a bootstrap storm.
pub(crate) async fn reset_yt_session(stale: &YtSession) -> YtSession {
    let mut slot = session_slot().lock().await;
    if let Some(current) = slot.as_ref() {
        if current.established_at != stale.established_at {
            return current.clone();
        }
    }
    let session = establish_session().await;
    *slot = Some(session.clone());
    session
}

/// Load youtube.com like a browser would to obtain a YouTube-issued visitor identity and its
/// accompanying cookies. Falls back to a synthetic identity if the page can't be read, which
/// keeps extraction attempting (and logging) rather than failing outright.
async fn establish_session() -> YtSession {
    let jar = new_yt_cookie_jar();
    let visitor_data = match fetch_visitor_data(&jar).await {
        Ok(vd) => {
            log::info!("Established YouTube session with page-issued visitorData");
            vd
        }
        Err(e) => {
            log::warn!("Could not obtain YouTube visitorData from the homepage ({e}); using a synthetic identity");
            generate_visitor_data()
        }
    };
    YtSession {
        jar,
        visitor_data,
        established_at: Instant::now(),
    }
}

async fn fetch_visitor_data(jar: &Arc<reqwest::cookie::Jar>) -> Result<String> {
    let client = build_yt_client_with_config(&YT_CLIENTS[0], Some(jar.clone()))?;
    let html = client
        .get("https://www.youtube.com/")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch YouTube homepage: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read YouTube homepage: {e}")))?;
    parse_visitor_data(&html)
        .ok_or_else(|| CrateError::Discovery("No VISITOR_DATA in YouTube homepage".into()))
}

/// Extract `"VISITOR_DATA":"..."` from a YouTube page's inline `ytcfg`.
pub(crate) fn parse_visitor_data(html: &str) -> Option<String> {
    let marker = "\"VISITOR_DATA\":\"";
    let start = html.find(marker)? + marker.len();
    let end = html[start..].find('"')?;
    let value = &html[start..start + end];
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Generate a randomized delay with jitter around a base duration.
/// Returns a `Duration` of `base_ms ± (0..base_ms/2)`, clamped to a minimum of 500ms.
pub(crate) fn jittered_delay(base_ms: u64) -> Duration {
    use rand::Rng;
    let jitter = rand::rng().random_range(0..=(base_ms / 2));
    let delay = if rand::rng().random_bool(0.5) {
        base_ms.saturating_add(jitter)
    } else {
        base_ms.saturating_sub(jitter)
    };
    Duration::from_millis(delay.max(500))
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
/// Only a fallback for when the homepage can't be read (see [`establish_session`]): YouTube
/// treats identities it did not issue as bot traffic for most videos.
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
///
/// `client` must carry the session's cookie jar: cookies are sent from the jar rather than
/// an explicit header because reqwest drops jar cookies whenever a request sets `Cookie`
/// itself, and the bot check wants the cookies that came with `visitor_data`.
pub(crate) async fn fetch_yt_player_response_with_config(
    client: &reqwest::Client,
    video_id: &str,
    config: &YtClientConfig,
    visitor_data: &str,
) -> Result<serde_json::Value> {
    let mut client_ctx = serde_json::json!({
        "clientName": config.client_name,
        "clientVersion": config.client_version,
        "hl": "en",
        "timeZone": "UTC",
        "utcOffsetMinutes": 0,
        "visitorData": visitor_data,
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
        .header("X-Goog-Visitor-Id", visitor_data)
        .json(&body)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch YouTube player data: {e}")))?
        .json()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to parse YouTube player response: {e}")))
}

/// Call YouTube's internal player API using the primary client config and the shared session.
async fn fetch_yt_player_response(video_id: &str) -> Result<serde_json::Value> {
    let session = yt_session().await;
    let config = &YT_CLIENTS[0];
    let client = build_yt_client_with_config(config, Some(session.jar))?;
    fetch_yt_player_response_with_config(&client, video_id, config, &session.visitor_data).await
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

// =============================================================================
// Playlist pages
// =============================================================================

#[derive(Debug, Clone)]
pub(crate) struct YouTubeVideo {
    pub video_id: String,
    pub title: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
}

/// Extract video entries from a playlist page's ytInitialData.
///
/// YouTube has two renderings of a playlist's item section: the legacy
/// `playlistVideoListRenderer` (a list of `playlistVideoRenderer`s) and, since mid 2026, a
/// flat run of `lockupViewModel`s. Both are read so the parser survives YouTube rolling the
/// change out (or back) per page variant.
pub(crate) fn extract_playlist_videos(yt_data: &serde_json::Value) -> Vec<YouTubeVideo> {
    let items = yt_data
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
        .and_then(|c| c.as_array());

    let Some(items) = items else {
        return Vec::new();
    };

    let mut videos = Vec::new();
    for item in items {
        if let Some(list) = item
            .get("playlistVideoListRenderer")
            .and_then(|r| r.get("contents"))
            .and_then(|c| c.as_array())
        {
            videos.extend(list.iter().filter_map(parse_playlist_video_renderer));
        } else if let Some(lockup) = item.get("lockupViewModel") {
            let position = videos.len() as i32 + 1;
            videos.extend(parse_lockup_video(lockup, position));
        }
    }
    videos
}

fn parse_playlist_video_renderer(item: &serde_json::Value) -> Option<YouTubeVideo> {
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
}

/// Lockups carry no playlist index, so the caller supplies the running position.
fn parse_lockup_video(lockup: &serde_json::Value, position: i32) -> Option<YouTubeVideo> {
    let content_type = lockup.get("contentType").and_then(|t| t.as_str());
    if content_type.is_some_and(|t| t != "LOCKUP_CONTENT_TYPE_VIDEO") {
        return None;
    }
    let video_id = lockup
        .get("contentId")
        .and_then(|v| v.as_str())?
        .to_string();
    let title = lockup
        .get("metadata")
        .and_then(|m| m.get("lockupMetadataViewModel"))
        .and_then(|m| m.get("title"))
        .and_then(|t| t.get("content"))
        .and_then(|t| t.as_str())
        .unwrap_or("Untitled")
        .to_string();

    // The duration is the thumbnail's bottom-corner badge ("3:39"), which lives under one of
    // two overlay shapes depending on the page variant.
    let duration_ms = lockup
        .get("contentImage")
        .and_then(|c| c.get("thumbnailViewModel"))
        .and_then(|t| t.get("overlays"))
        .and_then(|o| o.as_array())
        .and_then(|overlays| {
            overlays.iter().find_map(|overlay| {
                let badges = overlay
                    .get("thumbnailBottomOverlayViewModel")
                    .and_then(|b| b.get("badges"))
                    .or_else(|| {
                        overlay
                            .get("thumbnailOverlayBadgeViewModel")
                            .and_then(|b| b.get("thumbnailBadges"))
                    })?
                    .as_array()?;
                badges.iter().find_map(|badge| {
                    badge
                        .get("thumbnailBadgeViewModel")
                        .and_then(|b| b.get("text"))
                        .and_then(|t| t.as_str())
                        .and_then(parse_duration_text)
                })
            })
        });

    Some(YouTubeVideo {
        video_id,
        title,
        position,
        duration_ms,
    })
}

/// Parse a "H:MM:SS" / "M:SS" badge into milliseconds. Non-duration badges ("LIVE",
/// "SHORTS") yield `None`.
pub(crate) fn parse_duration_text(text: &str) -> Option<i64> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut seconds: i64 = 0;
    for part in parts {
        let value: i64 = part.parse().ok()?;
        seconds = seconds * 60 + value;
    }
    Some(seconds * 1000)
}

/// Extract `(title, owner)` from a playlist page's header, which is either the legacy
/// `playlistHeaderRenderer` or the newer `pageHeaderRenderer` view-model tree.
pub(crate) fn parse_playlist_header(
    yt_data: &serde_json::Value,
) -> (Option<String>, Option<String>) {
    let header = yt_data.get("header");

    if let Some(h) = header.and_then(|h| h.get("playlistHeaderRenderer")) {
        let title = h
            .get("title")
            .and_then(|t| t.get("simpleText"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        let owner = h
            .get("ownerText")
            .and_then(|o| o.get("runs"))
            .and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());
        return (title, owner);
    }

    if let Some(h) = header.and_then(|h| h.get("pageHeaderRenderer")) {
        let title = h
            .get("pageTitle")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());
        // The owner is the avatar-stack row of the header's metadata.
        let owner = h
            .get("content")
            .and_then(|c| c.get("pageHeaderViewModel"))
            .and_then(|v| v.get("metadata"))
            .and_then(|m| m.get("contentMetadataViewModel"))
            .and_then(|m| m.get("metadataRows"))
            .and_then(|r| r.as_array())
            .and_then(|rows| {
                rows.iter().find_map(|row| {
                    row.get("metadataParts")?
                        .as_array()?
                        .iter()
                        .find_map(|part| {
                            part.get("avatarStack")?
                                .get("avatarStackViewModel")?
                                .get("text")?
                                .get("content")?
                                .as_str()
                                .map(|s| s.to_string())
                        })
                })
            });
        return (title, owner);
    }

    (None, None)
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

    let (title, artist) = parse_playlist_header(&yt_data);
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
            url: None,
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
                url: t.url.clone(),
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
    let duration_ms = match fetch_yt_player_response(video_id).await {
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
            url: None,
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
