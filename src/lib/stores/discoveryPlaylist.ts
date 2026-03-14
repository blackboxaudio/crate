import { writable, derived } from 'svelte/store'
import type { DiscoveryRelease } from '$lib/types'
import { SvelteMap } from 'svelte/reactivity'

// =============================================================================
// State
// =============================================================================

interface DiscoveryPlaylistState {
	releases: DiscoveryRelease[]
}

const initialState: DiscoveryPlaylistState = {
	releases: [],
}

// Cache is external to store state — it's a simple map that persists across view switches
const cache = new SvelteMap<string, DiscoveryRelease[]>()

// =============================================================================
// Store
// =============================================================================

function createDiscoveryPlaylistStore() {
	const { subscribe, set, update } = writable<DiscoveryPlaylistState>(initialState)

	return {
		subscribe,

		setReleases(releases: DiscoveryRelease[]) {
			set({ releases })
		},

		clearReleases() {
			set({ releases: [] })
		},

		cacheAndSet(playlistId: string, releases: DiscoveryRelease[]) {
			cache.set(playlistId, releases)
			set({ releases })
		},

		getCached(playlistId: string): DiscoveryRelease[] | undefined {
			return cache.get(playlistId)
		},

		deleteFromCache(playlistId: string) {
			cache.delete(playlistId)
		},

		updateTagCategory(tagId: string, newCategoryId: string) {
			update((state) => ({
				releases: state.releases.map((r) => ({
					...r,
					tags: r.tags.map((tag) => (tag.id === tagId ? { ...tag, category_id: newCategoryId } : tag)),
				})),
			}))
		},

		filterOutReleases(releaseIds: string[]) {
			update((state) => ({
				releases: state.releases.filter((r) => !releaseIds.includes(r.id)),
			}))
		},

		filterOutAndCache(playlistId: string, releaseIds: string[]) {
			update((state) => {
				const filtered = state.releases.filter((r) => !releaseIds.includes(r.id))
				cache.set(playlistId, filtered)
				return { releases: filtered }
			})
		},

		async refreshFromApi(playlistId: string, fetchFn: () => Promise<DiscoveryRelease[]>) {
			const releases = await fetchFn()
			cache.set(playlistId, releases)
			set({ releases })
			return releases
		},

		replaceRelease(release: DiscoveryRelease) {
			update((state) => ({
				releases: state.releases.map((r) => (r.id === release.id ? release : r)),
			}))
			for (const [key, releases] of cache) {
				if (releases.some((r) => r.id === release.id)) {
					cache.set(
						key,
						releases.map((r) => (r.id === release.id ? release : r))
					)
				}
			}
		},

		updateTrackLiked(releaseId: string, trackId: string, isLiked: boolean) {
			const updateTracks = (releases: DiscoveryRelease[]) =>
				releases.map((r) =>
					r.id === releaseId
						? { ...r, tracks: r.tracks.map((t) => (t.id === trackId ? { ...t, is_liked: isLiked } : t)) }
						: r
				)
			update((state) => ({ releases: updateTracks(state.releases) }))
			for (const [key, releases] of cache) {
				if (releases.some((r) => r.id === releaseId)) {
					cache.set(key, updateTracks(releases))
				}
			}
		},

		getCache() {
			return cache
		},
	}
}

export const discoveryPlaylistStore = createDiscoveryPlaylistStore()

export const discoveryPlaylistReleases = derived(discoveryPlaylistStore, ($s) => $s.releases)
