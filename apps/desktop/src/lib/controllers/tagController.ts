import type {
	ActiveView,
	Tag,
	TagCategory,
	TagSelectionState,
	TagFilterMode,
	TrackFilter,
	DiscoveryFilter,
} from '$shared/types'
import type { tagsStore as TagsStoreType } from '$shared/stores/tags'
import type { libraryStore as LibraryStoreType } from '$lib/stores/library'
import type { discoveryStore as DiscoveryStoreType } from '$shared/stores/discovery'
import type { uiStore as UIStoreType } from '$shared/stores/ui'

// =============================================================================
// Types
// =============================================================================

export interface TagControllerDeps {
	tagsStore: typeof TagsStoreType
	libraryStore: typeof LibraryStoreType
	discoveryStore: typeof DiscoveryStoreType
	uiStore: typeof UIStoreType
	// Getters for reactive state (these return current values)
	getSelectedTagIds: () => string[]
	getSelectedPlaylistId: () => string | null
	getTagFilterMode: () => TagFilterMode
	getSelectedTrackIds: () => Set<string>
	getSelectedReleaseIds: () => Set<string>
	getRecentlyToggledMixedTags: () => Set<string>
	getActiveView: () => ActiveView
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
		discoveryStore,
		uiStore,
		getSelectedTagIds,
		getSelectedPlaylistId,
		getTagFilterMode,
		getSelectedTrackIds,
		getSelectedReleaseIds,
		getRecentlyToggledMixedTags,
		getActiveView,
	} = deps

	/**
	 * Toggle a tag in the filter and reload tracks/releases
	 */
	async function selectTag(tagId: string): Promise<void> {
		// Capture current state BEFORE toggling (subscription updates synchronously)
		const selectedTagIds = getSelectedTagIds()
		const selectedPlaylistId = getSelectedPlaylistId()
		const tagFilterMode = getTagFilterMode()

		const wasSelected = selectedTagIds.includes(tagId)
		const updatedTagIds = wasSelected ? selectedTagIds.filter((id) => id !== tagId) : [...selectedTagIds, tagId]

		uiStore.toggleTagFilter(tagId)

		if (getActiveView() === 'discovery') {
			const filter: DiscoveryFilter = {}
			if (updatedTagIds.length > 0) {
				filter.tag_ids = updatedTagIds
				filter.tag_filter_mode = tagFilterMode
			}
			await discoveryStore.loadReleases(Object.keys(filter).length > 0 ? filter : undefined)
		} else {
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
	}

	/**
	 * Toggle a tag on/off for the currently selected tracks or releases
	 */
	async function toggleTagOnTracks(tagId: string, currentState: TagSelectionState): Promise<void> {
		if (getActiveView() === 'discovery') {
			const releaseIds = Array.from(getSelectedReleaseIds())

			if (currentState === 'active') {
				await discoveryStore.removeTags(releaseIds, [tagId])
			} else if (currentState === 'inactive') {
				await discoveryStore.assignTags(releaseIds, [tagId])
			} else if (currentState === 'mixed') {
				const wasRecentlyToggled = getRecentlyToggledMixedTags().has(tagId)
				if (wasRecentlyToggled) {
					await discoveryStore.assignTags(releaseIds, [tagId])
					uiStore.clearRecentlyToggledTag(tagId)
				} else {
					await discoveryStore.removeTags(releaseIds, [tagId])
					uiStore.markTagAsRecentlyToggled(tagId)
				}
			}
		} else {
			const trackIds = Array.from(getSelectedTrackIds())

			if (currentState === 'active') {
				await tagsStore.removeTags(trackIds, [tagId])
			} else if (currentState === 'inactive') {
				await tagsStore.assignTags(trackIds, [tagId])
			} else if (currentState === 'mixed') {
				const wasRecentlyToggled = getRecentlyToggledMixedTags().has(tagId)
				if (wasRecentlyToggled) {
					await tagsStore.assignTags(trackIds, [tagId])
					uiStore.clearRecentlyToggledTag(tagId)
				} else {
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
	}

	/**
	 * Clear all tag filters and reload tracks/releases
	 */
	async function clearTagFilters(): Promise<void> {
		const selectedPlaylistId = getSelectedPlaylistId()

		uiStore.clearTagFilters()

		if (getActiveView() === 'discovery') {
			await discoveryStore.loadReleases()
		} else {
			libraryStore.clearFilters()
			if (selectedPlaylistId) {
				await libraryStore.loadPlaylistTracks(selectedPlaylistId)
			} else {
				await libraryStore.loadTracks()
			}
		}
	}

	/**
	 * Remove a single tag from the filter and reload tracks/releases
	 */
	async function removeTagFilter(tagId: string): Promise<void> {
		const selectedTagIds = getSelectedTagIds()
		const selectedPlaylistId = getSelectedPlaylistId()
		const tagFilterMode = getTagFilterMode()

		uiStore.removeTagFilter(tagId)
		const updatedTagIds = selectedTagIds.filter((id) => id !== tagId)

		if (getActiveView() === 'discovery') {
			const filter: DiscoveryFilter = {}
			if (updatedTagIds.length > 0) {
				filter.tag_ids = updatedTagIds
				filter.tag_filter_mode = tagFilterMode
			}
			await discoveryStore.loadReleases(Object.keys(filter).length > 0 ? filter : undefined)
		} else {
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
	}

	/**
	 * Toggle tag filter mode between AND and OR
	 */
	async function toggleTagFilterMode(): Promise<void> {
		const selectedTagIds = getSelectedTagIds()
		const selectedPlaylistId = getSelectedPlaylistId()
		const tagFilterMode = getTagFilterMode()

		uiStore.toggleTagFilterMode()

		// Reload with the new mode if tags are selected
		if (selectedTagIds.length > 0) {
			const newMode = tagFilterMode === 'or' ? 'and' : 'or'

			if (getActiveView() === 'discovery') {
				const filter: DiscoveryFilter = {
					tag_ids: selectedTagIds,
					tag_filter_mode: newMode,
				}
				await discoveryStore.loadReleases(filter)
			} else {
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
