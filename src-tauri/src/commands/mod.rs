// Command modules wrapping desktop-only services are gated behind the `desktop` feature.
#[cfg(feature = "desktop")]
pub mod analysis;
pub mod app;
pub mod backup;
pub mod cloud_sync;
#[cfg(feature = "desktop")]
pub mod device;
#[cfg(feature = "desktop")]
pub mod diagnostics;
pub mod discovery;
#[cfg(feature = "desktop")]
pub mod export;
pub mod follow;
#[cfg(feature = "desktop")]
pub mod library;
pub mod media_controls;
// iOS-only native preview playback engine commands (#54).
#[cfg(target_os = "ios")]
pub mod native_preview;
#[cfg(feature = "desktop")]
pub mod playback;
pub mod playlist;
pub mod settings;
#[cfg(feature = "desktop")]
pub mod sync;
pub mod tag;
