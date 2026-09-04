import type { DiscoveryFacet, DiscoveryFacetFilters, DiscoveryRelease, FilterTriState } from '../types'

// The one implementation of the discovery facet filters (Liked / New / Purchased / Downloaded). Every
// release list on both platforms — the feed, discovery playlists, mobile detail views, the Purchased
// view — narrows through `applyDiscoveryFilters`, so "exclude" means the same thing everywhere.

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

/** The id sets the purchased / downloaded predicates look up. Named (not positional) because both are
 *  `ReadonlySet<string>` and the two were being passed in opposite orders at different call sites. */
export interface FacetContext {
	/** Releases owned in the linked purchase collection(s) — `ownedReleaseIds`. */
	ownedIds: ReadonlySet<string>
	/** Releases whose every track's audio is cached on disk — `fullyCachedIds`. */
	cachedIds: ReadonlySet<string>
}

/** Narrow a release list by the four facets (AND-composed). Always returns a new array. */
export function applyDiscoveryFilters(
	releases: readonly DiscoveryRelease[],
	facets: DiscoveryFacetFilters,
	ctx: FacetContext
): DiscoveryRelease[] {
	if (!hasActiveFacets(facets)) return [...releases]
	return releases.filter(
		(r) =>
			matchesTriState(
				facets.liked,
				r.tracks.some((t) => t.is_liked)
			) &&
			matchesTriState(facets.new, r.is_new) &&
			matchesTriState(facets.purchased, ctx.ownedIds.has(r.id)) &&
			matchesTriState(facets.downloaded, ctx.cachedIds.has(r.id))
	)
}
