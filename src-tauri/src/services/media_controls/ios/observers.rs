//! `AVAudioSession` interruption + route-change observers, the item-finished observer, and the
//! app-lifecycle observers that detect a lock-screen return.
//!
//! - Interruption (phone call / Siri): pause on `.began`; on `.ended`, resume iff the system sets
//!   `ShouldResume` — the call-interruption auto-resume the WebView path can't do.
//! - Route change: pause when the old output disappears (e.g. headphones unplugged).
//! - `AVPlayerItemDidPlayToEndTime`: advance to the next track (native auto-advance, works locked).
//! - Protected-data-available / did-become-active / did-enter-background: detect when the app is
//!   foregrounded by a device *unlock* (lock-screen / Now Playing entry) vs a plain app-switch, and
//!   emit `native-preview-entered-from-lock` so the mobile UI can auto-open the full-screen player.
//!
//! Handlers are dispatched on the main queue so they can touch the player + the main-thread engine.
//!
//! objc2 reconciliation surface: the AVAudioSession notification-name / userInfo-key statics and the
//! `addObserverForName:object:queue:usingBlock:` signature are validated on device with
//! `cargo check --target aarch64-apple-ios`.

use core::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use block2::RcBlock;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVPlayerItemDidPlayToEndTimeNotification, AVPlayerItemFailedToPlayToEndTimeErrorKey,
    AVPlayerItemFailedToPlayToEndTimeNotification,
};
use objc2_avf_audio::{
    AVAudioSession, AVAudioSessionInterruptionNotification, AVAudioSessionInterruptionOptionKey,
    AVAudioSessionInterruptionTypeKey, AVAudioSessionRouteChangeNotification,
    AVAudioSessionRouteChangeReasonKey,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue, NSString};
use tauri::{AppHandle, Emitter};

use super::engine;

// AVAudioSession enum values (stable ABI constants).
const INTERRUPTION_TYPE_BEGAN: u64 = 1;
const INTERRUPTION_OPTION_SHOULD_RESUME: u64 = 1;
const ROUTE_CHANGE_REASON_OLD_DEVICE_UNAVAILABLE: u64 = 2;

/// Frontend event: the app came back to the foreground because the device was *unlocked* (the user
/// tapped the lock-screen / Now Playing controls to return to the track) rather than via a plain
/// app-switch. The mobile UI uses this to auto-open the full-screen player when one isn't already up.
const ENTERED_FROM_LOCK_EVENT: &str = "native-preview-entered-from-lock";

// UIApplication notification names. Their underlying NSString values equal these documented symbol
// names, so we build the names by value instead of pulling the whole `objc2-ui-kit` framework crate
// in for three string constants.
const UI_APP_DID_BECOME_ACTIVE: &str = "UIApplicationDidBecomeActiveNotification";
const UI_APP_DID_ENTER_BACKGROUND: &str = "UIApplicationDidEnterBackgroundNotification";
const UI_APP_PROTECTED_DATA_DID_BECOME_AVAILABLE: &str = "UIApplicationProtectedDataDidBecomeAvailable";

/// Register the engine's observers. Returns their tokens (retained for the engine's lifetime).
pub fn register(app: &AppHandle) -> Vec<Retained<AnyObject>> {
    let mut tokens = Vec::new();

    // SAFETY: NSNotificationCenter block-observer registration. Handlers run on the main queue so they
    // may touch the AVPlayer and the main-thread engine thread-local.
    unsafe {
        let center = NSNotificationCenter::defaultCenter();
        let main_queue = NSOperationQueue::mainQueue();

        // (av-foundation generates this notification name as a non-Option `&NSString`, unlike the
        // avf-audio statics below.)
        tokens.push(add_observer(
            &center,
            AVPlayerItemDidPlayToEndTimeNotification,
            &main_queue,
            |_note| engine::with_engine_mut(|e| e.on_item_ended()),
        ));
        // Playback started but couldn't finish (truncated stream / CDN drop). Complements the load
        // watchdog, which only covers the initial load window.
        tokens.push(add_observer(
            &center,
            AVPlayerItemFailedToPlayToEndTimeNotification,
            &main_queue,
            handle_failed_to_end,
        ));
        if let Some(name) = AVAudioSessionInterruptionNotification {
            tokens.push(add_observer(&center, name, &main_queue, handle_interruption));
        }
        if let Some(name) = AVAudioSessionRouteChangeNotification {
            tokens.push(add_observer(&center, name, &main_queue, handle_route_change));
        }

        // Lock-screen entry detection (drives the mobile UI's auto-open of the full-screen player).
        // The WebView can't distinguish returning from the lock screen from a plain app-switch — iOS
        // hands its JS the same "visible again" signal for both. Down here we can: a device *unlock*
        // immediately preceding the app becoming active means the user came in from the lock screen /
        // Now Playing, whereas an app-switch carries no unlock. So we track "unlocked since we last
        // backgrounded" and, on the next activation, fire ENTERED_FROM_LOCK_EVENT; the flag resets on
        // background so an ordinary foreground can't inherit a stale unlock. (Passcode-less devices
        // never make protected data unavailable, so they simply never auto-open — a no-false-positive
        // fallback.) Notification names are owned locals living through every registration below.
        let unlocked_since_background = Arc::new(AtomicBool::new(false));
        let unlock_name = NSString::from_str(UI_APP_PROTECTED_DATA_DID_BECOME_AVAILABLE);
        let background_name = NSString::from_str(UI_APP_DID_ENTER_BACKGROUND);
        let active_name = NSString::from_str(UI_APP_DID_BECOME_ACTIVE);

        let flag = unlocked_since_background.clone();
        tokens.push(add_observer(&center, &unlock_name, &main_queue, move |_note| {
            flag.store(true, Ordering::Relaxed)
        }));

        let flag = unlocked_since_background.clone();
        tokens.push(add_observer(&center, &background_name, &main_queue, move |_note| {
            flag.store(false, Ordering::Relaxed)
        }));

        let flag = unlocked_since_background;
        let app = app.clone();
        tokens.push(add_observer(&center, &active_name, &main_queue, move |_note| {
            if flag.swap(false, Ordering::Relaxed) {
                let _ = app.emit(ENTERED_FROM_LOCK_EVENT, ());
            }
        }));
    }

    tokens
}

