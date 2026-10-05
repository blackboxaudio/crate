import type {
	DiscoveryFacet,
	DiscoveryFacetFilters,
	DiscoveryRelease,
	DiscoverySortConfig,
	FilterTriState,
	TagFilterMode,
} from '../types'
import { sortDiscoveryReleases } from './sorting'

// The one implementation of the discovery filters (the Liked / New / Purchased / Downloaded facets,
// tags, search). Every release list on both platforms — the feed, discovery playlists, mobile detail
// views, the Purchased view — narrows through `filterDiscoveryReleases`, and the playback queue scopes
// its continuation through the same `trackMatches`, so what is on screen and what plays can't diverge.
//
// Every filter is judged per TRACK except New (a property of the release itself): a release is listed
// when it is New-compatible AND at least one of its tracks passes every track-level filter at once.
// A new filter only has to decide which of the two levels it belongs to.

export const DISCOVERY_FACETS: readonly DiscoveryFacet[] = ['liked', 'new', 'purchased', 'downloaded']

export const emptyFacetFilters = (): DiscoveryFacetFilters => ({
	liked: 'off',
	new: 'off',
	purchased: 'off',
	downloaded: 'off',
})

/** off → include → exclude → off */
export function cycleTriState(state: FilterTriState): FilterTriState {
	return state === 'off' ? 'include' : state === 'include' ? 'exclude' : 'off'
}

/** `include` keeps hits, `exclude` keeps misses, `off` keeps everything. */
export function matchesTriState(state: FilterTriState, hit: boolean): boolean {
	return state === 'off' || (state === 'include') === hit
}

export function countActiveFacets(facets: DiscoveryFacetFilters): number {
	return DISCOVERY_FACETS.reduce((n, facet) => n + (facets[facet] !== 'off' ? 1 : 0), 0)
}

export function hasActiveFacets(facets: DiscoveryFacetFilters): boolean {
	return DISCOVERY_FACETS.some((facet) => facets[facet] !== 'off')
}

/** Date Liked only means something over the liked pool: per-track like dates say nothing about unliked
 *  rows, so the sort is offered (and kept) only while the Liked facet is `include`. */
export function isDateLikedSortAllowed(facets: DiscoveryFacetFilters): boolean {
	return facets.liked === 'include'
}

/** One list's complete filter state. */
export interface DiscoveryCriteria {
	facets: DiscoveryFacetFilters
	tagIds: readonly string[]
	tagMode: TagFilterMode
	search: string
}

/** The id sets the purchased / downloaded predicates look up. Named (not positional) because all three
 *  are `ReadonlySet<string>`. */
export interface FacetContext {
	/** Releases owned whole (album bought, or every track bought) — `fullyOwnedReleaseIds`. */
	fullyOwnedIds: ReadonlySet<string>
	/** Tracks bought on their own — `ownedTrackIds`. */
	ownedTrackIds: ReadonlySet<string>
	/** Tracks whose audio is on disk — `cachedTrackIds`. */
	cachedTrackIds: ReadonlySet<string>
}

/** The tracks a playback context may continue into: the criteria plus the id sets they read. */
export interface TrackScope {
	criteria: DiscoveryCriteria
	ctx: FacetContext
}

function normalizeSearch(search: string): string {
	return search.trim().toLowerCase()
}

function releaseFieldsMatch(r: DiscoveryRelease, search: string): boolean {
	return !!(
		r.artist?.toLowerCase().includes(search) ||
		r.title?.toLowerCase().includes(search) ||
		r.label?.toLowerCase().includes(search) ||
		r.notes?.toLowerCase().includes(search)
	)
}

function matchesTags(ids: readonly string[], mode: TagFilterMode, has: (id: string) => boolean): boolean {
	return mode === 'and' ? ids.every(has) : ids.some(has)
}

/** Whether any filter judged per track is active — i.e. whether the list's tracks need narrowing. */
export function hasTrackCriteria(c: DiscoveryCriteria): boolean {
	return (
		c.facets.liked !== 'off' ||
		c.facets.purchased !== 'off' ||
		c.facets.downloaded !== 'off' ||
		c.tagIds.length > 0 ||
		normalizeSearch(c.search) !== ''
	)
}

/**
 * A per-track predicate for one release. The release-level parts (its search hit, its own tags,
 * whole-release ownership) are computed once, so scanning a release's tracks stays cheap. A track
 * carries its release's tags and matches a search on its release's fields.
 */
