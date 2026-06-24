import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { discoveryStore } from '$shared/stores/discovery'
import { tagsStore } from '$shared/stores/tags'
import { playlistsStore } from '$shared/stores/playlists'
import { settingsStore } from '$shared/stores/settings'

const BUCKET_FLAGS = {
	discovery_releases: 'discovery',
	discovery_tracks: 'discovery',
	discovery_release_tags: 'discovery,tags',
	discovery_release_sources: 'discovery',
	playlist_discovery_releases: 'discovery,playlists',
	tags: 'tags,discovery',
	tag_categories: 'tags,discovery',
	playlists: 'playlists',
	playlist_tracks: 'playlists',
	settings: 'settings',
} as const

function reloadStoresForBuckets(buckets: string[]): void {
	let discovery = false
	let playlists = false
	let tags = false
	let settings = false

	for (const bucket of buckets) {
		const flags = BUCKET_FLAGS[bucket as keyof typeof BUCKET_FLAGS]
		if (!flags) continue
		if (flags.includes('discovery')) discovery = true
		if (flags.includes('playlists')) playlists = true
		if (flags.includes('tags')) tags = true
		if (flags.includes('settings')) settings = true
	}

	if (discovery) discoveryStore.loadReleases()
	if (playlists) playlistsStore.load()
	if (tags) tagsStore.load()
	if (settings) settingsStore.load()
}

export async function setupCloudSyncMergeListener(): Promise<UnlistenFn> {
	return listen<string[]>('cloud-sync-merged', (event) => {
		reloadStoresForBuckets(event.payload)
	})
}
