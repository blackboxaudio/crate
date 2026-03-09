import { writable, derived, get } from 'svelte/store'
import type {
	DiscoveryRelease,
	DiscoveryReleaseCreate,
	DiscoveryReleaseUpdate,
	DiscoveryFilter,
	DiscoverySortConfig,
	ImportResultWithDuplicates,
} from '$lib/types'
import * as discoveryApi from '$lib/api/discovery'
import { playerStore } from './player'
import { discoveryPlaylistStore } from './discoveryPlaylist'
import { uiStore } from './ui'
import { toastStore } from './toast'
import { translate } from '$lib/i18n'

// =============================================================================
// State
// =============================================================================

interface DiscoveryState {
	releases: DiscoveryRelease[]
	loading: boolean
	error: string | null
	filter: DiscoveryFilter
	sort: DiscoverySortConfig
	refreshingIds: Set<string>
	likedOnly: boolean
}

const initialState: DiscoveryState = {
	releases: [],
	loading: false,
	error: null,
	filter: {},
	sort: {
		field: 'date_added',
		direction: 'desc',
	},
	refreshingIds: new Set(),
	likedOnly: false,
}

// =============================================================================
// Bulk Refresh
// =============================================================================

let bulkRefreshAbort = false
const bulkRefreshSkipIds = new Set<string>()

function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms))
}

// =============================================================================
// Store
// =============================================================================

