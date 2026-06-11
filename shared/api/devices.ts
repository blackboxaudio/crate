import { invoke } from '@tauri-apps/api/core'
import type { UsbDevice } from '../types'

/**
 * Get all connected removable USB devices
 */
export async function getDevices(): Promise<UsbDevice[]> {
	return invoke<UsbDevice[]>('get_devices')
}

/**
 * Eject a device by its mount point
 */
export async function ejectDevice(mountPoint: string): Promise<void> {
	return invoke<void>('eject_device', { mountPoint })
}

/**
 * Reformat a device to FAT32 with the given volume name
 * Requires elevated privileges (system password prompt)
 */
export async function reformatDevice(mountPoint: string, volumeName: string): Promise<void> {
	return invoke<void>('reformat_device', { mountPoint, volumeName })
}
