use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::Manager;

use crate::error::{CrateError, Result};

use super::metadata::{extract_query_param, YT_CONSENT_COOKIE};

const EJS_VERSION: &str = "0.5.0";
const EJS_CORE_URL: &str =
    "https://github.com/yt-dlp/ejs/releases/download/0.5.0/yt.solver.core.js";
const EJS_LIB_URL: &str = "https://github.com/yt-dlp/ejs/releases/download/0.5.0/yt.solver.lib.js";
const YT_IFRAME_API_URL: &str = "https://www.youtube.com/iframe_api";

/// Tauri-managed state for coordinating WebView <-> Rust solver callbacks.
pub struct NsigSolverState {
    /// Pending solve requests: request_id -> result sender
    pub pending: tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>,
    /// Whether solver scripts have been loaded into the WebView's global scope
    solver_loaded: AtomicBool,
    /// Cached preprocessed player (avoids re-parsing 1.5MB player JS each time)
    preprocessed: tokio::sync::RwLock<Option<(String, String)>>,
    /// In-memory n-value result cache: "n_value" -> "transformed_n"
    n_cache: tokio::sync::RwLock<HashMap<String, String>>,
}

impl NsigSolverState {
    pub fn new() -> Self {
        Self {
            pending: tokio::sync::Mutex::new(HashMap::new()),
            solver_loaded: AtomicBool::new(false),
            preprocessed: tokio::sync::RwLock::new(None),
            n_cache: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

/// Transform the `n` query parameter in a YouTube CDN stream URL.
/// Non-fatal: returns original URL on any failure.
///
/// # Safety of WebView eval usage
/// The only code executed via `Webview::eval()` is:
/// - yt-dlp's verified EJS solver scripts (downloaded from a pinned GitHub release URL)
/// - YouTube's player JS (fetched from youtube.com)
///   No user-supplied code is ever evaluated.
pub async fn transform_n_param(
    stream_url: &str,
    app_handle: &tauri::AppHandle,
    app_data_dir: &Path,
) -> String {
    match try_transform(stream_url, app_handle, app_data_dir).await {
        Ok(transformed) => {
            log::info!("n-param transformation succeeded");
            transformed
        }
        Err(e) => {
            log::warn!("n-param transformation failed, using original URL: {e}");
            stream_url.to_string()
        }
    }
}

async fn try_transform(
    stream_url: &str,
    app_handle: &tauri::AppHandle,
    app_data_dir: &Path,
) -> Result<String> {
    // Log the URL's query parameters for debugging (redact the full URL for privacy)
    if let Some(query) = stream_url.split('?').nth(1) {
        let param_keys: Vec<&str> = query
            .split('&')
            .filter_map(|p| p.split('=').next())
            .collect();
        log::debug!("Stream URL query param keys: {param_keys:?}");
    }

    // 1. Extract `n` param from URL — absent means no throttling, skip silently
    let n_value = match extract_query_param(stream_url, "n") {
        Some(v) => v,
        None => {
            log::debug!("No 'n' parameter in stream URL, skipping transformation");
            return Ok(stream_url.to_string());
        }
    };

    // 2. Check in-memory n_cache
    let state = app_handle.state::<NsigSolverState>();
    {
        let cache = state.n_cache.read().await;
        if let Some(cached) = cache.get(&n_value) {
            return Ok(replace_query_param(stream_url, "n", cached));
        }
    }

    let client = super::metadata::build_client()?;

    // 3. Fetch player version
    let player_version = fetch_player_version(&client).await?;

    // 4. Ensure solver scripts on disk
    let solver_dir = app_data_dir.join("ejs_solver").join(EJS_VERSION);
    ensure_solver_scripts(&client, &solver_dir).await?;

    // 5. Ensure player JS on disk
    let player_js = fetch_player_js(&client, &player_version, app_data_dir).await?;

    // 6. Ensure solver loaded into WebView (one-time)
    if !state.solver_loaded.load(Ordering::Acquire) {
        load_solver_into_webview(app_handle, &solver_dir).await?;
        state.solver_loaded.store(true, Ordering::Release);
    }

    // 7. Build solver invocation script
    let request_id = uuid::Uuid::new_v4().to_string();
    let invocation_script =
        build_invocation_script(&request_id, &n_value, &player_js, &player_version, &state).await;

    // 8. Set up oneshot channel
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut pending = state.pending.lock().await;
        pending.insert(request_id.clone(), tx);
    }

    // 9. Inject invocation into WebView
    let webview = app_handle
        .get_webview_window("main")
        .ok_or_else(|| CrateError::Discovery("WebView 'main' not found".into()))?;
    webview
        .eval(&invocation_script)
        .map_err(|e| CrateError::Discovery(format!("WebView eval failed: {e}")))?;

    // 10. Await result with timeout
    let result = match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => {
            return Err(CrateError::Discovery(
                "Solver oneshot channel dropped".into(),
            ));
        }
        Err(_) => {
            // Clean up pending entry on timeout
            state.pending.lock().await.remove(&request_id);
            return Err(CrateError::Discovery(
                "n-param solver timed out (30s)".into(),
            ));
        }
    };

    // 11. Parse result
    let (transformed_n, preprocessed_player) = parse_solver_output(&result, &n_value)?;

    // Cache preprocessed player if returned
    if let Some(pp) = preprocessed_player {
        let mut guard = state.preprocessed.write().await;
        *guard = Some((player_version.clone(), pp));
    }

    // Cache n-value result
    {
        let mut cache = state.n_cache.write().await;
        cache.insert(n_value, transformed_n.clone());
    }

    Ok(replace_query_param(stream_url, "n", &transformed_n))
}

/// Fetch the current YouTube player version from the iframe API.
async fn fetch_player_version(client: &reqwest::Client) -> Result<String> {
    let body = client
        .get(YT_IFRAME_API_URL)
        .header("Cookie", YT_CONSENT_COOKIE)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch iframe_api: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read iframe_api: {e}")))?;

    let re = regex::Regex::new(r"/s/player/([a-zA-Z0-9]+)/")
        .map_err(|e| CrateError::Discovery(format!("Regex error: {e}")))?;

    let version = re
        .captures(&body)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| {
            CrateError::Discovery("Could not find player version in iframe_api".into())
        })?;