/// # Safety
/// `center`/`name`/`queue` must be live objects obtained on the main thread.
unsafe fn add_observer(
    center: &NSNotificationCenter,
    name: &NSString,
    queue: &NSOperationQueue,
    handler: impl Fn(NonNull<NSNotification>) + 'static,
) -> Retained<AnyObject> {
    let block = RcBlock::new(move |note: NonNull<NSNotification>| handler(note));
    // Returns Retained<ProtocolObject<dyn NSObjectProtocol>>; erase to AnyObject for uniform storage.
    center
        .addObserverForName_object_queue_usingBlock(Some(name), None, Some(queue), &block)
        .into()
}

fn handle_interruption(note: NonNull<NSNotification>) {
    // SAFETY: read the interruption type/option out of the notification's userInfo via msg_send.
    unsafe {
        let Some(type_key) = AVAudioSessionInterruptionTypeKey else {
            return;
        };
        let info: *mut AnyObject = msg_send![note.as_ptr(), userInfo];
        if info.is_null() {
            return;
        }
        let type_val: *mut AnyObject = msg_send![info, objectForKey: type_key];
        if type_val.is_null() {
            return;
        }
        let interruption_type: u64 = msg_send![type_val, unsignedIntegerValue];

        if interruption_type == INTERRUPTION_TYPE_BEGAN {
            engine::with_engine_mut(|e| e.pause());
            return;
        }

        // Ended — resume only if the system says we should.
        let mut should_resume = false;
        if let Some(opt_key) = AVAudioSessionInterruptionOptionKey {
            let opt_val: *mut AnyObject = msg_send![info, objectForKey: opt_key];
            if !opt_val.is_null() {
                let opts: u64 = msg_send![opt_val, unsignedIntegerValue];
                should_resume = opts & INTERRUPTION_OPTION_SHOULD_RESUME != 0;
            }
        }
        if should_resume {
            reactivate_session();
            engine::with_engine_mut(|e| e.resume());
        }
    }
}

fn handle_route_change(note: NonNull<NSNotification>) {
    // SAFETY: read the route-change reason out of the notification's userInfo via msg_send.
    unsafe {
        let Some(reason_key) = AVAudioSessionRouteChangeReasonKey else {
            return;
        };
        let info: *mut AnyObject = msg_send![note.as_ptr(), userInfo];
        if info.is_null() {
            return;
        }
        let reason_val: *mut AnyObject = msg_send![info, objectForKey: reason_key];
        if reason_val.is_null() {
            return;
        }
        let reason: u64 = msg_send![reason_val, unsignedIntegerValue];
        if reason == ROUTE_CHANGE_REASON_OLD_DEVICE_UNAVAILABLE {
            engine::with_engine_mut(|e| e.pause());
        }
    }
}

/// Surface an `AVPlayerItemFailedToPlayToEndTime` failure: read the NSError out of the notification's
/// userInfo and route it through the engine (logs to the terminal + toasts the frontend).
fn handle_failed_to_end(note: NonNull<NSNotification>) {
    // SAFETY: read userInfo[AVPlayerItemFailedToPlayToEndTimeErrorKey] and its localizedDescription.
    let message = unsafe {
        let info: *mut AnyObject = msg_send![note.as_ptr(), userInfo];
        let mut message = "AVPlayer failed to play the stream to the end".to_string();
        if !info.is_null() {
            let err: *mut AnyObject =
                msg_send![info, objectForKey: AVPlayerItemFailedToPlayToEndTimeErrorKey];
            if !err.is_null() {
                let desc: Retained<NSString> = msg_send![err, localizedDescription];
                message = desc.to_string();
            }
        }
        message
    };
    engine::with_engine_mut(|e| e.fail(message));
}

/// Re-activate the audio session after an interruption ends (required before resuming).
fn reactivate_session() {
    // SAFETY: standard AVAudioSession activation; safe to call off the main thread, but we're on main.
    unsafe {
        let session = AVAudioSession::sharedInstance();
        if let Err(err) = session.setActive_error(true) {
            log::warn!("native preview: re-activate audio session failed: {err:?}");
        }
    }
}
