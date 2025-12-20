<script lang="ts" module>
	import type {
		Track,
		Playlist,
		Tag,
		TagCategory,
		UsbDevice,
		MoveConflict,
		DuplicateTrack,
		DuplicateResolutionAction,
	} from '$lib/types'

	// Discriminated union for all modal states
	export type ActiveModal =
		| { type: 'none' }
		// Creation modals
		| { type: 'createPlaylist'; parentId: string | null }
		| { type: 'createFolder'; parentId: string | null }
		| { type: 'createCategory' }
		| { type: 'createTag'; categoryId: string }
		// Rename modals
		| { type: 'renamePlaylist'; playlist: Playlist }
		| { type: 'renameTag'; tag: Tag }
		| { type: 'renameCategory'; category: TagCategory }
		// Delete/Confirmation modals
		| { type: 'deletePlaylist'; playlist: Playlist; hasChildren: boolean }
		| { type: 'deleteTag'; tag: Tag }
		| { type: 'deleteCategory'; category: TagCategory }
		| { type: 'removeFromPlaylist'; trackIds: string[]; playlistId: string }
		| { type: 'removeFromLibrary'; trackIds: string[] }
		// Feature modals
		| { type: 'tagInput' }
		| { type: 'deviceInfo'; device: UsbDevice }
		| { type: 'relocate'; track: Track }
		| {
				type: 'moveConflict'
				movingItem: Playlist
				existingItem: Playlist
				targetParentId: string | null
		  }
		| { type: 'settings' }
		| {
				type: 'duplicateTrack'
				duplicates: DuplicateTrack[]
				currentIndex: number
				applyToAllAction: DuplicateResolutionAction | null
				resolvedTracks: Track[]
				onComplete: (tracks: Track[]) => void
		  }

	// Move resolution result type
	export type MoveResult = {
		success: boolean
		nestedConflicts: MoveConflict[]
	}
</script>

