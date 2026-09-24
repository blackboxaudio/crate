//! Dual-mode access to the ambient Android `(JavaVM, Context)` for JNI bridges.
//!
//! Rust → Kotlin calls need a JVM handle and an `android.content.Context`. Where they come from
//! depends on how the process was started:
//!
//! * **Foreground** (normal app launch) — the Tauri/wry runtime populates [`ndk_context`] at
//!   Activity creation, before `.setup()` runs. Every command-path JNI bridge can rely on it.
//! * **Headless** (WorkManager cold-starts the process to run `CrateSyncWorker` — no Activity, no
//!   Tauri) — `ndk_context` is never initialized, and probing it would abort. The Worker instead
//!   passes its own `Context` into the JNI entry point, which must [`adopt_headless`] it here
//!   before touching any bridge that needs a `Context` (e.g. the Keystore-backed DB key provider,
//!   #144).
//!
//! [`with_env_and_context`] hides the difference: it prefers the adopted headless pair and falls
//! back to `ndk_context`, so callers like `db/key_provider.rs` work identically on both paths.

use std::sync::OnceLock;

use jni::objects::{GlobalRef, JObject};
use jni::{JNIEnv, JavaVM};

use crate::error::{CrateError, Result};

/// `(JavaVM, Context)` adopted from a headless JNI entry point (WorkManager Worker). Set at most
/// once per process; a foreground-started process never sets it and uses `ndk_context` instead.
static HEADLESS: OnceLock<(JavaVM, GlobalRef)> = OnceLock::new();

/// Adopt the JVM + `Context` handed to a headless JNI entry point (e.g. `runBackgroundSync`).
/// Idempotent — a repeat call (the periodic Worker re-firing in a live process) is a no-op.
/// Failures are logged, not fatal: the foreground `ndk_context` fallback may still apply.
pub(crate) fn adopt_headless(env: &JNIEnv, context: &JObject) {
    if HEADLESS.get().is_some() {
        return;
    }
    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(e) => {
            log::warn!("android_context: failed to get JavaVM from headless env: {e}");
            return;
        }
    };
    let global = match env.new_global_ref(context) {
        Ok(g) => g,
        Err(e) => {
            log::warn!("android_context: failed to pin headless context: {e}");
            return;
        }
    };
    let _ = HEADLESS.set((vm, global));
}

/// Attach to the JVM and run `f` with a `JNIEnv` and an Android `Context`, resolving them from
/// the headless pair if adopted, else from `ndk_context` (foreground). `wrap_err` lets each
/// caller keep its own `CrateError` variant (KeyStorage, CloudSync, …).
///
/// If `f` fails while a Java exception is pending, the exception is described to logcat and
/// cleared — returning to the JVM with one still pending (the Worker path) would abort.
pub(crate) fn with_env_and_context<T>(
    f: impl FnOnce(&mut JNIEnv, &JObject) -> Result<T>,
    wrap_err: impl Fn(String) -> CrateError,
) -> Result<T> {
    let run = |env: &mut JNIEnv, context: &JObject| -> Result<T> {
        let result = f(env, context);
        if result.is_err() && env.exception_check().unwrap_or(false) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
        }
        result
    };

    if let Some((vm, context)) = HEADLESS.get() {
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| wrap_err(format!("JNI attach (headless): {e}")))?;
        return run(&mut env, context.as_obj());
    }

    // Foreground: the Tauri/wry runtime owns these process-lifetime handles.
    let ctx = ndk_context::android_context();
    // SAFETY: `vm()`/`context()` are valid process-lifetime handles owned by the runtime.
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| wrap_err(format!("JNI JavaVM: {e}")))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| wrap_err(format!("JNI attach: {e}")))?;
    let context = unsafe { JObject::from_raw(ctx.context().cast()) };
    run(&mut env, &context)
}
