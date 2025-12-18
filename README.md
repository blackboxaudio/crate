# Crate

[![Build](https://github.com/blackboxaudio/crate/actions/workflows/ci.build.yml/badge.svg)](https://github.com/blackboxaudio/crate/actions/workflows/ci.build.yml)
[![Lint](https://github.com/blackboxaudio/crate/actions/workflows/ci.lint.yml/badge.svg)](https://github.com/blackboxaudio/crate/actions/workflows/ci.lint.yml)

> Convenient library management for DJs, sound designers, and collectors. Fast, precise, no bloat.

---

## Overview

Crate is a cross-platform desktop application for managing DJ audio libraries. Built for those who demand speed and control over their collections without the overhead of full DJ software suites.

The problem is simple: existing library management tools are buried inside bloated applications, locked to proprietary ecosystems, or painfully slow. Crate exists to solve this.

### Philosophy

- **Speed first.** Native performance through Rust. No Electron. No compromise.
- **Precision.** Every feature exists for a reason. Nothing more.
- **Control.** Your library, your rules. No cloud sync. No telemetry. Local-first.

### What it does

- Import and organize audio files with automatic metadata extraction
- Tag tracks with a flexible categorization system
- Build playlists manually or dynamically with smart rules
- Preview tracks with waveform display and cue point management
- Search and filter across your entire collection instantly

---

## Getting Started

### Prerequisites

- **Node.js** 22.x or higher
- **Yarn** (package manager)
- **Rust** stable toolchain

For Tauri development dependencies, see the [Tauri prerequisites guide](https://tauri.app/start/prerequisites/).

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
yarn tauri dev
```

This launches both the Vite dev server (port 1420) and the Tauri application window.

### Building

Build for production:

```bash
yarn tauri build
```

Output binaries are placed in `src-tauri/target/release/bundle/`.

Platform targets:
- **macOS** — `.dmg`, `.app`
- **Windows** — `.msi`, `.exe`

---

## Commands

| Command | Description |
|---------|-------------|
| `yarn dev` | Start Vite development server |
| `yarn build` | Build frontend for production |
| `yarn preview` | Preview production build |
| `yarn tauri` | Access Tauri CLI |
| `yarn check` | Run Svelte type checking |
| `yarn check:watch` | Watch mode type checking |
| `yarn format` | Format code (fix) |
| `yarn format:check` | Check code formatting |
| `yarn lint` | Lint code (fix) |
| `yarn lint:check` | Check linting |

## Roadmap

This project is under active development. Current version: **0.1.0**

Planned features and improvements are tracked in the repository issues.

---
