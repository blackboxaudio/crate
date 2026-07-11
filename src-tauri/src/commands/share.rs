//! Share-intent intake (#62): drain URLs shared into the app from other Android apps.
//!
//! Android delivers `ACTION_SEND` text shares to `MainActivity`, which queues the raw text in the
//! Kotlin `ShareIntentQueue` — a cold-start share arrives in `onCreate` long before the webview
//! (or even this library) is ready, so delivery is pull-based: the frontend drains the queue via
//! this command on boot, on the `crate-android-share` nudge event, and on foregrounding
//! (see apps/mobile/src/lib/androidShare.ts).
//!
//! On every other platform the command exists and returns an empty list, keeping the frontend
//! wiring unconditional-safe.

use crate::error::Result;

/// Take (and clear) all pending shared texts. Each entry is the raw `EXTRA_TEXT` of one share —
/// often a bare URL, but share sheets commonly wrap it in prose; URL extraction happens frontend-
/// side next to the rest of the discovery-link parsing (`shared/utils/discoveryLinks.ts`).
#[tauri::command]
pub fn take_shared_texts() -> Result<Vec<String>> {
    imp::take_shared_texts()
}

#[cfg(target_os = "android")]
mod imp {
    use jni::objects::JString;

    use crate::error::{CrateError, Result};

    /// Fully-qualified JNI class name of the Kotlin queue (see `gen/android/.../ShareIntentQueue.kt`).
    const QUEUE_CLASS: &str = "com/bbx_audio/crateapp/ShareIntentQueue";

    pub fn take_shared_texts() -> Result<Vec<String>> {
        crate::android_context::with_env_and_context(
            |env, _context| {
                // RECONCILE: signature `()Ljava/lang/String;` — a JSON array of the queued texts.
                let result = env
                    .call_static_method(QUEUE_CLASS, "takeAll", "()Ljava/lang/String;", &[])
                    .map_err(|e| {
                        CrateError::Discovery(format!("ShareIntentQueue.takeAll failed: {e}"))
                    })?;
                let json_obj = result
                    .l()
                    .map_err(|e| CrateError::Discovery(format!("share queue object: {e}")))?;
                let json: String = env
                    .get_string(&JString::from(json_obj))
                    .map_err(|e| CrateError::Discovery(format!("share queue decode: {e}")))?
                    .into();
                serde_json::from_str(&json)
                    .map_err(|e| CrateError::Discovery(format!("share queue parse: {e}")))
            },
            |m| CrateError::Discovery(format!("share queue: {m}")),
        )
    }
}

#[cfg(not(target_os = "android"))]
mod imp {
    use crate::error::Result;

    pub fn take_shared_texts() -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}
