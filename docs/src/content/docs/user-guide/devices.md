---
title: Device Management
description: Monitor and manage connected storage devices
---

Crate can monitor USB drives and external storage devices. Use this feature to track connected drives and their storage capacity.

## Device List

The Devices section in the sidebar shows all connected removable storage devices.

### Visible Information

For each device, you can see:

| Property | Description |
|----------|-------------|
| Name | Device name or volume label |
| Mount Point | Where the device is mounted |
| Storage | Used and total capacity |
| Free Space | Available space |
| File System | Format type (APFS, NTFS, FAT32, exFAT, ext4, etc.) |

### Device Types

Crate detects:
- USB flash drives
- External hard drives
- External SSDs
- SD cards and memory cards
- Other removable media

Internal drives are typically not shown in the device list.

## Monitoring Devices

### Automatic Detection

Crate automatically detects when devices are:
- **Connected** - Added to the device list
- **Disconnected** - Removed from the device list

The device list updates in real-time as you connect and disconnect devices.

### Refreshing

The device list refreshes automatically. If a device doesn't appear immediately:
1. Wait a few seconds for detection
2. Check that the device is properly connected
3. Verify the device mounts in your file manager

## Storage Information

### Reading Storage

Each device shows storage at a glance:

```
USB Drive (32 GB)
├── Used: 24.5 GB
├── Free: 7.5 GB
└── Total: 32 GB
```

### Storage Bar

A visual bar shows the proportion of used vs. free space:
- **Filled portion** - Used space
- **Empty portion** - Free space

### Low Space Warning

Devices with limited free space may be highlighted to warn you before running out of room.

## Ejecting Devices

Safely eject devices to prevent data corruption.

### How to Eject

1. Find the device in the sidebar
2. Right-click the device
3. Select **Eject**
4. Wait for confirmation
5. Physically disconnect the device

### Why Eject?

Ejecting ensures:
- All pending writes complete
- File system is properly closed
- No data corruption occurs

Never disconnect a device without ejecting if you've written to it.

### Eject Failures

If ejection fails:
- Close any files open from the device
- Close file manager windows showing the device
- Try ejecting again
- If still failing, check for background processes accessing the device

## Use Cases

### Managing Music Collection

Track available space on your USB drives:
1. Connect your music drives
2. View storage in Crate
3. Plan which tracks to add based on available space

### Export Preparation

Before exporting music for DJ equipment:
1. Check the target drive's free space
2. Ensure enough room for your export
3. Export tracks
4. Safely eject before moving to DJ equipment

### Multiple Drives

If you use multiple USB drives:
- Each appears in the device list
- Easily compare storage across drives
- Choose the best drive for your current needs

## Troubleshooting

### Device Not Appearing

If a connected device doesn't show in Crate:
1. Check physical connection
2. Verify the device mounts in your OS file manager
3. Check if the file system is supported by your OS
4. Try a different USB port

### Incorrect Storage Reading

If storage information seems wrong:
1. Eject and reconnect the device
2. Check storage in your OS file manager to verify
3. The device may need repair if values are inconsistent

### Cannot Eject

If ejection fails repeatedly:
1. Close all applications that might access the drive
2. Close any open file browser windows
3. Close any terminal sessions in that path
4. Try ejecting from your OS instead
