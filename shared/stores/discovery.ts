import { writable, derived, get } from 'svelte/store'
import type {
	DiscoveryRelease,
	DiscoveryReleaseCreate,
	DiscoveryReleaseUpdate,
	DiscoveryFacet,
	DiscoveryFacetFilters,
	DiscoveryFilter,
	DiscoverySortConfig,
	FilterTriState,
	ImportResultWithDuplicates,
} from '../types'
import * as discoveryApi from '../api/discovery'
import * as followApi from '../api/follow'
import { sortDiscoveryReleases } from '../utils/sorting'
import {
	applyDiscoveryFilters,
	cycleTriState,
	emptyFacetFilters,
	isDateLikedSortAllowed,
} from '../utils/discoveryFilters'
import { daysUntilRelease } from '../utils/format'
import { playerStore } from './player'
import { discoveryPlaylistStore } from './discoveryPlaylist'
import { uiStore } from './ui'
import { toastStore } from './toast'
import { hasLinkedCollection, ownedReleaseIds } from './collection'
import { fullyCachedIds } from './offlineCache'
import { translate } from '../i18n'

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
	/** The Liked / New / Purchased / Downloaded facet filters (off / include / exclude), shared by both
	 *  platforms and applied through `applyDiscoveryFilters` wherever a release list is displayed. */
	facets: DiscoveryFacetFilters
}

const DEFAULT_DISCOVERY_SORT: DiscoverySortConfig = { field: 'date_added', direction: 'desc' }

const initialState: DiscoveryState = {
	releases: [],
	loading: false,
	error: null,
	filter: {},
	sort: DEFAULT_DISCOVERY_SORT,
	refreshingIds: new Set(),
	facets: emptyFacetFilters(),
}

/** Replace the facets, dropping a Date Liked sort the moment the Liked facet stops being `include`. Done
 *  inside the mutators (not a subscription) so the meaningless combination is never published at all. */