function createDiscoveryStore() {
	const { subscribe, set, update } = writable<DiscoveryState>(initialState)

	return {
		subscribe,

		async loadReleases(filter?: DiscoveryFilter) {
			update((state) => ({ ...state, loading: true, error: null }))

			try {
				const releases = await discoveryApi.getReleases(filter)
				update((state) => ({
					...state,
					releases,
					loading: false,
					filter: filter ?? {},
				}))
			} catch (error) {
				update((state) => ({
					...state,
					loading: false,
					error: typeof error === 'string' ? error : error instanceof Error ? error.message : 'Failed to load releases',
				}))
			}
		},

		async createRelease(create: DiscoveryReleaseCreate): Promise<DiscoveryRelease | null> {
			try {
				const release = await discoveryApi.createRelease(create)
				update((state) => ({
					...state,
					releases: [release, ...state.releases],
				}))
				return release
			} catch (error) {
				toastStore.error(
					typeof error === 'string' ? error : error instanceof Error ? error.message : 'Failed to add release'
				)
				return null
			}
		},

		async updateRelease(id: string, updateData: DiscoveryReleaseUpdate): Promise<DiscoveryRelease | null> {
			try {
				const release = await discoveryApi.updateRelease(id, updateData)
				update((state) => ({
					...state,
					releases: state.releases.map((r) => (r.id === id ? release : r)),
				}))
				return release
			} catch (error) {
				toastStore.error(
					typeof error === 'string' ? error : error instanceof Error ? error.message : 'Failed to update release'
				)
				return null
			}
		},

		async deleteRelease(id: string) {
			try {
				await discoveryApi.deleteRelease(id)
				update((state) => ({
					...state,
					releases: state.releases.filter((r) => r.id !== id),
				}))
			} catch (error) {
				toastStore.error(
					typeof error === 'string' ? error : error instanceof Error ? error.message : 'Failed to delete release'
				)
			}
		},

		async deleteReleases(ids: string[]) {
			try {
				await discoveryApi.deleteReleases(ids)
				const idSet = new Set(ids)
				update((state) => ({
					...state,
					releases: state.releases.filter((r) => !idSet.has(r.id)),
				}))
			} catch (error) {
				toastStore.error(error instanceof Error ? error.message : 'Failed to delete releases')
			}
		},

		async assignTags(releaseIds: string[], tagIds: string[]) {
			try {
				await discoveryApi.assignTags(releaseIds, tagIds)
				await this.loadReleases()
			} catch (error) {
				toastStore.error(
					typeof error === 'string' ? error : error instanceof Error ? error.message : 'Failed to assign tags'
				)
			}
		},

		async refreshMetadata(id: string): Promise<DiscoveryRelease | null> {
			update((state) => ({ ...state, refreshingIds: new Set([...state.refreshingIds, id]) }))
			try {
				const release = await discoveryApi.refreshMetadata(id)
				update((state) => ({
					...state,
					releases: state.releases.map((r) => (r.id === id ? release : r)),
				}))
				return release
			} catch (error) {
				toastStore.error(
					typeof error === 'string' ? error : error instanceof Error ? error.message : 'Failed to refresh metadata'
				)
				return null
			} finally {
				update((state) => {
					const next = new Set(state.refreshingIds)
					next.delete(id)
					return { ...state, refreshingIds: next }
				})
			}
		},

		async removeTags(releaseIds: string[], tagIds: string[]) {
			try {
				await discoveryApi.removeTags(releaseIds, tagIds)
				await this.loadReleases()
			} catch (error) {
				toastStore.error(
					typeof error === 'string' ? error : error instanceof Error ? error.message : 'Failed to remove tags'
				)
			}
		},

		async purchaseRelease(
			releaseId: string,
			filePaths: string[],
			transferTags: boolean,
			removeAfterImport: boolean
		): Promise<ImportResultWithDuplicates | null> {
			try {
				const result = await discoveryApi.purchaseRelease(releaseId, filePaths, transferTags, removeAfterImport)
				if (removeAfterImport) {
					update((state) => ({
						...state,
						releases: state.releases.filter((r) => r.id !== releaseId),
					}))
				}
				return result
			} catch (error) {
				toastStore.error(
					typeof error === 'string' ? error : error instanceof Error ? error.message : 'Failed to import release'
				)
				return null
			}
		},

		async setArtwork(id: string, filePath: string) {
			try {
				const release = await discoveryApi.setDiscoveryReleaseArtwork(id, filePath)
				update((state) => ({
					...state,
					releases: state.releases.map((r) => (r.id === id ? release : r)),
				}))
			} catch (error) {
				toastStore.error(get(translate)('toast.failedToSetArtwork'))
			}
		},

		async deleteArtwork(id: string) {
			try {
				const release = await discoveryApi.deleteDiscoveryReleaseArtwork(id)
				update((state) => ({
					...state,
					releases: state.releases.map((r) => (r.id === id ? release : r)),
				}))
			} catch (error) {
				toastStore.error(get(translate)('toast.failedToRemoveArtwork'))
			}
		},

		async toggleTrackLiked(releaseId: string, trackId: string) {
			try {
				const isLiked = await discoveryApi.toggleTrackLiked(trackId)
				update((state) => ({
					...state,
					releases: state.releases.map((r) =>
						r.id === releaseId
							? { ...r, tracks: r.tracks.map((t) => (t.id === trackId ? { ...t, is_liked: isLiked } : t)) }
							: r
					),
				}))
				playerStore.setPreviewTrackLiked(trackId, isLiked)
				discoveryPlaylistStore.updateTrackLiked(releaseId, trackId, isLiked)
			} catch (error) {
				console.error('Failed to toggle track liked:', error)
			}
		},

		toggleLikedFilter() {
			update((state) => ({ ...state, likedOnly: !state.likedOnly }))
		},

		setFilter(filter: DiscoveryFilter) {
			update((state) => ({ ...state, filter }))
		},

		setSearch(search: string) {
			update((state) => ({
				...state,
				filter: { ...state.filter, search: search || undefined },
			}))
		},

		setSort(sort: DiscoverySortConfig) {
			update((state) => ({ ...state, sort }))
		},

		async mergeReleases(targetId: string, sourceIds: string[]): Promise<DiscoveryRelease | null> {
			try {
				const merged = await discoveryApi.mergeReleases(targetId, sourceIds)
				const sourceIdSet = new Set(sourceIds)
				update((state) => ({
					...state,
					releases: state.releases.filter((r) => !sourceIdSet.has(r.id)).map((r) => (r.id === targetId ? merged : r)),
				}))
				return merged
			} catch (error) {
				toastStore.error(
					typeof error === 'string' ? error : error instanceof Error ? error.message : 'Failed to merge releases'
				)
				return null
			}
		},

		/**
		 * Replace a release in the store with updated data (used by backend events).
		 */
		replaceRelease(release: DiscoveryRelease) {
			update((state) => ({
				...state,
				releases: state.releases.map((r) => (r.id === release.id ? release : r)),
			}))
		},

		/**
		 * Update category_id for a tag across all releases
		 */
		updateTagCategory(tagId: string, newCategoryId: string) {
			update((state) => ({
				...state,
				releases: state.releases.map((r) => ({
					...r,
					tags: r.tags.map((tag) => (tag.id === tagId ? { ...tag, category_id: newCategoryId } : tag)),
				})),
			}))
		},

		async bulkRefreshMetadata(releases: DiscoveryRelease[]) {
			bulkRefreshAbort = false
			bulkRefreshSkipIds.clear()

			// Show spinners on all selected releases immediately
			const allIds = releases.map((r) => r.id)
			update((state) => ({
				...state,
				refreshingIds: new Set([...state.refreshingIds, ...allIds]),
			}))

			for (let i = 0; i < releases.length; i++) {
				if (bulkRefreshAbort) break

				const release = releases[i]

				// Skip if individually cancelled
				if (bulkRefreshSkipIds.has(release.id)) continue

				// Throttle before Discogs releases to respect rate limits
				if (i > 0) {
					const delay = release.source_type === 'discogs' ? 2000 + Math.random() * 1000 : 500
					await sleep(delay)
				}

				if (bulkRefreshAbort || bulkRefreshSkipIds.has(release.id)) break

				try {
					const updated = await discoveryApi.refreshMetadata(release.id)
					update((state) => ({
						...state,
						releases: state.releases.map((r) => (r.id === release.id ? updated : r)),
					}))
				} catch (error) {
					console.error(`Failed to refresh metadata for release ${release.id}:`, error)
				} finally {
					update((state) => {
						const next = new Set(state.refreshingIds)
						next.delete(release.id)
						return { ...state, refreshingIds: next }
					})
				}
			}

			// Clear any remaining IDs if aborted early
			update((state) => {
				const next = new Set(state.refreshingIds)
				for (const id of allIds) next.delete(id)
				return { ...state, refreshingIds: next }
			})

			bulkRefreshAbort = false
			bulkRefreshSkipIds.clear()
		},

		cancelRefresh(id: string) {
			bulkRefreshSkipIds.add(id)
			update((state) => {
				const next = new Set(state.refreshingIds)
				next.delete(id)
				return { ...state, refreshingIds: next }
			})
		},

		cancelBulkRefresh() {
			bulkRefreshAbort = true
		},

		reset() {
			set(initialState)
		},
	}
}

