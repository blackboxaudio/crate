# Changelog

All notable changes to Crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Added interface zoom on desktop: scale the whole UI from 75% to 150% via Settings → Appearance or View → Zoom In / Zoom Out / Actual Size (⌘= / ⌘- / ⌘0); the level is remembered per device and never cloud-synced

## [0.3.0-staging.1] - 2026-06-24

### Added

- Added a Crate mobile app for iOS and Android: a discovery companion that syncs discovery releases, playlists, tags, and followed sources with desktop through cloud sync, works fully standalone without signing in, and never touches the local audio library (iOS ships via TestFlight, Android as a signed APK attached to GitHub releases)
- Added the mobile discovery feed: a virtualized list or three-column artwork grid with search, sorting, tag filters (AND/OR), swipe actions (Play Next, Add to Queue, Delete), iOS-style long-press context menus, multi-select batch actions, "New" badges on releases surfaced by followed sources, and pull-to-refresh
- Added release detail on mobile: artwork, metadata, track list, tag picker, editable notes, one-tap open in the source app, metadata refresh (automatic when a release has no tracks, or on demand), and sharing releases, tracks, and followed sources through the native share sheet or as a copied URL
- Added the mobile preview player: a mini-player docked above the tab bar that expands into a full-screen player with a blurred artwork backdrop, scrubber, shuffle, repeat, like, ±10% tempo, and swipe-to-skip cover pager; an Up Next queue sheet with drag-to-reorder and a 50-track listening history; and playback state (track, position, shuffle, tempo) restored on relaunch
- Added native lock-screen and CarPlay transport on iOS (play/pause, previous/next, scrubbing, repeat, and Like) driven by an AVPlayer engine that keeps playing gaplessly while the screen is locked, plus Android media-session controls with a notification progress bar and audio ducking
- Added offline listening to the mobile app: every played track is kept on the device, "Download for Offline" pins a whole release so it can never be evicted, artwork is cached for offline display, both caches have adjustable size limits with least-recently-used eviction, a "Downloaded" filter and badges show what plays in airplane mode, and a banner appears when the device is offline
- Added discovery playlists on mobile: create, rename, delete, and reorder playlists, smart playlists with a touch rule editor (create and edit), folders with 2×2 cover mosaics and full-screen drill-in navigation, add-to-playlist from any release or track, and the feed's search/sort/filter controls inside playlist, tag, and followed-source views
- Added tag management on mobile: browse categories as color-coded chips, create, rename, recolor, move, and delete tags and categories, tag releases and individual tracks, and drill into a tag to see everything carrying it
- Added a Following tab on mobile: follow artists and labels by URL, see new-release counts, check one or all sources via pull-to-refresh, unfollow, and drill into a source's releases, plus an inline Follow action on any release's menu
- Added adding releases on mobile: paste a release URL for an editable metadata preview, paste an artist or label page URL to bulk-import, a "Paste link" clipboard button (automatic prefill on Android), URLs shared from other apps opening straight into Add Release, and an offline add queue that retries with backoff and drains when connectivity returns
- Added cloud sync to mobile: native sign-in with Google (ASWebAuthenticationSession / Custom Tabs) and Sign in with Apple on iOS, a live sync-status account chip in the header that opens a sheet to sync now or sign out, sync on launch and on returning to the foreground, opportunistic Background App Refresh sync on iOS, and Firebase App Check attestation (App Attest / Play Integrity) on every request
- Added first-run onboarding on mobile (a dismissible carousel with an optional sign-in step and theme/accent selection), branded native launch screens, and per-channel app icons on both platforms
- Added iOS-style mobile Settings pages: General (language, date format), Appearance, Following (check cadence, release-day reminders, new-release summary), Cloud Sync, Storage (audio and artwork caches), and About
- Added navigation persistence on mobile: the app reopens on the tab, folder, detail screen, and feed scroll position you left
- Added secure on-device database key storage on mobile: the encrypted library key lives in the iOS Keychain or is wrapped by the Android Keystore
- Added track-level tagging for discovery releases on desktop and mobile: tracks can be tagged independently of their release, tagged tracks show their tag colors inline, a tag filter matches a release when it or any of its tracks carries the tag, and track tags sync across devices and are included in backups
- Added repeat controls to preview playback on desktop and mobile (Off → Repeat track → Repeat release → Repeat all) that compose with shuffle and the Play Next / Add to Queue queue, persist across restarts, and are mirrored on the iOS lock screen
- Added Bandcamp collection integration: link one or more Bandcamp fan accounts by username or fan-page URL and everything you've purchased shows as owned (release badges, per-track "x/y owned" markers, and a Purchased filter on both platforms), with a desktop "Missing from Library" view cross-referencing purchases against imported tracks; collections sync across devices and refresh on a configurable cadence, and only public collection data is read (no Bandcamp login)
- Added three-way discovery filters on desktop and mobile: Liked, New, Purchased, and Downloaded each switch between Off, Only, and Not, combine freely, and are available inside discovery playlist and folder views; "Clear all" resets every filter
- Added a "Date Liked" sort for release lists while the Liked filter is set to Only; likes now record when they happened, and the timestamp syncs across devices and is included in backups
- Added exporting discovery releases to JSON on desktop (the whole collection from Settings → Discovery, or a selection from the release context menu) in a curated shape for AI knowledge bases or a raw dump of every field, with a live release count and size estimate
- Added pre-order awareness to discovery previews: tracks the source can't stream yet are greyed out and unplayable, skipped by auto-advance and the queue, and re-checked automatically once the release is out
- Added "Tags" to every context menu that has a track or release: desktop library tracks and discovery releases (nested by category with applied / partial check marks), and the mobile release menu, release detail, and now-playing screen (with a searchable tag picker)
- Added in-app account deletion under Settings → Cloud Sync on desktop and mobile, permanently removing the account and all synced data while leaving local files untouched
- Added readable sync errors: sync failures now name their cause (session expired, permission denied, storage limit, server trouble, and more) and a "Copy sync diagnostics" button copies a persistent sync log for troubleshooting