    log::info!("YouTube player version: {version}");
    Ok(version)
}

/// Download solver scripts (core.js + lib.js) to disk if not already cached.
async fn ensure_solver_scripts(client: &reqwest::Client, solver_dir: &Path) -> Result<()> {
    let core_path = solver_dir.join("yt.solver.core.js");
    let lib_path = solver_dir.join("yt.solver.lib.js");

    if core_path.exists() && lib_path.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(solver_dir)?;

    download_file(client, EJS_CORE_URL, &core_path).await?;
    download_file(client, EJS_LIB_URL, &lib_path).await?;

    log::info!("Downloaded EJS solver scripts v{EJS_VERSION}");
    Ok(())
}

/// Download a file to disk.
async fn download_file(client: &reqwest::Client, url: &str, dest: &Path) -> Result<()> {
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to download {url}: {e}")))?
        .bytes()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read {url}: {e}")))?;

    std::fs::write(dest, &bytes)?;
    Ok(())
}

/// Download player base.js if not cached, clean old versions.
async fn fetch_player_js(
    client: &reqwest::Client,
    version: &str,
    app_data_dir: &Path,
) -> Result<String> {
    let cache_dir = app_data_dir.join("yt_player_cache");
    let player_path = cache_dir.join(format!("{version}.js"));

    if player_path.exists() {
        return std::fs::read_to_string(&player_path)
            .map_err(|e| CrateError::Discovery(format!("Failed to read cached player JS: {e}")));
    }

    std::fs::create_dir_all(&cache_dir)?;

    let url = format!("https://www.youtube.com/s/player/{version}/player_ias.vflset/en_US/base.js");
    let body = client
        .get(&url)
        .header("Cookie", YT_CONSENT_COOKIE)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to download player JS: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read player JS: {e}")))?;

    std::fs::write(&player_path, &body)?;

    // Clean old player versions
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".js") && name_str != format!("{version}.js") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    log::info!("Downloaded YouTube player JS for version {version}");
    Ok(body)
}

