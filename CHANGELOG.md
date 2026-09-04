# Changelog

All notable changes to Crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Added repeat controls to preview playback on desktop and mobile: a button beside shuffle cycles Off → Repeat track → Repeat release → Repeat all — looping the current track, the current release, or the whole list the session was started from (the discovery feed, a playlist, a tag, or a followed source). Repeat composes with shuffle (a repeated release shuffles within itself; a repeated context reshuffles each pass), tracks queued via Play Next / Add to Queue still play first, the chosen mode persists across restarts, and on iOS repeat-track loops gaplessly even while the phone is locked, with the lock screen's own repeat button reflecting and controlling the mode

- Added Bandcamp collection integration: link your Bandcamp account by entering your username (or pasting your fan page URL; multiple accounts supported — collections combine) — reachable from the discovery filter menu on both platforms as well as Settings — and everything you've purchased shows as owned across the app — an "Owned" badge on releases in the desktop discovery list (with a per-track "x/y owned" state when only some tracks were bought), owned indicators on mobile rows and grid tiles, and a "Purchased" filter on both platforms that shows your whole collection on mobile (purchases not yet in your discovery feed link out to Bandcamp or can be added with one tap). Purchases sync across devices via cloud sync, collections auto-refresh on a configurable cadence (with the same rate-limit safeguards as followed artists), and desktop gains a "Missing from Library" view that cross-references purchases against your imported tracks so you can see what you've bought but never downloaded. Only public collection data is read — no Bandcamp login, and private/hidden purchases stay private

