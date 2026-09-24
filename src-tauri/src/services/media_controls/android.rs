//! Android media-session backend: JNI bridge to `CrateMediaService.kt` (#62).
//!
//! Preview audio plays in the WebView (HTML5 `<audio>`); Android System WebView never surfaces
//! the W3C Media Session to the OS, so the Kotlin foreground service owns the lock-screen /
//! notification surface and audio focus. This backend forwards the `update_now_playing` /
//! `update_playback_state` / `clear_now_playing` IPC into the service's `@JvmStatic` entry
//! points, and the reverse JNI export below re-emits the service's remote-command and
//! audio-focus callbacks as Tauri events for `shared/services/androidMediaSession.ts`:
//!
//! - transport: the shared `media-toggle`/`media-play`/`media-pause`/`media-next`/
//!   `media-previous` (souvlaki convention) plus `media-seek { positionMs }`
//! - focus: `media-focus-loss`, `media-focus-loss-transient`, `media-focus-gain`,
//!   `media-duck { active }` (ducking must be applied to the HTML5 element in JS — the native
//!   side has no handle on WebView audio volume)
//!
//! All bridge calls run on the foreground command path, so `ndk_context` is populated (the
//! `android_context` helper is used for uniformity and its exception hygiene). Failures are
//! logged, never fatal — matching the souvlaki backend's infallible style.
//!
//! ## Reconciliation surface (validate on device)
//! This file only type-checks with `cargo check --target aarch64-linux-android`. `RECONCILE`
//! marks the JNI shapes against the Kotlin signatures.

use std::sync::OnceLock;
use std::time::Duration;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jlong;
use jni::JNIEnv;
use tauri::{AppHandle, Emitter};

use crate::error::CrateError;

use super::{MediaSession, NowPlayingMetadata, PlaybackStatus};

/// Fully-qualified JNI class name of the Kotlin service.
const SERVICE_CLASS: &str = "com/bbx_audio/crateapp/CrateMediaService";

/// AppHandle for the reverse (Kotlin → Rust) event path. Set once when the backend is built.
static EVENT_APP: OnceLock<AppHandle> = OnceLock::new();

pub struct AndroidMediaSession;

impl AndroidMediaSession {
    pub fn new(app_handle: &AppHandle) -> Self {
        let _ = EVENT_APP.set(app_handle.clone());
        Self
    }

    /// Run a JNI call against the Kotlin service, logging (not propagating) failures.
    fn call(&self, what: &str, f: impl FnOnce(&mut JNIEnv, &JObject) -> crate::error::Result<()>) {
        let result = crate::android_context::with_env_and_context(f, |m| {
            CrateError::Audio(format!("media controls JNI: {m}"))
        });
        if let Err(e) = result {
            log::warn!("media_controls: {what} failed: {e}");
        }
    }
}

/// A nullable Kotlin `String?` argument: a local ref for `Some`, JNI null for `None`.
fn opt_jstring<'local>(
    env: &mut JNIEnv<'local>,
    s: Option<&str>,
) -> crate::error::Result<JObject<'local>> {
    match s {
        Some(s) => Ok(JObject::from(env.new_string(s).map_err(|e| {
            CrateError::Audio(format!("media controls JNI string: {e}"))
        })?)),
        None => Ok(JObject::null()),
    }
}

impl MediaSession for AndroidMediaSession {
    fn set_metadata(&self, meta: &NowPlayingMetadata) {
        let meta = meta.clone();
        self.call("updateMetadata", move |env, context| {
            let title = opt_jstring(env, meta.title.as_deref())?;
            let artist = opt_jstring(env, meta.artist.as_deref())?;
            let album = opt_jstring(env, meta.album.as_deref())?;
            let artwork = opt_jstring(env, meta.cover_url.as_deref())?;
            let duration_ms = meta.duration.map(|d| d.as_millis() as i64).unwrap_or(0);

            // RECONCILE: signature
            // `(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;J)V`.
            env.call_static_method(
                SERVICE_CLASS,
                "updateMetadata",
                "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;J)V",
                &[
                    JValue::Object(context),
                    JValue::Object(&title),
                    JValue::Object(&artist),
                    JValue::Object(&album),
                    JValue::Object(&artwork),
                    JValue::Long(duration_ms),
                ],
            )
            .map_err(|e| CrateError::Audio(format!("updateMetadata: {e}")))?;
            Ok(())
        });
    }

    fn set_playback(&self, status: PlaybackStatus, progress: Option<Duration>) {
        if status == PlaybackStatus::Stopped {
            self.clear();
            return;
        }
        let is_playing = status == PlaybackStatus::Playing;
        let position_ms = progress.map(|d| d.as_millis() as i64).unwrap_or(0);
        self.call("updatePlayback", move |env, context| {
            // RECONCILE: signature `(Landroid/content/Context;ZJ)V`.
            env.call_static_method(
                SERVICE_CLASS,
                "updatePlayback",
                "(Landroid/content/Context;ZJ)V",
                &[
                    JValue::Object(context),
                    JValue::Bool(is_playing.into()),
                    JValue::Long(position_ms),
                ],
            )
            .map_err(|e| CrateError::Audio(format!("updatePlayback: {e}")))?;
            Ok(())
        });
    }

    fn clear(&self) {
        self.call("clear", |env, context| {
            // RECONCILE: signature `(Landroid/content/Context;)V`.
            env.call_static_method(
                SERVICE_CLASS,
                "clear",
                "(Landroid/content/Context;)V",
                &[JValue::Object(context)],
            )
            .map_err(|e| CrateError::Audio(format!("clear: {e}")))?;
            Ok(())
        });
    }
}

/// JNI entry called by the Kotlin `CrateMediaService` for remote commands and focus changes.
/// Note the JNI name mangling — package `com.bbx_audio.crateapp` becomes
/// `com_bbx_1audio_crateapp` (the `_` in `bbx_audio` escapes to `_1`). `value` carries the seek
/// position in ms and is 0 for every other event.
///
/// # Safety
/// Called by the JVM with valid `env`/`event` references for the call's duration.
#[no_mangle]
pub extern "system" fn Java_com_bbx_1audio_crateapp_CrateMediaService_nativeMediaEvent(
    mut env: JNIEnv,
    _class: JClass,
    event: JString,
    value: jlong,
) {
    let Some(app) = EVENT_APP.get() else {
        return;
    };
    let event: String = match env.get_string(&event) {
        Ok(s) => s.into(),
        Err(e) => {
            log::warn!("media_controls: bad native event string: {e}");
            return;
        }
    };

    let result = match event.as_str() {
        "play" => app.emit("media-play", ()),
        // `stop` (session callback) has no distinct frontend meaning for previews — treat as pause.
        "pause" | "stop" => app.emit("media-pause", ()),
        "toggle" => app.emit("media-toggle", ()),
        "next" => app.emit("media-next", ()),
        "previous" => app.emit("media-previous", ()),
        "seek" => app.emit("media-seek", serde_json::json!({ "positionMs": value })),
        "focus_loss" => app.emit("media-focus-loss", ()),
        "focus_loss_transient" => app.emit("media-focus-loss-transient", ()),
        "focus_gain" => app.emit("media-focus-gain", ()),
        "duck_start" => app.emit("media-duck", serde_json::json!({ "active": true })),
        "duck_end" => app.emit("media-duck", serde_json::json!({ "active": false })),
        other => {
            log::warn!("media_controls: unknown native event '{other}'");
            Ok(())
        }
    };
    if let Err(e) = result {
        log::warn!("media_controls: failed to emit '{event}': {e}");
    }
}
