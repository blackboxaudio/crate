# Crate Mobile — Product Requirements Document

## 1. Product Vision

Crate Mobile is a companion app that brings the Discovery feature from the Crate desktop application to iOS and Android. DJs use Discovery on desktop to save, tag, and preview releases they find online across Bandcamp, SoundCloud, YouTube, and Discogs. The mobile app extends this workflow to moments away from the desk — browsing discoveries at a record shop, saving a release shared on social media, or previewing tracks during a commute.

The app is local-first with zero infrastructure. It syncs with the desktop app over local WiFi, stores all data on-device, and works fully offline. There are no accounts, no cloud services, and no telemetry.

### Goals

- Let users browse and preview their discovery collection from their phone
- Enable adding new discoveries on the go (paste or share a URL)
- Keep desktop and mobile in sync without requiring cloud infrastructure
- Maintain the speed-first, local-only philosophy of the desktop app

### Non-Goals

- This is not a port of the full desktop application
- No library management (users don't have audio files on their phones)
- No export to USB / device management
- No cloud sync, user accounts, or remote infrastructure
- No social features or sharing between users

---

## 2. User Personas

### The Crate Digger
A DJ who actively hunts for new music across platforms. They find releases on Bandcamp while browsing on their phone, hear tracks at clubs they want to remember, and get recommendations via messages or social media. They want to capture these discoveries instantly without waiting to get back to their desk.

### The Organized Selector
A DJ who maintains a curated discovery list with tags and notes. They browse their collection before gigs to plan sets, reference releases at record shops, and use tags to quickly filter by genre, mood, or occasion. They primarily use the desktop app but want read access on mobile.

### The Weekend Warrior
A casual DJ who adds music sporadically and wants a simple, low-friction way to save releases. They don't want to manage sync or deal with setup complexity. Pairing should take seconds, and sync should happen transparently.

---

## 3. Feature Requirements

### 3.1 Discovery Feed

The primary screen. A scrollable list of all discovery releases, displaying:

- **Release artwork** (thumbnail)
- **Artist name**
- **Release title**
- **Label** (if available)
- **Source indicator** (Bandcamp, SoundCloud, YouTube, Discogs icon)
- **Tag dots** (colored indicators for assigned tags)
- **Date added**

**Sorting:** By date added (default, newest first), artist name, title, or label.

**Filtering:**
- Free-text search across artist, title, label, and notes
- Tag-based filtering with AND/OR mode toggle
- Tag filter chips displayed below search bar for quick access

**Interactions:**
- Tap a release to expand it (inline or full-screen detail view)
- Swipe left on a release to reveal delete action
- Long-press to enter multi-select mode (batch delete, batch tag)
- Pull-to-refresh triggers a sync attempt if desktop is reachable

### 3.2 Release Detail View

Shown when a release is tapped. Displays the full release information:

- **Full artwork** (large, top of view)
- **Artist, title, label, release date** (editable)
- **Source URL** with "Open in Browser" button
- **Notes** field (editable, free-form text)
- **Tags** with add/remove capability via tag picker
- **Track list** with:
  - Track name and position number
  - Duration
  - Play button (tapping plays the preview)

**Editing:** Users can modify artist, title, label, release date, and notes inline. Changes save to local DB immediately with an updated modification timestamp.

### 3.3 Add Release

Users can add new releases in two ways:

**Paste URL:**
1. Tap the floating action button (FAB) on the Discovery feed
2. A modal appears with a URL input field
3. Paste a URL from Bandcamp, SoundCloud, YouTube, or Discogs
4. App auto-detects the source and fetches metadata (artwork, artist, title, label, tracks)
5. Preview the fetched data — edit any fields before saving
6. Tap "Add" to save locally

**Share Sheet (iOS Share Extension / Android Intent):**
1. User is browsing in Safari/Chrome and finds a release
2. Taps the system share button and selects "Crate"
3. Crate processes the URL in the background
4. A confirmation notification appears (or the app opens to show the result)
5. The release appears in the Discovery feed

**Offline behavior:** If the device has no internet connection, the URL is saved as a placeholder release (URL only, no metadata). When internet becomes available, metadata is fetched automatically.

### 3.4 Preview Playback

Users can preview tracks from any release, exactly like the desktop app:

- Tap a track in the release detail view to start playback
- A **mini-player bar** appears above the tab bar showing:
  - Artwork thumbnail
  - Track name
  - Play/pause button
  - Progress indicator
- Swipe up on the mini-player for an **expanded player view** with:
  - Large artwork
  - Track name and artist
  - Scrubber / progress bar
  - Previous / Next track buttons (within the same release)
- Stream URLs are resolved on demand (identical to desktop behavior)
- Audio is cached locally after first play to reduce re-fetching

**Limitations:** Preview playback requires an internet connection (streams are fetched from source platforms). Previously cached tracks can play offline.

### 3.5 Tag Browsing and Assignment

Tags are shared between desktop and mobile. The tag system consists of categories (e.g., "Genre", "Mood", "Energy") each containing multiple tags with colors.

**Tag Browser (Tags tab):**
- Displays all tag categories as expandable sections
- Each category shows its tags with color indicators
- Tapping a tag filters the Discovery feed to show matching releases
- Multiple tags can be selected for combined filtering

**Tag Assignment (in Release Detail):**
- Tap "Tags" section in the release detail view
- Opens a tag picker showing all categories and tags
- Check/uncheck tags to assign or remove them
- Changes apply immediately

**Tag Management:**
- Tags are created and organized on the desktop app
- Mobile syncs the full tag/category structure from desktop
- Mobile can assign and remove tags from releases
- Mobile cannot create new tags or categories (desktop-only for now)

### 3.6 Discovery Playlists

Discovery releases can be organized into playlists (separate from library playlists):

- View existing discovery playlists
- Add/remove releases from playlists
- Reorder releases within a playlist (drag-and-drop)
- Playlist creation and deletion on mobile

### 3.7 Sync

The core feature that ties desktop and mobile together. Sync happens over the local WiFi network with no cloud involvement.

#### 3.7.1 Pairing

First-time setup between desktop and mobile:

1. Both devices must be on the same WiFi network
2. Desktop is running Crate (the sync service starts automatically)
3. On mobile, open Settings > "Connect Desktop"
4. Mobile scans the network and shows available desktops by name (e.g., "Max's MacBook Pro")
5. User taps the desktop name
6. Mobile displays a 6-digit confirmation code
7. Desktop shows a modal: "Crate Mobile wants to connect. Code: XXXXXX. Allow?"
8. User confirms on desktop
9. Pairing is complete — devices remember each other for future connections

**Unpairing:** Either device can unpair at any time from Settings. This revokes the connection token.

**Multiple devices:** A desktop can pair with multiple mobile devices. Each maintains independent sync state.

#### 3.7.2 Sync Behavior

Once paired, sync is designed to be invisible:

- **Auto-sync on open:** When the mobile app comes to foreground and the paired desktop is reachable, sync happens automatically
- **Manual sync:** Pull-to-refresh on the Discovery feed, or "Sync Now" button in Settings
- **Background sync:** Periodic sync when the app is in the background (subject to OS limits)
- **First sync:** Downloads the entire discovery collection from desktop (full pull)
- **Subsequent syncs:** Only exchanges changes since the last sync (delta sync)

#### 3.7.3 Sync Status

The Settings screen shows:

- **Connection status:** "Connected to [desktop name]" / "Desktop not found" / "Not paired"
- **Last synced:** Timestamp of the most recent successful sync
- **Sync indicator:** A subtle sync icon in the navigation bar animates during active sync

If sync fails (desktop unreachable, network error), the app continues working with local data. No error modals — just a status update in Settings.

#### 3.7.4 Conflict Resolution

When the same release is modified on both devices between syncs:

- The most recent edit wins (based on modification timestamp)
- This applies per-release, not per-field (the entire release state from the winning side is used)
- Tags are the exception: tag assignments are merged (union of both sides' tags)
- Deletions propagate: if one device deletes a release, it's deleted on the other after sync

Users do not see conflict resolution UI. It happens transparently.

#### 3.7.5 What Syncs

| Data | Syncs? | Notes |
|------|--------|-------|
| Releases (metadata) | Yes | Artist, title, label, date, notes, source URL |
| Tracks | Yes | Names, positions, durations |
| Tags & categories | Yes | Full structure from desktop; assignments bidirectional |
| Discovery playlists | Yes | Playlist membership and ordering |
| Artwork (URL-based) | Yes | URL reference syncs; image loaded from source |
| Artwork (user-uploaded) | Yes | File transferred during sync |
| Stream cache | No | Ephemeral; re-fetched on demand per device |
| Audio cache | No | Local to each device |
| Notes | Yes | Part of release metadata |

### 3.8 Settings

Minimal settings, focused on sync and cache management:

- **Sync section:**
  - Connected desktop name and status
  - Last sync timestamp
  - "Sync Now" button
  - "Pair Desktop" / "Unpair" button
- **Cache section:**
  - Audio cache size display
  - "Clear Audio Cache" button
- **About section:**
  - App version
  - Link to project website

---

## 4. Navigation and Screen Map

### Tab Bar (bottom)

```
[ Discover ]    [ Tags ]    [ Settings ]
```

A Playlists tab is added between Tags and Settings once playlist support ships.

### Screen Hierarchy

```
Discover (tab)
  |-- Discovery Feed (list)
  |     |-- Release Detail (expanded/full-screen)
  |     |     |-- Tag Picker (modal)
  |     |     |-- Edit Fields (inline)
  |     |-- Multi-Select Mode (batch actions bar)
  |-- Add Release (modal, from FAB)
  |     |-- URL Input
  |     |-- Metadata Preview
  |-- Mini Player (persistent bar above tabs)
        |-- Expanded Player (swipe up)

Tags (tab)
  |-- Category List (expandable sections)
  |     |-- Tag List (tappable filters)
  |-- Filtered Discovery Feed (reuses Discover feed)

Settings (tab)
  |-- Sync Status
  |-- Pair Desktop (flow)
  |     |-- Network Scan
  |     |-- Code Confirmation
  |-- Cache Management
  |-- About
```

---

## 5. User Flows

### 5.1 First Launch

1. App opens to an empty Discovery feed
2. A welcome message explains: "Connect to your desktop to sync your discoveries"
3. User taps "Connect Desktop" (or navigates to Settings)
4. Pairing flow begins (see 3.7.1)
5. After pairing, first sync pulls all data from desktop
6. Discovery feed populates with releases

### 5.2 Daily Use — Browse and Preview

1. User opens the app
2. Auto-sync runs in the background (if desktop reachable)
3. User scrolls through releases or searches/filters by tags
4. User taps a release to see details and track list
5. User taps a track to preview — mini-player appears
6. User continues browsing while audio plays

### 5.3 Add a Discovery from Mobile

1. User finds a Bandcamp link in a message or on social media
2. User copies the URL
3. Opens Crate Mobile, taps the FAB (+) button
4. Pastes the URL — metadata loads automatically
5. Optionally edits fields or assigns tags
6. Taps "Add" — release appears in feed
7. On next sync, release appears on desktop too

### 5.4 Add via Share Sheet

1. User is browsing Bandcamp in Safari
2. Taps the share button, selects "Crate"
3. App briefly opens (or processes in background on iOS)
4. Confirmation appears: "Release added: Artist — Title"
5. Release is in the feed, ready to preview

### 5.5 Tag and Organize

1. User opens a release detail view
2. Taps the tags section
3. Tag picker opens showing categories (Genre, Mood, etc.)
4. User checks "Techno" and "Dark"
5. Tags appear on the release immediately
6. User can also swipe to enter multi-select, then batch-assign tags

### 5.6 Offline Usage

1. User opens app with no WiFi
2. All previously synced releases are available instantly
3. User can browse, search, filter, edit, and delete
4. Adding a new release: user can enter URL (metadata fetches when internet returns)
5. Previewing cached tracks works; uncached tracks show "Available online only"
6. Settings shows "Desktop not found" — all changes are queued
7. When WiFi returns and desktop is reachable, sync happens automatically

---

## 6. Platform Requirements

### 6.1 Supported Platforms

- **iOS 16+** (primary target)
- **Android 10+** (secondary target, follows iOS launch)

iOS is prioritized because:
- macOS (primary desktop platform) to iOS has the best local sync story
- Bonjour/mDNS is native on Apple platforms
- The Tauri WebView (WKWebView) has already been validated for audio proxy playback
- AirDrop provides a natural fallback for file-based sync if needed

### 6.2 Performance Requirements

- App launch to interactive feed: < 1 second (from local DB)
- First sync (500 releases): < 10 seconds on WiFi
- Delta sync (< 50 changes): < 2 seconds
- Preview playback start: < 3 seconds (stream resolution + buffering)
- Search/filter response: < 100ms (local DB query)

### 6.3 Storage

- Base app size: < 50 MB
- Discovery data: ~5 KB per release (metadata + tracks), so 1000 releases ~ 5 MB
- Audio cache: user-managed, with clear option in Settings
- Artwork cache: loaded from URLs on demand, cached by the WebView

### 6.4 Network

- Sync: WiFi only (local network, not cellular)
- Metadata fetch: WiFi or cellular (fetching from Bandcamp/SoundCloud/etc.)
- Preview streams: WiFi or cellular
- Offline: full functionality except metadata fetch and uncached preview playback

### 6.5 Security and Privacy

- All local data encrypted at rest (SQLCipher)
- Sync traffic is local network only (never leaves WiFi)
- Pairing requires physical confirmation on both devices (code matching)
- No analytics, telemetry, or crash reporting
- No user accounts or personal data collection
- Paired device tokens stored securely in the app's encrypted database

---

## 7. Constraints and Risks

### 7.1 Constraints

- **No internet required for core usage** — browsing, searching, filtering, editing all work offline
- **No cloud dependency** — sync is strictly local WiFi, pairing is device-to-device
- **Desktop must be running** for sync to work — there's no intermediary
- **Same WiFi network required** for sync — no remote sync over the internet
- **Tag creation is desktop-only** — mobile can assign existing tags but not create new ones

### 7.2 Known Limitations

- **YouTube preview quality** is variable (depends on available streams)
- **SoundCloud streams** require periodic client_id refresh (handled automatically but may occasionally fail)
- **Large collections** (5000+ releases) may have slower first sync but should be fine for delta sync
- **Background sync** is limited by iOS/Android OS restrictions on background network activity

### 7.3 Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| YouTube n-param solver doesn't work on mobile WebView | Low | High (no YT previews) | Validate early in development; fallback to no YT preview |
| mDNS unreliable on certain Android devices | Medium | Medium (manual IP entry fallback) | Add manual IP:port entry as alternative to mDNS |
| iOS Share Extension complexity | Medium | Low (feature can ship later) | Defer to Phase 3 |
| Large first sync timeout on slow WiFi | Low | Low (retry mechanism) | Chunk first sync into batches with progress indicator |
| Audio proxy doesn't work on Android WebView | Low | High (no playback) | Validate early; Android WebView is less restrictive than WKWebView |

---

## 8. Phased Delivery

### Phase 1 — Read and Preview (MVP)

The minimum viable companion: browse your desktop discoveries from your phone with preview playback.

**Features:**
- Discovery feed with search and tag filtering
- Release detail view (read-only)
- Preview playback with mini-player
- One-way sync: pull from desktop
- Pairing flow
- Settings (sync status, cache management)
- iOS only

**User value:** "I can check my discovery list and preview tracks from my phone."

### Phase 2 — Create and Edit

Full bidirectional sync — the phone becomes a first-class discovery tool.

**Features:**
- Add releases from mobile (URL paste + auto-metadata)
- Edit release fields (artist, title, label, date, notes)
- Assign and remove tags
- Delete releases
- Bidirectional sync (push + pull)
- Conflict resolution (last-write-wins)

**User value:** "I can save releases I find on my phone and they show up on my desktop."

### Phase 3 — Companion Features

Quality-of-life features that make mobile a natural extension of the desktop workflow.

**Features:**
- Share sheet integration (add releases directly from browser)
- Discovery playlists (Playlists tab)
- Background sync
- Artwork sync for user-uploaded artwork
- Offline URL queue (save URL now, fetch metadata later)
- Bulk import from artist/label pages
- Android support

**User value:** "Sharing a link to Crate from my browser is my default workflow for saving music."

### Phase 4 — Polish

Refinements and platform-specific enhancements.

**Features:**
- Animations and gesture refinements
- iOS home screen widget (latest discoveries)
- Handoff support (browse on desktop, continue on mobile)
- Manual IP entry for sync (fallback when mDNS fails)
- Tag creation on mobile

**User value:** "The app feels native and integrated into my phone's ecosystem."

---

## 9. Success Metrics

Since Crate has no telemetry, success is measured qualitatively:

- **Adoption:** Users who pair their mobile device with their desktop
- **Engagement:** Frequency of mobile app opens (self-reported or inferred from sync frequency)
- **Utility of bidirectional sync:** Number of releases created on mobile that appear on desktop
- **Share sheet usage:** How often users add releases via the system share sheet
- **Sync reliability:** User-reported issues with sync failures or data inconsistencies
- **Offline satisfaction:** Whether users find the offline experience complete enough

---

## 10. Open Questions

1. **Should tag creation be available on mobile?** Currently scoped as desktop-only, but mobile users may want to create tags on the fly. This adds complexity to the sync model (new tag categories need to sync both ways). Deferred to Phase 4.

2. **Should there be a "Recent" or "Activity" view?** A chronological feed showing sync events, recently added releases, and modifications. Could help users understand what changed after a sync. Not in current scope.

3. **What about tablet layouts?** iPad and Android tablet could show a split-view (feed on left, detail on right). Worth considering but not in initial scope.

4. **Should mobile support multiple paired desktops?** A user with a studio machine and a laptop. Currently scoped as one-to-one pairing. Multiple desktop pairing adds complexity (which desktop is the source of truth?).

5. **What happens when desktop is reinstalled or data is reset?** Mobile has data that desktop doesn't. Need to define whether mobile can serve as a backup source for discovery data. Not in current scope but worth planning for.
