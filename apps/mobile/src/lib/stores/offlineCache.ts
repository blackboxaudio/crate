import { derived, writable } from 'svelte/store'
import { getCachedReleaseStates } from '$shared/api/discovery'

// Bulk audio-cache state for the UI: which releases are fully downloaded (playable offline) and
// which are pinned ("Download for Offline"). Refreshed on boot, after download/remove/clear
// actions, and on the backend's `discovery-cache-changed` event (debounced by the +layout
// listener). Kept as id-Sets so row badges and the Downloaded filter stay O(1) per release.

interface OfflineCacheState {
	/** Releases whose every track's audio is on disk (fully offline-ready). */
	fullyCached: Set<string>
	/** Releases with at least one pinned (explicitly downloaded) track. */
	pinned: Set<string>
	loaded: boolean
}

function createOfflineCacheStore() {
	const { subscribe, set } = writable<OfflineCacheState>({
		fullyCached: new Set(),
		pinned: new Set(),
		loaded: false,
	})

	let inFlight: Promise<void> | null = null

	return {
		subscribe,
		/** Refetch the bulk cached-state; concurrent calls share one in-flight request. */
		refresh(): Promise<void> {
			if (inFlight) return inFlight
			inFlight = getCachedReleaseStates()
				.then((states) => {
					const fullyCached = new Set<string>()
					const pinned = new Set<string>()
					for (const s of states) {
						if (s.fully_cached) fullyCached.add(s.release_id)
						if (s.pinned) pinned.add(s.release_id)
					}
					set({ fullyCached, pinned, loaded: true })
				})
				.catch(() => {
					// Startup races / transient failures: stale badge state beats an error surface.
				})
				.finally(() => {
					inFlight = null
				})
			return inFlight
		},
	}
}

export const offlineCacheStore = createOfflineCacheStore()

/** Releases whose every track's audio is cached on disk — playable in airplane mode. */
export const fullyCachedIds = derived(offlineCacheStore, ($s) => $s.fullyCached)
