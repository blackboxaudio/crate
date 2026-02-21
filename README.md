# Crate

[![Build](https://github.com/blackboxaudio/crate/actions/workflows/ci.build.yml/badge.svg)](https://github.com/blackboxaudio/crate/actions/workflows/ci.build.yml)
[![Lint](https://github.com/blackboxaudio/crate/actions/workflows/ci.lint.yml/badge.svg)](https://github.com/blackboxaudio/crate/actions/workflows/ci.lint.yml)
[![Version](https://img.shields.io/github/v/release/blackboxaudio/crate?label=version)](https://github.com/blackboxaudio/crate/releases)

> Cross-platform DJ library manager with music discovery, track analysis, and USB export 📦

---

## Overview

Crate is a cross-platform desktop application for managing DJ audio libraries. Built for those who demand speed and control over their collections without the overhead of full DJ software suites.

Existing library management tools are buried inside bloated applications, locked to proprietary ecosystems, or painfully slow. Crate exists to solve this.

### Philosophy

- **Speed first.** Native performance through Rust and Tauri. No Electron. No compromise.
- **Precision.** Every feature exists for a reason. Nothing more.
- **Control.** Your library, your rules. No cloud sync. No telemetry. Local-first.

### Features

- **Library management** — Import and organize audio files with automatic metadata extraction, search, and filtering
- **Tagging** — Categorize tracks with a flexible tag system for fast filtering
- **Playlists** — Build playlists manually or with smart rules
- **Track analysis** — Waveform generation, key detection, BPM analysis, and energy profiling
- **Music discovery** — Browse and preview releases from Bandcamp and SoundCloud
- **Audio playback** — Preview tracks with waveform display and cue point management
- **USB export** — Export to Pioneer CDJ/XDJ devices with full rekordbox database generation
- **Device sync** — Detect connected USB devices and sync library changes incrementally
- **Metadata editing** — Edit track metadata in bulk or individually
- **Localization** — Available in 11 languages (EN, JA, NL, FR, DE, ES, IT, SV, KO, PT, ZH)

---

## Getting Started

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
- **macOS** — `.dmg`, `.app`
- **Windows** — `.msi`, `.exe`

---

## Commands

| Command | Description |
|---------|-------------|
| `yarn dev` | Start Tauri dev mode with hot reload |
| `yarn build` | Production build |
| `yarn build:staging` | Staging build with devtools |
| `yarn check` | Run Cargo and Svelte type checking |
| `yarn check:cargo` | Rust type checking only |
| `yarn check:svelte` | Svelte/TypeScript checking only |
| `yarn format` | Format all code (TypeScript/Svelte + Rust) |
| `yarn lint` | Lint all code (ESLint + Clippy) |

## Tech Stack

| Layer | Technology |
|-------|------------|
| Framework | [Tauri v2](https://v2.tauri.app/) |
| Frontend | [SvelteKit](https://svelte.dev/) + TypeScript |
| Backend | Rust |
| Database | SQLite ([rusqlite](https://github.com/rusqlite/rusqlite)) |
| Audio | [rodio](https://github.com/RustAudio/rodio) (playback), [symphonia](https://github.com/pdeljanov/Symphonia) (decoding), [lofty](https://github.com/Serial-ATA/lofty-rs) (metadata) |
| Analysis | [stratum-dsp](https://github.com/blackboxaudio/stratum) (waveform, key, tempo, energy) |

## Roadmap

This project is under active development. Planned features and improvements are tracked in the [repository issues](https://github.com/blackboxaudio/crate/issues).

---