### Changed

- Discovery playlists are now track-based: adding a release adds each of its tracks, single tracks can be added from their menus, rows still group by release with a "3 of 12 tracks" label when a release is only partly included, tracks inside expanded releases can be selected, dragged, and removed individually on desktop, and existing playlists carry over with identical contents
- Unified playback around one queue on both platforms, bringing desktop library playback repeat track / album / all, queue-aware next / previous with listening history, and shuffle that remembers what it played across list changes; with repeat off, playback now stops at the end of the list instead of looping forever (turn on Repeat all for the old behavior)
- Made every context menu on desktop and mobile follow one layout (actions on the item, then filing, then editing, then links and sharing, destructive last), turned the desktop "Add to Playlist" and track "Tags" menus into nested sub-menus that follow the sidebar hierarchy, and made sidebar tags apply to selected discovery tracks on click or drag
- Unified wording across the app ("Date Added" / "Date Released", "Delete", "Artwork", one capitalization convention) and routed every remaining hardcoded English string through translation
- Preview stream links are now resolved only when a track is played (plus a small look-ahead for the queue) instead of being pre-fetched in bulk for every import, since the links expired within hours anyway
- Followed-source refresh now throttles automatic page fetches to once per 30 minutes per source and backs off sources that return rate-limit responses; mobile checks sources via pull-to-refresh
- Mobile cloud sync runs on launch, on returning to the foreground, and opportunistically in the background instead of polling continuously; desktop keeps its always-on background sync
- Polished the mobile app to platform conventions: iOS-style large titles, the native system font with Dynamic Type, portrait lock, inset-grouped form sheets that lift above the keyboard, native confirm dialogs, full-screen folder drill-in with edge-swipe back, and re-tapping the active tab scrolls to the top or backs out of a drill-in
- Made mobile rendering and scrolling smooth: frosted-glass surfaces pause their blur during motion, gestures track the display's frame rate, feed rows use pre-sized thumbnails decoded off the main thread, releases load in pages, and large syncs no longer block taps or playback

### Fixed

- Fixed YouTube previews (and the YouTube-backed previews of Discogs releases) after YouTube retired the API clients the app used and started flagging its visitor identity as bot traffic; YouTube playlist import works again, and a probe script plus a scheduled canary workflow now flag the next breakage
- Fixed the desktop update prompt appearing on top of first-run onboarding or the feature tour and freezing the screen; it now waits until they're done
- Fixed the auto-updater comparing versions across release channels: a staging build only accepts newer staging builds and a stable build only newer stable builds
- Fixed the desktop app starting playback by itself when Bluetooth headphones or speakers connected or disconnected
- Fixed the desktop playlist sidebar getting sluggish with large playlist trees: only the rows that changed re-render, and tall folders open instantly
- Fixed a thin sliver of background showing above a selected release in the desktop discovery list, and a flash of unstyled text or a white page at launch before the splash screen on both platforms
- Fixed tagging a track from its context menu failing with an error
- Fixed the repeat modes: Repeat track literally repeats the playing track, Repeat release no longer drifts into the next release on rapid skips, a single track queued from another release plays once as an interlude and hands back to the list it interrupted, and toggling shuffle or repeat in the last seconds of a track no longer applies one track late
- Fixed most Bandcamp purchases not being recognized as owned (matching now also uses the release page and track title), and a linked collection stalling partway through its import when the initial scan was interrupted
- Fixed cloud sync duplicating a release's tracks when two devices fetched them independently: identities are now derived from content, duplicate copies already in the cloud are merged, and likes are preserved
- Fixed cloud sync flipping to "Offline" shortly after signing in despite a working connection, failing forever on large collections over slow connections, and getting permanently stuck on "Sync error" after deleting a release another device still had
- Fixed the published Android APKs being unsigned and impossible to install; the release pipeline now refuses to publish an unsigned APK
- Fixed mobile preview playback failing on every track after the app had been suspended (the internal audio server now restarts itself, and cached audio on iOS plays straight from storage), dead cached stream links being re-served until they expired, a playback error deleting a release's downloaded audio, and duplicate error toasts
- Fixed the mobile app being killed by iOS during preview playback (a runaway artwork-caching loop) and going white under memory pressure with large libraries
- Fixed iOS previews of YouTube and Discogs tracks reporting twice their real length and going silent once scrubbed past the midpoint
- Fixed locked-iPhone shuffle silently stopping after a few tracks, playback dead-ending when the playing track dropped out of the queue's list, the lock-screen repeat button widening "repeat release" to the whole feed, and the delay between tapping a track and hearing it
- Fixed the mobile "Sign in with Google" button spinning forever on TestFlight and release builds when App Check attestation stalled
- Fixed the mobile smart-playlist editor locking up the app when opened, and the Playlists tab search only matching the folder level being viewed
- Fixed mobile bottom sheets being covered by the keyboard, the player scrubber snapping back after seeking on iOS, drawer close gestures nudging the content underneath, an invisible strip below the mini-player swallowing taps, the launch splash handoff, missing cover art showing a broken-image icon, and refreshing a sorted list briefly scrambling its order
- Fixed the followed-source check running doubled and firing while the mobile app was backgrounded with audio playing

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
[0.3.0-staging.1]: https://github.com/blackboxaudio/crate/compare/v0.2.9...v0.3.0-staging.1
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