export const discoveryStore = createDiscoveryStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const likedOnly = derived(discoveryStore, ($discovery) => $discovery.likedOnly)

export const sortedReleases = derived(discoveryStore, ($discovery) => {
	let releases = [...$discovery.releases]

	// Apply liked filter
	if ($discovery.likedOnly) {
		releases = releases.filter((r) => r.tracks.some((t) => t.is_liked))
	}

	// Apply client-side search filter
	if ($discovery.filter.search) {
		const search = $discovery.filter.search.toLowerCase()
		releases = releases.filter(
			(r) =>
				r.artist?.toLowerCase().includes(search) ||
				r.title?.toLowerCase().includes(search) ||
				r.label?.toLowerCase().includes(search) ||
				r.notes?.toLowerCase().includes(search) ||
				r.tracks.some((t) => t.name?.toLowerCase().includes(search))
		)
	}

	// Apply sorting
	const { field, direction } = $discovery.sort
	const dir = direction === 'asc' ? 1 : -1

	releases.sort((a, b) => {
		if (field === 'release_date') {
			const aDate = a.release_date ? new Date(a.release_date).getTime() : NaN
			const bDate = b.release_date ? new Date(b.release_date).getTime() : NaN
			const aValid = !isNaN(aDate)
			const bValid = !isNaN(bDate)
			if (!aValid && !bValid) return 0
			if (!aValid) return 1
			if (!bValid) return -1
			if (aDate < bDate) return -1 * dir
			if (aDate > bDate) return 1 * dir
			return 0
		}
		const aVal = a[field] ?? ''
		const bVal = b[field] ?? ''
		if (aVal < bVal) return -1 * dir
		if (aVal > bVal) return 1 * dir
		return 0
	})

	return releases
})

export const displayedReleases = derived(
	[sortedReleases, discoveryStore, uiStore, discoveryPlaylistStore],
	([$sortedReleases, $discovery, $ui, $playlist]) => {
		if ($ui.activeView !== 'discovery' || !$ui.selectedPlaylistId) {
			return $sortedReleases
		}

		// Inside a discovery playlist — apply client-side filters to playlist releases
		let releases = [...$playlist.releases]

		if ($discovery.likedOnly) {
			releases = releases.filter((r) => r.tracks.some((t) => t.is_liked))
		}

		if ($ui.selectedTagIds.length > 0) {
			const tagIds = new Set($ui.selectedTagIds)
			if ($ui.tagFilterMode === 'and') {
				releases = releases.filter((r) => [...tagIds].every((id) => r.tags.some((t) => t.id === id)))
			} else {
				releases = releases.filter((r) => r.tags.some((t) => tagIds.has(t.id)))
			}
		}

		if ($discovery.filter.search) {
			const search = $discovery.filter.search.toLowerCase()
			releases = releases.filter(
				(r) =>
					r.artist?.toLowerCase().includes(search) ||
					r.title?.toLowerCase().includes(search) ||
					r.label?.toLowerCase().includes(search) ||
					r.notes?.toLowerCase().includes(search)
			)
		}

		return releases
	}
)

export const releaseCount = derived(sortedReleases, ($releases) => $releases.length)

export const isDiscoveryLoading = derived(discoveryStore, ($discovery) => $discovery.loading)

export const refreshingReleaseIds = derived(discoveryStore, ($s) => $s.refreshingIds)
