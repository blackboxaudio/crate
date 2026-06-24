---
title: Getting Started
description: Get up and running with Crate in minutes
---

This guide will help you get started with Crate and import your first tracks.

## Installation

Download the latest version of Crate for your operating system from the [releases page](https://github.com/blackboxaudio/crate/releases).

### macOS

1. Download the `.dmg` file
2. Open the disk image
3. Drag Crate to your Applications folder
4. Open Crate from Applications

### Windows

1. Download the `.exe` installer
2. Run the installer
3. Follow the installation prompts
4. Launch Crate from the Start menu

### Linux

1. Download the `.AppImage` file
2. Make it executable: `chmod +x Crate-*.AppImage`
3. Run the AppImage

### iOS (TestFlight)

1. Install [TestFlight](https://apps.apple.com/app/testflight/id899247664) from the App Store
2. Open the Crate TestFlight public link (shared by the developer)
3. Tap **Accept** and then **Install**
4. Open Crate from your Home Screen

TestFlight builds are available worldwide (including EU and Japan). When the app is live on the App Store, you can install it directly from there instead.

### Android

1. Download the `Crate_<version>_android.apk` and its `.sha256` file from the [releases page](https://github.com/blackboxaudio/crate/releases)
2. Verify the download integrity:
   ```bash
   sha256sum -c Crate_<version>_android.apk.sha256
   ```
3. Enable "Install from unknown sources" for your browser (Settings → Apps → Special access → Install unknown apps)
4. Open the APK to install
5. Launch Crate from your app drawer

## First Launch

When you first open Crate, you'll see an empty library. The interface consists of:

- **Sidebar** (left) - Library, playlists, folders, and tags
- **Main area** (center) - Track list
- **Inspector** (right, toggleable) - Track details and editing
- **Player** (bottom) - Playback controls

## Importing Your Music

There are two ways to add tracks to your library:

### Drag and Drop

Simply drag audio files or folders from your file manager onto the Crate window. Crate will:

1. Scan each file for metadata
2. Extract album artwork (if embedded)
3. Add the tracks to your library

### Import Dialog

1. Click the **Import** button in the toolbar (or use the File menu)
2. Select files or folders
3. Crate will import all supported audio files

**Supported formats:** MP3, FLAC, WAV, AIFF, M4A, AAC

## Basic Workflow

### Browsing Your Library

- Click **Library** in the sidebar to see all tracks
- Use the **search bar** to filter tracks by title, artist, or album
- Click column headers to sort by that field
- Double-click a track to play it

### Creating Playlists

1. Right-click in the sidebar (or use the toolbar menu)
2. Select **New Playlist**
3. Name your playlist
4. Drag tracks from the library onto the playlist

### Tagging Tracks

1. Select one or more tracks
2. Right-click and choose **Tags**
3. Select tags to apply (or create new ones)
4. Use the tag sidebar to filter your library

### Playing Music

- **Double-click** a track to play it
- Use **Space** to pause/resume
- Use the player controls at the bottom for seek, volume, and stop

## Next Steps

Now that you have the basics, explore more:

- [Library Management](/crate/user-guide/library-management/) - Advanced import options and metadata
- [Playlists & Folders](/crate/user-guide/playlists-and-folders/) - Organize with hierarchies
- [Tagging](/crate/user-guide/tagging/) - Create a flexible categorization system
- [Settings](/crate/user-guide/settings/) - Customize the look and feel
- [Keyboard Shortcuts](/crate/reference/keyboard-shortcuts/) - Work faster
