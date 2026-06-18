//! `MPRemoteCommandCenter` — the lock-screen / Control Center transport buttons.
//!
//! Enables play/pause/toggle, next/previous, and the scrubber (changePlaybackPosition); disables the
//! ±10s skip buttons so prev/next render instead. Handlers run on the main thread and drive the engine
//! via the thread-local — so they keep working while the WebView's JS is suspended on lock.
//!
//! objc2 reconciliation surface: command accessors + `addTargetWithHandler` + the handler-status enum
//! are validated on device with `cargo check --target aarch64-apple-ios`.

use core::ptr::NonNull;

use block2::RcBlock;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_media_player::{
    MPRemoteCommand, MPRemoteCommandCenter, MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
};
use tauri::AppHandle;

use super::engine;

/// Wire all lock-screen commands. Returns the target tokens (retained for the engine's lifetime so the
/// handler blocks stay alive).
pub fn configure(_app: &AppHandle) -> Vec<Retained<AnyObject>> {
    let mut targets = Vec::new();

    // SAFETY: standard MPRemoteCommandCenter wiring; constructed on the main thread.
    unsafe {
        let center = MPRemoteCommandCenter::sharedCommandCenter();

        let play = center.playCommand();
        play.setEnabled(true);
        targets.push(add_handler(&play, || {
            engine::with_engine_mut(|e| e.resume())
        }));

        let pause = center.pauseCommand();
        pause.setEnabled(true);
        targets.push(add_handler(&pause, || {
            engine::with_engine_mut(|e| e.pause())
        }));

        let toggle = center.togglePlayPauseCommand();
        toggle.setEnabled(true);
        targets.push(add_handler(&toggle, || {
            engine::with_engine_mut(|e| e.toggle())
        }));

        let next = center.nextTrackCommand();
        next.setEnabled(true);
        targets.push(add_handler(&next, || {
            engine::with_engine_mut(|e| e.advance(1))
        }));

        let prev = center.previousTrackCommand();
        prev.setEnabled(true);
        targets.push(add_handler(&prev, || {
            engine::with_engine_mut(|e| e.previous())
        }));

        // Suppress the default ±10s skip buttons so prev/next show on the lock screen.
        center.skipForwardCommand().setEnabled(false);
        center.skipBackwardCommand().setEnabled(false);

        // Lock-screen scrubber.
        let scrub = center.changePlaybackPositionCommand();
        scrub.setEnabled(true);
        targets.push(add_position_handler(&scrub));
    }

    targets
}

/// Register a no-argument command handler that always reports success.
///
/// # Safety
/// `command` must be a live MPRemoteCommand obtained on the main thread.
unsafe fn add_handler(command: &MPRemoteCommand, action: impl Fn() + 'static) -> Retained<AnyObject> {
    let block = RcBlock::new(
        move |_event: NonNull<MPRemoteCommandEvent>| -> MPRemoteCommandHandlerStatus {
            action();
            MPRemoteCommandHandlerStatus::Success
        },
    );
    command.addTargetWithHandler(&block)
}

/// Register the scrubber handler: reads `positionTime` (seconds) off the event and seeks.
///
/// # Safety
/// `command` must be the live changePlaybackPositionCommand obtained on the main thread.
unsafe fn add_position_handler(command: &MPRemoteCommand) -> Retained<AnyObject> {
    let block = RcBlock::new(
        move |event: NonNull<MPRemoteCommandEvent>| -> MPRemoteCommandHandlerStatus {
            // The concrete event is an MPChangePlaybackPositionCommandEvent; read its positionTime
            // (NSTimeInterval, seconds) via msg_send rather than downcasting the typed class.
            let position: f64 = msg_send![event.as_ptr(), positionTime];
            let ms = (position.max(0.0) * 1000.0) as u64;
            engine::with_engine_mut(|e| e.seek(ms));
            MPRemoteCommandHandlerStatus::Success
        },
    );
    command.addTargetWithHandler(&block)
}