<script lang="ts">
	import InputModal from './InputModal.svelte'
	import ConfirmModal from './ConfirmModal.svelte'
	import MoveConflictModal from './MoveConflictModal.svelte'
	import DuplicateTrackModal from './DuplicateTrackModal.svelte'
	import { TagInputModal } from '$lib/components/tags'
	import { DeviceInfoModal } from '$lib/components/devices'
	import { SettingsModal } from '$lib/components/settings'
	import { RelocateTrackModal } from '$lib/components/library'
	import { toastStore } from '$lib/stores/toast'
	import { resolveDuplicate } from '$lib/api/library'

	// =========================================================================
	// Props - Callback handlers passed from parent
	// =========================================================================
	type Props = {
		// Data needed by modals
		playlists: Playlist[]
		tagCategories: TagCategory[]

		// Creation callbacks
		onCreatePlaylist: (name: string, parentId: string | null) => Promise<Playlist | null>
		onCreateFolder: (name: string, parentId: string | null) => Promise<Playlist | null>
		onCreateCategory: (name: string) => Promise<void>
		onCreateTag: (categoryId: string, name: string) => Promise<void>

		// Rename callbacks
		onRenamePlaylist: (id: string, name: string) => Promise<void>
		onRenameTag: (id: string, name: string) => Promise<void>
		onRenameCategory: (id: string, name: string) => Promise<void>

		// Delete callbacks
		onDeletePlaylist: (id: string, deleteTracksFromCollection: boolean) => Promise<void>
		onDeleteTag: (id: string) => Promise<void>
		onDeleteCategory: (id: string) => Promise<void>
		onRemoveFromPlaylist: (trackIds: string[], playlistId: string) => Promise<void>
		onRemoveFromLibrary: (trackIds: string[]) => Promise<void>

		// Move conflict callbacks
		onMoveConflictOverwrite: (movingItemId: string, targetParentId: string | null) => Promise<boolean>
		onMoveConflictMerge: (movingItemId: string, targetParentId: string | null) => Promise<MoveResult>

		// Tag input callback
		onTagInputSubmit: (categoryId: string, tagName: string) => Promise<void>

		// Relocate callback
		onRelocateComplete: (track: Track) => void
	}

	let {
		playlists,
		tagCategories,
		onCreatePlaylist,
		onCreateFolder,
		onCreateCategory,
		onCreateTag,
		onRenamePlaylist,
		onRenameTag,
		onRenameCategory,
		onDeletePlaylist,
		onDeleteTag,
		onDeleteCategory,
		onRemoveFromPlaylist,
		onRemoveFromLibrary,
		onMoveConflictOverwrite,
		onMoveConflictMerge,
		onTagInputSubmit,
		onRelocateComplete,
	}: Props = $props()

	// =========================================================================
	// Internal State
	// =========================================================================
	let activeModal = $state<ActiveModal>({ type: 'none' })
	let pendingMergeConflicts = $state<MoveConflict[]>([])
	let deleteTracksFromCollection = $state(false)

	// =========================================================================
	// Exported Functions - API for parent component
	// =========================================================================

	export function closeAll() {
		activeModal = { type: 'none' }
		deleteTracksFromCollection = false
	}

	// Creation modals
	export function openCreatePlaylistModal(parentId: string | null) {
		activeModal = { type: 'createPlaylist', parentId }
	}

	export function openCreateFolderModal(parentId: string | null) {
		activeModal = { type: 'createFolder', parentId }
	}

	export function openCreateCategoryModal() {
		activeModal = { type: 'createCategory' }
	}

	export function openCreateTagModal(categoryId: string) {
		activeModal = { type: 'createTag', categoryId }
	}

	// Rename modals
	export function openRenamePlaylistModal(playlist: Playlist) {
		activeModal = { type: 'renamePlaylist', playlist }
	}

	export function openRenameTagModal(tag: Tag) {
		activeModal = { type: 'renameTag', tag }
	}

	export function openRenameCategoryModal(category: TagCategory) {
		activeModal = { type: 'renameCategory', category }
	}

	// Delete/Confirmation modals
	export function openDeletePlaylistModal(playlist: Playlist, hasChildren: boolean) {
		deleteTracksFromCollection = false
		activeModal = { type: 'deletePlaylist', playlist, hasChildren }
	}

	export function openDeleteTagModal(tag: Tag) {
		activeModal = { type: 'deleteTag', tag }
	}

	export function openDeleteCategoryModal(category: TagCategory) {
		activeModal = { type: 'deleteCategory', category }
	}

	export function openRemoveFromPlaylistModal(trackIds: string[], playlistId: string) {
		activeModal = { type: 'removeFromPlaylist', trackIds, playlistId }
	}

	export function openRemoveFromLibraryModal(trackIds: string[]) {
		activeModal = { type: 'removeFromLibrary', trackIds }
	}

	// Feature modals
	export function openTagInputModal() {
		activeModal = { type: 'tagInput' }
	}

	export function openDeviceInfoModal(device: UsbDevice) {
		activeModal = { type: 'deviceInfo', device }
	}

	export function openRelocateModal(track: Track) {
		activeModal = { type: 'relocate', track }
	}

	export function openMoveConflictModal(movingItem: Playlist, existingItem: Playlist, targetParentId: string | null) {
		activeModal = { type: 'moveConflict', movingItem, existingItem, targetParentId }
	}

	export function openSettingsModal() {
		activeModal = { type: 'settings' }
	}

	export function openDuplicateTrackModal(duplicates: DuplicateTrack[], onComplete: (tracks: Track[]) => void) {
		if (duplicates.length === 0) {
			onComplete([])
			return
		}

		activeModal = {
			type: 'duplicateTrack',
			duplicates,
			currentIndex: 0,
			applyToAllAction: null,
			resolvedTracks: [],
			onComplete,
		}
	}

	// =========================================================================
	// Internal Handlers - Close modal and invoke parent callback
	// =========================================================================

	// Creation handlers
	async function handleCreatePlaylistSubmit(name: string) {
		if (activeModal.type === 'createPlaylist') {
			const parentId = activeModal.parentId
			closeAll()
			await onCreatePlaylist(name, parentId)
		}
	}

	async function handleCreateFolderSubmit(name: string) {
		if (activeModal.type === 'createFolder') {
			const parentId = activeModal.parentId
			closeAll()
			await onCreateFolder(name, parentId)
		}
	}

	async function handleCreateCategorySubmit(name: string) {
		closeAll()
		await onCreateCategory(name)
	}

	async function handleCreateTagSubmit(name: string) {
		if (activeModal.type === 'createTag') {
			const categoryId = activeModal.categoryId
			closeAll()
			await onCreateTag(categoryId, name)
		}
	}

	// Rename handlers
	async function handleRenamePlaylistSubmit(name: string) {
		if (activeModal.type === 'renamePlaylist') {
			const id = activeModal.playlist.id
			closeAll()
			await onRenamePlaylist(id, name)
		}
	}

	async function handleRenameTagSubmit(name: string) {
		if (activeModal.type === 'renameTag') {
			const id = activeModal.tag.id
			closeAll()
			await onRenameTag(id, name)
		}
	}

	async function handleRenameCategorySubmit(name: string) {
		if (activeModal.type === 'renameCategory') {
			const id = activeModal.category.id
			closeAll()
			await onRenameCategory(id, name)
		}
	}

	// Delete/Confirmation handlers
	async function handleDeletePlaylistConfirm(deleteTracksToo: boolean) {
		if (activeModal.type === 'deletePlaylist') {
			const id = activeModal.playlist.id
			closeAll()
			await onDeletePlaylist(id, deleteTracksToo)
		}
	}

	async function handleDeleteTagConfirm() {
		if (activeModal.type === 'deleteTag') {
			const id = activeModal.tag.id
			closeAll()
			await onDeleteTag(id)
		}
	}

	async function handleDeleteCategoryConfirm() {
		if (activeModal.type === 'deleteCategory') {
			const id = activeModal.category.id
			closeAll()
			await onDeleteCategory(id)
		}
	}

	async function handleRemoveFromPlaylistConfirm() {
		if (activeModal.type === 'removeFromPlaylist') {
			const { trackIds, playlistId } = activeModal
			closeAll()
			await onRemoveFromPlaylist(trackIds, playlistId)
		}
	}

	async function handleRemoveFromLibraryConfirm() {
		if (activeModal.type === 'removeFromLibrary') {
			const trackIds = activeModal.trackIds
			closeAll()
			await onRemoveFromLibrary(trackIds)
		}
	}

	// Tag input handler
	async function handleTagInputSubmit(categoryId: string, tagName: string) {
		closeAll()
		await onTagInputSubmit(categoryId, tagName)
	}

	// Relocate handler
	function handleRelocateComplete(track: Track) {
		closeAll()
		onRelocateComplete(track)
	}

	// Move conflict handlers
	function handleMoveConflictCancel() {
		closeAll()
		pendingMergeConflicts = []
	}

	async function handleMoveConflictOverwrite() {
		if (activeModal.type === 'moveConflict') {
			const { movingItem, targetParentId } = activeModal
			closeAll()
			const result = await onMoveConflictOverwrite(movingItem.id, targetParentId)
			if (result) {
				toastStore.success('Replaced existing item')
			}
			pendingMergeConflicts = []
		}
	}

	async function handleMoveConflictMerge() {
		if (activeModal.type === 'moveConflict') {
			const { movingItem, targetParentId } = activeModal
			closeAll()
			const result = await onMoveConflictMerge(movingItem.id, targetParentId)

			if (result.success) {
				// Check for nested conflicts from merge
				if (result.nestedConflicts.length > 0) {
					// Queue them for sequential resolution
					pendingMergeConflicts = result.nestedConflicts
					processNextMergeConflict()
					return
				} else {
					toastStore.success('Merged successfully')
				}
			}
			pendingMergeConflicts = []
		}
	}

	function processNextMergeConflict() {
		if (pendingMergeConflicts.length === 0) {
			toastStore.success('Merge completed')
			return
		}

		const next = pendingMergeConflicts[0]
		pendingMergeConflicts = pendingMergeConflicts.slice(1)

		// The target is the existing item's parent (which is the folder we're merging into)
		activeModal = {
			type: 'moveConflict',
			movingItem: next.movingItem,
			existingItem: next.existingItem,
			targetParentId: next.existingItem.parent_id,
		}
	}

	// Duplicate track handlers
	async function handleDuplicateResolve(action: DuplicateResolutionAction, applyToAll: boolean) {
		if (activeModal.type !== 'duplicateTrack') return

		const { duplicates, currentIndex, resolvedTracks, onComplete } = activeModal
		const currentDuplicate = duplicates[currentIndex]

		// Process the current duplicate
		const track = await processDuplicateAction(action, currentDuplicate)
		const newResolvedTracks = track ? [...resolvedTracks, track] : resolvedTracks

		// If apply to all, process remaining duplicates with the same action
		if (applyToAll && currentIndex < duplicates.length - 1) {
			const remainingTracks = await processRemainingDuplicates(action, duplicates.slice(currentIndex + 1))
			const allResolved = [...newResolvedTracks, ...remainingTracks]
			closeAll()
			onComplete(allResolved)
			return
		}

		// Check if there are more duplicates to process
		if (currentIndex < duplicates.length - 1) {
			activeModal = {
				type: 'duplicateTrack',
				duplicates,
				currentIndex: currentIndex + 1,
				applyToAllAction: null,
				resolvedTracks: newResolvedTracks,
				onComplete,
			}
		} else {
			// All done
			closeAll()
			onComplete(newResolvedTracks)
		}
	}

	async function processDuplicateAction(
		action: DuplicateResolutionAction,
		duplicate: DuplicateTrack
	): Promise<Track | null> {
		try {
			if (action === 'skip') {
				return await resolveDuplicate({ action: 'skip' })
			} else if (action === 'update_path') {
				return await resolveDuplicate({
					action: 'update_path',
					new_path: duplicate.new_file_path,
				})
			} else {
				return await resolveDuplicate({
					action: 'replace',
					new_path: duplicate.new_file_path,
					new_hash: duplicate.new_file_hash,
				})
			}
		} catch (error) {
			toastStore.error(`Failed to resolve duplicate: ${error}`)
			return null
		}
	}

	async function processRemainingDuplicates(
		action: DuplicateResolutionAction,
		duplicates: DuplicateTrack[]
	): Promise<Track[]> {
		const tracks: Track[] = []
		for (const dup of duplicates) {
			const track = await processDuplicateAction(action, dup)
			if (track) {
				tracks.push(track)
			}
		}
		return tracks
	}

	function handleDuplicateCancel() {
		if (activeModal.type === 'duplicateTrack') {
			const { resolvedTracks, onComplete } = activeModal
			closeAll()
			// Return any tracks that were already resolved
			onComplete(resolvedTracks)
		}
	}

	// Derived values for delete warnings
	const deleteWarnings = $derived.by(() => {
		if (activeModal.type !== 'deletePlaylist') return []
		const warnings: string[] = []
		if (activeModal.playlist.is_folder && activeModal.hasChildren) {
			warnings.push('This folder contains playlists that will also be deleted.')
		}
		return warnings
	})

	const deletePlaylistTitle = $derived(
		activeModal.type === 'deletePlaylist' && activeModal.playlist.is_folder ? 'Delete Folder' : 'Delete Playlist'
	)

	const deletePlaylistMessage = $derived(
		activeModal.type === 'deletePlaylist' && activeModal.playlist.is_folder
			? 'Are you sure you want to delete this folder?'
			: 'Are you sure you want to delete this playlist?'
	)
