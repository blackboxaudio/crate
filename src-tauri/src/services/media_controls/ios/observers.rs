//! `AVAudioSession` interruption + route-change observers, and the item-finished observer.
//!
//! - Interruption (phone call / Siri): pause on `.began`; on `.ended`, resume iff the system sets
//!   `ShouldResume` — the call-interruption auto-resume the WebView path can't do.
//! - Route change: pause when the old output disappears (e.g. headphones unplugged).
//! - `AVPlayerItemDidPlayToEndTime`: advance to the next track (native auto-advance, works locked).
//!
//! Handlers are dispatched on the main queue so they can touch the player + the main-thread engine.
//!
//! objc2 reconciliation surface: the AVAudioSession notification-name / userInfo-key statics and the
//! `addObserverForName:object:queue:usingBlock:` signature are validated on device with
//! `cargo check --target aarch64-apple-ios`.

use core::ptr::NonNull;

use block2::RcBlock;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_av_foundation::AVPlayerItemDidPlayToEndTimeNotification;
use objc2_avf_audio::{
    AVAudioSession, AVAudioSessionInterruptionNotification, AVAudioSessionInterruptionOptionKey,
    AVAudioSessionInterruptionTypeKey, AVAudioSessionRouteChangeNotification,
    AVAudioSessionRouteChangeReasonKey,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue, NSString};
use tauri::AppHandle;

use super::engine;

// AVAudioSession enum values (stable ABI constants).
const INTERRUPTION_TYPE_BEGAN: u64 = 1;
const INTERRUPTION_OPTION_SHOULD_RESUME: u64 = 1;
const ROUTE_CHANGE_REASON_OLD_DEVICE_UNAVAILABLE: u64 = 2;

/// Register the three observers. Returns their tokens (retained for the engine's lifetime).
pub fn register(_app: &AppHandle) -> Vec<Retained<AnyObject>> {
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
        if let Some(name) = AVAudioSessionInterruptionNotification {
            tokens.push(add_observer(&center, name, &main_queue, handle_interruption));
        }
        if let Some(name) = AVAudioSessionRouteChangeNotification {
            tokens.push(add_observer(&center, name, &main_queue, handle_route_change));
        }
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
