import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { AppSettings, AudioDevice } from '../types'

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

/**
 * Set the desktop webview page zoom (snapped to the zoom ladder) and persist it.
 * Desktop-only command; resolves to the applied level.
 */
export async function setUiZoom(level: number): Promise<number> {
	return invoke<number>('set_ui_zoom', { level })
}

/**
 * Step the desktop webview page zoom up (+1) or down (-1) the ladder.
 * Desktop-only command; resolves to the applied level.
 */
export async function stepUiZoom(delta: number): Promise<number> {
	return invoke<number>('step_ui_zoom', { delta })
}

/**
 * Listen for zoom changes applied by the backend (native menu shortcuts)
 */
export async function onUiZoomChanged(handler: (level: number) => void): Promise<UnlistenFn> {
	return listen<number>('ui-zoom-changed', (event) => {
		handler(event.payload)
	})
}
