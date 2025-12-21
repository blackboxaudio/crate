import { invoke } from '@tauri-apps/api/core'
import type { UsbDevice } from '$lib/types'

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
