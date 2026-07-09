# Changelog

All notable changes to Crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Added a first-run onboarding flow to the mobile app: a one-time, dismissible carousel that welcomes you, explains adding and previewing releases, offers an optional "Sign in to sync with desktop" step, and lets you pick a theme and accent color — the app is fully usable standalone with zero sync setup, and onboarding can be skipped at any point. The theme/accent step is skipped only when signing in restores your existing appearance settings from another device
- Added branded native launch screens to the mobile app on iOS and Android (the crate mark on the app background, respecting light/dark mode) so there's no plain-white flash before the app loads
- Added a "New" badge to mobile discovery releases surfaced by a followed source: newly surfaced releases are marked everywhere they appear (the discovery feed, playlists, followed-source feeds, and the release detail) until you open or play them, at which point the badge clears automatically and syncs across devices
- Added metadata refresh to the mobile release detail: opening a release that has no tracks yet now auto-fetches them from the source, and a "Refresh Metadata" action in the release's menu (or a pull-down on the release) re-fetches metadata and tracks on demand
- Added pull-to-refresh across the mobile app: pull down on the discovery feed or Following tab to check all followed sources for new releases, on a followed source's release list to check just that source, and on a release to refresh its metadata — with freshly surfaced releases reloading into the feed inline
- Added the iOS tab-bar convention to the mobile app: re-tapping the active tab scrolls its list back to the top, or — when a drill-in (release, playlist, tag, or followed-source detail) is open — backs out of it a level at a time
- Added offline downloads to the mobile app: a "Download for Offline" action on a release's menu pre-fetches its audio to the device so it plays with no network (airplane mode), a "Downloaded" badge marks fully-cached releases, and "Remove Download" reclaims the space; the on-device audio cache is capped at 500 MB and evicts the least-recently-played tracks when full
- Added offline album art to the mobile app: release covers are saved to the device the first time they're shown, so they still appear with no network (airplane mode); mobile Settings now shows the artwork cache size next to the audio cache, each can be cleared separately, and both have an adjustable size limit that evicts the least-recently-shown / least-recently-played items when full
- Added smart-playlist editing to the mobile app: a smart playlist's long-press menu now offers "Edit Smart Playlist", opening the rule editor prefilled with the playlist's name, match mode, and conditions — and a release limit set on desktop survives a mobile edit untouched

### Changed

- Gave the mobile app iOS-style large-title navigation: each tab (Discovery, Following, Playlists, Tags) shows a large title at the top of its content that scrolls away and collapses into a compact centered title in the top bar as you scroll, with the per-tab search / sort / filter controls scrolling away alongside it. The brand mark, account chip, and settings gear stay pinned in the top bar. Following can be filtered by name or URL and Playlists by name, matching the existing Discovery search
- Changed the mobile app to render in the native system font (San Francisco on iOS) instead of a network-served web font, and to honor the system Dynamic Type text-size setting so text scales with your accessibility preference — the app no longer fetches a font over the network, so it paints instantly and works offline
- Locked the mobile app to portrait orientation
- Moved mobile Settings out of the bottom tab bar into a right-side drawer opened from a gear button in the top bar, so the four remaining tabs (Discovery, Following, Playlists, Tags) have room for their labels in every language; the mini-player is hidden while the settings drawer is open
- Changed the mobile cloud sign-in button to the standard Google-branded "Sign in with Google" button
- Changed the mobile Following tab to check sources via pull-to-refresh instead of a "Check all" button (checking a single source stays on its long-press menu)
- Polished the mobile Playlists tab to match the rest of the app: the search/add toolbar stays pinned while lists scroll, rows show a release/item count subtitle in the standard list typography, the first load shows a skeleton instead of flashing "No playlists yet", empty states gained an icon and a create shortcut, reorder mode keeps rows visually identical to normal browsing (and the list can now be scrolled by touch while reordering), and smart playlists no longer offer the reorder and remove actions that don't apply to rule-based lists
- Polished the mobile Tags tab to match the rest of the app: each category header now shows a visible "…" menu button (long-press still works), tags can be moved to another category from their long-press menu, tag chips are comfortably larger to tap, empty categories and the empty tab now explain the next step with a create shortcut, adding and removing tags or categories animates smoothly, and a tag's release feed supports pull-to-refresh
- Enabled iOS App Attest device attestation for staging and production builds (Firebase App Check): the App Attest entitlement is now stamped per release channel, while dev builds continue to use an App Check debug token
- Followed-source refresh now throttles its network checks to stay under Bandcamp, SoundCloud, and Discogs rate limits: each source's page is re-fetched at most once every 30 minutes on automatic refreshes (an explicit single-source "check now" still runs immediately), and a source that returns a rate-limit response is backed off with an increasing delay before it's checked again
- Changed mobile cloud sync to run when the app launches and each time it returns to the foreground, instead of polling continuously — the desktop app keeps its always-on background sync, but the phone no longer ticks in the background, saving battery
- Added opportunistic background cloud sync on iOS: beyond syncing on launch and when returning to the foreground, the app now also syncs occasionally in the background when iOS schedules it (Background App Refresh), still with no always-on poll, so edits from your other devices are more often already waiting when you open the app
- Mobile releases added by URL while offline now retry automatically with an increasing (exponential) backoff delay once metadata can't be fetched, in addition to retrying the moment connectivity returns, so a transient failure no longer leaves an item stuck until the app is restarted

