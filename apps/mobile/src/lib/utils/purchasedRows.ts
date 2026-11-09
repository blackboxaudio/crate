import type { CollectionItem, DiscoveryRelease } from '$shared/types'
import {
	filterDiscoveryReleases,
	hasActiveFacets,
	type DiscoveryCriteria,
	type FacetContext,
} from '$shared/utils/discoveryFilters'

// The Purchased view's rows: the linked Bandcamp collection, newest purchase first (the backend's order),
// each item paired with the discovery release it matched. Kept out of the component because the playback
// queue has to span exactly these rows in exactly this order — see `mobilePurchasedRows` in mobileUI.

export interface PurchasedRow {
	id: string
	item: CollectionItem
	release: DiscoveryRelease | undefined
}

export function pairCollectionItems(
	items: readonly CollectionItem[],
	releases: readonly DiscoveryRelease[]
): PurchasedRow[] {
	const releaseById = new Map(releases.map((r) => [r.id, r]))
	return items.map((item) => ({
		id: item.id,
		item,
		release: item.matchedReleaseId ? releaseById.get(item.matchedReleaseId) : undefined,
	}))
}

/** The rows' matched releases in row order, each once: a release bought track by track has a row per
 *  purchase, but the playback queue keys its items by release. */
export function matchedReleases(rows: readonly PurchasedRow[]): DiscoveryRelease[] {
	const seen = new Set<string>()
	const releases: DiscoveryRelease[] = []
	for (const { release } of rows) {
		if (!release || seen.has(release.id)) continue
		seen.add(release.id)
		releases.push(release)
	}
	return releases
}

/**
 * Narrow the rows by the view's criteria (the Purchased facet already forced off). Matched rows go through
 * the shared engine, so "Purchased + X" and the search mean the same thing here as on the feed. Unmatched
 * items have no tracks, likes, or tags to judge, so any facet or tag hides them rather than letting them
 * ride along unfiltered under an active filter; a search still finds them by the purchase's own artist /
 * title.
 */
export function filterPurchasedRows(rows: PurchasedRow[], c: DiscoveryCriteria, ctx: FacetContext): PurchasedRow[] {
	const search = c.search.trim().toLowerCase()
	const releaseFiltersActive = hasActiveFacets(c.facets) || c.tagIds.length > 0
	if (!releaseFiltersActive && !search) return rows
	const kept = new Set(filterDiscoveryReleases(matchedReleases(rows), c, ctx).map((r) => r.id))
	return rows.filter(({ item, release }) => {
		if (release) return kept.has(release.id)
		if (releaseFiltersActive) return false
		return !!(item.artist?.toLowerCase().includes(search) || item.title?.toLowerCase().includes(search))
	})
}
