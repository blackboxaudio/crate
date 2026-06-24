//! `MPNowPlayingInfoCenter` — the lock-screen / Control Center "Now Playing" surface.
//!
//! Text metadata (title/artist/album/duration) + the live elapsed-time and playback-rate that drive
//! the lock-screen scrubber. Artwork is downloaded asynchronously and patched in once ready.
//!
//! objc2 reconciliation surface: dictionary mutation uses `msg_send!` (the typed generic-dictionary
//! API is version-sensitive); the `MP*` property-key statics + `MPMediaItemArtwork::initWithImage`
//! are validated on device with `cargo check --target aarch64-apple-ios`.

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSMutableDictionary, NSNumber, NSString};
use objc2_media_player::{
    MPMediaItemPropertyAlbumTitle, MPMediaItemPropertyArtist, MPMediaItemPropertyArtwork,
    MPMediaItemPropertyPlaybackDuration, MPMediaItemPropertyTitle, MPNowPlayingInfoCenter,
    MPNowPlayingInfoPropertyElapsedPlaybackTime, MPNowPlayingInfoPropertyPlaybackRate,
};
use tauri::AppHandle;

use super::engine::NativeTrackEntry;

/// Set the full Now Playing dictionary for a freshly-started track (text + duration + elapsed/rate),
/// then kick off the async artwork download.
pub fn update(app: &AppHandle, entry: &NativeTrackEntry, elapsed_secs: f64, rate: f32) {
    // SAFETY: build an NSMutableDictionary<NSString, id> and populate it with the MP* keys. The key
    // constants are nullable `extern` statics (`Option<&NSString>`); skip any that are absent.
    unsafe {
        let dict: Retained<NSMutableDictionary<NSString, AnyObject>> = NSMutableDictionary::new();

        set_string(&dict, MPMediaItemPropertyTitle, &entry.title);
        set_string(&dict, MPMediaItemPropertyArtist, &entry.artist);
        set_string(&dict, MPMediaItemPropertyAlbumTitle, &entry.album);
        set_number(
            &dict,
            MPMediaItemPropertyPlaybackDuration,
            entry.duration_ms as f64 / 1000.0,
        );
        set_number(&dict, MPNowPlayingInfoPropertyElapsedPlaybackTime, elapsed_secs);
        set_number(&dict, MPNowPlayingInfoPropertyPlaybackRate, rate as f64);

        let center = MPNowPlayingInfoCenter::defaultCenter();
        center.setNowPlayingInfo(Some(&dict));
    }

    if let Some(url) = entry.artwork_url.clone() {
        download_artwork(app.clone(), url);
    }
}

/// Refresh only the elapsed time + playback rate on the existing Now Playing entry (keeps the
/// lock-screen scrubber in sync without re-sending text/artwork). The OS interpolates between updates
/// from the rate, so a ~1Hz cadence is enough.
pub fn set_playback(elapsed_secs: f64, rate: f32) {
    // SAFETY: copy the current dict (if any), patch the two keys, set it back.
    unsafe {
        let center = MPNowPlayingInfoCenter::defaultCenter();
        let dict: Retained<NSMutableDictionary<NSString, AnyObject>> = match center.nowPlayingInfo() {
            Some(existing) => msg_send![&*existing, mutableCopy],
            None => NSMutableDictionary::new(),
        };
        set_number(&dict, MPNowPlayingInfoPropertyElapsedPlaybackTime, elapsed_secs);
        set_number(&dict, MPNowPlayingInfoPropertyPlaybackRate, rate as f64);
        center.setNowPlayingInfo(Some(&dict));
    }
}

/// Empty the Now Playing surface (on stop).
pub fn clear() {
    // SAFETY: setting nil clears the lock-screen entry.
    unsafe {
        MPNowPlayingInfoCenter::defaultCenter().setNowPlayingInfo(None);
    }
}

/// # Safety
/// `key` must be a valid MP* property-key static; `dict` a live mutable dictionary.
unsafe fn set_string(dict: &NSMutableDictionary<NSString, AnyObject>, key: &NSString, value: &str) {
    let val = NSString::from_str(value);
    let _: () = msg_send![dict, setObject: &*val, forKey: key];
}

/// # Safety
/// `key` must be a valid MP* property-key static; `dict` a live mutable dictionary.
unsafe fn set_number(dict: &NSMutableDictionary<NSString, AnyObject>, key: &NSString, value: f64) {
    let val = NSNumber::numberWithDouble(value);
    let _: () = msg_send![dict, setObject: &*val, forKey: key];
}

/// Download artwork off-thread, then build a `UIImage` + `MPMediaItemArtwork` on the main thread and
/// patch it into the current Now Playing dict.
fn download_artwork(app: AppHandle, url: String) {
    tauri::async_runtime::spawn(async move {
        let bytes = match reqwest::get(&url).await {
            Ok(resp) => match resp.bytes().await {
                Ok(b) => b.to_vec(),
                Err(_) => return,
            },
            Err(_) => return,
        };
        let _ = app.run_on_main_thread(move || set_artwork(bytes));
    });
}

/// Build a `UIImage` from raw bytes and set it as the Now Playing artwork. Main thread only.
fn set_artwork(bytes: Vec<u8>) {
    use objc2::runtime::AnyClass;
    use objc2::AnyThread;
    use objc2_foundation::NSData;
    use objc2_media_player::MPMediaItemArtwork;

    // SAFETY: NSData → UIImage → MPMediaItemArtwork via the runtime (the typed UIImage / `initWithImage`
    // bindings aren't generated for this target), then patch the current Now Playing dict. Main thread
    // only (guaranteed by run_on_main_thread). `initWithImage:` is the simple (deprecated) initializer
    // — avoids the CGSize request-handler block.
    unsafe {
        let data = NSData::with_bytes(&bytes);
        let Some(ui_image_class) = AnyClass::get(c"UIImage") else {
            return;
        };
        let image: Option<Retained<AnyObject>> = msg_send![ui_image_class, imageWithData: &*data];
        let Some(image) = image else {
            return;
        };
        let artwork: Retained<MPMediaItemArtwork> =
            msg_send![MPMediaItemArtwork::alloc(), initWithImage: &*image];

        let center = MPNowPlayingInfoCenter::defaultCenter();
        let dict: Retained<NSMutableDictionary<NSString, AnyObject>> = match center.nowPlayingInfo() {
            Some(existing) => msg_send![&*existing, mutableCopy],
            None => NSMutableDictionary::new(),
        };
        let _: () = msg_send![&*dict, setObject: &*artwork, forKey: MPMediaItemPropertyArtwork];
        center.setNowPlayingInfo(Some(&dict));
    }
}
