//! iOS opportunistic background sync via `BGTaskScheduler` (BackgroundTasks.framework).
//!
//! [`register`] installs a `BGAppRefreshTask` launch handler once at startup (from `lib.rs`'s
//! `.setup()`, which runs synchronously inside `didFinishLaunchingWithOptions` — the window
//! Apple requires for registration). When iOS later launches/foregrounds the app for the task,
//! the app boots normally (its managed [`CloudSyncState`] is constructed), the OS invokes our
//! stored launch-handler block, and [`handle_task`] runs one sync pass and reports completion.
//!
//! The task is one-shot, so we resubmit the next request at every launch and at the top of each
//! handler. The OS decides real cadence (it clamps to ~15 min minimum and schedules
//! opportunistically by usage/battery); [`submit_refresh_request`] only expresses the floor.
//!
//! objc2 reconciliation surface (validate on device with `cargo check --target aarch64-apple-ios`):
//! the `BGTaskScheduler` / `BGAppRefreshTaskRequest` / `BGTask` selectors used below via
//! `msg_send!`, and that `registerForTaskWithIdentifier:usingQueue:launchHandler:` copies the
//! launch-handler block (so the local `RcBlock` may drop after registration, as with GCD blocks).

use core::ptr::NonNull;
use std::sync::Arc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::NSString;
use tauri::{AppHandle, Manager};

use crate::services::cloud_sync::runtime::CloudSyncState;

/// Reverse-DNS task identifier. MUST match `BGTaskSchedulerPermittedIdentifiers` in Info.plist
/// (stamped by `scripts/write-ios-plist.mjs`) or registration/submission throw at runtime.
const TASK_IDENTIFIER: &str = "audio.bbx.crate.sync.refresh";

/// Requested floor before the OS may run the refresh again (seconds). iOS clamps sub-15-min
/// requests up to ~15 min and treats this as opportunistic, not a guarantee.
const MIN_INTERVAL_SECS: f64 = 15.0 * 60.0;

/// Register the BGAppRefresh launch handler (once, at startup) and submit the first request.
/// The captured `AppHandle` lets the handler resolve the managed [`CloudSyncState`] when the OS
/// fires the task — possibly minutes/hours later, in a fresh (background) launch.
pub fn register(app: AppHandle) {
    // SAFETY: BGTaskScheduler registration on the main thread during app launch. The launch
    // handler is copied by the OS and retained for the process lifetime.
    unsafe {
        let identifier = NSString::from_str(TASK_IDENTIFIER);
        let scheduler: *mut AnyObject = msg_send![class!(BGTaskScheduler), sharedScheduler];
        if scheduler.is_null() {
            log::warn!("BGTaskScheduler unavailable; skipping background sync registration");
            return;
        }

        // void (^launchHandler)(BGTask *task) — run the sync, then mark the task complete.
        let block = RcBlock::new(move |task: NonNull<AnyObject>| {
            handle_task(&app, task.as_ptr());
        });

        let queue: *mut AnyObject = core::ptr::null_mut();
        let registered: bool = msg_send![
            scheduler,
            registerForTaskWithIdentifier: &*identifier,
            usingQueue: queue,
            launchHandler: &*block
        ];
        if !registered {
            log::warn!("BGTaskScheduler registration failed for {TASK_IDENTIFIER}");
            return;
        }
    }

    // Queue the first refresh from launch.
    submit_refresh_request();
}

/// Submit a `BGAppRefreshTaskRequest` for the next opportunistic run. Idempotent from iOS's
/// perspective (a pending request for the same identifier is replaced). Called at launch, after
/// sign-in (via `schedule()`), and at the top of every handler (the task is one-shot).
pub fn submit_refresh_request() {
    // SAFETY: construct + submit a BGAppRefreshTaskRequest via its documented selectors. The
    // alloc/init request is +1 owned; `Retained` releases it when this scope ends (submit copies
    // what it needs), so repeated rescheduling doesn't leak request objects.
    unsafe {
        let identifier = NSString::from_str(TASK_IDENTIFIER);
        let allocated: *mut AnyObject = msg_send![class!(BGAppRefreshTaskRequest), alloc];
        let initialized: *mut AnyObject = msg_send![allocated, initWithIdentifier: &*identifier];
        let Some(request) = Retained::<AnyObject>::from_raw(initialized) else {
            return;
        };

        let date: *mut AnyObject =
            msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: MIN_INTERVAL_SECS];
        let _: () = msg_send![&*request, setEarliestBeginDate: date];

        let scheduler: *mut AnyObject = msg_send![class!(BGTaskScheduler), sharedScheduler];
        if scheduler.is_null() {
            return;
        }
        let mut error: *mut AnyObject = core::ptr::null_mut();
        let submitted: bool = msg_send![scheduler, submitTaskRequest: &*request, error: &mut error];
        if !submitted {
            log::warn!("BGTaskScheduler: failed to submit background sync request");
        }
    }
}

/// Run one opportunistic sync pass for a fired `BGAppRefreshTask`, then tell iOS whether it
/// succeeded. The launch handler runs on a background GCD queue (not the main thread and not a
/// tokio worker), so we drive the async pass to completion with `block_on` and report result
/// synchronously — this keeps the non-`Send` `BGTask` pointer on one thread instead of moving it
/// into a spawned future. Reschedules the next refresh first (the task is one-shot).
fn handle_task(app: &AppHandle, task: *mut AnyObject) {
    submit_refresh_request();

    let success = match app.try_state::<Arc<CloudSyncState>>() {
        Some(state) => {
            let state = state.inner().clone();
            tauri::async_runtime::block_on(async move {
                match state.run_foreground_pass().await {
                    Ok(()) => true,
                    Err(e) => {
                        log::warn!("background sync pass failed: {e}");
                        false
                    }
                }
            })
        }
        None => {
            log::warn!("background sync: CloudSyncState unavailable");
            false
        }
    };

    // SAFETY: report completion on the BGTask the OS handed us. Must be called within the task's
    // time budget or iOS marks it expired.
    unsafe {
        let _: () = msg_send![task, setTaskCompletedWithSuccess: success];
    }
}
