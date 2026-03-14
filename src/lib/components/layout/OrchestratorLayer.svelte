<script lang="ts">
	import { get } from 'svelte/store'

	import type {
		ActiveView,
		Track,
		DiscoveryRelease,
		DiscoveryReleaseCreate,
		ImportResultWithDuplicates,
		Playlist,
		TagCategory,
		UsbDevice,
	} from '$lib/types'
	import { pickTagCategoryColor } from '$lib/types'
	import {
		libraryStore,
		playerStore,
		tagsStore,
		playlistsStore,
		uiStore,
		activeView,
		settingsStore,
		isDragging,
		dragData,
		dragPosition,
		analysisStore,
		discoveryStore,
		updaterStore,
		updateAvailable,
		expandedReleaseIds,
	} from '$lib/stores'
	import { toastStore } from '$lib/stores/toast'
	import { discoveryPlaylistStore } from '$lib/stores/discoveryPlaylist'
	import { translate } from '$lib/i18n'
	import { setMenuItemEnabled } from '$lib/api/app'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import * as discoveryApi from '$lib/api/discovery'
	import * as playlistsApi from '$lib/api/playlists'

	import { ContextMenuOrchestrator, ModalOrchestrator, DragPreview, UpdateModal } from '$lib/components/common'
	import { AddReleaseModal, MergeReleasesModal, PurchaseReleaseModal } from '$lib/components/discovery'

	import type { TagController } from '$lib/controllers/tagController'
	import type { TrackController } from '$lib/controllers/trackController'
	import type { DeviceController } from '$lib/controllers/deviceController'
	import type { ExportController } from '$lib/controllers/exportController'
	import type { PlaylistController } from '$lib/controllers/playlistController'

	// =============================================================================
	// Props
	// =============================================================================

	interface Props {
		playlists: Playlist[]
		tagCategories: TagCategory[]
		devices: UsbDevice[]
		selectedPlaylistId: string | null
		allAvailableReleases: DiscoveryRelease[]
		tagController: TagController
		trackController: TrackController
		deviceController: DeviceController
		exportController: ExportController
		playlistController: PlaylistController
		onEditorSave: () => void
	}

	let {
		playlists,
		tagCategories,
		devices,
		selectedPlaylistId,
		allAvailableReleases,
		tagController,
		trackController,
		deviceController,
		exportController,
		playlistController,
		onEditorSave,
	}: Props = $props()

	// =============================================================================
	// Orchestrator Bindings
	// =============================================================================

	let contextMenuOrchestrator: ReturnType<typeof ContextMenuOrchestrator>
	let modalOrchestrator: ReturnType<typeof ModalOrchestrator>

	// =============================================================================
	// Local Modal State
	// =============================================================================

	let showAddReleaseModal = $state(false)
	let purchaseRelease = $state<DiscoveryRelease | null>(null)
	let mergeReleases = $state<DiscoveryRelease[] | null>(null)

	// =============================================================================
	// Derived
	// =============================================================================

	const playlistFolders = $derived(playlists.filter((p) => p.context === $activeView && p.is_folder))

	// =============================================================================
	// Handlers
	// =============================================================================

	async function handleTrackAnalyze(tracks: Track[]) {
		const trackIds = tracks.map((t) => t.id)
		try {
			await analysisStore.analyzeTracks(trackIds)
		} catch (error) {
			console.error('Analysis failed:', error)
			toastStore.error(get(translate)('errors.analysisFailed'))
		}
	}

	async function handleAddRelease(create: DiscoveryReleaseCreate) {
		const release = await discoveryStore.createRelease(create)
		if (release) {
			showAddReleaseModal = false
			if (release.tracks.length > 0) {
				expandedReleaseIds.expand(release.id)
			}
			const playlist = playlists.find((p) => p.id === selectedPlaylistId)
			if (playlist?.is_smart && playlist.context === 'discovery') {
				await discoveryPlaylistStore.refreshFromApi(playlist.id, () =>
					playlistsApi.getSmartPlaylistReleases(playlist.id)
				)
			}
			await playlistsStore.load()
		} else {
			// Fallback: reload releases from DB to catch cases where the backend
			// succeeded but the IPC response was lost (e.g. macOS Tahoe WebKit issue)
			await discoveryStore.loadReleases()
			showAddReleaseModal = false
		}
	}

	async function handlePurchaseComplete(result: ImportResultWithDuplicates) {
		purchaseRelease = null
		uiStore.clearReleaseSelection()

		if (result.tracks.length > 0) {
			libraryStore.addTracksToState(result.tracks)

			if (get(settingsStore).autoAnalyzeOnImport) {
				analysisStore
					.analyzeTracks(result.tracks.map((t) => t.id))
					.catch((error) => console.error('Auto-analysis failed:', error))
			}
		}

		if (result.duplicates.length > 0) {
			modalOrchestrator.openDuplicateTrackModal(result.duplicates, () => {
				libraryStore.loadTracks()
			})
		}
	}

	function handleRelocateComplete(updatedTrack: Track) {
		libraryStore.loadTracks()
		toastStore.success(`Relocated "${updatedTrack.title || 'track'}"`)
	}

	// =============================================================================
	// Public API (via bind:this)
	// =============================================================================

	export function getContextMenuOrchestrator() {
		return contextMenuOrchestrator
	}

	export function getModalOrchestrator() {
		return modalOrchestrator
	}

	export function openAddReleaseModal() {
		showAddReleaseModal = true
	}

	export function setPurchaseRelease(release: DiscoveryRelease) {
		purchaseRelease = release
	}

	export function setMergeReleases(releases: DiscoveryRelease[]) {
		mergeReleases = releases
	}

	export async function addRelease(create: DiscoveryReleaseCreate) {
		await handleAddRelease(create)
	}
