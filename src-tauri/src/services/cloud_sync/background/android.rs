//! Android opportunistic background sync via WorkManager (JNI ↔ Kotlin).
//!
//! WorkManager runs [`CrateSyncWorker`] (a `CoroutineWorker`) which calls the [`runBackgroundSync`]
//! JNI entry below. Because the Worker may cold-start the process WITHOUT the Tauri Activity, there
//! is no managed `CloudSyncState`, so the entry rebuilds `{conn, config, backend, session}` from
//! scratch and calls the bare [`super::run_background_sync`] core.
//!
//! ## ⚠️ Blocked on #144 (Android Keystore DB-key provider)
//! [`crate::db::Database::new`] opens the SQLCipher DB using `db::key_provider::for_platform`, which
//! on Android is `AndroidKeystoreKeyProvider` — currently unimplemented (returns an error, #144). So
//! [`run_headless`] fails at DB-open until #144 lands and the Worker just retries. Scheduling +
//! Kotlin wiring are complete so this activates the moment #144 does.
//!
//! ## Deferred: headless App Check context
//! In a headless Worker, `ndk_context::android_context()` is unset, so App Check (Play Integrity)
//! minting during pull/push has no `Context`. The Worker passes its `Context` into [`runBackgroundSync`]
//! for this reason; threading it into a headless App Check mint path is follow-up work bundled with #144.
//! ([`ensure_scheduled`]/[`cancel_scheduled`] run on the foreground command path where `ndk_context`
//! IS populated, so scheduling works today.)

use std::path::PathBuf;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jboolean;
use jni::JNIEnv;

use crate::error::{CrateError, Result};

/// Fully-qualified Kotlin scheduler helper (WorkManager enqueue/cancel). Mirrors the
/// `AppCheckPlayIntegrity` JNI bridge convention.
const SCHEDULER_CLASS: &str = "com/bbx_audio/crateapp/CrateSyncScheduler";

/// Enqueue the periodic WorkManager sync (unique work; KEEP policy so relaunch doesn't reset it).
/// Runs on the foreground command path, where `ndk_context` is populated.
pub fn ensure_scheduled() {
    if let Err(e) = call_scheduler("schedule") {
        log::warn!("cloud_sync: failed to schedule background sync: {e}");
    }
}

/// Cancel the periodic WorkManager sync (sign-out).
pub fn cancel_scheduled() {
    if let Err(e) = call_scheduler("cancel") {
        log::warn!("cloud_sync: failed to cancel background sync: {e}");
    }
}

/// Invoke a static `void <method>(Context)` on the Kotlin `CrateSyncScheduler` via JNI, using the
/// runtime's ambient Android context (Rust → Kotlin, the inverse of the App Check bridge).
fn call_scheduler(method: &str) -> Result<()> {
    let ctx = ndk_context::android_context();
    // SAFETY: `vm()`/`context()` are process-lifetime handles owned by the Tauri/wry runtime.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| CrateError::CloudSync(format!("bg sched JNI vm: {e}")))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| CrateError::CloudSync(format!("bg sched JNI attach: {e}")))?;
    let context = unsafe { JObject::from_raw(ctx.context().cast()) };

    env.call_static_method(
        SCHEDULER_CLASS,
        method,
        "(Landroid/content/Context;)V",
        &[JValue::Object(&context)],
    )
    .map_err(|e| CrateError::CloudSync(format!("bg sched {method} failed: {e}")))?;
    Ok(())
}

/// JNI entry called by the Kotlin `CrateSyncWorker`. Blocking; the Worker runs it on a background
/// thread and treats `false` / an exception as retry. Note the JNI name mangling — package
/// `com.bbx_audio.crateapp` becomes `com_bbx_1audio_crateapp` (the `_` in `bbx_audio` escapes to
/// `_1`). Signature: `(Landroid/content/Context;Ljava/lang/String;)Z`.
///
/// # Safety
/// Called by the JVM with valid `env`/`context`/`files_dir` references for the call's duration.
#[no_mangle]
pub extern "system" fn Java_com_bbx_1audio_crateapp_CrateSyncWorker_runBackgroundSync(
    mut env: JNIEnv,
    _class: JClass,
    _context: JObject,
    files_dir: JString,
) -> jboolean {
    let path: String = match env.get_string(&files_dir) {
        Ok(s) => s.into(),
        Err(e) => {
            log::warn!("cloud_sync: background sync bad files_dir: {e}");
            return 0;
        }
    };

    match run_headless(PathBuf::from(path)) {
        Ok(()) => 1,
        Err(e) => {
            log::warn!("cloud_sync: android background sync failed: {e}");
            0
        }
    }
}

/// Rebuild the sync dependencies from scratch (no Tauri app / `CloudSyncState`) and run one pass.
/// `files_dir` must be the SAME directory the app's `.setup()` uses for `crate.db`, or a second
/// empty DB opens and the pass silently no-ops.
fn run_headless(files_dir: PathBuf) -> Result<()> {
    let db = crate::db::Database::new(files_dir.join("crate.db"))?; // blocked on #144 (Keystore key)
    let conn = db.connection();

    let config = crate::services::cloud_sync::config::load_cloud_config(None)?
        .ok_or_else(|| CrateError::CloudSync("cloud sync not configured".into()))?;
    let backend = crate::services::cloud_sync::backend::build_default_backend(&config)?;

    // Same device id derivation as `lib.rs` (`hlc::load_node_id` → 8-hex), so manifests agree.
    let device_id = {
        let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        crate::services::cloud_sync::hlc::load_node_id(&guard)
            .map(|n| format!("{n:08x}"))
            .unwrap_or_else(|_| "00000000".to_string())
    };

    // No ambient tokio runtime on a WorkManager thread — build a current-thread one.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CrateError::CloudSync(format!("bg runtime: {e}")))?;

    rt.block_on(async {
        let Some(session) =
            crate::services::cloud_sync::auth::current_session(&backend, conn.clone()).await?
        else {
            return Ok(()); // signed out — nothing to do
        };
        super::run_background_sync(conn.clone(), &backend, &session, &device_id).await
    })
}
