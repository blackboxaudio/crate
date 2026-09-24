import type { TagCategory, TagSelectionState, DiscoveryRelease } from '../types'
import { SvelteMap } from 'svelte/reactivity'

/**
 * Whether a release carries a tag at either level: on the release itself or on any of its
 * tracks. Every release-list tag filter goes through this so a track tagged "the one for the
 * mix" surfaces its release under that tag.
 */
export function releaseHasTag(release: DiscoveryRelease, tagId: string): boolean {
	if (release.tags.some((t) => t.id === tagId)) return true
	return release.tracks.some((track) => track.tags?.some((t) => t.id === tagId) ?? false)
}

type TagStates = { states: Map<string, TagSelectionState>; counts: Map<string, number> }

/** Per-tag state over a selection, from the tag lists of the `totalSelected` selected items. */
function computeStatesFromTagLists(
	tagCategories: TagCategory[],
	tagLists: Iterable<{ id: string }[]>,
	totalSelected: number
): TagStates {
	const states = new SvelteMap<string, TagSelectionState>()
	const counts = new SvelteMap<string, number>()
	if (totalSelected === 0) return { states, counts }

	const tagCountMap = new SvelteMap<string, number>()
	for (const tags of tagLists) {
		for (const tag of tags) {
			tagCountMap.set(tag.id, (tagCountMap.get(tag.id) || 0) + 1)
		}
	}
	for (const tag of tagCategories.flatMap((c) => c.tags)) {
		const count = tagCountMap.get(tag.id) || 0
		counts.set(tag.id, count)
		if (count === 0) states.set(tag.id, 'inactive')
		else if (count === totalSelected) states.set(tag.id, 'active')
		else states.set(tag.id, 'mixed')
	}
	return { states, counts }
}

export function computeDiscoveryTagStates(
	tagCategories: TagCategory[],
	releases: DiscoveryRelease[],
	selectedIds: Set<string>
): TagStates {
	const selectedReleases = selectedIds.size > 0 ? releases.filter((r) => selectedIds.has(r.id)) : []
	return computeStatesFromTagLists(
		tagCategories,
		selectedReleases.map((r) => r.tags),
		selectedReleases.length
	)
}

/** Same as `computeDiscoveryTagStates`, but over selected TRACKS and their track-level tags. */
export function computeDiscoveryTrackTagStates(
	tagCategories: TagCategory[],
	releases: DiscoveryRelease[],
	selectedTrackIds: Set<string>
): TagStates {
	const selectedTracks =
		selectedTrackIds.size > 0 ? releases.flatMap((r) => r.tracks.filter((t) => selectedTrackIds.has(t.id))) : []
	return computeStatesFromTagLists(
		tagCategories,
		selectedTracks.map((t) => t.tags ?? []),
		selectedTracks.length
	)
}
