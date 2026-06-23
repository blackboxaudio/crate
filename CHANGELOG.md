# Changelog

All notable changes to Crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Added native cloud sync sign-in on mobile (iOS/Android) using the platform's secure web-auth session (ASWebAuthenticationSession / Custom Tabs) instead of the desktop loopback flow; mobile syncs discovery data only, never the local library
- Added secure SQLCipher database key storage on iOS using the Keychain (device-only, after-first-unlock accessibility) via a new platform `KeyProvider` abstraction; desktop keeps its local key-file behavior and the unencrypted-to-encrypted database migration is now desktop-only
- Added the mobile app's navigation shell: a Discovery main view with a left drawer (playlists and tags) opened by the hamburger button or a left-edge swipe, a right drawer (appearance and cloud sign-in) opened by the settings button, and touch-optimized base components
- Added a branded launch splash screen and a spinning-record loading indicator to the mobile app
- Added the mobile app's branded launcher icons (iOS app icon and Android adaptive icon) with per-channel variants matching desktop (development, staging, production)
- Added the mobile release detail screen (artwork, metadata, one-tap open in the source app — Bandcamp, SoundCloud, YouTube, Discogs — the track list, an assignable tag picker, and inline-editable notes) with an iOS-style left-edge swipe-back, plus an integrated preview player — a liquid-glass mini-player bar with progress whose artwork morphs up into a full-screen player (blurred album-art backdrop, drag-to-dismiss, scrubber, shuffle, previous/next, like, and a ±10% tempo control) whose shuffle and previous/next span the whole discovery feed on screen rather than just the current release — and, on iOS, a native lock-screen transport (play/pause, previous/next, and scrubbing that keep working while the screen is locked, with auto-resume after call interruptions) powered by a native AVPlayer engine
- Added the mobile discovery feed: a virtualized, searchable list of release cards (artwork, artist, title, label) with tag-filter chips (AND/OR), sorting (date added, artist, title, label), swipe-to-delete, and long-press multi-select for batch delete and batch tag assignment
- Added an account & cloud-sync control to the mobile top bar: a live status chip (synced, syncing, offline, or error) showing your account avatar that opens a sheet to sync now, see when the library last synced, or sign out — and the bar now shows the active section's title (Playlists, Tags, Settings) in place of the wordmark
- Added a first-class playback queue to the mobile preview player: an "Up Next" sheet (opened from the full-screen player) listing songs you've queued ahead of what the discovery feed will play next, with "Play next" and "Add to queue" from each track's menu in the release detail and a swipe action on feed cards, drag-to-reorder and swipe-to-remove for queued songs, and persistence across relaunch; queued songs always play before the shuffle/sequence resumes and are never reshuffled, and on iOS a lazily-resolved native-engine window keeps queued songs and cross-release advances gapless while the screen is locked
- Added restoring your listening state on the mobile app's launch: the preview you were last playing reappears in the mini-player — with its track progress, shuffle, and tempo — paused and ready to resume, without auto-opening the full-screen player
- Added a mobile settings page with cloud-sync account management, audio preview cache controls, and app info, plus live auto-sync so discovery changes from another device appear instantly without a tab-switch
- Added discovery playlists on mobile: create, rename, and delete playlists and folders with Spotify-style 2×2 cover-art thumbnails and drill-in folder navigation on the Playlists tab, tap a playlist to view its releases in a full-screen detail overlay (the same fast, virtualized list as the discovery feed), add releases from the feed or release detail via a long-press context menu or multi-select batch action, remove releases with swipe or context menu, and long-press-drag to reorder releases in a dedicated reorder mode — all backed by a new `reorder_playlist_releases` backend command and bidirectional cloud sync
- Added add, edit, and bulk-import for discovery releases on mobile: paste a release URL to auto-fetch metadata with an editable preview, paste a page URL (Bandcamp artist/label, Discogs artist/label) to scan and bulk-import, edit release metadata via a dedicated sheet, clipboard prefill on open, and an offline add queue that persists pending URLs and drains them automatically when connectivity returns

### Changed

- Changed the mobile long-press menus (discovery releases and playlists/folders) from bottom action sheets to native-style iOS context menus: the pressed item lifts above a blurred, dimmed backdrop while a spring-in action menu anchors to it (placed below or above as space allows), with a haptic on open and tap-outside / swipe-down to dismiss
- Prepared the backend to compile for mobile targets (iOS/Android) by gating desktop-only services (audio playback, USB export/sync, file import, track analysis, media keys, device detection) behind a default-on `desktop` Cargo feature, keeping desktop builds unchanged
- Set up the iOS application project (Tauri mobile) so the mobile app builds and runs on iOS, with the background-audio mode and cloud sign-in OAuth redirect scheme configured

## [0.2.9] - 2026-06-09

### Added

- Added shuffle mode to the audio player
- Added opt-in cross-device cloud sync for libraries, playlists, tags, cues, and discovery releases (audio files stay local)
- Added macOS keyboard shortcuts for hide/hide others/show all
- Added a right-click context menu for discovery tracks (like/unlike, play preview, search on YouTube, open/copy release URL) plus a "Search on YouTube" action on the release menu
- Added the ability to follow artists and labels (Bandcamp, SoundCloud, Discogs) to automatically surface their new releases in Discovery, with upcoming-release badges, release-day notifications, and a Following manager

