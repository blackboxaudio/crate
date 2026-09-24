import { readable } from 'svelte/store'
import { appDataDir } from '@tauri-apps/api/path'

/**
 * The app data directory, resolved once at load. Starts `null` and fills in asynchronously
 * (the path API is async). Used to build Tauri asset URLs for on-disk cached discovery
 * artwork so covers render offline / in airplane mode. Until it resolves, the artwork
 * resolver falls back to the remote URL.
 */
export const mobileAppDataDir = readable<string | null>(null, (set) => {
	void appDataDir()
		.then((dir) => set(dir))
		.catch(() => set(null))
})
