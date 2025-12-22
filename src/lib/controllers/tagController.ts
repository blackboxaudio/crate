import type { Tag, TagCategory, TagSelectionState, TagFilterMode, TrackFilter } from '$lib/types'
import type { tagsStore as TagsStoreType } from '$lib/stores/tags'
import type { libraryStore as LibraryStoreType } from '$lib/stores/library'
import type { uiStore as UIStoreType } from '$lib/stores/ui'

// =============================================================================
// Types
// =============================================================================

export interface TagControllerDeps {
	tagsStore: typeof TagsStoreType
	libraryStore: typeof LibraryStoreType
	uiStore: typeof UIStoreType
	// Getters for reactive state (these return current values)
	getSelectedTagIds: () => string[]
	getSelectedPlaylistId: () => string | null
	getTagFilterMode: () => TagFilterMode
	getSelectedTrackIds: () => Set<string>
	getRecentlyToggledMixedTags: () => Set<string>
}

export interface TagControllerModalActions {
	openRenameTagModal: (tag: Tag) => void
	openDeleteTagModal: (tag: Tag) => void
	openRenameCategoryModal: (category: TagCategory) => void
	openDeleteCategoryModal: (category: TagCategory) => void
}

export interface TagController {
	// Tag filter operations
	selectTag: (tagId: string) => Promise<void>
	clearTagFilters: () => Promise<void>
	removeTagFilter: (tagId: string) => Promise<void>
	toggleTagFilterMode: () => Promise<void>

	// Tag assignment operations (when tracks are selected)
	toggleTagOnTracks: (tagId: string, currentState: TagSelectionState) => Promise<void>

	// Category operations
	changeCategoryColor: (category: TagCategory, color: string | null) => Promise<void>

	// Modal-opening operations (optional, requires modalActions)
	renameTag: (tag: Tag) => void
	deleteTag: (tag: Tag) => void
	renameCategory: (category: TagCategory) => void
	deleteCategory: (category: TagCategory) => void
}

// =============================================================================
// Controller Factory
// =============================================================================

