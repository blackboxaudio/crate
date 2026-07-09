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

// During the initial restore of a large account, `cloud-sync-merged` fires once per merge batch —
// reloading every touched store on each event re-fetches the ENTIRE release list (thousands of
// rows with nested tracks/tags) over IPC dozens of times in quick succession, which can pressure
// the WKWebView content process into a jetsam kill. Coalesce: accumulate flags and flush on a
// trailing debounce, with a max-wait so a long merge stream still paints progressively.
const RELOAD_DEBOUNCE_MS = 750
const RELOAD_MAX_WAIT_MS = 4000

const pendingFlags = new Set<string>()
let debounceTimer: ReturnType<typeof setTimeout> | null = null
let maxWaitTimer: ReturnType<typeof setTimeout> | null = null

function flushPendingReloads(): void {
	if (debounceTimer) clearTimeout(debounceTimer)
	if (maxWaitTimer) clearTimeout(maxWaitTimer)
	debounceTimer = null
	maxWaitTimer = null

	if (pendingFlags.has('discovery')) discoveryStore.loadReleases()
	if (pendingFlags.has('playlists')) playlistsStore.load()
	if (pendingFlags.has('tags')) tagsStore.load()
	if (pendingFlags.has('settings')) settingsStore.load()
	pendingFlags.clear()
}

function scheduleReloadForBuckets(buckets: string[]): void {
	for (const bucket of buckets) {
		const flags = BUCKET_FLAGS[bucket as keyof typeof BUCKET_FLAGS]
		if (!flags) continue
		for (const flag of flags.split(',')) pendingFlags.add(flag)
	}
	if (pendingFlags.size === 0) return

	if (debounceTimer) clearTimeout(debounceTimer)
	debounceTimer = setTimeout(flushPendingReloads, RELOAD_DEBOUNCE_MS)
	if (!maxWaitTimer) {
		maxWaitTimer = setTimeout(flushPendingReloads, RELOAD_MAX_WAIT_MS)
	}
}

export async function setupCloudSyncMergeListener(): Promise<UnlistenFn> {
	const unlisten = await listen<string[]>('cloud-sync-merged', (event) => {
		scheduleReloadForBuckets(event.payload)
	})
	return () => {
		if (debounceTimer) clearTimeout(debounceTimer)
		if (maxWaitTimer) clearTimeout(maxWaitTimer)
		debounceTimer = null
		maxWaitTimer = null
		pendingFlags.clear()
		unlisten()
	}
}
