import { invoke } from '@tauri-apps/api/core'
import type { AppSettings, AudioDevice } from '$lib/types'

/**
 * Get all application settings
 */
export async function getSettings(): Promise<AppSettings> {
	return invoke<AppSettings>('get_settings')
}

/**
 * Set a single setting by key
 */
export async function setSetting(key: string, value: string): Promise<void> {
	return invoke<void>('set_setting', { key, value })
}

/**
 * Get available audio output devices
 */
export async function getAudioDevices(): Promise<AudioDevice[]> {
	return invoke<AudioDevice[]>('get_audio_devices')
}

/**
 * Set the audio output device
 */
export async function setAudioDevice(deviceName: string | null): Promise<void> {
	return invoke<void>('set_audio_device', { deviceName })
}