function withFacets(state: DiscoveryState, facets: DiscoveryFacetFilters): DiscoveryState {
	const sort =
		state.sort.field === 'date_liked' && !isDateLikedSortAllowed(facets) ? DEFAULT_DISCOVERY_SORT : state.sort
	return { ...state, facets, sort }
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
// Chunked loading
// =============================================================================

// Releases load in pages instead of one giant IPC response: with thousands of releases
// (each carrying nested tracks/tags), a single multi-MB payload injected and parsed on the
// webview main thread in one shot both freezes the UI and can push the mobile WKWebView
// content process into an out-of-memory kill. A small first page paints fast; follow-up
// pages stream in with an event-loop yield between them so user interactions (taps,
// scrolling, playback) are handled between pages.
const FIRST_PAGE_SIZE = 200
const PAGE_SIZE = 500

// Monotonic token: a newer loadReleases() call silently cancels any in-flight paged load
// (each page checks it fetched under the current generation before touching the store).
let loadGeneration = 0

// =============================================================================
// Store
// =============================================================================

function createDiscoveryStore() {
	const { subscribe, set, update } = writable<DiscoveryState>(initialState)

	// Releases already availability-rechecked this session — the recheck is a full page
	// fetch at the source, so once per app run per release is plenty.
	const availabilityChecked = new Set<string>()

	return {
		subscribe,

		async loadReleases(filter?: DiscoveryFilter) {
			const generation = ++loadGeneration
			// Publish pages progressively only while the feed is EMPTY (first load / boot restore). Pages
			// arrive in backend (date-added) order, so publishing them mid-RELOAD visibly collapses a
			// custom-sorted list to a re-sorted subset of the newest page and then snaps back once the
			// last page lands — the "sort flash" on pull-to-refresh. With content already on screen,
			// keep showing it untouched and swap to the fresh set once, at the end.
			let progressive = true
			update((state) => {
				progressive = state.releases.length === 0
				return { ...state, loading: true, error: null }
			})

			try {
				let offset = 0
				let pageSize = FIRST_PAGE_SIZE
				let accumulated: DiscoveryRelease[] = []
				for (;;) {
					const page = await discoveryApi.getReleases({ ...(filter ?? {}), limit: pageSize, offset })
					if (generation !== loadGeneration) return // superseded by a newer load
					accumulated = offset === 0 ? page : accumulated.concat(page)
					const done = page.length < pageSize
					const releases = accumulated
					// `loading` stays true until the last page so empty-state logic still waits for a
					// complete load. NOTE: an in-place mutation (delete/update) landing mid-load can be
					// transiently overwritten by the next publish — it self-heals on the next reload;
					// the window is sub-second per page (progressive) or one load (buffered reload).
					if (progressive || done) {
						update((state) => ({
							...state,
							releases,
							loading: !done,
							filter: filter ?? {},
						}))
					}
					if (done) break
					offset += page.length
					pageSize = PAGE_SIZE
					// Yield the main thread between pages so queued user input runs first.
					await sleep(0)
				}
			} catch (error) {
				if (generation !== loadGeneration) return
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
				if (!release || !release.id) {
					return null
				}
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
				this.applyTrackLiked(releaseId, trackId, isLiked)
			} catch (error) {
				console.error('Failed to toggle track liked:', error)
			}
		},

		/**
		 * Apply an is_liked change that already happened in the DB (the API toggle above, or the iOS
		 * lock-screen Like, which the native engine writes directly) to every in-memory holder.
		 */
		applyTrackLiked(releaseId: string, trackId: string, isLiked: boolean) {
			// Stamped here rather than returned by the backend: same device clock as the DB's own stamp
			// (milliseconds apart), every reload re-reads DB truth, and the iOS lock-screen path lands
			// here too — so one line keeps the Date Liked sort live without widening the IPC/event payloads.
			const likedAt = isLiked ? new Date().toISOString() : null
			update((state) => ({
				...state,
				releases: state.releases.map((r) =>
					r.id === releaseId
						? {
								...r,
								tracks: r.tracks.map((t) => (t.id === trackId ? { ...t, is_liked: isLiked, liked_at: likedAt } : t)),
							}
						: r
				),
			}))
			playerStore.setPreviewTrackLiked(trackId, isLiked)
			discoveryPlaylistStore.updateTrackLiked(releaseId, trackId, isLiked, likedAt)
		},

		/**
		 * Apply a preview-availability change that already happened in the DB (the backend emits
		 * `discovery-availability-changed` whenever a stream extraction refreshes a release's flags)
		 * to every in-memory holder, so rows grey out / un-grey without a full reload.
		 */
		applyPreviewAvailability(releaseId: string, unavailablePositions: number[]) {
			const unavailable = new Set(unavailablePositions)
			update((state) => ({
				...state,
				releases: state.releases.map((r) =>
					r.id === releaseId
						? { ...r, tracks: r.tracks.map((t) => ({ ...t, preview_unavailable: unavailable.has(t.position) })) }
						: r
				),
			}))
			discoveryPlaylistStore.applyPreviewAvailability(releaseId, unavailablePositions)
		},

		/**
		 * Silently refresh a release's per-track preview availability (one background page
		 * fetch, once per release per session). Fires when the release shows flagged tracks
		 * (a pre-order's unreleased tracks — heals them once the album is out) or has a
		 * future release date with no flags yet (flags a fresh pre-order on first view).
		 */
		maybeRecheckAvailability(release: DiscoveryRelease) {
			if (release.source_type !== 'bandcamp' && release.source_type !== 'soundcloud') return
			if (availabilityChecked.has(release.id)) return
			// Same "upcoming" semantics as the release row's badge (null once out / unknown).
			const isPreRelease = daysUntilRelease(release.release_date) != null
			// Duration-less tracks are the pre-flag symptom of an unstreamable track (Bandcamp serves
			// no duration for unreleased pre-order tracks) — and many pre-orders carry no release date,
			// so the date check alone would never fire for them.
			const hasSuspectTracks =
				release.tracks.length > 0 && release.tracks.some((t) => t.preview_unavailable || !t.duration_ms)
			if (!hasSuspectTracks && !isPreRelease) return
			availabilityChecked.add(release.id)
			discoveryApi
				.recheckPreviewAvailability(release.id)
				.then((tracks) => {
					update((state) => ({
						...state,
						releases: state.releases.map((r) => (r.id === release.id ? { ...r, tracks } : r)),
					}))
					// Release-day transition: a track just proved streamable but still lacks a duration
					// (pre-order rows are created without one, and stream extraction doesn't supply it) —
					// the duration gate would keep the row greyed, so backfill via a metadata refresh.
					if (tracks.some((t) => !t.preview_unavailable && !t.duration_ms)) {
						void this.refreshMetadata(release.id)
					}
				})
				.catch((error) => console.error('Preview availability recheck failed:', error))
		},

		/** Set one facet explicitly. A no-op when unchanged, so re-tapping the active segment doesn't
		 *  re-run the filter chain over the whole feed. */
		setFacetFilter(facet: DiscoveryFacet, value: FilterTriState) {
			update((state) =>
				state.facets[facet] === value ? state : withFacets(state, { ...state.facets, [facet]: value })
			)
		},

		/** off → include → exclude → off (the row-label tap). */
		cycleFacetFilter(facet: DiscoveryFacet) {
			update((state) => withFacets(state, { ...state.facets, [facet]: cycleTriState(state.facets[facet]) }))
		},

		/** Reset every facet at once ("Clear all"). Leaves the search term alone — that has its own
		 *  affordance in the search bar. */
		clearFacetFilters() {
			update((state) => withFacets(state, emptyFacetFilters()))
		},

		/** Manual "mark as new / not-new" override (the auto-clear rule lives in clearNew). */
		async markReleaseNew(id: string, isNew: boolean) {
			try {
				await followApi.setReleaseNewFlag(id, isNew)
				update((state) => ({
					...state,
					releases: state.releases.map((r) => (r.id === id ? { ...r, is_new: isNew } : r)),
				}))
			} catch (error) {
				toastStore.error(
					typeof error === 'string' ? error : error instanceof Error ? error.message : 'Failed to update release'
				)
			}
		},

		/** Clear the "new" flag on preview-play or a decisive action. No-op if already not-new. */
		clearNew(id: string) {
			let wasNew = false
			update((state) => {
				const release = state.releases.find((r) => r.id === id)
				if (!release?.is_new) return state
				wasNew = true
				return {
					...state,
					releases: state.releases.map((r) => (r.id === id ? { ...r, is_new: false } : r)),
				}
			})
			if (wasNew) followApi.setReleaseNewFlag(id, false).catch(() => {})
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
		 * Also clears the release from refreshingIds since enrichment is complete.
		 */
		replaceRelease(release: DiscoveryRelease) {
			update((state) => {
				const next = new Set(state.refreshingIds)
				next.delete(release.id)
				return {
					...state,
					releases: state.releases.map((r) => (r.id === release.id ? release : r)),
					refreshingIds: next,
				}
			})
			discoveryPlaylistStore.replaceRelease(release)
		},

		/**
		 * Mark release IDs as enriching (shows spinner in UI).
		 */
		markEnriching(ids: string[]) {
			update((state) => ({
				...state,
				refreshingIds: new Set([...state.refreshingIds, ...ids]),
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
			discoveryApi.skipEnrichment(id)
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

// A discovery preview starting means the user has listened to the release — clear its "new" flag.
// Registered here (rather than imported by the player store) because the player store must not depend
// on this store: discovery already imports playerStore, so wiring it the other way would be circular.
playerStore.setPreviewPlayedHandler((releaseId) => discoveryStore.clearNew(releaseId))

// iOS lock-screen Like: the native engine has already toggled the DB; mirror the change into the
// in-memory stores once JS is running again. Same inversion rationale as above.
playerStore.setNativeLikeChangedHandler((trackId, isLiked) => {
	const release = get(discoveryStore).releases.find((r) => r.tracks.some((t) => t.id === trackId))
	if (release) discoveryStore.applyTrackLiked(release.id, trackId, isLiked)
})

// Unlinking the last collection account hides the Purchased row in every filter UI, so an active
// Purchased facet would otherwise become an invisible filter (emptying the feed on `include`).
hasLinkedCollection.subscribe((linked) => {
	if (!linked) discoveryStore.setFacetFilter('purchased', 'off')
})

// =============================================================================
// Derived Stores
// =============================================================================

export const facetFilters = derived(discoveryStore, ($discovery) => $discovery.facets)

export const likedFilter = derived(discoveryStore, ($discovery) => $discovery.facets.liked)

export const newFilter = derived(discoveryStore, ($discovery) => $discovery.facets.new)

export const purchasedFilter = derived(discoveryStore, ($discovery) => $discovery.facets.purchased)

export const downloadedFilter = derived(discoveryStore, ($discovery) => $discovery.facets.downloaded)

export const sortedReleases = derived(
	[discoveryStore, ownedReleaseIds, fullyCachedIds],
	([$discovery, $owned, $cached]) => {
		let releases = applyDiscoveryFilters($discovery.releases, $discovery.facets, {
			ownedIds: $owned,
			cachedIds: $cached,
		})

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
		return sortDiscoveryReleases(releases, $discovery.sort)
	}
)

export const displayedReleases = derived(
	[sortedReleases, discoveryStore, uiStore, discoveryPlaylistStore, ownedReleaseIds, fullyCachedIds],
	([$sortedReleases, $discovery, $ui, $playlist, $owned, $cached]) => {
		if ($ui.activeView !== 'discovery' || !$ui.selectedPlaylistId) {
			return $sortedReleases
		}

		// Inside a discovery playlist — apply client-side filters to playlist releases
		let releases = applyDiscoveryFilters($playlist.releases, $discovery.facets, {
			ownedIds: $owned,
			cachedIds: $cached,
		})

		const discoveryFilters = $ui.viewFilters.discovery
		if (discoveryFilters.selectedTagIds.length > 0) {
			const tagIds = new Set(discoveryFilters.selectedTagIds)
			if (discoveryFilters.tagFilterMode === 'and') {
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
					r.notes?.toLowerCase().includes(search) ||
					r.tracks.some((t) => t.name?.toLowerCase().includes(search))
			)
		}

		return sortDiscoveryReleases(releases, $discovery.sort)
	}
)

export const releaseCount = derived(sortedReleases, ($releases) => $releases.length)

export const isDiscoveryLoading = derived(discoveryStore, ($discovery) => $discovery.loading)

export const refreshingReleaseIds = derived(discoveryStore, ($s) => $s.refreshingIds)
