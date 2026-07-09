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

// Reloading every touched store per `cloud-sync-merged` event re-fetches the release list
// (thousands of rows with nested tracks/tags) once per sync cycle — and back-to-back cycles
// (foreground returns, background refresh) would repeat it. Coalesce: accumulate flags and
// flush on a trailing debounce with a max-wait ceiling.
const RELOAD_DEBOUNCE_MS = 750
const RELOAD_MAX_WAIT_MS = 4000

// User actions take priority over sync-triggered reloads: never start a heavy store reload
// while the user is actively touching the screen — wait for a quiet gap, up to a cap so the
// reload can't be starved forever by continuous scrolling.
const INTERACTION_QUIET_MS = 500
const INTERACTION_DEFER_CAP_MS = 8000

const pendingFlags = new Set<string>()
let debounceTimer: ReturnType<typeof setTimeout> | null = null
let maxWaitTimer: ReturnType<typeof setTimeout> | null = null
let lastInteractionAt = 0
let flushDeferredSince = 0

function bumpInteraction(): void {
	lastInteractionAt = Date.now()
}

function flushPendingReloads(): void {
	// Defer while the user is mid-interaction (bounded): reschedule and check again after
	// the quiet window. The timers are left as-is here — this reschedule IS the new timer.
	const now = Date.now()
	if (now - lastInteractionAt < INTERACTION_QUIET_MS) {
		if (!flushDeferredSince) flushDeferredSince = now
		if (now - flushDeferredSince < INTERACTION_DEFER_CAP_MS) {
			if (debounceTimer) clearTimeout(debounceTimer)
			debounceTimer = setTimeout(flushPendingReloads, INTERACTION_QUIET_MS)
			return
		}
	}
	flushDeferredSince = 0

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
	window.addEventListener('touchstart', bumpInteraction, { passive: true, capture: true })
	window.addEventListener('touchmove', bumpInteraction, { passive: true, capture: true })
	window.addEventListener('pointerdown', bumpInteraction, { passive: true, capture: true })

	const unlisten = await listen<string[]>('cloud-sync-merged', (event) => {
		scheduleReloadForBuckets(event.payload)
	})
	return () => {
		window.removeEventListener('touchstart', bumpInteraction, { capture: true })
		window.removeEventListener('touchmove', bumpInteraction, { capture: true })
		window.removeEventListener('pointerdown', bumpInteraction, { capture: true })
		if (debounceTimer) clearTimeout(debounceTimer)
		if (maxWaitTimer) clearTimeout(maxWaitTimer)
		debounceTimer = null
		maxWaitTimer = null
		pendingFlags.clear()
		unlisten()
	}
}