</script>

<!-- Context Menu Orchestrator -->
<ContextMenuOrchestrator
	bind:this={contextMenuOrchestrator}
	{playlists}
	currentPlaylistId={selectedPlaylistId}
	{playlistFolders}
	categoryCount={tagCategories.length}
	onTrackAddToPlaylist={trackController.addToPlaylist}
	onTrackRevealInExplorer={trackController.revealInExplorer}
	onTrackRemoveFromPlaylist={trackController.removeFromPlaylistClick}
	onTrackRemoveFromLibrary={trackController.removeFromLibraryClick}
	onTrackRelocate={(track) => modalOrchestrator.openRelocateModal(track)}
	onTrackSetColor={trackController.setColorFromContextMenu}
	onTrackAnalyze={handleTrackAnalyze}
	onPlaylistCreatePlaylist={(p) => modalOrchestrator.openCreatePlaylistModal(p.id)}
	onPlaylistCreateSmartPlaylist={(p) => modalOrchestrator.openCreateSmartPlaylistModal(p.id, p.context)}
	onPlaylistCreateFolder={(p) => modalOrchestrator.openCreateFolderModal(p.id)}
	onPlaylistEditSmartPlaylist={(p) => modalOrchestrator.openEditSmartPlaylistModal(p)}
	onPlaylistRename={playlistController.handlePlaylistRename}
	onPlaylistDelete={playlistController.handlePlaylistDelete}
	onPlaylistBulkDelete={(playlists) => modalOrchestrator.openDeletePlaylistBulkModal(playlists)}
	onPlaylistBulkMove={async (playlists, folderId) => {
		await playlistController.handleBulkPlaylistMove(
			playlists.map((p) => p.id),
			folderId
		)
	}}
	onPlaylistMove={playlistController.handlePlaylistMove}
	onFolderViewCreatePlaylist={(folderId) => modalOrchestrator.openCreatePlaylistModal(folderId)}
	onFolderViewCreateSmartPlaylist={(folderId) => modalOrchestrator.openCreateSmartPlaylistModal(folderId, $activeView)}
	onFolderViewCreateFolder={(folderId) => modalOrchestrator.openCreateFolderModal(folderId)}
	onPlaylistTreeCreatePlaylist={() => modalOrchestrator.openCreatePlaylistModal(null)}
	onPlaylistTreeCreateSmartPlaylist={() => modalOrchestrator.openCreateSmartPlaylistModal(null, $activeView)}
	onPlaylistTreeCreateFolder={() => modalOrchestrator.openCreateFolderModal(null)}
	onLibraryViewImport={trackController.handleImport}
	onDiscoveryViewAddRelease={() => (showAddReleaseModal = true)}
	onPlaylistViewImport={playlistController.handlePlaylistViewImport}
	{tagCategories}
	onTagAddTag={(categoryId) => modalOrchestrator.openCreateTagModal(categoryId)}
	onTagRename={(tag) => modalOrchestrator.openRenameTagModal(tag)}
	onTagDelete={(tag) => modalOrchestrator.openDeleteTagModal(tag)}
	onTagMove={async (tag, targetCategoryId) => {
		try {
			await tagsStore.moveTag(tag.id, targetCategoryId)
			libraryStore.updateTagCategory(tag.id, targetCategoryId)
			discoveryStore.updateTagCategory(tag.id, targetCategoryId)
			discoveryPlaylistStore.updateTagCategory(tag.id, targetCategoryId)
		} catch (error) {
			const message = error instanceof Error ? error.message : get(translate)('errors.tagNameConflict')
			toastStore.error(message)
		}
	}}
	onCategoryRename={(category) => modalOrchestrator.openRenameCategoryModal(category)}
	onCategoryDelete={(category) => modalOrchestrator.openDeleteCategoryModal(category)}
	onCategoryChangeColor={tagController.changeCategoryColor}
	onTagsSidebarAddCategory={() => modalOrchestrator.openCreateCategoryModal()}
	onDeviceViewInfo={deviceController.handleViewDeviceInfo}
	onDeviceRevealInFinder={deviceController.handleDeviceRevealInFinder}
	onDeviceReformat={deviceController.handleDeviceReformat}
	onDeviceEject={deviceController.handleEjectDevice}
	onDeviceExport={exportController.handleDeviceExport}
	onDeviceIgnore={deviceController.handleDeviceIgnore}
	onPlaylistExport={exportController.handlePlaylistExport}
	onDiscoveryReleaseOpenInBrowser={(release) => openUrl(release.url)}
	onDiscoveryReleaseRefreshMetadata={async (releases) => {
		if (releases.length === 1) {
			await discoveryStore.refreshMetadata(releases[0].id)
		} else {
			discoveryStore.bulkRefreshMetadata(releases)
		}
	}}
	onDiscoveryReleaseImport={(release) => (purchaseRelease = release)}
	onDiscoveryReleaseDelete={(releaseIds) => modalOrchestrator.openRemoveDiscoveryReleasesModal(releaseIds)}
	onDiscoveryReleaseRemoveFromPlaylist={(playlistId, releaseIds) =>
		modalOrchestrator.openRemoveDiscoveryReleasesFromPlaylistModal(releaseIds, playlistId)}
	onDiscoveryReleaseMerge={(releases) => (mergeReleases = releases)}
	onDiscoveryReleaseAddToPlaylist={async (playlistId, releases) => {
		const releaseIds = releases.map((r) => r.id)
		await playlistsStore.addReleases(playlistId, releaseIds)
	}}
	onClose={() => {
		uiStore.clearContextMenuPlaylistId()
	}}
