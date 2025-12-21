---
title: Settings
description: Customize Crate to your preferences
---

Crate offers several customization options. Access settings with `Cmd+,` (macOS) or `Ctrl+,` (Windows/Linux).

## Opening Settings

- Press `Cmd+,` / `Ctrl+,`
- Or click the gear icon in the toolbar

Settings are organized into tabs: Appearance, Sound, and About.

## Appearance

Customize how Crate looks.

### Theme

Choose your preferred color scheme:

| Theme | Description |
|-------|-------------|
| **Light** | Light background with dark text |
| **Dark** | Dark background with light text (default) |
| **System** | Follows your operating system preference |

The theme affects all UI elements including the sidebar, track list, and player.

### Accent Color

The accent color is used for selections, buttons, and interactive elements.

Choose from 10 colors:
- **Blue** (default)
- **Indigo**
- **Violet**
- **Purple**
- **Pink**
- **Rose**
- **Orange**
- **Amber**
- **Emerald**
- **Teal**

### Font

Choose your preferred font for the interface:

| Font | Style |
|------|-------|
| **IBM Plex Mono** | Monospace (default) |
| **JetBrains Mono** | Monospace |
| **Fira Code** | Monospace |
| **Inter** | Sans-serif |
| **Open Sans** | Sans-serif |

The font is used throughout the application for all text.

## Sound

Configure audio settings.

### Output Device

Select which audio device Crate uses for playback.

Available options:
- **System Default** - Uses your system's default audio output
- **Specific devices** - Any detected audio output devices

Detected devices include:
- Built-in speakers and headphones
- USB audio interfaces
- Bluetooth audio devices
- Virtual audio devices

### Changing Devices

1. Open Settings
2. Go to the **Sound** tab
3. Select a device from the dropdown
4. Close settings

The change applies to the next track played.

## About

View information about Crate.

| Field | Description |
|-------|-------------|
| Version | Current Crate version number |
| Environment | Build type (Development, Staging, Production) |
| Data Directory | Location of Crate's database and cache |

## Persisted Settings

Crate remembers your preferences automatically:

### Application Settings
- Theme
- Accent color
- Font family
- Audio output device

### UI State
- Sidebar width
- Inspector visibility
- Expanded folders and categories
- Sort column and direction
- Window size and position

All settings are stored locally. They persist across application restarts.

## Resetting Settings

To reset settings to defaults:

1. Quit Crate
2. Navigate to the data directory (shown in About)
3. Delete the settings file
4. Restart Crate

This resets all application settings while preserving your library.

## Tips

### Dark Theme + Blue Accent

The default combination offers high contrast and is easy on the eyes for extended sessions.

### Monospace Fonts

Monospace fonts (IBM Plex Mono, JetBrains Mono, Fira Code) work well for:
- Consistent column alignment
- Clear character distinction
- Technical aesthetic

### Sans-Serif Fonts

Sans-serif fonts (Inter, Open Sans) offer:
- Warmer appearance
- More traditional look
- Slightly better readability at small sizes
