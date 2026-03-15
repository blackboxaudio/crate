import { get } from 'svelte/store'
import { playerStore } from '$shared/stores/player'
import { playlistsStore } from '$shared/stores/playlists'
import { settingsStore } from '$shared/stores/settings'
import { missingTracksStore } from '$lib/stores/missingTracks'
import { syncStore } from '$lib/stores/sync'
import { appStore } from '$lib/stores/app'
import { rebuildMenu } from '$shared/api/app'

function getAppName(): string {
	const appState = get(appStore)
	const environment = appState.info?.environment ?? 'development'
	if (environment === 'production') {
		return 'Crate'
	}
	if (environment === 'development') {
		return 'Crate Dev'
	}
	const suffix = environment.charAt(0).toUpperCase() + environment.slice(1)
	return `Crate ${suffix}`
}

/**
 * Register desktop-specific hooks on shared stores.
 * Must be called before any store's load() method.
 */
export function registerDesktopStoreHooks() {
	playerStore.registerHooks({
		onTrackMissing: (trackId) => {
			missingTracksStore.markMissing(trackId)
		},
	})

	playlistsStore.registerHooks({
		onPlaylistChanged: (playlistIds) => {
			syncStore.notifyPlaylistChanges(playlistIds)
		},
	})

	settingsStore.registerHooks({
		getAppName,
		onLanguageChanged: async () => {
			try {
				await rebuildMenu(settingsStore.getMenuTranslations())
			} catch (error) {
				console.error('Failed to rebuild menu:', error)
			}
		},
	})
}