/>

<!-- Modal Orchestrator -->
<ModalOrchestrator
	bind:this={modalOrchestrator}
	{playlists}
	{tagCategories}
	onModalOpenChange={(isOpen) => {
		setMenuItemEnabled('toggle_view', !isOpen)
	}}
	onCreatePlaylist={async (name, parentId) => {
		const context = $activeView
		const playlist = await playlistsStore.createPlaylist(name, parentId ?? undefined, context)
		if (playlist) {
			uiStore.selectPlaylist(playlist.id)
			if (context === 'library') {
				await libraryStore.loadPlaylistTracks(playlist.id)
			}
		}
		return playlist
	}}
	onCreateFolder={async (name, parentId) => {
		const context = $activeView
		const folder = await playlistsStore.createFolder(name, parentId ?? undefined, context)
		if (folder) {
			uiStore.selectFolder(folder.id)
		}
		return folder
	}}
	onCreateCategory={async (name) => {
		const categories = get(tagsStore).categories
		const accent = get(settingsStore).accentColor
		const color = pickTagCategoryColor(categories, accent)
		await tagsStore.createCategory(name, color)
	}}
	onCreateTag={async (categoryId, name) => {
		await tagsStore.createTag(categoryId, name)
	}}
	onRenamePlaylist={async (id, name) => {
		await playlistsStore.rename(id, name)
	}}
	onRenameTag={async (id, name) => {
		await tagsStore.updateTag(id, name)
	}}
	onRenameCategory={async (id, name) => {
		await tagsStore.updateCategory(id, name)
	}}
	onDeletePlaylist={async (id, deleteTracksToo) => {
		const playlist = playlists.find((p) => p.id === id)
		const parentId = playlist?.parent_id ?? null
		const context = playlist?.context
		discoveryPlaylistStore.deleteFromCache(id)
		await playlistsStore.delete(id, deleteTracksToo)
		if (deleteTracksToo) {
			if (context === 'discovery') {
				await discoveryStore.loadReleases()
			} else {
				await libraryStore.loadTracks()
			}
			await playlistsStore.load()
		}
		if (parentId) {
			const parentFolder = playlists.find((p) => p.id === parentId)
			if (parentFolder) {
				uiStore.selectFolder(parentId)
			} else {
				playlistController.handleLibraryClick()
			}
		} else {
			playlistController.handleLibraryClick()
		}
	}}
	onDeletePlaylistBulk={async (ids, deleteTracksToo) => {
		const idSet = new Set(ids)
		const topLevel = ids.filter((id) => {
			let current = playlists.find((p) => p.id === id)
			while (current?.parent_id) {
				if (idSet.has(current.parent_id)) return false
				current = playlists.find((p) => p.id === current!.parent_id)
			}
			return true
		})

		for (const id of topLevel) {
			discoveryPlaylistStore.deleteFromCache(id)
			await playlistsStore.delete(id, deleteTracksToo)
		}

		if (deleteTracksToo) {
			await libraryStore.loadTracks()
			await discoveryStore.loadReleases()
			await playlistsStore.load()
		}

		uiStore.clearSelectedTreeIds()
		playlistController.handleLibraryClick()
	}}
	onDeleteTag={async (id) => {
		await tagsStore.deleteTag(id)
		await libraryStore.loadTracks()
	}}
	onDeleteCategory={async (id) => {
		await tagsStore.deleteCategory(id)
		await libraryStore.loadTracks()
	}}
	onRemoveFromPlaylist={async (trackIds, playlistId, deleteFromCollection) => {
		await playlistsStore.removeTracks(playlistId, trackIds)
		if (deleteFromCollection) {
			await libraryStore.deleteTracks(trackIds)
			await playlistsStore.load()
		} else {
			await libraryStore.loadPlaylistTracks(playlistId)
		}
		uiStore.clearSelection()
		const count = trackIds.length
		toastStore.success(count === 1 ? '1 track removed from playlist' : `${count} tracks removed from playlist`)
	}}
	onRemoveDiscoveryReleases={async (releaseIds) => {
		await discoveryStore.deleteReleases(releaseIds)
		uiStore.clearReleaseSelection()
		await playlistsStore.load()
	}}
	onRemoveDiscoveryReleasesFromPlaylist={async (releaseIds, playlistId, deleteFromCollection) => {
		await playlistsStore.removeReleases(playlistId, releaseIds)
		discoveryPlaylistStore.filterOutAndCache(playlistId, releaseIds)
		if (deleteFromCollection) {
			await discoveryStore.deleteReleases(releaseIds)
		}
		uiStore.clearReleaseSelection()
	}}
	onRemoveFromLibrary={async (trackIds) => {
		await libraryStore.deleteTracks(trackIds)
		uiStore.clearSelection()
		if (selectedPlaylistId) {
			await libraryStore.loadPlaylistTracks(selectedPlaylistId)
		}
		await playlistsStore.load()
		const count = trackIds.length
		toastStore.success(count === 1 ? '1 track removed from library' : `${count} tracks removed from library`)
	}}
	onMoveConflictOverwrite={async (movingItemId, targetParentId) => {
		const result = await playlistsStore.moveWithResolution(movingItemId, targetParentId, 'overwrite')
		return !!result
	}}
	onMoveConflictMerge={async (movingItemId, targetParentId) => {
		const result = await playlistsStore.moveWithResolution(movingItemId, targetParentId, 'merge')
		return {
			success: !!result,
			nestedConflicts: result?.nestedConflicts ?? [],
		}
	}}
	onTagInputSubmit={async (categoryId, tagName) => {
		await tagsStore.createTag(categoryId, tagName)
	}}
	onCreateSmartPlaylist={async (name, smartRules, parentId, context) => {
		const playlist = await playlistsStore.createSmartPlaylist(name, smartRules, parentId ?? undefined, context)
		if (playlist) {
			playlistController.handlePlaylistSelect(playlist)
		}
		return playlist
	}}
	onUpdateSmartRules={async (id, smartRules) => {
		await playlistsStore.updateSmartRules(id, smartRules)
		const playlist = playlists.find((p) => p.id === id)
		if (playlist && selectedPlaylistId === id) {
			if (playlist.context === 'discovery') {
				await discoveryPlaylistStore.refreshFromApi(id, () => playlistsApi.getSmartPlaylistReleases(id))
			} else {
				await libraryStore.loadSmartPlaylistTracks(id)
			}
		}
		await playlistsStore.load()
	}}
	onRelocateComplete={handleRelocateComplete}
	{devices}
	onExport={exportController.handleExportSubmit}
	onQuickExport={exportController.handleQuickExportSubmit}
	onExportFailureKeep={exportController.handleExportFailureKeep}
	onExportFailureCleanup={exportController.handleExportFailureCleanup}
	onReformatDevice={deviceController.handleReformatDevice}
