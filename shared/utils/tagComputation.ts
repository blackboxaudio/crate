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

export function computeDiscoveryTagStates(
	tagCategories: TagCategory[],
	releases: DiscoveryRelease[],
	selectedIds: Set<string>
): { states: Map<string, TagSelectionState>; counts: Map<string, number> } {
	const states = new SvelteMap<string, TagSelectionState>()
	const counts = new SvelteMap<string, number>()

	if (selectedIds.size > 0) {
		const selectedReleases = releases.filter((r) => selectedIds.has(r.id))
		const totalSelected = selectedReleases.length
		if (totalSelected > 0) {
			const tagCountMap = new SvelteMap<string, number>()
			for (const release of selectedReleases) {
				for (const tag of release.tags) {
					tagCountMap.set(tag.id, (tagCountMap.get(tag.id) || 0) + 1)
				}
			}
			const allTags = tagCategories.flatMap((c) => c.tags)
			for (const tag of allTags) {
				const count = tagCountMap.get(tag.id) || 0
				counts.set(tag.id, count)
				if (count === 0) states.set(tag.id, 'inactive')
				else if (count === totalSelected) states.set(tag.id, 'active')
				else states.set(tag.id, 'mixed')
			}
		}
	}

	return { states, counts }
}
