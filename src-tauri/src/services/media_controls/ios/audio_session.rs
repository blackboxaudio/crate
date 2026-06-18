//! `AVAudioSession` activation for background audio (#79).

/// Activate the process-wide `AVAudioSession` in the `.playback` category so audio survives
/// backgrounding / lock. Failures are non-fatal: foreground playback works regardless (per the #80
/// spike), so we log and continue rather than aborting app startup. Called once at startup (from
/// [`super::IosMediaSession::new`]); the interruption observer re-activates it after a phone call.
pub fn configure_playback_audio_session() {
    use objc2_avf_audio::{AVAudioSession, AVAudioSessionCategoryPlayback};

    // SAFETY: standard AVFAudio FFI. `sharedInstance` returns the singleton audio session;
    // `AVAudioSessionCategoryPlayback` is the framework's `extern` NSString category constant.
    // Setting the category / active flag is safe to call off the main thread.
    unsafe {
        let session = AVAudioSession::sharedInstance();
        // The category constant is an `Option<&NSString>` (a nullable `extern` static). It is always
        // present at runtime; handle `None` gracefully rather than panicking.
        let Some(category) = AVAudioSessionCategoryPlayback else {
            log::warn!("AVAudioSessionCategoryPlayback unavailable; skipping audio session setup");
            return;
        };
        if let Err(err) = session.setCategory_error(category) {
            log::warn!("AVAudioSession.setCategory(.playback) failed: {err:?}");
            return;
        }
        if let Err(err) = session.setActive_error(true) {
            log::warn!("AVAudioSession.setActive(true) failed: {err:?}");
        }
    }
}