export function createTagController(deps: TagControllerDeps, modalActions?: TagControllerModalActions): TagController {
	const {
		tagsStore,
		libraryStore,
		uiStore,
		getSelectedTagIds,
		getSelectedPlaylistId,
		getTagFilterMode,
		getSelectedTrackIds,
		getRecentlyToggledMixedTags,
	} = deps

	/**
	 * Toggle a tag in the filter and reload tracks
	 */
	async function selectTag(tagId: string): Promise<void> {
		// Capture current state BEFORE toggling (subscription updates synchronously)
		const selectedTagIds = getSelectedTagIds()
		const selectedPlaylistId = getSelectedPlaylistId()
		const tagFilterMode = getTagFilterMode()

		const wasSelected = selectedTagIds.includes(tagId)
		const updatedTagIds = wasSelected ? selectedTagIds.filter((id) => id !== tagId) : [...selectedTagIds, tagId]

		uiStore.toggleTagFilter(tagId)

		const filter: TrackFilter = {}
		if (updatedTagIds.length > 0) {
			filter.tag_ids = updatedTagIds
			filter.tag_filter_mode = tagFilterMode
		}
		if (selectedPlaylistId) {
			filter.playlist_id = selectedPlaylistId
		}
		await libraryStore.loadTracks(Object.keys(filter).length > 0 ? filter : undefined)
	}

	/**
	 * Toggle a tag on/off for the currently selected tracks
	 */
	async function toggleTagOnTracks(tagId: string, currentState: TagSelectionState): Promise<void> {
		const trackIds = Array.from(getSelectedTrackIds())

		if (currentState === 'active') {
			// Remove from all selected tracks
			await tagsStore.removeTags(trackIds, [tagId])
		} else if (currentState === 'inactive') {
			// Add to all selected tracks
			await tagsStore.assignTags(trackIds, [tagId])
		} else if (currentState === 'mixed') {
			// Check if this tag was recently toggled
			const wasRecentlyToggled = getRecentlyToggledMixedTags().has(tagId)

			if (wasRecentlyToggled) {
				// Second click on mixed = add to all
				await tagsStore.assignTags(trackIds, [tagId])
				uiStore.clearRecentlyToggledTag(tagId)
			} else {
				// First click on mixed = remove from all
				await tagsStore.removeTags(trackIds, [tagId])
				uiStore.markTagAsRecentlyToggled(tagId)
			}
		}

		// Reload tracks to reflect tag changes
		const selectedPlaylistId = getSelectedPlaylistId()
		if (selectedPlaylistId) {
			await libraryStore.loadPlaylistTracks(selectedPlaylistId)
		} else {
			await libraryStore.loadTracks()
		}
	}

	/**
	 * Clear all tag filters and reload tracks
	 */
	async function clearTagFilters(): Promise<void> {
		const selectedPlaylistId = getSelectedPlaylistId()

		uiStore.clearTagFilters()
		libraryStore.clearFilters()

		if (selectedPlaylistId) {
			await libraryStore.loadPlaylistTracks(selectedPlaylistId)
		} else {
			await libraryStore.loadTracks()
		}
	}

	/**
	 * Remove a single tag from the filter and reload tracks
	 */
	async function removeTagFilter(tagId: string): Promise<void> {
		const selectedTagIds = getSelectedTagIds()
		const selectedPlaylistId = getSelectedPlaylistId()
		const tagFilterMode = getTagFilterMode()

		uiStore.removeTagFilter(tagId)
		const updatedTagIds = selectedTagIds.filter((id) => id !== tagId)

		const filter: TrackFilter = {}
		if (updatedTagIds.length > 0) {
			filter.tag_ids = updatedTagIds
			filter.tag_filter_mode = tagFilterMode
		}
		if (selectedPlaylistId) {
			filter.playlist_id = selectedPlaylistId
		}

		if (Object.keys(filter).length > 0) {
			await libraryStore.loadTracks(filter)
		} else {
			libraryStore.clearFilters()
			await libraryStore.loadTracks()
		}
	}

	/**
	 * Toggle tag filter mode between AND and OR
	 */
	async function toggleTagFilterMode(): Promise<void> {
		const selectedTagIds = getSelectedTagIds()
		const selectedPlaylistId = getSelectedPlaylistId()
		const tagFilterMode = getTagFilterMode()

		uiStore.toggleTagFilterMode()

		// Reload tracks with the new mode if tags are selected
		if (selectedTagIds.length > 0) {
			// Note: tagFilterMode still has the OLD value here since we just called toggleTagFilterMode
			// The actual new mode is the opposite
			const newMode = tagFilterMode === 'or' ? 'and' : 'or'
			const filter: TrackFilter = {
				tag_ids: selectedTagIds,
				tag_filter_mode: newMode,
			}
			if (selectedPlaylistId) {
				filter.playlist_id = selectedPlaylistId
			}
			await libraryStore.loadTracks(filter)
		}
	}

	/**
	 * Change a category's color
	 */
	async function changeCategoryColor(category: TagCategory, color: string | null): Promise<void> {
		await tagsStore.updateCategory(category.id, undefined, color ?? undefined)
	}

	/**
	 * Open the rename tag modal
	 */
	function renameTag(tag: Tag): void {
		if (!modalActions) {
			console.warn('TagController: modalActions not provided, cannot open rename modal')
			return
		}
		modalActions.openRenameTagModal(tag)
	}

	/**
	 * Open the delete tag confirmation modal
	 */
	function deleteTag(tag: Tag): void {
		if (!modalActions) {
			console.warn('TagController: modalActions not provided, cannot open delete modal')
			return
		}
		modalActions.openDeleteTagModal(tag)
	}

	/**
	 * Open the rename category modal
	 */
	function renameCategory(category: TagCategory): void {
		if (!modalActions) {
			console.warn('TagController: modalActions not provided, cannot open rename modal')
			return
		}
		modalActions.openRenameCategoryModal(category)
	}

	/**
	 * Open the delete category confirmation modal
	 */
	function deleteCategory(category: TagCategory): void {
		if (!modalActions) {
			console.warn('TagController: modalActions not provided, cannot open delete modal')
			return
		}
		modalActions.openDeleteCategoryModal(category)
	}

	return {
		selectTag,
		toggleTagOnTracks,
		clearTagFilters,
		removeTagFilter,
		toggleTagFilterMode,
		changeCategoryColor,
		renameTag,
		deleteTag,
		renameCategory,
		deleteCategory,
	}
}
