use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{CrateError, Result};
use crate::services::DiscoveryService;

use tauri::Manager;

/// Maximum bytes returned per proxy response (~1 MB) when forwarding to CDN.
/// Used only during the brief window before the background download finishes.
const MAX_PROXY_CHUNK: u64 = 1_048_576;

/// Maximum number of cached audio entries kept in memory.
const MAX_CACHE_ENTRIES: usize = 3;

/// Fully downloaded audio file held in memory for instant range serving.
struct CachedAudio {
    data: Vec<u8>,
    content_type: String,
}

/// Shared state threaded into every axum proxy request handler.
#[derive(Clone)]
pub(crate) struct ProxyServerState {
    pub app_handle: tauri::AppHandle,
    pub client: reqwest::Client,
    /// Cache of fully downloaded audio files, keyed by "{release_id}/{track_position}".
    cache: Arc<tokio::sync::RwLock<HashMap<String, Arc<CachedAudio>>>>,
    /// Tracks which keys have a background download in flight to prevent duplicates.
    downloads_in_progress: Arc<tokio::sync::Mutex<std::collections::HashSet<String>>>,
}

impl ProxyServerState {
    pub fn new(app_handle: tauri::AppHandle, client: reqwest::Client) -> Self {
        Self {
            app_handle,
            client,
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            downloads_in_progress: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        }
    }
}

/// Parse a `bytes=start-[end]` Range header value and return `(start, optional_end)`.
fn parse_bytes_range(header: Option<&str>) -> (u64, Option<u64>) {
    let s = match header {
        Some(h) => h.trim_start_matches("bytes="),
        None => return (0, None),
    };
    let mut parts = s.splitn(2, '-');
    let start = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let end = parts.next().and_then(|v| v.parse().ok());
    (start, end)
}

/// Top-level axum handler for `GET /{release_id}/{track_position}`.
///
/// YouTube stream URLs are signed for non-browser user-agents. The HTML5 Audio element sends a
/// browser UA, causing a 403. This server fetches the stream with the correct UA server-side and
/// proxies the bytes back to the media element. Using a real HTTP server (rather than a custom URI
/// scheme) ensures WKWebView's AVFoundation media layer can seek correctly via Range requests.
pub(crate) async fn proxy_http_handler(
    axum::extract::Path((release_id, track_position)): axum::extract::Path<(String, i32)>,
    req_headers: axum::http::HeaderMap,
    axum::extract::State(state): axum::extract::State<ProxyServerState>,
) -> axum::http::Response<axum::body::Body> {
    match proxy_http_handler_inner(&release_id, track_position, &req_headers, &state).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Stream proxy error: {e}");
            axum::http::Response::builder()
                .status(502)
                .header("Content-Type", "text/plain")
                .body(axum::body::Body::from("Internal proxy error"))
                .unwrap()
        }
    }
}

async fn proxy_http_handler_inner(
    release_id: &str,
    track_position: i32,
    req_headers: &axum::http::HeaderMap,
    state: &ProxyServerState,
) -> Result<axum::http::Response<axum::body::Body>> {
    let cache_key = format!("{release_id}/{track_position}");
    let incoming_range = req_headers.get("Range").and_then(|v| v.to_str().ok());
    let (start, end_opt) = parse_bytes_range(incoming_range);

    // 1. Check cache — if the full file is already downloaded, serve from memory.
    {
        let cache = state.cache.read().await;
        if let Some(cached) = cache.get(&cache_key) {
            return serve_range_from_cache(cached, start, end_opt);
        }
    }

    // 2. Look up the stream URL and proxy UA from the database.
    let discovery = state.app_handle.state::<DiscoveryService>();
    let cached_stream = discovery
        .get_cached_stream(release_id, track_position)?
        .ok_or_else(|| CrateError::Discovery("No cached stream for proxy request".into()))?;

    let proxy_ua = cached_stream.proxy_ua.ok_or_else(|| {
        CrateError::Discovery("Cached stream has no proxy_ua — should use direct URL".into())
    })?;

    // 3. Kick off a background download if one isn't already running for this key.
    {
        let mut in_progress = state.downloads_in_progress.lock().await;
        if !in_progress.contains(&cache_key) {
            in_progress.insert(cache_key.clone());

            let state_clone = state.clone();
            let url = cached_stream.stream_url.clone();
            let ua = proxy_ua.clone();
            let key = cache_key.clone();

            tokio::spawn(async move {
                match download_full_stream(&state_clone, &url, &ua).await {
                    Ok(cached_audio) => {
                        let entry = Arc::new(cached_audio);
                        let mut cache = state_clone.cache.write().await;

                        // Evict oldest entries if at capacity.
                        while cache.len() >= MAX_CACHE_ENTRIES {
                            if let Some(oldest_key) = cache.keys().next().cloned() {
                                cache.remove(&oldest_key);
                            }
                        }

                        cache.insert(key.clone(), entry);
                        log::info!("Cached full audio for proxy key: {key}");
                    }
                    Err(e) => {
                        log::warn!("Background stream download failed for {key}: {e}");
                    }
                }

                state_clone.downloads_in_progress.lock().await.remove(&key);
            });
        }
    }

    // 4. Forward this request to the CDN while the background download runs.
    forward_to_cdn(state, &cached_stream.stream_url, &proxy_ua, start, end_opt).await
}

