import { writable, derived } from 'svelte/store'
import type { DiscoveryRelease, Tag } from '../types'
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
			const moveTag = (tag: Tag) => (tag.id === tagId ? { ...tag, category_id: newCategoryId } : tag)
			update((state) => ({
				releases: state.releases.map((r) => ({
					...r,
					tags: r.tags.map(moveTag),
					tracks: r.tracks.map((t) => (t.tags ? { ...t, tags: t.tags.map(moveTag) } : t)),
				})),
			}))
		},

		/** Mirror of discoveryStore.applyTrackTags for this store's (member-filtered) release copies. */
		applyTrackTags(releaseId: string, tagsByTrack: Map<string, Tag[]>) {
			const updateTracks = (releases: DiscoveryRelease[]) =>
				releases.map((r) =>
					r.id === releaseId
						? {
								...r,
								tracks: r.tracks.map((t) => (tagsByTrack.has(t.id) ? { ...t, tags: tagsByTrack.get(t.id) } : t)),
							}
						: r
				)
			update((state) => ({ releases: updateTracks(state.releases) }))
			for (const [key, releases] of cache) {
				if (releases.some((r) => r.id === releaseId)) {
					cache.set(key, updateTracks(releases))
				}
			}
		},

		/**
		 * Drop member tracks from a playlist's release groups (a group with no members left goes
		 * too), in the current view and the cache.
		 */
		filterOutTracks(playlistId: string, trackIds: string[]) {
			const ids = new Set(trackIds)
			const prune = (releases: DiscoveryRelease[]) =>
				releases
					.map((r) => ({ ...r, tracks: r.tracks.filter((t) => !ids.has(t.id)) }))
					.filter((r) => r.tracks.length > 0)
			update((state) => ({ releases: prune(state.releases) }))
			const cached = cache.get(playlistId)
			if (cached) cache.set(playlistId, prune(cached))
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

		filterOutFromAll(releaseIds: string[]) {
			update((state) => ({
				releases: state.releases.filter((r) => !releaseIds.includes(r.id)),
			}))
			for (const [key, releases] of cache) {
				cache.set(
					key,
					releases.filter((r) => !releaseIds.includes(r.id))
				)
			}
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

		updateTrackLiked(releaseId: string, trackId: string, isLiked: boolean, likedAt: string | null) {
			const updateTracks = (releases: DiscoveryRelease[]) =>
				releases.map((r) =>
					r.id === releaseId
						? {
								...r,
								tracks: r.tracks.map((t) => (t.id === trackId ? { ...t, is_liked: isLiked, liked_at: likedAt } : t)),
							}
						: r
				)
			update((state) => ({ releases: updateTracks(state.releases) }))
			for (const [key, releases] of cache) {
				if (releases.some((r) => r.id === releaseId)) {
					cache.set(key, updateTracks(releases))
				}
			}
		},

		/** Mirror of discoveryStore.applyPreviewAvailability for this store's release copies. */
		applyPreviewAvailability(releaseId: string, unavailablePositions: number[]) {
			const unavailable = new Set(unavailablePositions)
			const updateTracks = (releases: DiscoveryRelease[]) =>
				releases.map((r) =>
					r.id === releaseId
						? { ...r, tracks: r.tracks.map((t) => ({ ...t, preview_unavailable: unavailable.has(t.position) })) }
						: r
				)
			update((state) => ({ releases: updateTracks(state.releases) }))
			for (const [key, releases] of cache) {
				if (releases.some((r) => r.id === releaseId)) {
					cache.set(key, updateTracks(releases))
				}
			}
		},

		reorderInCache(playlistId: string, releaseIds: string[]) {
			update((state) => {
				const byId = new Map(state.releases.map((r) => [r.id, r]))
				const reordered = releaseIds.map((id) => byId.get(id)).filter(Boolean) as DiscoveryRelease[]
				cache.set(playlistId, reordered)
				return { releases: reordered }
			})
		},

		getCache() {
			return cache
		},
	}
}

export const discoveryPlaylistStore = createDiscoveryPlaylistStore()

export const discoveryPlaylistReleases = derived(discoveryPlaylistStore, ($s) => $s.releases)
