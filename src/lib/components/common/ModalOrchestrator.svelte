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
		ExportRequest,
		SettingsPage,
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
		| { type: 'settings'; initialTab?: SettingsPage }
		| {
				type: 'duplicateTrack'
				duplicates: DuplicateTrack[]
				currentIndex: number
				applyToAllAction: DuplicateResolutionAction | null
				updatedTracks: Track[]
				newTracks: Track[]
				replacedTrackIds: string[]
				onComplete: (updatedTracks: Track[], newTracks: Track[], replacedTrackIds: string[]) => void
		  }
		// Export modals
		| { type: 'exportToDevice'; mode: 'selectPlaylists'; device: UsbDevice }
		| { type: 'exportToDevice'; mode: 'selectDevice'; playlist: Playlist }
		| { type: 'quickExport' }
		| {
				type: 'exportFailure'
				error: string
				deviceId: string
				mountPoint: string
				filesCopied: number
		  }
		// Device modals
		| { type: 'reformatDevice'; device: UsbDevice }

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
	import { DeviceInfoModal, ReformatDeviceModal } from '$lib/components/devices'
	import { SettingsModal } from '$lib/components/settings'
	import { RelocateTrackModal } from '$lib/components/library'
	import { ExportModal, ExportFailureModal, QuickExportModal } from '$lib/components/export'
	import { toastStore } from '$lib/stores/toast'
	import { resolveDuplicate } from '$lib/api/library'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	// =========================================================================
	// Props - Callback handlers passed from parent
	// =========================================================================
	type Props = {
		// Data needed by modals
		playlists: Playlist[]
		tagCategories: TagCategory[]
		devices: UsbDevice[]

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

		// Export callbacks
		onExport: (request: ExportRequest) => Promise<void>
		onQuickExport: (requests: ExportRequest[]) => Promise<void>
		onExportFailureKeep: () => void
		onExportFailureCleanup: (deviceId: string, mountPoint: string) => Promise<void>

		// Device callbacks
		onReformatDevice: (device: UsbDevice, volumeName: string) => Promise<void>
	}

	let {
		playlists,
		tagCategories,
		devices,
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
		onExport,
		onQuickExport,
		onExportFailureKeep,
		onExportFailureCleanup,
		onReformatDevice,
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

	export function openSettingsModal(initialTab?: SettingsPage) {
		activeModal = { type: 'settings', initialTab }
	}

	export function openDuplicateTrackModal(
		duplicates: DuplicateTrack[],
		onComplete: (updatedTracks: Track[], newTracks: Track[], replacedTrackIds: string[]) => void
	) {
		if (duplicates.length === 0) {
			onComplete([], [], [])
			return
		}

		activeModal = {
			type: 'duplicateTrack',
			duplicates,
			currentIndex: 0,
			applyToAllAction: null,
			updatedTracks: [],
			newTracks: [],
			replacedTrackIds: [],
			onComplete,
		}
	}

	// Export modals
	export function openExportToDeviceModal(device: UsbDevice) {
		activeModal = { type: 'exportToDevice', mode: 'selectPlaylists', device }
	}

	export function openExportPlaylistModal(playlist: Playlist) {
		activeModal = { type: 'exportToDevice', mode: 'selectDevice', playlist }
	}

	export function openQuickExportModal() {
		activeModal = { type: 'quickExport' }
	}

	export function openExportFailureModal(error: string, deviceId: string, mountPoint: string, filesCopied: number) {
		activeModal = { type: 'exportFailure', error, deviceId, mountPoint, filesCopied }
	}

	// Device modals
	export function openReformatDeviceModal(device: UsbDevice) {
		activeModal = { type: 'reformatDevice', device }
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
				toastStore.success(get(translate)('toast.replacedExisting'))
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
					toastStore.success(get(translate)('toast.mergedSuccessfully'))
				}
			}
			pendingMergeConflicts = []
		}
	}

	function processNextMergeConflict() {
		if (pendingMergeConflicts.length === 0) {
			toastStore.success(get(translate)('toast.mergeCompleted'))
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

		const { duplicates, currentIndex, updatedTracks, newTracks, replacedTrackIds, onComplete } = activeModal
		const currentDuplicate = duplicates[currentIndex]

		// Process the current duplicate and categorize the result
		const track = await processDuplicateAction(action, currentDuplicate)

		let newUpdatedTracks = updatedTracks
		let newNewTracks = newTracks
		let newReplacedTrackIds = replacedTrackIds

		if (track) {
			if (action === 'update_path') {
				newUpdatedTracks = [...updatedTracks, track]
			} else if (action === 'replace') {
				newNewTracks = [...newTracks, track]
				newReplacedTrackIds = [...replacedTrackIds, currentDuplicate.existing_track.id]
			}
			// 'skip' action: no state changes needed
		}

		// If apply to all, process remaining duplicates with the same action
		if (applyToAll && currentIndex < duplicates.length - 1) {
			const remaining = await processRemainingDuplicates(action, duplicates.slice(currentIndex + 1))
			closeAll()
			onComplete(
				[...newUpdatedTracks, ...remaining.updatedTracks],
				[...newNewTracks, ...remaining.newTracks],
				[...newReplacedTrackIds, ...remaining.replacedTrackIds]
			)
			return
		}

		// Check if there are more duplicates to process
		if (currentIndex < duplicates.length - 1) {
			activeModal = {
				type: 'duplicateTrack',
				duplicates,
				currentIndex: currentIndex + 1,
				applyToAllAction: null,
				updatedTracks: newUpdatedTracks,
				newTracks: newNewTracks,
				replacedTrackIds: newReplacedTrackIds,
				onComplete,
			}
		} else {
			// All done
			closeAll()
			onComplete(newUpdatedTracks, newNewTracks, newReplacedTrackIds)
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
			toastStore.error(get(translate)('toast.failedToResolve', { values: { error: String(error) } }))
			return null
		}
	}

	async function processRemainingDuplicates(
		action: DuplicateResolutionAction,
		duplicates: DuplicateTrack[]
	): Promise<{ updatedTracks: Track[]; newTracks: Track[]; replacedTrackIds: string[] }> {
		const updatedTracks: Track[] = []
		const newTracks: Track[] = []
		const replacedTrackIds: string[] = []

		for (const dup of duplicates) {
			const track = await processDuplicateAction(action, dup)
			if (track) {
				if (action === 'update_path') {
					updatedTracks.push(track)
				} else if (action === 'replace') {
					newTracks.push(track)
					replacedTrackIds.push(dup.existing_track.id)
				}
			}
		}

		return { updatedTracks, newTracks, replacedTrackIds }
	}

	function handleDuplicateCancel() {
		if (activeModal.type === 'duplicateTrack') {
			const { updatedTracks, newTracks, replacedTrackIds, onComplete } = activeModal
			closeAll()
			// Return any tracks that were already resolved
			onComplete(updatedTracks, newTracks, replacedTrackIds)
		}
	}

	// Export handlers
	async function handleExportSubmit(request: ExportRequest) {
		closeAll()
		await onExport(request)
	}

	async function handleQuickExportSubmit(requests: ExportRequest[]) {
		closeAll()
		await onQuickExport(requests)
	}

	function handleExportFailureKeep() {
		closeAll()
		onExportFailureKeep()
	}

	async function handleExportFailureCleanup() {
		if (activeModal.type === 'exportFailure') {
			const { deviceId, mountPoint } = activeModal
			closeAll()
			await onExportFailureCleanup(deviceId, mountPoint)
		}
	}

	// Device handlers
	async function handleReformatDeviceSubmit(volumeName: string) {
		if (activeModal.type === 'reformatDevice') {
			const device = activeModal.device
			closeAll()
			await onReformatDevice(device, volumeName)
		}
	}

	// Derived values for delete warnings
	const deleteWarnings = $derived.by(() => {
		if (activeModal.type !== 'deletePlaylist') return []
		const warnings: string[] = []
		if (activeModal.playlist.is_folder && activeModal.hasChildren) {
			warnings.push(get(translate)('modals.confirm.deleteFolderWarning'))
		}
		return warnings
	})

	const deletePlaylistTitle = $derived(
		activeModal.type === 'deletePlaylist' && activeModal.playlist.is_folder
			? get(translate)('modals.confirm.deleteFolderTitle')
			: get(translate)('modals.confirm.deletePlaylistTitle')
	)

	const deletePlaylistMessage = $derived(
		activeModal.type === 'deletePlaylist' && activeModal.playlist.is_folder
			? get(translate)('modals.confirm.deleteFolderMessage')
			: get(translate)('modals.confirm.deletePlaylistMessage')
	)
</script>

<!-- Create Playlist Modal -->
{#if activeModal.type === 'createPlaylist'}
	<InputModal
		open={true}
		title={$translate('modals.createPlaylist.title')}
		placeholder={$translate('modals.createPlaylist.placeholder')}
		submitLabel={$translate('common.create')}
		onSubmit={handleCreatePlaylistSubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Create Folder Modal -->
{#if activeModal.type === 'createFolder'}
	<InputModal
		open={true}
		title={$translate('modals.createFolder.title')}
		placeholder={$translate('modals.createFolder.placeholder')}
		submitLabel={$translate('common.create')}
		onSubmit={handleCreateFolderSubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Create Category Modal -->
{#if activeModal.type === 'createCategory'}
	<InputModal
		open={true}
		title={$translate('modals.createCategory.title')}
		placeholder={$translate('modals.createCategory.placeholder')}
		submitLabel={$translate('common.create')}
		onSubmit={handleCreateCategorySubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Create Tag Modal -->
{#if activeModal.type === 'createTag'}
	<InputModal
		open={true}
		title={$translate('modals.createTag.title')}
		placeholder={$translate('modals.createTag.tagPlaceholder')}
		submitLabel={$translate('common.create')}
		onSubmit={handleCreateTagSubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Rename Playlist Modal -->
{#if activeModal.type === 'renamePlaylist'}
	<InputModal
		open={true}
		title={$translate('modals.rename.title')}
		placeholder={$translate('modals.rename.placeholder')}
		submitLabel={$translate('common.save')}
		initialValue={activeModal.playlist.name}
		onSubmit={handleRenamePlaylistSubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Rename Tag Modal -->
{#if activeModal.type === 'renameTag'}
	<InputModal
		open={true}
		title={$translate('modals.renameTag.title')}
		placeholder={$translate('modals.renameTag.placeholder')}
		submitLabel={$translate('common.save')}
		initialValue={activeModal.tag.name}
		onSubmit={handleRenameTagSubmit}
		onCancel={closeAll}
	/>
{/if}

<!-- Rename Category Modal -->
{#if activeModal.type === 'renameCategory'}
	<InputModal
		open={true}
		title={$translate('modals.renameCategory.title')}
		placeholder={$translate('modals.renameCategory.placeholder')}
		submitLabel={$translate('common.save')}
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
		checkboxLabel={$translate('modals.confirm.deleteTracksFromCollection')}
		bind:checkboxChecked={deleteTracksFromCollection}
		confirmLabel={$translate('common.delete')}
		destructive={true}
		onConfirm={handleDeletePlaylistConfirm}
		onCancel={closeAll}
	/>
{/if}

<!-- Delete Tag Confirmation -->
{#if activeModal.type === 'deleteTag'}
	<ConfirmModal
		open={true}
		title={$translate('modals.confirm.deleteTagTitle')}
		message={$translate('modals.confirm.deleteTagMessage')}
		confirmLabel={$translate('common.delete')}
		destructive={true}
		onConfirm={handleDeleteTagConfirm}
		onCancel={closeAll}
	/>
{/if}

<!-- Delete Category Confirmation -->
{#if activeModal.type === 'deleteCategory'}
	<ConfirmModal
		open={true}
		title={$translate('modals.confirm.deleteCategoryTitle')}
		message={$translate('modals.confirm.deleteCategoryMessage')}
		confirmLabel={$translate('common.delete')}
		destructive={true}
		onConfirm={handleDeleteCategoryConfirm}
		onCancel={closeAll}
	/>
{/if}

<!-- Remove from Playlist Confirmation -->
{#if activeModal.type === 'removeFromPlaylist'}
	<ConfirmModal
		open={true}
		title={$translate('modals.confirm.removeFromPlaylistTitle')}
		message={$translate('modals.confirm.removeFromPlaylistMessage', { values: { count: activeModal.trackIds.length } })}
		confirmLabel={$translate('common.remove')}
		destructive={true}
		onConfirm={handleRemoveFromPlaylistConfirm}
		onCancel={closeAll}
	/>
{/if}

<!-- Remove from Library Confirmation -->
{#if activeModal.type === 'removeFromLibrary'}
	<ConfirmModal
		open={true}
		title={$translate('modals.confirm.removeFromLibraryTitle')}
		message={$translate('modals.confirm.removeFromLibraryMessage', { values: { count: activeModal.trackIds.length } })}
		warnings={[$translate('modals.confirm.removeFromLibraryWarning')]}
		confirmLabel={$translate('common.remove')}
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
	<SettingsModal open={true} initialTab={activeModal.initialTab} onClose={closeAll} />
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

<!-- Export to Device Modal -->
{#if activeModal.type === 'exportToDevice'}
	<ExportModal
		open={true}
		mode={activeModal.mode}
		device={activeModal.mode === 'selectPlaylists' ? activeModal.device : undefined}
		playlist={activeModal.mode === 'selectDevice' ? activeModal.playlist : undefined}
		{playlists}
		{devices}
		onExport={handleExportSubmit}
		onClose={closeAll}
	/>
{/if}

<!-- Quick Export Modal -->
{#if activeModal.type === 'quickExport'}
	<QuickExportModal open={true} {playlists} {devices} onExport={handleQuickExportSubmit} onClose={closeAll} />
{/if}

<!-- Export Failure Modal -->
{#if activeModal.type === 'exportFailure'}
	<ExportFailureModal
		open={true}
		error={activeModal.error}
		filesCopied={activeModal.filesCopied}
		onKeepPartial={handleExportFailureKeep}
		onCleanUp={handleExportFailureCleanup}
	/>
{/if}

<!-- Reformat Device Modal -->
{#if activeModal.type === 'reformatDevice'}
	<ReformatDeviceModal
		open={true}
		device={activeModal.device}
		onSubmit={handleReformatDeviceSubmit}
		onClose={closeAll}
	/>
{/if}
