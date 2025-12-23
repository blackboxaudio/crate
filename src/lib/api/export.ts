import { invoke } from '@tauri-apps/api/core'
import type { ExportRequest, ExportResult, DeviceExport, ExportCheckpoint } from '$lib/types'

/**
 * Export playlists to a USB device
 */
export async function exportToDevice(request: ExportRequest): Promise<ExportResult> {
	return invoke('export_playlists', { request })
}

/**
 * Get all exports for a device
 */
export async function getDeviceExports(deviceId: string): Promise<DeviceExport[]> {
	return invoke('get_device_exports', { deviceId })
}

/**
 * Cancel the current export operation
 */
export async function cancelExport(): Promise<void> {
	return invoke('cancel_export')
}

/**
 * Clean up a failed export by removing copied files
 */
export async function cleanupFailedExport(deviceId: string, mountPoint: string): Promise<void> {
	return invoke('cleanup_failed_export', { deviceId, mountPoint })
}

/**
 * Get a pending checkpoint for a device
 */
export async function getPendingCheckpoint(deviceId: string): Promise<ExportCheckpoint | null> {
	return invoke('get_pending_checkpoint', { deviceId })
}

/**
 * Delete a checkpoint
 */
export async function deleteCheckpoint(checkpointId: string): Promise<void> {
	return invoke('delete_checkpoint', { checkpointId })
}

/**
 * Resume a previously interrupted export
 */
export async function resumeExport(deviceId: string, mountPoint: string): Promise<ExportResult> {
	return invoke('resume_export', { deviceId, mountPoint })
}
