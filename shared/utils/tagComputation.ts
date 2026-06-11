import type { TagCategory, TagSelectionState, DiscoveryRelease } from '../types'
import { SvelteMap } from 'svelte/reactivity'

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