### Fixed

- Fixed the mobile app going permanently white under memory pressure with large libraries: the initial cloud restore no longer re-fetches the entire release list for every sync batch, cover downloads while scrolling are now limited to a few at a time, and if iOS still kills the app's web view it now reloads itself automatically (with audio playing through uninterrupted) instead of staying blank until relaunch
- Fixed the mobile player scrubber snapping back to the old position right after seeking on iOS
- Fixed the mobile "Sign in with Google" button spinning forever after consent on TestFlight/release builds: a stalled or crashed App Check device attestation could leave the sign-in request permanently pending — native attestation and Keychain calls now run off the async runtime so their timeouts always apply, an attestation panic degrades to signing in without an attestation header, sign-in as a whole is bounded to two minutes, and a failure now shows its error under the sign-in button instead of spinning silently; mobile app logs are also now visible on-device (iOS Console.app / Android logcat) for diagnosing issues like this
- Fixed mobile bottom sheets being covered by the on-screen keyboard (most noticeably the "Add Release" sheet, where the URL field and action buttons were hidden): a sheet now lifts to rest just above the keyboard when a field is focused and drops back down when it's dismissed
- Fixed mobile (iOS) cloud sign-in hanging on "Signing in…" and never completing, from two causes: the Google OAuth redirect scheme is no longer registered in the iOS app's `Info.plist` (it was intercepting the callback and preventing the native web-auth session from resolving), and App Check token minting on the sign-in path is now bounded by a timeout with a short failure cooldown so a slow or unregistered device attestation degrades to "no header" instead of stalling sign-in

## [0.3.0-staging.1] - 2026-06-24

### Added