/>

<!-- Add Release Modal -->
{#if showAddReleaseModal}
	<AddReleaseModal
		open={true}
		onClose={() => (showAddReleaseModal = false)}
		onSubmit={handleAddRelease}
		onAddToExisting={async (releaseId, tracks) => {
			const release = await discoveryApi.addTracksToRelease(releaseId, tracks)
			if (release) {
				discoveryStore.updateRelease(releaseId, {})
				showAddReleaseModal = false
				await discoveryStore.loadReleases()
			}
		}}
		onBulkImportComplete={async () => {
			await discoveryStore.loadReleases()
			await playlistsStore.load()
			const playlist = playlists.find((p) => p.id === selectedPlaylistId)
			if (playlist?.context === 'discovery') {
				if (playlist.is_smart) {
					await discoveryPlaylistStore.refreshFromApi(playlist.id, () =>
						playlistsApi.getSmartPlaylistReleases(playlist.id)
					)
				} else {
					await discoveryPlaylistStore.refreshFromApi(playlist.id, () => playlistsApi.getPlaylistReleases(playlist.id))
				}
			}
		}}
	/>
{/if}

<!-- Merge Releases Modal -->
{#if mergeReleases && mergeReleases.length >= 2}
	<MergeReleasesModal
		open={true}
		releases={mergeReleases}
		onClose={() => (mergeReleases = null)}
		onMerge={async (targetId, sourceIds) => {
			const merged = await discoveryStore.mergeReleases(targetId, sourceIds)
			if (merged) {
				uiStore.setSelectedReleases(new Set([merged.id]))
				mergeReleases = null
			}
		}}
	/>
{/if}

<!-- Purchase Release Modal -->
{#if purchaseRelease}
	<PurchaseReleaseModal
		open={true}
		release={purchaseRelease}
		onClose={() => (purchaseRelease = null)}
		onComplete={handlePurchaseComplete}
	/>
{/if}

<!-- Update Modal -->
{#if $updateAvailable}
	<UpdateModal open={true} onClose={() => updaterStore.dismiss()} />
{/if}

<!-- Drag Preview -->
{#if $isDragging && $dragPosition}
	<DragPreview
		data={$dragData}
		tracks={$libraryStore.tracks}
		releases={allAvailableReleases}
		{playlists}
		{tagCategories}
		x={$dragPosition.x}
		y={$dragPosition.y}
	/>
{/if}