export function releaseTrackMatcher(
	release: DiscoveryRelease,
	c: DiscoveryCriteria,
	ctx: FacetContext
): (trackIndex: number) => boolean {
	const { liked, purchased, downloaded } = c.facets
	const search = normalizeSearch(c.search)
	const releaseSearchHit = search !== '' && releaseFieldsMatch(release, search)
	const fullyOwned = ctx.fullyOwnedIds.has(release.id)
	const releaseTagIds = c.tagIds.length > 0 ? new Set(release.tags.map((t) => t.id)) : null
	return (trackIndex) => {
		const track = release.tracks[trackIndex]
		if (!track) return false
		if (!matchesTriState(liked, track.is_liked)) return false
		if (!matchesTriState(purchased, fullyOwned || ctx.ownedTrackIds.has(track.id))) return false
		if (!matchesTriState(downloaded, ctx.cachedTrackIds.has(track.id))) return false
		if (search && !releaseSearchHit && !(track.name?.toLowerCase().includes(search) ?? false)) return false
		if (
			releaseTagIds &&
			!matchesTags(
				c.tagIds,
				c.tagMode,
				(id) => releaseTagIds.has(id) || (track.tags?.some((t) => t.id === id) ?? false)
			)
		) {
			return false
		}
		return true
	}
}

/** Whether one track passes every track-level filter. Scanning a whole release? Use `releaseTrackMatcher`. */
export function trackMatches(
	release: DiscoveryRelease,
	trackIndex: number,
	c: DiscoveryCriteria,
	ctx: FacetContext
): boolean {
	return releaseTrackMatcher(release, c, ctx)(trackIndex)
}

// A release with no tracks yet (a follow-watch import awaiting enrichment) has nothing to judge per
// track, so it stands in as ONE track carrying only what the release knows: never liked or downloaded,
// purchased only when owned whole, its own tags, its own fields for search.
function standInMatches(release: DiscoveryRelease, c: DiscoveryCriteria, ctx: FacetContext): boolean {
	const search = normalizeSearch(c.search)
	return (
		matchesTriState(c.facets.liked, false) &&
		matchesTriState(c.facets.purchased, ctx.fullyOwnedIds.has(release.id)) &&
		matchesTriState(c.facets.downloaded, false) &&
		(!search || releaseFieldsMatch(release, search)) &&
		(c.tagIds.length === 0 || matchesTags(c.tagIds, c.tagMode, (id) => release.tags.some((t) => t.id === id)))
	)
}

function releaseMatches(
	release: DiscoveryRelease,
	c: DiscoveryCriteria,
	ctx: FacetContext,
	judgeTracks: boolean
): boolean {
	if (!matchesTriState(c.facets.new, release.is_new)) return false
	if (!judgeTracks) return true
	if (release.tracks.length === 0) return standInMatches(release, c, ctx)
	const matches = releaseTrackMatcher(release, c, ctx)
	for (let i = 0; i < release.tracks.length; i++) {
		if (matches(i)) return true
	}
	return false
}

/** Narrow a release list by its criteria (all filters AND-composed). Always returns a new array. */
export function filterDiscoveryReleases(
	releases: readonly DiscoveryRelease[],
	c: DiscoveryCriteria,
	ctx: FacetContext
): DiscoveryRelease[] {
	const judgeTracks = hasTrackCriteria(c)
	if (!judgeTracks && c.facets.new === 'off') return [...releases]
	return releases.filter((r) => releaseMatches(r, c, ctx, judgeTracks))
}

/** The track scope a list's criteria impose (null when no track-level filter is active). */
export function trackScopeOf(c: DiscoveryCriteria, ctx: FacetContext): TrackScope | null {
	return hasTrackCriteria(c) ? { criteria: c, ctx } : null
}

/** What a preview started from a list plays through: its releases, and the tracks it may continue into
 *  (null = every playable track). */
export interface PreviewPlaybackContext {
	releases: DiscoveryRelease[]
	scope: TrackScope | null
}

/**
 * The playback context of a filtered list. With "Apply filters to playback" on, it is exactly what's
 * on screen — the filtered releases, narrowed to their matching tracks. Off, filters stop shaping
 * playback: the view's unfiltered source in the same order (a null `sort` keeps the source's natural
 * order, e.g. a playlist's).
 */
export function previewPlaybackContext(
	displayed: DiscoveryRelease[],
	source: readonly DiscoveryRelease[],
	c: DiscoveryCriteria,
	ctx: FacetContext,
	sort: DiscoverySortConfig | null,
	followsFilters: boolean
): PreviewPlaybackContext {
	if (followsFilters) return { releases: displayed, scope: trackScopeOf(c, ctx) }
	return { releases: sort ? sortDiscoveryReleases([...source], sort) : [...source], scope: null }
}