/// Load the EJS solver scripts into the WebView's global scope (one-time).
///
/// # Safety of WebView eval usage
/// Only executes yt-dlp's verified EJS solver scripts downloaded from a pinned GitHub release.
async fn load_solver_into_webview(app_handle: &tauri::AppHandle, solver_dir: &Path) -> Result<()> {
    let lib_js = std::fs::read_to_string(solver_dir.join("yt.solver.lib.js"))
        .map_err(|e| CrateError::Discovery(format!("Failed to read solver lib: {e}")))?;
    let core_js = std::fs::read_to_string(solver_dir.join("yt.solver.core.js"))
        .map_err(|e| CrateError::Discovery(format!("Failed to read solver core: {e}")))?;

    // Wrap in IIFE to avoid polluting the global scope, expose only the solver function
    let loader_script =
        format!("(function() {{\n{lib_js}\n{core_js}\nglobalThis.__crate_jsc = jsc;\n}})();",);

    let webview = app_handle
        .get_webview_window("main")
        .ok_or_else(|| CrateError::Discovery("WebView 'main' not found".into()))?;
    webview
        .eval(&loader_script)
        .map_err(|e| CrateError::Discovery(format!("Failed to load solver scripts: {e}")))?;

    // Give the WebView a moment to parse the solver scripts (~500ms for ~1MB JS)
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    log::info!("Loaded EJS solver scripts into WebView");
    Ok(())
}

/// Build the JS invocation script for a single n-param solve.
async fn build_invocation_script(
    request_id: &str,
    n_value: &str,
    player_js: &str,
    player_version: &str,
    state: &NsigSolverState,
) -> String {
    // Check if we have a preprocessed player for this version
    let preprocessed = state.preprocessed.read().await;
    let (input_type, player_data) = if let Some((ver, pp)) = preprocessed.as_ref() {
        if ver == player_version {
            ("preprocessed", format!("\"preprocessed_player\": {pp}"))
        } else {
            drop(preprocessed);
            let escaped = escape_js_string(player_js);
            ("player", format!("\"player\": \"{escaped}\""))
        }
    } else {
        drop(preprocessed);
        let escaped = escape_js_string(player_js);
        ("player", format!("\"player\": \"{escaped}\""))
    };

    let output_preprocessed = if input_type == "player" {
        "true"
    } else {
        "false"
    };

    // The invocation script calls the solver and sends the result back via Tauri command
    format!(
        r#"(async function() {{
  try {{
    var input = {{
      "type": "{input_type}",
      {player_data},
      "requests": [{{"type": "n", "challenges": ["{n_value}"]}}],
      "output_preprocessed": {output_preprocessed}
    }};
    var result = globalThis.__crate_jsc(input);
    var resultStr = JSON.stringify(result);
    window.__TAURI_INTERNALS__.invoke('nsig_solve_callback', {{
      requestId: '{request_id}',
      result: resultStr
    }});
  }} catch (e) {{
    window.__TAURI_INTERNALS__.invoke('nsig_solve_callback', {{
      requestId: '{request_id}',
      error: e.toString()
    }});
  }}
}})();"#,
    )
}

/// Escape a JavaScript string for embedding in JS source.
fn escape_js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 8);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            '<' => out.push_str("\\x3c"),
            other => out.push(other),
        }
    }
    out
}

/// Parse the solver output JSON and extract the transformed n value.
fn parse_solver_output(json_str: &str, n_value: &str) -> Result<(String, Option<String>)> {
    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| CrateError::Discovery(format!("Failed to parse solver output: {e}")))?;

    if let Some(err) = parsed.get("error").and_then(|e| e.as_str()) {
        return Err(CrateError::Discovery(format!("Solver error: {err}")));
    }

    // Extract the n-transform result from the responses array
    let responses = parsed
        .get("responses")
        .and_then(|r| r.as_array())
        .ok_or_else(|| CrateError::Discovery("No 'responses' in solver output".into()))?;

    let n_response = responses
        .first()
        .ok_or_else(|| CrateError::Discovery("Empty 'responses' array in solver output".into()))?;

    let results = n_response
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| CrateError::Discovery("No 'results' in n response".into()))?;

    let transformed = results
        .first()
        .and_then(|r| r.as_str())
        .ok_or_else(|| CrateError::Discovery("Empty n-transform results".into()))?;

    if transformed == n_value {
        return Err(CrateError::Discovery(
            "n-transform returned same value (likely solver failure)".into(),
        ));
    }

    // Extract optional preprocessed_player
    let preprocessed = parsed.get("preprocessed_player").map(|pp| pp.to_string());

    log::debug!("n-transform: {n_value} -> {transformed}");
    Ok((transformed.to_string(), preprocessed))
}

/// Replace a query parameter value in a URL.
fn replace_query_param(url: &str, key: &str, new_value: &str) -> String {
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b, q),
        None => return url.to_string(),
    };

    let prefix = format!("{key}=");
    let new_params: Vec<String> = query
        .split('&')
        .map(|param| {
            if param.starts_with(&prefix) {
                format!("{prefix}{new_value}")
            } else {
                param.to_string()
            }
        })
        .collect();

    format!("{base}?{}", new_params.join("&"))
}