/// Download the entire audio file in one request and return a `CachedAudio`.
async fn download_full_stream(
    state: &ProxyServerState,
    stream_url: &str,
    proxy_ua: &str,
) -> Result<CachedAudio> {
    let response = state
        .client
        .get(stream_url)
        .header("User-Agent", proxy_ua)
        // YouTube CDN rejects plain GET requests without a Range header (403).
        // "bytes=0-" requests the entire file as a single range response.
        .header("Range", "bytes=0-")
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Background download request failed: {e:#}")))?;

    if !response.status().is_success() && response.status().as_u16() != 206 {
        return Err(CrateError::Discovery(format!(
            "Background download returned {}",
            response.status()
        )));
    }

    let content_type = response
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("audio/mp4")
        .to_string();

    let data = response
        .bytes()
        .await
        .map_err(|e| CrateError::Discovery(format!("Background download body failed: {e:#}")))?
        .to_vec();

    log::info!(
        "Background download complete: {} bytes, content-type: {content_type}",
        data.len()
    );

    Ok(CachedAudio { data, content_type })
}

/// Serve a byte range from a fully cached audio file.
fn serve_range_from_cache(
    cached: &CachedAudio,
    start: u64,
    end_opt: Option<u64>,
) -> Result<axum::http::Response<axum::body::Body>> {
    let total = cached.data.len() as u64;

    if start >= total {
        return Ok(axum::http::Response::builder()
            .status(416)
            .header("Content-Range", format!("bytes */{total}"))
            .body(axum::body::Body::empty())
            .unwrap());
    }

    let end = end_opt.map_or(total - 1, |e| e.min(total - 1));
    let slice = &cached.data[start as usize..=end as usize];

    Ok(axum::http::Response::builder()
        .status(206)
        .header("Content-Type", &cached.content_type)
        .header("Content-Range", format!("bytes {start}-{end}/{total}"))
        .header("Content-Length", slice.len().to_string())
        .header("Accept-Ranges", "bytes")
        .header("Access-Control-Allow-Origin", "*")
        .body(axum::body::Body::from(slice.to_vec()))
        .unwrap())
}

/// Forward a range request to the upstream CDN (used before cache is populated).
async fn forward_to_cdn(
    state: &ProxyServerState,
    stream_url: &str,
    proxy_ua: &str,
    start: u64,
    end_opt: Option<u64>,
) -> Result<axum::http::Response<axum::body::Body>> {
    let clamped_end = match end_opt {
        Some(e) => e.min(start.saturating_add(MAX_PROXY_CHUNK - 1)),
        None => start.saturating_add(MAX_PROXY_CHUNK - 1),
    };

    let cdn_response = state
        .client
        .get(stream_url)
        .header("User-Agent", proxy_ua)
        .header("Range", format!("bytes={start}-{clamped_end}"))
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch stream: {e:#}")))?;

    if !cdn_response.status().is_success() && cdn_response.status().as_u16() != 206 {
        return Ok(axum::http::Response::builder()
            .status(502)
            .body(axum::body::Body::from(format!(
                "CDN returned {}",
                cdn_response.status()
            )))
            .unwrap());
    }

    let status = cdn_response.status().as_u16();
    let content_type = cdn_response
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("audio/mp4")
        .to_string();
    let content_range = cdn_response
        .headers()
        .get("Content-Range")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let content_length = cdn_response
        .headers()
        .get("Content-Length")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let stream = cdn_response.bytes_stream();

    let mut builder = axum::http::Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        .header("Access-Control-Allow-Origin", "*")
        .header("Accept-Ranges", "bytes");

    if let Some(cr) = content_range {
        builder = builder.header("Content-Range", cr);
    }
    if let Some(cl) = content_length {
        builder = builder.header("Content-Length", cl);
    }

    Ok(builder.body(axum::body::Body::from_stream(stream)).unwrap())
}
