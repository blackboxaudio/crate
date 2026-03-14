import { writable, derived } from 'svelte/store'
import type { TagCategory, Tag, TagSelectionState, Track } from '$lib/types'
import * as tagsApi from '$lib/api/tags'
import { toastStore } from './toast'

// =============================================================================
// State
// =============================================================================

interface TagsState {
	categories: TagCategory[]
	loading: boolean
	error: string | null
}

const initialState: TagsState = {
	categories: [],
	loading: false,
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createTagsStore() {
	const { subscribe, set, update } = writable<TagsState>(initialState)

	return {
		subscribe,

		/**
		 * Load all tag categories and tags
		 */
		async load() {
			update((state) => ({ ...state, loading: true, error: null }))

			try {
				const categories = await tagsApi.getTagCategories()
				update((state) => ({
					...state,
					categories,
					loading: false,
				}))
			} catch (error) {
				const errorMessage = error instanceof Error ? error.message : 'Failed to load tags'
				update((state) => ({
					...state,
					loading: false,
					error: errorMessage,
				}))
				toastStore.error(errorMessage)
			}
		},

		/**
		 * Create a new tag category
		 */
		async createCategory(name: string, color?: string) {
			try {
				const category = await tagsApi.createTagCategory(name, color)
				update((state) => ({
					...state,
					categories: [...state.categories, category],
				}))
				return category
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to create category',
				}))
				return null
			}
		},

		/**
		 * Update a tag category
		 */
		async updateCategory(id: string, name?: string, color?: string) {
			try {
				const updated = await tagsApi.updateTagCategory(id, name, color)
				update((state) => ({
					...state,
					categories: state.categories.map((c) => (c.id === id ? updated : c)),
				}))
				return updated
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to update category',
				}))
				return null
			}
		},

		/**
		 * Delete a tag category
		 */
		async deleteCategory(id: string) {
			try {
				await tagsApi.deleteTagCategory(id)
				update((state) => ({
					...state,
					categories: state.categories.filter((c) => c.id !== id),
				}))
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to delete category',
				}))
			}
		},

		/**
		 * Create a new tag
		 */
		async createTag(categoryId: string, name: string, color?: string) {
			try {
				const tag = await tagsApi.createTag(categoryId, name, color)
				update((state) => ({
					...state,
					categories: state.categories.map((c) => {
						if (c.id === categoryId) {
							return { ...c, tags: [...c.tags, tag] }
						}
						return c
					}),
				}))
				return tag
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to create tag',
				}))
				return null
			}
		},

		/**
		 * Update a tag
		 */
		async updateTag(id: string, name?: string, color?: string) {
			try {
				const updated = await tagsApi.updateTag(id, name, color)
				update((state) => ({
					...state,
					categories: state.categories.map((c) => ({
						...c,
						tags: c.tags.map((t) => (t.id === id ? updated : t)),
					})),
				}))
				return updated
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to update tag',
				}))
				return null
			}
		},

		/**
		 * Move a tag to a different category
		 */
		async moveTag(tagId: string, targetCategoryId: string) {
			const tag = await tagsApi.moveTag(tagId, targetCategoryId)
			update((state) => ({
				...state,
				categories: state.categories.map((c) => {
					if (c.id === targetCategoryId) {
						// Add tag to target category (if not already there)
						const exists = c.tags.some((t) => t.id === tag.id)
						return exists ? c : { ...c, tags: [...c.tags, tag] }
					}
					// Remove tag from other categories
					return { ...c, tags: c.tags.filter((t) => t.id !== tag.id) }
				}),
			}))
			return tag
		},

		/**
		 * Delete a tag
		 */
		async deleteTag(id: string) {
			try {
				await tagsApi.deleteTag(id)
				update((state) => ({
					...state,
					categories: state.categories.map((c) => ({
						...c,
						tags: c.tags.filter((t) => t.id !== id),
					})),
				}))
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to delete tag',
				}))
			}
		},

		/**
		 * Assign tags to tracks
		 */
		async assignTags(trackIds: string[], tagIds: string[]) {
			try {
				await tagsApi.assignTags(trackIds, tagIds)
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to assign tags',
				}))
			}
		},

		/**
		 * Remove tags from tracks
		 */
		async removeTags(trackIds: string[], tagIds: string[]) {
			try {
				await tagsApi.removeTags(trackIds, tagIds)
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to remove tags',
				}))
			}
		},

		/**
		 * Reset store to initial state
		 */
		reset() {
			set(initialState)
		},
	}
}

export const tagsStore = createTagsStore()

// =============================================================================
// Derived Stores
// =============================================================================

/**
 * All tags flattened
 */
export const allTags = derived(tagsStore, ($tags) => $tags.categories.flatMap((c) => c.tags))

/**
 * Get tag by ID
 */
export function getTagById(tags: Tag[], id: string): Tag | undefined {
	return tags.find((t) => t.id === id)
}

/**
 * Get category by ID
 */
export function getCategoryById(categories: TagCategory[], id: string): TagCategory | undefined {
	return categories.find((c) => c.id === id)
}

/**
 * Compute tag selection states for selected tracks
 * Returns a map of tag ID to its selection state (active, inactive, mixed)
 * and a map of tag ID to count (how many selected tracks have that tag)
 */
export function computeTagStates(
	categories: TagCategory[],
	tracks: Track[],
	selectedIds: Set<string>
): { states: Map<string, TagSelectionState>; counts: Map<string, number> } {
	const states = new Map<string, TagSelectionState>()
	const counts = new Map<string, number>()

	if (selectedIds.size === 0) {
		return { states, counts }
	}

	// Get selected tracks
	const selectedTracks = tracks.filter((t) => selectedIds.has(t.id))
	const totalSelected = selectedTracks.length

	if (totalSelected === 0) {
		return { states, counts }
	}

	// Count occurrences of each tag across selected tracks
	const tagCounts = new Map<string, number>()
	for (const track of selectedTracks) {
		for (const tag of track.tags) {
			tagCounts.set(tag.id, (tagCounts.get(tag.id) || 0) + 1)
		}
	}

	// Compute states for all tags
	const allTags = categories.flatMap((c) => c.tags)
	for (const tag of allTags) {
		const count = tagCounts.get(tag.id) || 0
		counts.set(tag.id, count)

		if (count === 0) {
			states.set(tag.id, 'inactive')
		} else if (count === totalSelected) {
			states.set(tag.id, 'active')
		} else {
			states.set(tag.id, 'mixed')
		}
	}

	return { states, counts }
}
