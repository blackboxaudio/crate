import { writable } from 'svelte/store'
import { getStoredSet, setStoredSet } from '$lib/utils/storage'

const STORAGE_KEY = 'expandedReleaseIds'

function createExpandedReleaseIdsStore() {
	const { subscribe, set, update } = writable<Set<string>>(getStoredSet(STORAGE_KEY))

	// Persist on every change
	subscribe((ids) => {
		setStoredSet(STORAGE_KEY, ids)
	})

	return {
		subscribe,

		toggle(id: string) {
			update((ids) => {
				const next = new Set(ids)
				if (next.has(id)) {
					next.delete(id)
				} else {
					next.add(id)
				}
				return next
			})
		},

		expand(id: string) {
			update((ids) => {
				if (ids.has(id)) return ids
				const next = new Set(ids)
				next.add(id)
				return next
			})
		},

		expandAll(ids: string[]) {
			update((current) => {
				const next = new Set(current)
				for (const id of ids) next.add(id)
				return next
			})
		},

		collapseAll() {
			set(new Set())
		},

		toggleSelection(releaseIds: string[], expandableFilter: (id: string) => boolean) {
			update((current) => {
				const expandableIds = releaseIds.filter(expandableFilter)
				if (expandableIds.length === 0) return current
				const allExpanded = expandableIds.every((id) => current.has(id))
				const next = new Set(current)
				if (allExpanded) {
					for (const id of expandableIds) next.delete(id)
				} else {
					for (const id of expandableIds) next.add(id)
				}
				return next
			})
		},
	}
}

export const expandedReleaseIds = createExpandedReleaseIdsStore()
