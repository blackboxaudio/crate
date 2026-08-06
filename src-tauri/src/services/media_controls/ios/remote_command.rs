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
pub fn configure(app: &AppHandle) -> Vec<Retained<AnyObject>> {
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

        // Lock-screen Like (MPFeedbackCommand). Where iOS surfaces it varies by version
        // (reliable in CarPlay; some lock screens hide feedback commands) — enabling it is
        // harmless when hidden. RECONCILE: `likeCommand` accessor + MPFeedbackCommand deref.
        let like = center.likeCommand();
        like.setEnabled(true);
        targets.push(add_handler(&like, || {
            engine::with_engine_mut(|e| e.like_pressed())
        }));

        // Suppress the default ±10s skip buttons so prev/next show on the lock screen.
        center.skipForwardCommand().setEnabled(false);
        center.skipBackwardCommand().setEnabled(false);

        // Lock-screen scrubber.
        let scrub = center.changePlaybackPositionCommand();
        scrub.setEnabled(true);
        targets.push(add_position_handler(&scrub));

        // Lock-screen repeat (MPChangeRepeatModeCommand). The frontend owns the app-level mode
        // (off/track/release/context): a press here just reports the chosen MPRepeatType to JS,
        // which sets the mode and echoes the resulting display state back via `set_repeat_display`
        // (through the `native_preview_set_repeat_mode` command). Where iOS surfaces the button
        // varies (Control Center's expanded player, CarPlay); enabling it is harmless when hidden.
        // RECONCILE: `changeRepeatModeCommand` accessor + MPChangeRepeatModeCommand deref.
        let repeat = center.changeRepeatModeCommand();
        repeat.setEnabled(true);
        targets.push(add_repeat_handler(&repeat, app.clone()));
    }

    targets
}

/// Reflect the current track's liked state on the lock-screen Like glyph. Main thread only.
/// RECONCILE: `setActive` (MPFeedbackCommand.active) against objc2-media-player on device; fall
/// back to `msg_send![&*like, setActive: liked]` if the typed accessor isn't generated.
pub(super) fn set_like_state(liked: bool) {
    // SAFETY: MPRemoteCommandCenter accessors; every caller runs on the main thread.
    unsafe {
        let center = MPRemoteCommandCenter::sharedCommandCenter();
        let like = center.likeCommand();
        like.setActive(liked);
    }
}

/// Register a no-argument command handler that always reports success.
///
/// # Safety
/// `command` must be a live MPRemoteCommand obtained on the main thread.
unsafe fn add_handler(
    command: &MPRemoteCommand,
    action: impl Fn() + 'static,
) -> Retained<AnyObject> {
    let block = RcBlock::new(
        move |_event: NonNull<MPRemoteCommandEvent>| -> MPRemoteCommandHandlerStatus {
            action();
            MPRemoteCommandHandlerStatus::Success
        },
    );
    command.addTargetWithHandler(&block)
}

/// Reflect the app's repeat mode on the lock-screen repeat button: 0 = Off, 1 = One, 2 = All
/// (MPRepeatType raw values). Main thread only.
/// RECONCILE: `setCurrentRepeatType` (MPChangeRepeatModeCommand.currentRepeatType) via msg_send —
/// switch to the typed accessor if objc2-media-player generates it.
pub(super) fn set_repeat_display(repeat_type: isize) {
    // SAFETY: MPRemoteCommandCenter accessors; every caller runs on the main thread.
    unsafe {
        let center = MPRemoteCommandCenter::sharedCommandCenter();
        let repeat = center.changeRepeatModeCommand();
        let _: () = msg_send![&*repeat, setCurrentRepeatType: repeat_type];
    }
}

/// Register the lock-screen repeat handler: reads the chosen `repeatType` off the event and reports
/// it to the frontend (see the wiring comment in [`configure`]).
///
/// # Safety
/// `command` must be the live changeRepeatModeCommand obtained on the main thread.
unsafe fn add_repeat_handler(command: &MPRemoteCommand, app: AppHandle) -> Retained<AnyObject> {
    let block = RcBlock::new(
        move |event: NonNull<MPRemoteCommandEvent>| -> MPRemoteCommandHandlerStatus {
            // The concrete event is an MPChangeRepeatModeCommandEvent; read its repeatType
            // (MPRepeatType: 0 = Off, 1 = One, 2 = All) via msg_send rather than downcasting.
            // RECONCILE: `repeatType` selector on the event.
            let repeat_type: isize = msg_send![event.as_ptr(), repeatType];
            let mode = match repeat_type {
                1 => "one",
                2 => "all",
                _ => "off",
            };
            engine::emit_repeat_changed(&app, mode);
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