</script>

<!-- Create Playlist Modal -->
{#if activeModal.type === 'createPlaylist'}
	<InputModal
		open={true}
		title="New Playlist"
		placeholder="Playlist name"
		submitLabel="Create"
		onSubmit={handleCreatePlaylistSubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Create Folder Modal -->
{#if activeModal.type === 'createFolder'}
	<InputModal
		open={true}
		title="New Folder"
		placeholder="Folder name"
		submitLabel="Create"
		onSubmit={handleCreateFolderSubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Create Category Modal -->
{#if activeModal.type === 'createCategory'}
	<InputModal
		open={true}
		title="New Tag Category"
		placeholder="Category name"
		submitLabel="Create"
		onSubmit={handleCreateCategorySubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Create Tag Modal -->
{#if activeModal.type === 'createTag'}
	<InputModal
		open={true}
		title="New Tag"
		placeholder="Tag name"
		submitLabel="Create"
		onSubmit={handleCreateTagSubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Rename Playlist Modal -->
{#if activeModal.type === 'renamePlaylist'}
	<InputModal
		open={true}
		title="Rename"
		placeholder="Name"
		submitLabel="Save"
		initialValue={activeModal.playlist.name}
		onSubmit={handleRenamePlaylistSubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Rename Tag Modal -->
{#if activeModal.type === 'renameTag'}
	<InputModal
		open={true}
		title="Rename Tag"
		placeholder="Tag name"
		submitLabel="Save"
		initialValue={activeModal.tag.name}
		onSubmit={handleRenameTagSubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Rename Category Modal -->
{#if activeModal.type === 'renameCategory'}
	<InputModal
		open={true}
		title="Rename Category"
		placeholder="Category name"
		submitLabel="Save"
		initialValue={activeModal.category.name}
		onSubmit={handleRenameCategorySubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Delete Playlist Confirmation -->
{#if activeModal.type === 'deletePlaylist'}
	<ConfirmModal
		open={true}
		title={deletePlaylistTitle}
		message={deletePlaylistMessage}
		warnings={deleteWarnings}
		checkboxLabel="Also delete tracks from my collection"
		bind:checkboxChecked={deleteTracksFromCollection}
		confirmLabel="Delete"
		destructive={true}
		onConfirm={handleDeletePlaylistConfirm}
		onCancel={closeAll}
	/>
{/if}

<!-- Delete Tag Confirmation -->
{#if activeModal.type === 'deleteTag'}
	<ConfirmModal
		open={true}
		title="Delete Tag"
		message="Are you sure you want to delete this tag? It will be removed from all tracks."
		confirmLabel="Delete"
		destructive={true}
		onConfirm={handleDeleteTagConfirm}
		onCancel={closeAll}
	/>
{/if}

<!-- Delete Category Confirmation -->
{#if activeModal.type === 'deleteCategory'}
	<ConfirmModal
		open={true}
		title="Delete Category"
		message="Are you sure you want to delete this category? All tags in this category will be removed from all tracks."
		confirmLabel="Delete"
		destructive={true}
		onConfirm={handleDeleteCategoryConfirm}
		onCancel={closeAll}
	/>
{/if}

<!-- Remove from Playlist Confirmation -->
{#if activeModal.type === 'removeFromPlaylist'}
	<ConfirmModal
		open={true}
		title="Remove from Playlist"
		message={activeModal.trackIds.length === 1
			? 'Are you sure you want to remove this track from the playlist?'
			: `Are you sure you want to remove ${activeModal.trackIds.length} tracks from the playlist?`}
		confirmLabel="Remove"
		destructive={true}
		onConfirm={handleRemoveFromPlaylistConfirm}
		onCancel={closeAll}
	/>
{/if}

<!-- Remove from Library Confirmation -->
{#if activeModal.type === 'removeFromLibrary'}
	<ConfirmModal
		open={true}
		title="Remove from Library"
		message={activeModal.trackIds.length === 1
			? 'Are you sure you want to remove this track from your library?'
			: `Are you sure you want to remove ${activeModal.trackIds.length} tracks from your library?`}
		warnings={['This action cannot be undone. Tracks will be removed from all playlists.']}
		confirmLabel="Remove"
		destructive={true}
		onConfirm={handleRemoveFromLibraryConfirm}
		onCancel={closeAll}
	/>
{/if}

<!-- Tag Input Modal -->
{#if activeModal.type === 'tagInput'}
	<TagInputModal open={true} categories={tagCategories} onSubmit={handleTagInputSubmit} onCancel={closeAll} />
{/if}

<!-- Device Info Modal -->
{#if activeModal.type === 'deviceInfo'}
	<DeviceInfoModal open={true} device={activeModal.device} onClose={closeAll} />
{/if}

<!-- Relocate Track Modal -->
{#if activeModal.type === 'relocate'}
	<RelocateTrackModal open={true} track={activeModal.track} onClose={closeAll} onRelocate={handleRelocateComplete} />
{/if}

<!-- Move Conflict Modal -->
{#if activeModal.type === 'moveConflict'}
	<MoveConflictModal
		open={true}
		movingItem={activeModal.movingItem}
		conflictingItem={activeModal.existingItem}
		pendingCount={pendingMergeConflicts.length}
		onCancel={handleMoveConflictCancel}
		onOverwrite={handleMoveConflictOverwrite}
		onMerge={handleMoveConflictMerge}
	/>
{/if}

<!-- Settings Modal -->
{#if activeModal.type === 'settings'}
	<SettingsModal open={true} onClose={closeAll} />
{/if}

<!-- Duplicate Track Modal -->
{#if activeModal.type === 'duplicateTrack'}
	<DuplicateTrackModal
		open={true}
		duplicate={activeModal.duplicates[activeModal.currentIndex]}
		currentIndex={activeModal.currentIndex}
		totalCount={activeModal.duplicates.length}
		onResolve={handleDuplicateResolve}
		onCancel={handleDuplicateCancel}
	/>
{/if}