- Added reliable offline listening to the mobile app: any track you've played is kept on the device and replays with no network until the cache evicts it, releases downloaded via "Download for Offline" are now pinned so everyday listening can never silently evict them (only "Remove Download" or clearing the cache reclaims that space), auto-advance skips tracks that aren't downloaded while offline instead of stopping with an error, a "Downloaded" filter and per-row badges show at a glance what's playable in airplane mode, and a slim banner under the header says when the device is offline
- Added sharing to the mobile app: releases, individual tracks, and followed artists/labels can be shared through the native iOS/Android share sheet or copied as a URL from their long-press and "…" menus — and tracks now carry their own page links (Bandcamp track pages, SoundCloud permalinks), captured for new releases and backfilled when metadata is refreshed, with the release URL as the fallback (the desktop per-track menu picked this up too)
- Added the discovery feed's sorting and filtering to the rest of the mobile app: playlist, tag, and followed-source release lists get the same search / sort / filter controls (applied to the view without touching the playlist's stored order — manual reorder stays available whenever the playlist's own order is shown), the Playlists tab can sort its folders and playlists by name or date, the Following roster gained the sort options desktop has (new count, name, recently released) plus a search box that survives switching tabs, and playing from a sorted or filtered view queues exactly what's on screen
- Added a grid view to the mobile discovery feed: a toolbar toggle switches between the classic list and a three-column artwork grid (remembered across launches, scroll position preserved when toggling), with tap, long-press menu, select mode, and the New/Downloaded indicators all working on tiles
- Added a listening history to the mobile queue sheet: an "Upcoming | History" switch shows the last 50 tracks you played (kept across restarts), and tapping one plays it again
- Added a swipe-right quick action to mobile release rows: swiping right reveals Play Next (complementing the existing swipe-left Add to Queue / Delete), with a haptic tick when a row opens
- Added a Like button to the iOS lock screen / CarPlay for the playing preview track: it toggles the track's liked state even while the phone is locked, stays in sync with likes made in the app, and syncs across devices like any other like
- Added clipboard intake to the mobile Add Release flow: a "Paste link" button fills the URL field from the clipboard (an explicit tap, so iOS's paste-permission prompt only appears when you asked for it), and on Android an eligible copied link prefills automatically when the form opens
- Added in-app account deletion: signed-in users can permanently delete their account and all synced data (tracks, playlists, tags, discovery) from the cloud via Settings → Cloud Sync → Delete account on both mobile and desktop, while the audio files and library on the device are left untouched — satisfying the App Store and Google Play requirement that in-app account creation come with in-app deletion
- Added Sign in with Apple to the iOS app: the cloud-sync sign-in (in Settings → Cloud Sync and first-run onboarding) now offers a native "Sign in with Apple" button alongside Google, authenticating with Face ID / Touch ID and supporting Apple's private-relay email — satisfying the App Store requirement to offer an equivalent privacy-focused login wherever a third-party sign-in is available. Android and desktop are unchanged (Google sign-in)

- Added pre-order awareness to discovery previews: tracks the source doesn't stream yet (an unreleased track on a Bandcamp pre-order album) are now greyed out and unplayable instead of failing with a generic stream error when tapped, playback auto-advance and the queue skip over them, and once the album is released the app quietly re-checks availability and the tracks become playable on their own

- Brought the mobile discovery filters and owned markers to the desktop discovery list: the filter menu gained a "Downloaded" toggle (releases whose previews are all cached on disk, so they play with no network) alongside Liked, New, and Purchased, those filters are now also reachable from discovery playlist and folder views instead of only the main feed, "Clear all" resets every filter (not just tags) and shows up even with no tag categories, and each track inside an expanded release now carries a small bag marker when that track is owned in your linked Bandcamp collection — so a partly-owned release shows exactly which tracks you already bought

### Changed

- With repeat off (the default), preview playback now stops at the end of the playing context instead of silently looping back to the start — the old always-loop behavior lives on as the explicit "Repeat all" mode on the new repeat button

- Modernized the mobile form sheets to platform conventions: form fields now sit in iOS-style inset-grouped sections with footnote text (Add Release, Edit Release, the smart-playlist editor, and follow-by-URL), small forms present at a content-hugging height that lifts above the keyboard instead of a fixed near-full-screen sheet, text fields gained the platform keyboard affordances (a Go key that submits URL forms, sensible auto-capitalization, an in-field clear button), and the primary action responds with a light haptic tick

- Unified the wording across the app so the same thing is called the same thing everywhere: date columns and sort options now read "Date Added" and "Date Released" on both desktop and mobile (previously a mix of "Date Added", "Added", "Released", and "Release Date"), deleting a release is called "Delete" everywhere instead of alternating with "Remove from Discovery", artwork is "Artwork" rather than "Album art", and buttons and menu items follow one capitalization convention instead of the two that had drifted apart. Alongside this, every remaining piece of hardcoded English — most visibly the Relocate Track and Add Tag dialogs, several context-menu items, and a batch of notifications — now goes through translation, so it appears in your language like the rest of the app
- Made the mobile app render at native-feel smoothness: bottom sheets and drawers animate faster and no longer drop frames (their frosted-glass material pauses its blur while a sheet is in motion and returns seamlessly at rest), long-press menus dim first and gain their background blur once open, finger-drags on sheets and the player's cover pager track the display's full frame rate, the mini-player repositions without competing with the animation beneath it, and release covers decode off the main thread — without re-decoding or flashing when a cover is cached — so scrolling stays smooth while artwork streams in
- Made mobile scrolling itself dramatically smoother: flinging a release list no longer waits on the app for every frame (the pull-to-refresh gesture only engages at the very top of the list instead of shadowing all touch movement), the mini player's frosted glass pauses its blur while the list underneath it moves and fades back in at rest, feed rows now render small pre-sized cover thumbnails instead of decoding full-size artwork per row, playback progress ticks no longer re-render every visible row while a preview plays, and reordering a large playlist tracks the finger at the display's frame rate
- Unified the mobile app's overlay surfaces around platform conventions: every multi-field form (Add Release, the smart-playlist editor, Edit Release, and following an artist/label by URL — previously a small centered popup) now presents as the same full-height sheet with Cancel and the primary action in a top nav bar; single-field naming prompts consistently use the centered iOS-style dialog (including "New playlist" inside the add-to-playlist picker, which previously expanded an inline text field); and dismissing any form — scrim tap, swipe-down, Android back, or Cancel — now always cancels, asking "Discard changes?" first when there are unsaved edits (previously the same gesture saved the edits in one sheet and silently discarded them in another). Onboarding also now reliably layers above every other surface, and unfollowing an artist/label asks a proper confirmation question naming the source
- Restructured mobile Settings into iOS-style pages: the root is now a grouped list that opens General (a new language picker — the stored language finally applies on mobile — and a date-format choice), Appearance, Following (check cadence, release-day reminders, new-releases summary, and a "Check all now" button, previously desktop-only), Cloud Sync (the account chip in the header now jumps straight to it), Storage (the audio/artwork caches), and About (which now also shows the build environment) — with the platform back conventions throughout (header chevron, iOS edge-swipe, Android back button popping one level at a time)

### Fixed

- Fixed the desktop playlist sidebar getting sluggish with large playlist trees: expanding or collapsing a folder visibly lagged because the whole tree was rebuilt and re-rendered on every toggle — the tree now only re-renders the rows that actually changed, and very tall folders open instantly instead of playing an animation that couldn't keep up

- Fixed the published Android APKs being unsigned and therefore impossible to install on any device (Android rejects them with a generic "App not installed" error): the release pipeline wrote the signing configuration where the build never looked for it, so it silently produced unsigned builds even though the signing key was set up — the release build is now signed again, and the pipeline refuses to publish an unsigned APK instead of shipping one quietly

- Fixed a thin sliver of background showing above a selected release in the desktop discovery list, between the row's highlight and the border it shares with the release above it

- Fixed shuffle and repeat misbehaving in a handful of ways, and unified playback around one queue on both platforms: shuffle no longer forgets which tracks it already played whenever the on-screen list changes mid-session (playing through a filtered feed used to reset the no-repeat memory constantly, so the same tracks came back), locked-iPhone shuffle no longer silently stops after a few tracks (the app now tops up the native player's upcoming window right before it's suspended), toggling shuffle or repeat in the last seconds of a track now takes effect on the very next track instead of playing one more track under the old mode, the iOS lock screen's repeat button no longer silently widens "repeat release" to the whole feed, and tracks queued with nothing playing survive an app relaunch again. Desktop's separate playback logic was replaced by the same queue mobile uses, which brings library playback the full feature set: repeat track / release ("release" = the track's album) / all for local files, queue-aware next/previous with listening history, and — with repeat off — playback now stops at the end of the library list instead of looping forever (turn on "Repeat all" for the old behavior)

- Fixed a single track queued from another release taking its whole release along: once the queued track started playing, the release it came from hijacked the queue — with Repeat release the loop re-anchored onto that release instead of returning to the one that was looping, and with Repeat track the Up Next list filled with the queued track's release instead of continuing where the session left off. A queued track now plays once as an interlude and hands playback straight back to the list (or release loop) it interrupted, in every repeat and shuffle mode

- Fixed the repeat modes not doing what they say: Repeat track now literally repeats the playing track — the Next button (and lock-screen skip) replays it and Up Next shows it, instead of skipping ahead Spotify-style, which read as repeat not working; Repeat release no longer drifts into the next release when tapping Next rapidly right after changing the mode (the iOS player could briefly keep following its stale pre-fed track window — the queue is now always the authority on what a skip plays); and a single track queued from another release now plays once as an interlude and hands playback straight back to the release or list it interrupted, instead of dragging its entire release into the queue

- Fixed mobile preview playback failing on every track after the app had been away for a while: a few tracks would play fine, then — typically after locking the phone or switching apps — every play failed instantly with a stream error, including tracks already downloaded for offline listening, and only force-quitting the app brought playback back. The internal audio server the player streams from is shut down by the system while the app is suspended and now restarts itself the moment it's needed again; on iOS, tracks whose audio is already on the device now also play straight from storage without involving that server at all, so anything downloaded or previously played keeps working regardless

- Fixed the mobile app hanging when opening the smart-playlist editor: choosing "New Smart Playlist" from the Playlists tab's + menu (or "Edit Smart Playlist" from a smart playlist's own menu) locked the app up — the menu stayed stuck open over the blurred background, nothing responded to taps, and force-quitting was the only way out
- Fixed most Bandcamp purchases not being recognized as owned: individual track purchases showed "Not in Crate" (and releases missed their Owned badges) even when the containing release was in the discovery collection, because matching relied on per-track page URLs most stored tracks don't have yet — a track purchase now also matches by the release's page and the track's title — and releases saved from label-page scans carried query-string junk in their URLs that broke even exact album matching, which is now normalized going forward and repaired once on launch (merging any duplicate spellings of the same release)
- Fixed a linked Bandcamp collection permanently stalling partway through its import: if the initial scan was interrupted (a network error, or the app going to background), every later refresh saw the newest purchases already known and stopped immediately, so the rest of the collection never arrived — the app now remembers whether a scan ever reached the end and re-walks the whole collection until one does
- Fixed mobile drawer close gestures nudging the content underneath: swiping a drawer or sheet closed (most visibly the release details page) used to let the slight vertical wobble of a mostly-horizontal thumb scroll the page a few pixels mid-dismiss — content now locks in place the moment the close drag takes hold, on every drawer and sheet
- Fixed the jarring app-launch splash handoff on mobile: the system launch screen used to crossfade into a differently sized and positioned logo (two ghosted crate icons visible at once) — or, when the page hadn't rendered yet, into a blank white screen that washed the logo out before it popped back in. The in-app splash is now a pixel-identical copy of the native launch frame, and on iOS a native copy of the launch screen stays over the page until the in-app splash has actually painted, so launch reads as one continuous splash with the wordmark and version easing in. Staging builds also no longer show a doubled version suffix ("v0.3.0-staging.33-staging") on the splash and About screens
- Fixed mobile preview playback dead-ending when the playing track was no longer part of the queue's list — after relaunching the app mid-session, playing a track queued from another view, or filtering the feed so the playing release dropped out of it. The Next button greyed out and playback stopped at the end of the current release in every repeat mode except Repeat release; the queue now rejoins the list it was playing from (after the last of its tracks that played, or from its start), and the Next button is no longer ever disabled — matching Previous, with a tap at a genuine end of the queue simply doing nothing
- Fixed an invisible strip across the mobile release, playlist, tag, and followed-source screens that silently swallowed taps whenever a track was loaded in the mini-player: buttons under it (most noticeably "Add Tags" on a release) didn't even light up when pressed, let alone do anything. The mini-player sits lower on these screens than it does over the tab bar, but only its visible card moved down — the space it vacated kept catching taps
- Fixed the gap between tapping a track and hearing it on mobile: the spinner cleared and the player switched to "playing" the instant playback was *requested*, leaving a stretch of silence that looked like it had already started, and the tapped track was made to wait behind the pre-resolution of up to twenty upcoming tracks before it was even handed to the player. The chosen track now starts immediately (the upcoming queue fills in behind it, so gapless advances and lock-screen skips are unaffected), the loading indicator stays up until audio is genuinely rolling and reappears if playback stalls mid-track, and a track that never starts now returns to idle with an error instead of sitting on a spinner or a silent "playing" state
- Fixed Crate starting to play music by itself when Bluetooth headphones or speakers connect or disconnect: the desktop app treated every transport command the system sent it as a play/pause *toggle*, so the pause macOS delivers when a Bluetooth device drops (or the stray play some devices send on connect) started playback that was never running — and with nothing loaded it would even begin playing the first track in view. Play now only ever plays and pause only ever pauses, and a command arriving from the system can no longer start playback on its own. Relatedly, losing the output device mid-playback now pauses cleanly and moves playback to the new default device, instead of leaving the app silently "playing" into a device that's gone — and on mobile, a stream error, an ended phone call, or a reconnecting Bluetooth device can no longer resume a track you had deliberately paused (Android now also pauses when headphones are unplugged, as iOS already did)
- Fixed the mobile app being killed by iOS during or shortly after preview playback: opening the full-screen player kicked off a runaway internal loop (an artwork-caching feedback cycle re-requesting the same cover thousands of times per second) that made taps lag, hammered the database with writes, and ballooned memory until the system terminated the app — including in the background while audio kept playing. The loop is fixed; playback now costs what it should
- Fixed the followed-artists check running doubled and at the wrong time on mobile: two sweeps could run at once (every followed page fetched twice back-to-back), and the automatic launch sweep could fire while the app was backgrounded with audio playing, burning background CPU and battery — sweeps are now serialized (a duplicate trigger is skipped) and on mobile the automatic sweep waits until the app is actually in the foreground
- Fixed a split-second flash of unstyled text (the app name and version in the top-left corner) that could appear at launch before the branded splash screen, on both desktop and mobile — the splash now always renders fully styled and centered, and the app no longer flashes a white page behind the splash fade-in in dark mode
- Fixed cloud sync duplicating a release's tracks when two devices fetched them independently (each track showed twice, and a like landed on only one of the copies): track and release identities are now derived from their content (release + track name, release URL) so independent fetches produce the same rows, syncing devices recognize and merge duplicate copies already in the cloud — keeping likes, tags, and playlist membership — and existing local duplicates are cleaned up automatically on launch. This also fixes the related split where two devices adding the same release URL before syncing ended up with permanently diverged copies whose likes and tags never synced
- Fixed refreshing a sorted list briefly scrambling its order: reloading the releases (pull-to-refresh, or after a tag change) published each page of the reload as it arrived, so a custom-sorted feed visibly collapsed to a partial, recency-flavored list and then snapped back — a reload now keeps showing the current list and swaps to the fresh one in a single step (first loads still fill in progressively)
- Fixed track preview failures showing two error toasts on mobile — one failure now surfaces exactly one message, and when the cause is being offline it says so ("This track isn't available offline") instead of a generic stream error
- Fixed a playback error deleting a release's downloaded audio: the automatic retry after a transient player error wiped the on-disk copy before re-fetching (which then also failed offline), so a downloaded release could destroy itself in airplane mode — the retry now only refreshes stream links and replays from the intact download, and a corrupted cache entry is repaired per-track instead of discarding the whole release
- Fixed "Failed to load preview stream" errors sticking around on iOS once they started, even across force-quitting the app: a cached stream link that had gone dead before its recorded expiry (typically after switching between WiFi and cellular) was re-served on every attempt until it expired hours later — a failed download now discards the release's cached links immediately, and the iOS player auto-retries once with freshly resolved links (matching desktop) before showing an error. Stream failures are also now recorded in the device log so field issues can be diagnosed

- Added a first-run onboarding flow to the mobile app: a one-time, dismissible carousel that welcomes you, explains adding and previewing releases, offers an optional "Sign in to sync with desktop" step, and lets you pick a theme and accent color — the app is fully usable standalone with zero sync setup, and onboarding can be skipped at any point. The theme/accent step is skipped only when signing in restores your existing appearance settings from another device
- Added branded native launch screens to the mobile app on iOS and Android (the crate mark on the app background, respecting light/dark mode) so there's no plain-white flash before the app loads
- Added a "New" badge to mobile discovery releases surfaced by a followed source: newly surfaced releases are marked everywhere they appear (the discovery feed, playlists, followed-source feeds, and the release detail) until you open or play them, at which point the badge clears automatically and syncs across devices
- Added metadata refresh to the mobile release detail: opening a release that has no tracks yet now auto-fetches them from the source, and a "Refresh Metadata" action in the release's menu (or a pull-down on the release) re-fetches metadata and tracks on demand
- Added pull-to-refresh across the mobile app: pull down on the discovery feed or Following tab to check all followed sources for new releases, on a followed source's release list to check just that source, and on a release to refresh its metadata — with freshly surfaced releases reloading into the feed inline
- Added the iOS tab-bar convention to the mobile app: re-tapping the active tab scrolls its list back to the top, or — when a drill-in (release, playlist, tag, or followed-source detail) is open — backs out of it a level at a time
- Added offline downloads to the mobile app: a "Download for Offline" action on a release's menu pre-fetches its audio to the device so it plays with no network (airplane mode), a "Downloaded" badge marks fully-cached releases, and "Remove Download" reclaims the space; the on-device audio cache is capped at 500 MB and evicts the least-recently-played tracks when full
- Added offline album art to the mobile app: release covers are saved to the device the first time they're shown, so they still appear with no network (airplane mode); mobile Settings now shows the artwork cache size next to the audio cache, each can be cleared separately, and both have an adjustable size limit that evicts the least-recently-shown / least-recently-played items when full
- Added smart-playlist editing to the mobile app: a smart playlist's long-press menu now offers "Edit Smart Playlist", opening the rule editor prefilled with the playlist's name, match mode, and conditions — and a release limit set on desktop survives a mobile edit untouched
- Added restoring your place in the mobile app across restarts: the app reopens on the tab you left, inside the same playlist folder (the folder path now also survives switching tabs), with any open release, playlist, tag, or followed-source screen restored in place, and the discovery feed scrolled back to the release you were looking at — anything deleted from another device in the meantime is skipped gracefully
- Added swipe-to-skip and track-change animations to the mobile full-screen player: swipe the cover left/right to go to the next/previous track — the neighboring track's cover follows your finger in, with a rubber-band stop when there's nothing further — and every track change (buttons, swipes, auto-advance, lock-screen skips) now slides the cover through like a card deck while the title and artist ticker-roll in after it; moving to a different release also slowly cross-dissolves the blurred backdrop, while tracks of the same release keep it perfectly still
- Added Android support to the mobile app: the encrypted library database now opens on-device (its key is protected by the Android Keystore), previews keep playing with the screen off or the app backgrounded — with lock-screen and notification playback controls, a progress bar, and polite pausing/ducking when another app takes over the audio — URLs shared from other apps (Bandcamp, SoundCloud, a browser) open Crate with the add-release form prefilled, the hardware back button/gesture closes the topmost screen or sheet a level at a time like a native app, and each release channel (dev, staging, production) installs side-by-side as its own app with its own icon and name; Android release APKs are now built and published alongside the other platforms

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
- Preview stream links are now resolved only when you play a track (plus a small look-ahead for the upcoming queue), instead of being pre-fetched in bulk for every imported or updated release — the pre-fetched links expired within hours anyway, so large imports no longer burn network and battery on wasted work or make playback laggy while they run
- Sync failures now say what actually went wrong: instead of a generic "Sync error", the status shows a specific, human-readable message per cause (session expired, permission denied, storage limit, server trouble, and more) with a sanitized detail line, and a "Copy sync diagnostics" button copies a persistent sync log for troubleshooting

### Fixed

- Fixed the mobile app going permanently white (and feeling frozen) under memory pressure with large libraries: releases now load in pages instead of one giant payload so the screen stays responsive and memory spikes are gone, syncing thousands of releases no longer blocks taps and playback while it writes (user actions take priority over sync work), cover downloads while scrolling are limited to a few at a time, sync-triggered refreshes wait until you stop touching the screen, and if iOS still kills the app's web view it now reloads itself automatically (with audio playing through uninterrupted) instead of staying blank until relaunch
- Fixed missing or dead cover-art URLs showing the browser's broken-image icon (a small blue question mark) across the mobile app: every artwork spot — the discovery feed, release detail, players, queue, playlist mosaics, followed-source avatars, and import previews — now falls back to a polished placeholder tile (or the source's type icon) when a cover is absent or fails to load
- Fixed the mobile player scrubber snapping back to the old position right after seeking on iOS
- Fixed the mobile "Sign in with Google" button spinning forever after consent on TestFlight/release builds: a stalled or crashed App Check device attestation could leave the sign-in request permanently pending — native attestation and Keychain calls now run off the async runtime so their timeouts always apply, an attestation panic degrades to signing in without an attestation header, sign-in as a whole is bounded to two minutes, and a failure now shows its error under the sign-in button instead of spinning silently; mobile app logs are also now visible on-device (iOS Console.app / Android logcat) for diagnosing issues like this
- Fixed mobile bottom sheets being covered by the on-screen keyboard (most noticeably the "Add Release" sheet, where the URL field and action buttons were hidden): a sheet now lifts to rest just above the keyboard when a field is focused and drops back down when it's dismissed
- Fixed cloud sync flipping to "Offline — changes will sync when you reconnect" shortly after signing in (and periodically afterwards) despite a working connection: a background sync could reuse an idle connection the server had already closed, and the failed attempt read as lost connectivity — idle connections are now discarded before the server closes them
- Fixed cloud sync failing forever on large collections over slow connections: a fixed 60-second cap on every request meant a big library upload could never finish and silently retried in a loop — transfers now get time proportional to their size (only a stalled connection is cut off), a hiccup at Google's sign-in service no longer surfaces as a hard sync error requiring re-sign-in, and a rejected session is silently refreshed and retried once before giving up
- Fixed cloud sync permanently stuck on "Sync error" after deleting a release that another device still had: the deleted release's incoming tracks violated a database constraint and aborted the whole sync batch on every retry — those orphaned rows are now skipped (all devices converge on the deletion) and the rest of the batch applies normally
- Fixed preview playback stuttering or hanging while the app syncs or caches in the background with a large collection: playback lookups now read the library on separate connections that never wait behind sync writes (the database now runs in WAL mode), cover caching while scrolling no longer rescans the whole cache folder for every saved image, and streamed audio is served from disk instead of being held in memory (up to ~150 MB before) — the main cause of iOS killing the app under memory pressure
- Fixed mobile (iOS) cloud sign-in hanging on "Signing in…" and never completing, from two causes: the Google OAuth redirect scheme is no longer registered in the iOS app's `Info.plist` (it was intercepting the callback and preventing the native web-auth session from resolving), and App Check token minting on the sign-in path is now bounded by a timeout with a short failure cooldown so a slow or unregistered device attestation degrades to "no header" instead of stalling sign-in
- Fixed cloud sync wedging permanently with "Cloud sync blob not found" on every device: the background cleanup of superseded cloud data could delete a file the sync index had started referencing again, and once it was gone neither pulls nor pushes could ever get past it — cleanup now never deletes data the index still references, a sync that finds such a dangling reference skips it and repairs the index from local data instead of failing, and a device holding entries the repaired copy lacks re-uploads them so nothing is lost

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
