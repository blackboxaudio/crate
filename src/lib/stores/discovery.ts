import { writable, derived } from 'svelte/store'
import type {
	DiscoveryRelease,
	DiscoveryReleaseCreate,
	DiscoveryReleaseUpdate,
	DiscoveryFilter,
	DiscoverySortConfig,
	ImportResultWithDuplicates,
} from '$lib/types'
import * as discoveryApi from '$lib/api/discovery'
import { toastStore } from './toast'

// =============================================================================
// State
// =============================================================================

interface DiscoveryState {
	releases: DiscoveryRelease[]
	loading: boolean
	error: string | null
	filter: DiscoveryFilter
	sort: DiscoverySortConfig
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

		reset() {
			set(initialState)
		},
	}
}

export const discoveryStore = createDiscoveryStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const sortedReleases = derived(discoveryStore, ($discovery) => {
	let releases = [...$discovery.releases]

	// Apply client-side search filter
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

	// Apply sorting
	const { field, direction } = $discovery.sort
	const dir = direction === 'asc' ? 1 : -1

	releases.sort((a, b) => {
		const aVal = a[field] ?? ''
		const bVal = b[field] ?? ''
		if (aVal < bVal) return -1 * dir
		if (aVal > bVal) return 1 * dir
		return 0
	})

	return releases
})

export const releaseCount = derived(sortedReleases, ($releases) => $releases.length)

export const isDiscoveryLoading = derived(discoveryStore, ($discovery) => $discovery.loading)
