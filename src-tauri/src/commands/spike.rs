//! Throwaway validation harness for the iOS audio-proxy spike (#80).
//!
//! Mirrors the WebView <-> Rust round-trip that `services::discovery::n_transform` relies on for
//! YouTube n-param de-throttling — inject a script with `Webview::eval`, have the injected JS call
//! back into Rust via a Tauri command, and unblock the awaiting future through a oneshot channel —
//! but without the ~1.5 MB EJS solver / player-JS download. It isolates the three iOS-specific risks
//! that path depends on: (1) injected JS actually executes in WKWebView, (2) the JS -> Rust IPC
//! callback works, and (3) `get_webview_window("main")` resolves on mobile.
//!
//! Remove (or fold into the real preview feature, #54) once the spike findings are captured.

use std::collections::HashMap;

use tauri::{Manager, State};

use crate::error::{CrateError, Result};

/// Tauri-managed state coordinating the WebView -> Rust callback for the spike round-trip.
pub struct SpikeEvalState {
    /// Pending round-trips: request_id -> result sender.
    pub pending: tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>,
}

impl SpikeEvalState {
    pub fn new() -> Self {
        Self {
            pending: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

/// Inject a trivial script into the `"main"` WebView and wait for it to call back into Rust.
///
/// Returns the JSON string the WebView produced (a `{ sum, userAgent }` payload) so the harness can
/// confirm both that the injected script ran and what WKWebView reports as its user agent. On iOS
/// this exercises the exact mechanism `n_transform` needs; a failure here would predict a broken
/// YouTube de-throttle on iOS.
#[tauri::command]
pub async fn spike_webview_roundtrip(
    app: tauri::AppHandle,
    state: State<'_, SpikeEvalState>,
) -> Result<String> {
    let request_id = uuid::Uuid::new_v4().to_string();

    let (tx, rx) = tokio::sync::oneshot::channel();
    state.pending.lock().await.insert(request_id.clone(), tx);

    // Compute a trivial value (2 + 2) and report the WKWebView user agent, then hand the result back
    // to Rust via the same `window.__TAURI_INTERNALS__.invoke(...)` callback shape `n_transform` uses.
    let script = format!(
        r#"(function() {{
  try {{
    var payload = JSON.stringify({{ sum: 2 + 2, userAgent: navigator.userAgent }});
    window.__TAURI_INTERNALS__.invoke('spike_webview_callback', {{
      requestId: '{request_id}',
      value: payload
    }});
  }} catch (e) {{
    window.__TAURI_INTERNALS__.invoke('spike_webview_callback', {{
      requestId: '{request_id}',
      error: e.toString()
    }});
  }}
}})();"#,
    );

    let webview = app
        .get_webview_window("main")
        .ok_or_else(|| CrateError::Discovery("WebView 'main' not found".into()))?;
    webview
        .eval(&script)
        .map_err(|e| CrateError::Discovery(format!("WebView script injection failed: {e}")))?;

    match tokio::time::timeout(std::time::Duration::from_secs(10), rx).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err(CrateError::Discovery(
            "Spike WebView callback channel dropped".into(),
        )),
        Err(_) => {
            state.pending.lock().await.remove(&request_id);
            Err(CrateError::Discovery(
                "Spike WebView round-trip timed out (10s)".into(),
            ))
        }
    }
}

/// Callback invoked from the WebView once the injected script runs. Unblocks the awaiting round-trip.
#[tauri::command]
pub async fn spike_webview_callback(
    request_id: String,
    value: Option<String>,
    error: Option<String>,
    state: State<'_, SpikeEvalState>,
) -> Result<()> {
    let response = error
        .map(|e| format!("ERROR: {e}"))
        .unwrap_or_else(|| value.unwrap_or_default());
    if let Some(tx) = state.pending.lock().await.remove(&request_id) {
        let _ = tx.send(response);
    }
    Ok(())
}
