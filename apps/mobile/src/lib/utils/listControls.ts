import type {
	DiscoveryFacetFilters,
	DiscoveryRelease,
	DiscoverySortConfig,
	SortDirection,
	TagFilterMode,
} from '$shared/types'
import {
	applyDiscoveryFilters,
	countActiveFacets,
	emptyFacetFilters,
	isDateLikedSortAllowed,
} from '$shared/utils/discoveryFilters'
import { applyTagFilter } from '$lib/stores/mobileUI'

// Shared shapes for the generalized list controls (SortSheet / FilterSheet / ListControlsBar):
// the discovery feed keeps its state in the discovery + mobileUI stores, while the detail views
// (playlist / tag / follow) hold a session-local ReleaseViewFilter + sort and derive their
// displayed list with these helpers — one comparator (shared/utils/sorting.ts) and one filter
// semantics everywhere.

export interface SortOption {
	field: string
	labelKey: string
	defaultDir: SortDirection
	/** Fixed-order option (follow sorts, "Playlist order"): no direction arrow, no flip on re-tap. */
	directionless?: boolean
}

/** The release sort fields every release list offers (detail views prepend their own natural-order
 *  option where applicable, e.g. "Playlist order"). Labels reuse existing i18n keys. Source groups
 *  the list by platform (Bandcamp / SoundCloud / YouTube / Discogs); Tracks orders by track count
 *  (albums/EPs down to singles), each group alphabetical inside (the comparator's title tie-break). */
export const RELEASE_SORT_OPTIONS: SortOption[] = [
	{ field: 'date_added', labelKey: 'discovery.columns.dateAdded', defaultDir: 'desc' },
	{ field: 'release_date', labelKey: 'discovery.columns.dateReleased', defaultDir: 'desc' },
	{ field: 'artist', labelKey: 'discovery.editor.artist', defaultDir: 'asc' },
	{ field: 'title', labelKey: 'discovery.editor.title', defaultDir: 'asc' },
	{ field: 'label', labelKey: 'discovery.editor.label', defaultDir: 'asc' },
	{ field: 'source_type', labelKey: 'discovery.source', defaultDir: 'asc' },
	{ field: 'track_count', labelKey: 'discovery.tracks', defaultDir: 'desc' },
]

const DATE_LIKED_SORT_OPTION: SortOption = {
	field: 'date_liked',
	labelKey: 'discovery.columns.dateLiked',
	defaultDir: 'desc',
}

/** The release sort options for the current facets: Date Liked joins the list (right after Date Added)
 *  only while the Liked facet is `include` — see `isDateLikedSortAllowed`. */
export function releaseSortOptions(facets: DiscoveryFacetFilters): SortOption[] {
	if (!isDateLikedSortAllowed(facets)) return RELEASE_SORT_OPTIONS
	return [RELEASE_SORT_OPTIONS[0], DATE_LIKED_SORT_OPTION, ...RELEASE_SORT_OPTIONS.slice(1)]
}

/** The detail views' counterpart of the feed store's fallback: a session-local Date Liked sort drops
 *  back to the view's natural order (null) once the Liked facet stops being `include`. */
export function reconcileViewSort(
	sort: DiscoverySortConfig | null,
	facets: DiscoveryFacetFilters
): DiscoverySortConfig | null {
	return sort?.field === 'date_liked' && !isDateLikedSortAllowed(facets) ? null : sort
}

/** Per-view (session-local) filter state for the detail views' release lists. The detail views don't
 *  expose the New facet, so `facets.new` simply stays `off`. */
export interface ReleaseViewFilter {
	search: string
	facets: DiscoveryFacetFilters
	tagIds: string[]
	tagMode: TagFilterMode
}

export const emptyViewFilter = (): ReleaseViewFilter => ({
	search: '',
	facets: emptyFacetFilters(),
	tagIds: [],
	tagMode: 'or',
})

export function hasActiveViewFilter(f: ReleaseViewFilter): boolean {
	return f.search.trim() !== '' || countActiveFacets(f.facets) > 0 || f.tagIds.length > 0
}

/** Count for the filter button's badge — mirrors the feed toolbar (tags + non-off facets). */
export function countActiveViewFilters(f: ReleaseViewFilter): number {
	return f.tagIds.length + countActiveFacets(f.facets)
}

/**
 * Apply a per-view filter over an in-memory release list. Search semantics mirror the feed's
 * `sortedReleases` (artist/title/label/notes/track names); tags reuse the feed's AND/OR filter; the
 * facets go through the one shared `applyDiscoveryFilters` so include/exclude mean the same as in the feed.
 */
export function applyViewFilter(
	list: DiscoveryRelease[],
	f: ReleaseViewFilter,
	cachedIds: ReadonlySet<string>,
	ownedIds: ReadonlySet<string>
): DiscoveryRelease[] {
	let releases = applyDiscoveryFilters(list, f.facets, { ownedIds, cachedIds })
	releases = applyTagFilter(releases, f.tagIds, f.tagMode)
	const search = f.search.trim().toLowerCase()
	if (search) {
		releases = releases.filter(
			(r) =>
				r.artist?.toLowerCase().includes(search) ||
				r.title?.toLowerCase().includes(search) ||
				r.label?.toLowerCase().includes(search) ||
				r.notes?.toLowerCase().includes(search) ||
				r.tracks.some((t) => t.name?.toLowerCase().includes(search))
		)
	}
	return releases
}
