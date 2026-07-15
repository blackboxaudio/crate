import type { DiscoveryRelease, SortDirection, TagFilterMode } from '$shared/types'
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
	{ field: 'release_date', labelKey: 'discovery.columns.released', defaultDir: 'desc' },
	{ field: 'artist', labelKey: 'discovery.editor.artist', defaultDir: 'asc' },
	{ field: 'title', labelKey: 'discovery.editor.title', defaultDir: 'asc' },
	{ field: 'label', labelKey: 'discovery.editor.label', defaultDir: 'asc' },
	{ field: 'source_type', labelKey: 'discovery.source', defaultDir: 'asc' },
	{ field: 'track_count', labelKey: 'discovery.tracks', defaultDir: 'desc' },
]

/** Per-view (session-local) filter state for the detail views' release lists. */
export interface ReleaseViewFilter {
	search: string
	likedOnly: boolean
	downloadedOnly: boolean
	tagIds: string[]
	tagMode: TagFilterMode
}

export const emptyViewFilter = (): ReleaseViewFilter => ({
	search: '',
	likedOnly: false,
	downloadedOnly: false,
	tagIds: [],
	tagMode: 'or',
})

export function hasActiveViewFilter(f: ReleaseViewFilter): boolean {
	return f.search.trim() !== '' || f.likedOnly || f.downloadedOnly || f.tagIds.length > 0
}

/** Count for the filter button's badge — mirrors the feed toolbar (tags + liked + downloaded). */
export function countActiveViewFilters(f: ReleaseViewFilter): number {
	return f.tagIds.length + (f.likedOnly ? 1 : 0) + (f.downloadedOnly ? 1 : 0)
}

/**
 * Apply a per-view filter over an in-memory release list. Search semantics mirror the feed's
 * `sortedReleases` (artist/title/label/notes/track names); tags reuse the feed's AND/OR filter;
 * downloaded checks the offline-cache id set.
 */
export function applyViewFilter(
	list: DiscoveryRelease[],
	f: ReleaseViewFilter,
	cachedIds: ReadonlySet<string>
): DiscoveryRelease[] {
	let releases = list
	if (f.likedOnly) releases = releases.filter((r) => r.tracks.some((t) => t.is_liked))
	if (f.downloadedOnly) releases = releases.filter((r) => cachedIds.has(r.id))
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