### Fixed

- Fixed backup progress bar not visible due to invalid Tailwind color classes
- Fixed locate track functionality to check current playlist first
- Fixed continuous playback selecting next track from wrong context when navigating between views
- Fixed discovery row buttons (import and open URL) not working in playlist view

## [0.2.8] - 2026-03-15

### Added

- Added guided feature tour for first-time users

### Fixed

- Fixed metadata auto-fetching for unsupported URL domains in discovery
- Fixed editor form resetting during bulk metadata refresh for discovery releases
- Fixed particular strings not being translated on locale change

## [0.2.7] - 2026-03-14

### Added

- Added clickable track name in the player bar to scroll to and highlight the currently playing track
- Added unified filter panel for library and discovery views with per-context filter state
- Added click-to-enlarge artwork modal for discovery releases
- Added dynamic sidebar header that updates to match the active context (Library / Discovery)
- Added bulk drag-and-drop and "Move to Folder" context menu for multi-selected playlists
- Added persistence of navigation state, playlist tree scroll position, and discovery release expansion across restarts
- Added information display when restoring from a backup

### Fixed

- Fixed support for bulk-adding Bandcamp pages that use alternative indexing
- Fixed discovery playlist search not filtering by track name
- Fixed multi-select drag clearing selection when clicking to initiate a drag
- Fixed renaming smart playlist names in the modal to edit smart rules
- Fixed metadata refreshing in discovery playlists views

## [0.2.6] - 2026-03-14

### Added

- Added Ukrainian, Romanian, Polish, and Turkish locale support
- Added first-run onboarding setup wizard with language, theme, accent color, and font customization
- Added persistence of player state, including current track, playhead position, tempo control, and volume control 
- Added Apple code signing and notarization for macOS builds

### Changed

- Improved rendering of lists for library, discovery, and playlist views

### Fixed

- Fixed macOS Tahoe (26) compatibility issues
- Fixed database foreign key violations during restore across app installations
- Fixed discovery row buttons not working intermittently

## [0.2.5] - 2026-03-09

### Changed

- Improved metadata enrichment for discovery releases during bulk imports

### Fixed

- Fixed discovery selection bugs when navigating in-context
- Fixed Bandcamp discography parsing to include all releases

## [0.2.4] - 2026-03-09

### Fixed

- Fixed the "is liked" toggling of discovery tracks

## [0.2.3] - 2026-03-09

### Changed

- Improved search logic for discovery releases

### Fixed

- Fixed bug where bulk operations on filtered selections was misleading

## [0.2.2] - 2026-03-09

### Added

- Track-level likes for discovery releases with heart toggle and filter to show only releases with liked tracks

## [0.2.1] - 2026-03-08

### Added

- Smart playlists with rule-based auto-population for both library and discovery contexts
- Library backup and restore functionality in Settings > General

### Changed

- Replaced OS keyring with local key file for database encryption to avoid first-launch Keychain prompt

## [0.2.0] - 2026-03-08

### Added

- Seamless in-app updates via Tauri updater plugin; checks on launch and hourly, shows update modal with release notes and download progress
- Continuous playback setting for automatically playing the next track
- Music discovery feature for tracking releases from Bandcamp, SoundCloud, YouTube, and Discogs
- Discovery settings tab with auto-fetch metadata, transfer tags on import, and remove release after import preferences
- Automatic metadata fetching for discovery releases from Bandcamp, SoundCloud, YouTube, and Discogs URLs
- Playlist support for discovery releases with separate playlist hierarchies per view
- Export playlists to USB devices with Pioneer/Rekordbox compatibility
- Multi-language support with 11 locales: English, Japanese, Dutch, French, German, Spanish, Italian, Swedish, Korean, Portuguese, and Chinese
- Automatic system language detection with user preference override in Settings
- Track BPM and key analysis
- Discovery release deduplication with overlap detection during add flow
- Expandable track sub-rows in the discovery list with expand/collapse all
- Merge releases action for combining duplicate discovery entries
- SoundCloud set/playlist URL support for fetching all tracks in a set
- Bandcamp parent album detection for individual track pages
- YouTube preview playback support for single videos and playlists in discovery

## [0.1.0] - 2024-12-20

### Added

- Library management with automatic metadata extraction
- Playlist and folder organization
- Tag system with AND/OR filtering
- Audio playback with device selection
- USB device monitoring
- Waveform display with cue point management
- Search and filter across entire collection

[Unreleased]: https://github.com/blackboxaudio/crate/compare/v0.2.9...HEAD
[0.2.9]: https://github.com/blackboxaudio/crate/compare/v0.2.9...v0.2.9
[0.2.9]: https://github.com/blackboxaudio/crate/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/blackboxaudio/crate/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/blackboxaudio/crate/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/blackboxaudio/crate/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/blackboxaudio/crate/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/blackboxaudio/crate/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/blackboxaudio/crate/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/blackboxaudio/crate/compare/v0.2.2-staging.1...v0.2.2
[0.2.1]: https://github.com/blackboxaudio/crate/compare/v0.2.1-staging.1...v0.2.1
[0.2.0]: https://github.com/blackboxaudio/crate/compare/v0.2.0-staging.1...v0.2.0
[0.1.0]: https://github.com/blackboxaudio/crate/releases/tag/v0.1.0
