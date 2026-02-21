# Crate

[![Build](https://github.com/blackboxaudio/crate/actions/workflows/ci.build.yml/badge.svg)](https://github.com/blackboxaudio/crate/actions/workflows/ci.build.yml)
[![Lint](https://github.com/blackboxaudio/crate/actions/workflows/ci.lint.yml/badge.svg)](https://github.com/blackboxaudio/crate/actions/workflows/ci.lint.yml)

> Cross-platform DJ library manager with music discovery, track analysis, and USB export

---

## 🎧 Overview

Crate is a cross-platform desktop application for managing DJ audio libraries. It handles everything from discovery, organization, analysis, and USB export.

## 🎵 Features

- **Library management** - Import and organize audio files (MP3, FLAC, WAV, AIFF, M4A/AAC) with automatic metadata extraction, search, and filtering
- **Tagging** - Categorize tracks with a flexible tag system for fast filtering
- **Playlists** - Build playlists manually or with smart rules
- **Track analysis** - Waveform generation, key detection, BPM analysis, and energy profiling
- **Music discovery** - Browse and preview releases from Bandcamp, SoundCloud, YouTube, and Discogs
- **Audio playback** - Preview tracks with waveform display and cue point management
- **USB export** - Export to Pioneer CDJ/XDJ devices with full Rekordbox database generation
- **Device sync** - Detect connected USB devices and sync library changes incrementally
- **Metadata editing** - Edit track metadata in bulk or individually
- **Customization** - Themes, accent colors, and font preferences
- **Localization** - Available in 11 languages (EN, JA, NL, FR, DE, ES, IT, SV, KO, PT, ZH)
- **Auto-updates** - Stay on the latest version with minimal effort

## 🚀 Getting Started

### Prerequisites

- **Node.js** 22.x or higher
- **Yarn** (package manager)
- **Rust** nightly toolchain

For Tauri development dependencies, see the [Tauri v2 prerequisites guide](https://v2.tauri.app/start/prerequisites/).

### Installation

Clone the repository:

```bash
git clone https://github.com/blackboxaudio/crate.git
cd crate
```

Install dependencies:

```bash
yarn install
```

### Development

Start the development server with hot reload:

```bash
yarn dev
```

This launches both the Vite dev server (port 1420) and the Tauri application window.

### Building

Build for production:

```bash
yarn build
```

Build for staging (with devtools):

```bash
yarn build:staging
```

Output binaries are placed in `src-tauri/target/release/bundle/`.

Platform targets:
- **macOS** - `.dmg`, `.app`
- **Windows** - `.msi`, `.exe`

## 🔗 Links

- [Website & Downloads](https://crate.bbx-audio.com)
- [Issues](https://github.com/blackboxaudio/crate/issues)
- [Releases](https://github.com/blackboxaudio/crate/releases)

## 📄 License

This project is source-available under the [PolyForm Shield License 1.0.0](LICENSE). You can read, learn from, and contribute to the code, but you can't use it to build a competing product.

## 🗺️ Roadmap

This project is under active development. Planned features and improvements are tracked in the [repository issues](https://github.com/blackboxaudio/crate/issues).
