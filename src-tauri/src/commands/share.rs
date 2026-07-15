//! Share-intent intake (#62) + outbound share sheet: drain URLs shared INTO the app from other
//! Android apps, and open the OS share sheet to share URLs OUT (iOS/Android).
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

/// Open the OS share sheet for a URL: iOS presents `UIActivityViewController`, Android opens the
/// `ACTION_SEND` chooser, everywhere else is a safe no-op (desktop keeps its own Open/Copy URL
/// menu items and never surfaces the Share action). `title` becomes Android's `EXTRA_TITLE`; iOS
/// derives its own preview from the URL.
#[tauri::command]
pub fn share_url(url: String, title: Option<String>, app: tauri::AppHandle) -> Result<()> {
    share_imp::share_url(app, url, title)
}

#[cfg(target_os = "ios")]
mod share_imp {
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::msg_send;
    use objc2_foundation::{NSArray, NSString, NSURL};
    use tauri::AppHandle;

    use crate::error::Result;

    /// Present the share sheet from the root view controller. Fire-and-forget on the main
    /// thread (mirrors the native preview engine's ops): presentation problems log rather than
    /// error, since the command has already returned by the time the closure runs.
    pub fn share_url(app: AppHandle, url: String, _title: Option<String>) -> Result<()> {
        if let Err(err) = app.run_on_main_thread(move || present_share_sheet(&url)) {
            log::warn!("share_url: run_on_main_thread failed: {err}");
        }
        Ok(())
    }

    /// Main thread only. RECONCILE: UIKit has no generated objc2 bindings in this crate, so
    /// every UIKit selector below is runtime-looked-up (`AnyClass::get` + `msg_send!`, the
    /// pattern proven in `media_controls/ios/now_playing.rs`); validate with
    /// `cargo check --target aarch64-apple-ios` and an on-device present (incl. iPad popover).
    fn present_share_sheet(url: &str) {
        // SAFETY: UIApplication / UIActivityViewController FFI via the runtime; main thread
        // only (guaranteed by run_on_main_thread). All objects are nil-checked before use.
        unsafe {
            let Some(ns_url) = NSURL::URLWithString(&NSString::from_str(url)) else {
                log::warn!("share_url: invalid url");
                return;
            };
            // Share as a URL object (not text) so receivers render rich previews.
            let items: Retained<NSArray<NSURL>> = NSArray::from_retained_slice(&[ns_url]);

            let Some(app_class) = AnyClass::get(c"UIApplication") else {
                return;
            };
            let shared: Option<Retained<AnyObject>> = msg_send![app_class, sharedApplication];
            let Some(shared) = shared else { return };
            // keyWindow is deprecated but correct for this single-scene app; fall back to the
            // first window if nil (e.g. mid scene-transition).
            let mut window: Option<Retained<AnyObject>> = msg_send![&*shared, keyWindow];
            if window.is_none() {
                let windows: Option<Retained<AnyObject>> = msg_send![&*shared, windows];
                if let Some(windows) = windows {
                    window = msg_send![&*windows, firstObject];
                }
            }
            let Some(window) = window else {
                log::warn!("share_url: no window to present from");
                return;
            };
            let root_vc: Option<Retained<AnyObject>> = msg_send![&*window, rootViewController];
            let Some(root_vc) = root_vc else {
                log::warn!("share_url: no root view controller");
                return;
            };

            let Some(avc_class) = AnyClass::get(c"UIActivityViewController") else {
                return;
            };
            // `Allocated` (not a raw pointer) is what gives msg_send the init-family retain
            // semantics: init consumes the +1 alloc and returns +1, which `Retained` then owns.
            let avc_alloc: Allocated<AnyObject> = msg_send![avc_class, alloc];
            let no_activities: Option<&AnyObject> = None;
            let avc: Option<Retained<AnyObject>> = msg_send![
                avc_alloc,
                initWithActivityItems: &*items,
                applicationActivities: no_activities
            ];
            let Some(avc) = avc else {
                log::warn!("share_url: failed to init UIActivityViewController");
                return;
            };

            // iPad presents this as a popover, which CRASHES without an anchor — give it the
            // root view. sourceRect is left at its default (needs CGRect bindings we don't
            // have); with no permitted arrow directions UIKit centers the popover. On iPhone
            // popoverPresentationController is nil and this whole block is skipped.
            let popover: Option<Retained<AnyObject>> =
                msg_send![&*avc, popoverPresentationController];
            if let Some(popover) = popover {
                let root_view: Option<Retained<AnyObject>> = msg_send![&*root_vc, view];
                if let Some(root_view) = root_view {
                    let _: () = msg_send![&*popover, setSourceView: &*root_view];
                    let _: () = msg_send![&*popover, setPermittedArrowDirections: 0usize];
                }
            }

            let no_completion: Option<&block2::Block<dyn Fn()>> = None;
            let _: () = msg_send![
                &*root_vc,
                presentViewController: &*avc,
                animated: true,
                completion: no_completion
            ];
        }
    }
}

#[cfg(target_os = "android")]
mod share_imp {
    use jni::objects::{JObject, JValue};
    use tauri::AppHandle;

    use crate::error::{CrateError, Result};

    /// Fully-qualified JNI class name of the Kotlin helper (see `gen/android/.../CrateShare.kt`).
    const SHARE_CLASS: &str = "com/bbx_audio/crateapp/CrateShare";

    pub fn share_url(_app: AppHandle, url: String, title: Option<String>) -> Result<()> {
        crate::android_context::with_env_and_context(
            |env, context| {
                let jurl = JObject::from(
                    env.new_string(&url)
                        .map_err(|e| CrateError::Discovery(format!("share_url string: {e}")))?,
                );
                let jtitle = match title.as_deref() {
                    Some(t) => JObject::from(env.new_string(t).map_err(|e| {
                        CrateError::Discovery(format!("share_url title string: {e}"))
                    })?),
                    None => JObject::null(),
                };
                // RECONCILE: signature `(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)V`.
                env.call_static_method(
                    SHARE_CLASS,
                    "share",
                    "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)V",
                    &[
                        JValue::Object(context),
                        JValue::Object(&jurl),
                        JValue::Object(&jtitle),
                    ],
                )
                .map_err(|e| CrateError::Discovery(format!("CrateShare.share failed: {e}")))?;
                Ok(())
            },
            |m| CrateError::Discovery(format!("share_url: {m}")),
        )
    }
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
mod share_imp {
    use tauri::AppHandle;

    use crate::error::Result;

    pub fn share_url(_app: AppHandle, _url: String, _title: Option<String>) -> Result<()> {
        Ok(())
    }
}