- Added signed mobile distribution: iOS builds upload to TestFlight automatically on release tags, and signed Android APKs with SHA-256 checksums are attached to GitHub Releases
- Added native cloud sync sign-in on mobile (iOS/Android) using the platform's secure web-auth session (ASWebAuthenticationSession / Custom Tabs) instead of the desktop loopback flow; mobile syncs discovery data only, never the local library
- Added secure SQLCipher database key storage on iOS using the Keychain (device-only, after-first-unlock accessibility) via a new platform `KeyProvider` abstraction; desktop keeps its local key-file behavior and the unencrypted-to-encrypted database migration is now desktop-only
- Added the mobile app's navigation shell: a Discovery main view with a left drawer (playlists and tags) opened by the hamburger button or a left-edge swipe, a right drawer (appearance and cloud sign-in) opened by the settings button, and touch-optimized base components
- Added a branded launch splash screen and a spinning-record loading indicator to the mobile app
- Added the mobile app's branded launcher icons (iOS app icon and Android adaptive icon) with per-channel variants matching desktop (development, staging, production)
- Added the mobile release detail screen (artwork, metadata, one-tap open in the source app — Bandcamp, SoundCloud, YouTube, Discogs — the track list, an assignable tag picker, and inline-editable notes) with an iOS-style left-edge swipe-back, plus an integrated preview player — a liquid-glass mini-player bar with progress whose artwork morphs up into a full-screen player (blurred album-art backdrop, drag-to-dismiss, scrubber, shuffle, previous/next, like, and a ±10% tempo control) whose shuffle and previous/next span the whole discovery feed on screen rather than just the current release — and, on iOS, a native lock-screen transport (play/pause, previous/next, and scrubbing that keep working while the screen is locked, with auto-resume after call interruptions) powered by a native AVPlayer engine
- Added the mobile discovery feed: a virtualized, searchable list of release cards (artwork, artist, title, label) with tag-filter chips (AND/OR), sorting (date added, artist, title, label), swipe-to-delete, and long-press multi-select for batch delete and batch tag assignment
- Added an account & cloud-sync control to the mobile top bar: a live status chip (synced, syncing, offline, or error) showing your account avatar that opens a sheet to sync now, see when the library last synced, or sign out — and the bar now shows the active section's title (Playlists, Tags, Settings) in place of the wordmark
- Added a first-class playback queue to the mobile preview player: an "Up Next" sheet (opened from the full-screen player) listing songs you've queued ahead of what the discovery feed will play next, with "Play next" and "Add to queue" for a single track from each track's menu in the release detail or for a whole release (all of its tracks, in order) from a feed card's long-press menu or swipe action, drag-to-reorder and swipe-to-remove for queued songs, and persistence across relaunch; queued songs always play before the shuffle/sequence resumes and are never reshuffled, and on iOS a lazily-resolved native-engine window keeps queued songs and cross-release advances gapless while the screen is locked
- Added restoring your listening state on the mobile app's launch: the preview you were last playing reappears in the mini-player — with its track progress, shuffle, and tempo — paused and ready to resume, without auto-opening the full-screen player
- Added a mobile settings page with cloud-sync account management, audio preview cache controls, and app info, plus live auto-sync so discovery changes from another device appear instantly without a tab-switch
- Added discovery playlists on mobile: create, rename, and delete playlists, smart playlists (built with a touch rule editor that filters on title, artist, label, tags, and more with a live match count), and folders with Spotify-style 2×2 cover-art thumbnails and animated, directional drill-in folder navigation on the Playlists tab, tap a playlist to view its releases in a full-screen detail overlay (the same fast, virtualized list as the discovery feed), add releases from the feed or release detail via a long-press context menu or multi-select batch action — picking a destination in an Add-to-Playlist sheet you can browse by folder or search by name, with the same 2×2 cover thumbnails throughout — remove releases with swipe or context menu, and long-press-drag to reorder releases in a dedicated reorder mode — all backed by a new `reorder_playlist_releases` backend command and bidirectional cloud sync
- Added add, edit, and bulk-import for discovery releases on mobile: paste a release URL to auto-fetch metadata with an editable preview, paste a page URL (Bandcamp artist/label, Discogs artist/label) to scan and bulk-import, edit release metadata via a dedicated sheet, clipboard prefill on open, and an offline add queue that persists pending URLs and drains them automatically when connectivity returns
- Added tag management on the mobile Tags tab so the app works standalone: browse your tag categories as sections of color-coded chips, create categories (auto-assigned a color) and tags, and rename, recolor (a 10-swatch picker), or delete them via iOS-style long-press menus; tap a tag chip to drill into a full-screen feed of the releases carrying it (the same fast, virtualized list as the discovery feed, with preview playback, long-press actions, and multi-select batch tagging) — all backed by the existing tag commands and bidirectional cloud sync, so categories and tags created on mobile converge with desktop
- Added a Following tab to the mobile app: browse the artists and labels you follow with their new-release counts, follow a source by pasting its page URL, check for new releases individually or all at once, and unfollow via iOS-style long-press menus; tap a followed source to drill into a full-screen feed of its releases (the same fast, virtualized list as the discovery feed, with preview playback and long-press actions), plus an inline Follow action on any discovery release's context menu to follow its artist or label — and new releases from your follows keep surfacing in the discovery feed
- Added Firebase App Check attestation to mobile cloud sync: every Firestore, Storage, and sign-in request now carries a token proving it came from the genuine app on a genuine device (Apple App Attest on iOS, Play Integrity on Android), hardening the backend against replayed credentials; desktop is unaffected and the feature ships in monitoring mode

### Changed

- Changed the mobile long-press menus (discovery releases, release-detail tracks, and playlists/folders) from bottom action sheets to native-style iOS context menus: the pressed item lifts above a blurred, dimmed backdrop while a spring-in action menu anchors to it (placed below or above as space allows), with a haptic on open and tap-outside / swipe-down to dismiss
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

[Unreleased]: https://github.com/blackboxaudio/crate/compare/v0.3.0-staging.1...HEAD
[0.3.0-staging.1]: https://github.com/blackboxaudio/crate/compare/v0.2.9...v0.3.0-staging.1
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
