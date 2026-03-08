<script lang="ts">
	import { onMount, onDestroy } from 'svelte'
	import { fade } from 'svelte/transition'
	import { get } from 'svelte/store'

	import type {
		ActiveView,
		Track,
		SortConfig,
		DiscoverySortConfig,
		DiscoveryFilter,
		DiscoveryRelease,
		Playlist,
		TagCategory,
		UsbDevice,
		TrackFilter,
		BreadcrumbItem,
	} from '$lib/types'
	import {
		appStore,
		libraryStore,
		sortedTracks,
		displayedTracks,
		trackCount,
		playerStore,
		currentTrack,
		tagsStore,
		playlistsStore,
		uiStore,
		activeView,
		selectedTrackIds,
		selectedReleaseIds,
		tagFilterMode,
		visibleDevices,
		rightSidebarVisible,
		rightSidebarWidth,
		devToolsOpen,
		analysisStore,
		discoveryStore,
		sortedReleases,
		releaseCount,
		previewInfo,
		pageActions,
	} from '$lib/stores'
	import { isPlaying } from '$lib/stores/player'
	import { buildBreadcrumbItems, getPlaylistChildren } from '$lib/stores/playlists'
	import { createAppSetup } from '$lib/hooks'

	import { RightSidebar, OrchestratorLayer } from '$lib/components/layout'
	import { LibraryView } from '$lib/components/library'
	import { DiscoveryView, DiscoveryEditor } from '$lib/components/discovery'
	import { TrackEditor } from '$lib/components/editor'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { PlaylistView, FolderView } from '$lib/components/playlists'
	import { openDevTools, closeDevTools } from '$lib/api/app'
	import { updateNowPlaying, updatePlaybackState, clearNowPlaying } from '$lib/api/mediaControls'
	import * as playlistsApi from '$lib/api/playlists'
	import { discoveryPlaylistStore, discoveryPlaylistReleases } from '$lib/stores/discoveryPlaylist'

	// =============================================================================
	// State
	// =============================================================================

	let sortConfig = $state<SortConfig>({ field: 'date_added', direction: 'desc' })
	let discoverySortConfig = $state<DiscoverySortConfig>({ field: 'date_added', direction: 'desc' })
	let playlists = $state<Playlist[]>([])
	let tagCategories = $state<TagCategory[]>([])
	let devices = $state<UsbDevice[]>([])
	let selectedPlaylistId = $state<string | null>(null)
	let selectedFolderId = $state<string | null>(null)
	let selectedTagIds = $state<string[]>([])

	// Drag and drop state (for external file drops)
	let isDragOver = $state(false)

	// Cleanup function from onMount
	let cleanupOnMount: (() => void) | undefined

	// =============================================================================
	// Orchestrator Layer
	// =============================================================================

	let orchestratorLayer: ReturnType<typeof OrchestratorLayer>

	// =============================================================================
	// App Setup (controllers, mount, track navigation, drag-drop)
	// =============================================================================

	const {
		tagController,
		trackController,
		deviceController,
		exportController,
		playlistController,
		playNextTrack,
		playPreviousTrack,
		onMountSetup,
		setupDragDrop,
	} = createAppSetup({
		getPlaylists: () => playlists,
		getDevices: () => devices,
		getSelectedPlaylistId: () => selectedPlaylistId,
		getSelectedFolderId: () => selectedFolderId,
		getSelectedTagIds: () => selectedTagIds,
		getModalOrchestrator: () => orchestratorLayer?.getModalOrchestrator(),
		handleViewChange,
		setShowAddReleaseModal: () => orchestratorLayer?.openAddReleaseModal(),
		setIsDragOver: (dragOver) => (isDragOver = dragOver),
	})

	// =============================================================================
	// Page Actions Bridge
	// =============================================================================

	pageActions.set({
		tagController,
		trackController,
		deviceController,
		exportController,
		playlistController,
		handleViewChange,
		handleToggleDevTools,
		playNextTrack,
		playPreviousTrack,
		openAddReleaseModal: () => orchestratorLayer?.openAddReleaseModal(),
		getModalOrchestrator: () => orchestratorLayer?.getModalOrchestrator(),
		getContextMenuOrchestrator: () => orchestratorLayer?.getContextMenuOrchestrator(),
	})

	// =============================================================================
	// Store Subscriptions
	// =============================================================================

	$effect(() => {
		const unsubPlaylists = playlistsStore.subscribe((state) => {
			playlists = state.playlists
		})
		const unsubTags = tagsStore.subscribe((state) => {
			tagCategories = state.categories
		})
		const unsubUI = uiStore.subscribe((state) => {
			selectedPlaylistId = state.selectedPlaylistId
			selectedFolderId = state.selectedFolderId
			selectedTagIds = state.selectedTagIds
		})
		const unsubDevices = visibleDevices.subscribe((visibleDevicesList) => {
			devices = visibleDevicesList
		})

		return () => {
			unsubPlaylists()
			unsubTags()
			unsubUI()
			unsubDevices()
		}
	})

	// =============================================================================
	// Now Playing Sync
	// =============================================================================

	$effect(() => {
		const preview = $previewInfo
		const track = $currentTrack
		if (preview) {
			const previewTrack = preview.release.tracks[preview.trackIndex]
			updateNowPlaying(
				previewTrack?.name || null,
				preview.release.artist || null,
				preview.release.title || null,
				preview.release.artwork_url || preview.release.artwork_path || null,
				previewTrack?.duration_ms ?? null
			).catch(() => {})
		} else if (track) {
			updateNowPlaying(
				track.title || null,
				track.artist || null,
				track.album || null,
				track.artwork_path || null,
				track.duration_ms
			).catch(() => {})
		} else {
			clearNowPlaying().catch(() => {})
		}
	})

	$effect(() => {
		const playing = $isPlaying
		updatePlaybackState(playing).catch(() => {})
	})

	// =============================================================================
	// Drag-Drop Coordination
	// =============================================================================

	let cleanupDragDrop: (() => void) | undefined

	$effect(() => {
		cleanupDragDrop = setupDragDrop()
		return () => {
			cleanupDragDrop?.()
		}
	})

	// =============================================================================
	// Lifecycle
	// =============================================================================

	onMount(() => {
		onMountSetup().then((cleanupFn) => {
			cleanupOnMount = cleanupFn
		})
	})

	onDestroy(() => {
		cleanupOnMount?.()
		pageActions.set(null)
	})

	// =============================================================================
	// Simple Handlers
	// =============================================================================

	function handleSortChange(config: SortConfig) {
		sortConfig = config
		libraryStore.setSort(config)
	}

	function handleRightSidebarResize(delta: number) {
		uiStore.setRightSidebarWidth($rightSidebarWidth + delta)
	}

	async function handleToggleDevTools() {
		if ($devToolsOpen) {
			await closeDevTools()
		} else {
			await openDevTools()
		}
		appStore.toggleDevTools()
	}

	async function handleCancelAnalysis(trackId: string) {
		await analysisStore.cancelTrackAnalysis(trackId)
	}

	// =============================================================================
	// Discovery Handlers
	// =============================================================================

	function handleViewChange(view: ActiveView) {
		// Cache discovery playlist releases before switching away
		if (selectedPlaylistId && $discoveryPlaylistReleases.length > 0) {
			discoveryPlaylistStore.getCache().set(selectedPlaylistId, $discoveryPlaylistReleases)
		}

		// setActiveView saves current navigation and restores the target view's cached state
		uiStore.setActiveView(view)

		// Read the restored navigation state
		const ui = get(uiStore)
		const restoredPlaylistId = ui.selectedPlaylistId
		const restoredFolderId = ui.selectedFolderId

		if (restoredPlaylistId) {
			// Restore data for the cached playlist
			const playlist = playlists.find((p) => p.id === restoredPlaylistId)
			if (playlist) {
				if (playlist.context === 'discovery') {
					const cached = discoveryPlaylistStore.getCached(restoredPlaylistId)
					if (cached) {
						discoveryPlaylistStore.setReleases(cached)
					} else {
						const fetchReleases = playlist.is_smart
							? playlistsApi.getSmartPlaylistReleases(restoredPlaylistId)
							: playlistsStore.getPlaylistReleases(restoredPlaylistId)
						fetchReleases.then((releases) => {
							discoveryPlaylistStore.setReleases(releases)
						})
					}
				} else if (playlist.is_smart) {
					libraryStore.loadSmartPlaylistTracks(restoredPlaylistId)
				} else {
					libraryStore.loadPlaylistTracks(restoredPlaylistId)
				}
			}
		} else if (!restoredFolderId) {
			// Base-level view: load main data
			discoveryPlaylistStore.clearReleases()
			if (view === 'discovery') {
				const filter: DiscoveryFilter = {}
				if (selectedTagIds.length > 0) {
					filter.tag_ids = selectedTagIds
					filter.tag_filter_mode = $tagFilterMode
				}
				discoveryStore.loadReleases(Object.keys(filter).length > 0 ? filter : undefined)
			} else {
				const filter: TrackFilter = {}
				if (selectedTagIds.length > 0) {
					filter.tag_ids = selectedTagIds
					filter.tag_filter_mode = $tagFilterMode
				}
				libraryStore.loadTracks(Object.keys(filter).length > 0 ? filter : undefined)
			}
		}
	}

	function handleDiscoverySortChange(config: DiscoverySortConfig) {
		discoverySortConfig = config
		discoveryStore.setSort(config)
	}

	const PREVIEWABLE_SOURCES = new Set(['bandcamp', 'soundcloud', 'youtube'])

	function releaseHasAnyPreviewableTrack(release: DiscoveryRelease): boolean {
		if (PREVIEWABLE_SOURCES.has(release.source_type)) return release.tracks.some((t) => t.duration_ms !== null)
		return release.tracks.some((t) => t.video_id !== null && t.duration_ms !== null)
	}

	function handleReleaseOpen(release: DiscoveryRelease) {
		if (releaseHasAnyPreviewableTrack(release)) {
			const firstPlayable = release.tracks.findIndex((t) => {
				if (!t.duration_ms) return false
				if (release.source_type === 'discogs') return t.video_id !== null
				return true
			})
			if (firstPlayable >= 0) {
				playerStore.playPreview(release, firstPlayable)
				return
			}
		}
		openUrl(release.url)
	}

	function handleTrackPlayInRelease(release: DiscoveryRelease, trackIndex: number) {
		uiStore.clearReleaseSelection()
		const track = release.tracks[trackIndex]
		const canPlay =
			track?.duration_ms &&
			(PREVIEWABLE_SOURCES.has(release.source_type) || (release.source_type === 'discogs' && track?.video_id !== null))
		if (canPlay && release.tracks.length > 0) {
			playerStore.playPreview(release, trackIndex)
		}
	}

	function handleReleaseSelectionChange(ids: Set<string>) {
		uiStore.setSelectedReleases(ids)
	}

	// =============================================================================
	// Context Menu Handlers
	// =============================================================================

	function handleTrackContextMenu(e: MouseEvent, track: Track) {
		const currentSelection = $selectedTrackIds
		let tracks: Track[]
		if (currentSelection.has(track.id)) {
			tracks = $sortedTracks.filter((t) => currentSelection.has(t.id))
		} else {
			tracks = [track]
		}
		orchestratorLayer?.getContextMenuOrchestrator()?.openTrackMenu(e, tracks)
	}

	function handleReleaseContextMenu(e: MouseEvent, release: DiscoveryRelease) {
		const currentSelection = $selectedReleaseIds
		let releases: DiscoveryRelease[]
		if (currentSelection.has(release.id)) {
			releases = $sortedReleases.filter((r) => currentSelection.has(r.id))
		} else {
			releases = [release]
		}
		orchestratorLayer?.getContextMenuOrchestrator()?.openDiscoveryReleaseMenu(e, releases)
	}

	async function handleEditorSave() {
		const playlist = playlists.find((p) => p.id === selectedPlaylistId)
		if (playlist?.is_smart) {
			if (playlist.context === 'discovery') {
				await discoveryPlaylistStore.refreshFromApi(playlist.id, () =>
					playlistsApi.getSmartPlaylistReleases(playlist.id)
				)
			} else {
				await libraryStore.loadSmartPlaylistTracks(playlist.id)
			}
			await playlistsStore.load()
		}
	}

	function handleBreadcrumbNavigate(item: BreadcrumbItem) {
		if (item.id === null) {
			playlistController.handleLibraryClick()
		} else if (item.playlist) {
			playlistController.handlePlaylistSelect(item.playlist)
		}
	}

	function handleBreadcrumbContextMenu(e: MouseEvent, item: BreadcrumbItem) {
		if (item.type === 'library') return
		if (item.playlist) {
			orchestratorLayer?.getContextMenuOrchestrator()?.openPlaylistMenu(e, item.playlist, 'tree')
		}
	}

	// =============================================================================
	// Derived State
	// =============================================================================

	let selectedTracksArray = $derived($displayedTracks.filter((t) => $selectedTrackIds.has(t.id)))
	let selectedReleasesArray = $derived($sortedReleases.filter((r) => $selectedReleaseIds.has(r.id)))
	const contextPlaylists = $derived(playlists.filter((p) => p.context === $activeView))
	const categoryColors = $derived(new Map(tagCategories.map((c) => [c.id, c.color])))
	const categorySortOrders = $derived(new Map(tagCategories.map((c) => [c.id, c.sort_order])))

	const currentFolderChildCount = $derived(
		selectedFolderId ? getPlaylistChildren(contextPlaylists, selectedFolderId).length : 0
	)

	// Combine all available releases for drag preview lookups
	const allAvailableReleases = $derived.by(() => {
		const sorted = $sortedReleases
		const dpReleases = $discoveryPlaylistReleases
		if (dpReleases.length === 0) return sorted
		const idSet = new Set(sorted.map((r) => r.id))
		const extra = dpReleases.filter((r) => !idSet.has(r.id))
		return extra.length > 0 ? [...sorted, ...extra] : sorted
	})

	const breadcrumbItems = $derived(
		buildBreadcrumbItems(
			contextPlaylists,
			selectedFolderId,
			selectedPlaylistId,
			selectedPlaylistId
				? $activeView === 'discovery'
					? $discoveryPlaylistReleases.length
					: $displayedTracks.length
				: undefined,
			currentFolderChildCount,
			$activeView
		)
	)
</script>

<div class="flex-1 overflow-hidden bg-surface-0">
	{#if selectedFolderId}
		<FolderView
			folderId={selectedFolderId}
			playlists={contextPlaylists}
			onSelect={playlistController.handlePlaylistSelect}
			{breadcrumbItems}
			onBreadcrumbNavigate={handleBreadcrumbNavigate}
			onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
			onEmptySpaceContextMenu={(e, folderId) =>
				orchestratorLayer?.getContextMenuOrchestrator()?.openFolderViewMenu(e, folderId)}
			onCardContextMenu={(e, playlist) =>
				orchestratorLayer?.getContextMenuOrchestrator()?.openPlaylistMenu(e, playlist, 'folder')}
		/>
	{:else if selectedPlaylistId}
		{@const playlist = contextPlaylists.find((p) => p.id === selectedPlaylistId)}
		{#if playlist}
			{#if playlist.context === 'discovery'}
				<PlaylistView
					{playlist}
					isDiscovery
					releases={$discoveryPlaylistReleases}
					tracks={[]}
					selectedIds={$selectedReleaseIds}
					{sortConfig}
					{categoryColors}
					{categorySortOrders}
					{breadcrumbItems}
					editorVisible={$rightSidebarVisible}
					hasSelection={selectedReleasesArray.length > 0}
					onSelectionChange={handleReleaseSelectionChange}
					onDiscoveryTrackPlay={handleTrackPlayInRelease}
					onContextMenu={(e, item) => {
						handleReleaseContextMenu(e, item as unknown as DiscoveryRelease)
					}}
					onBreadcrumbNavigate={handleBreadcrumbNavigate}
					onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
					onToggleEditor={() => uiStore.toggleRightSidebar()}
				/>
			{:else}
				<PlaylistView
					{playlist}
					tracks={$displayedTracks}
					selectedIds={$selectedTrackIds}
					playingTrackId={$currentTrack?.id ?? null}
					{sortConfig}
					{isDragOver}
					{categoryColors}
					{categorySortOrders}
					{breadcrumbItems}
					editorVisible={$rightSidebarVisible}
					hasSelection={selectedTracksArray.length > 0}
					onSelectionChange={trackController.handleSelectionChange}
					onTrackPlay={trackController.play}
					onSortChange={handleSortChange}
					onContextMenu={handleTrackContextMenu}
					onEmptySpaceContextMenu={(e, pl) =>
						orchestratorLayer?.getContextMenuOrchestrator()?.openPlaylistViewMenu(e, pl)}
					onBreadcrumbNavigate={handleBreadcrumbNavigate}
					onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
					onTrackColorChange={trackController.setColor}
					onCancelAnalysis={handleCancelAnalysis}
					onToggleEditor={() => uiStore.toggleRightSidebar()}
				/>
			{/if}
		{/if}
	{:else if $activeView === 'discovery'}
		<DiscoveryView
			releases={$sortedReleases}
			releaseCount={$releaseCount}
			selectedIds={$selectedReleaseIds}
			sortConfig={discoverySortConfig}
			{categoryColors}
			{categorySortOrders}
			editorVisible={$rightSidebarVisible}
			hasSelection={selectedReleasesArray.length > 0}
			onSelectionChange={handleReleaseSelectionChange}
			onReleaseOpen={handleReleaseOpen}
			onReleaseOpenUrl={(release) => openUrl(release.url)}
			onReleaseImport={(release) => orchestratorLayer?.setPurchaseRelease(release)}
			onTrackPlay={handleTrackPlayInRelease}
			onSortChange={handleDiscoverySortChange}
			onContextMenu={handleReleaseContextMenu}
			onEmptySpaceContextMenu={(e) => orchestratorLayer?.getContextMenuOrchestrator()?.openDiscoveryViewMenu(e)}
			onUrlDrop={async (url) => {
				await orchestratorLayer?.addRelease({ url })
			}}
			onToggleEditor={() => uiStore.toggleRightSidebar()}
		/>
	{:else}
		<LibraryView
			tracks={$displayedTracks}
			trackCount={$trackCount}
			selectedIds={$selectedTrackIds}
			playingTrackId={$currentTrack?.id ?? null}
			{sortConfig}
			{isDragOver}
			{categoryColors}
			{categorySortOrders}
			editorVisible={$rightSidebarVisible}
			hasSelection={selectedTracksArray.length > 0}
			onSelectionChange={trackController.handleSelectionChange}
			onTrackPlay={trackController.play}
			onSortChange={handleSortChange}
			onContextMenu={handleTrackContextMenu}
			onEmptySpaceContextMenu={(e) => orchestratorLayer?.getContextMenuOrchestrator()?.openLibraryViewMenu(e)}
			onTrackColorChange={trackController.setColor}
			onCancelAnalysis={handleCancelAnalysis}
			onToggleEditor={() => uiStore.toggleRightSidebar()}
		/>
	{/if}
</div>

<RightSidebar
	hasContent={$activeView === 'discovery' ? selectedReleasesArray.length > 0 : selectedTracksArray.length > 0}
	isVisible={$rightSidebarVisible}
	width={$rightSidebarWidth}
	onResize={handleRightSidebarResize}
>
	{#if $activeView === 'discovery'}
		<div class="h-full" in:fade={{ duration: 150 }}>
			<DiscoveryEditor
				selectedReleases={selectedReleasesArray}
				onImport={(release) => orchestratorLayer?.setPurchaseRelease(release)}
				onSave={handleEditorSave}
			/>
		</div>
	{:else}
		<div class="h-full" in:fade={{ duration: 150 }}>
			<TrackEditor selectedTracks={selectedTracksArray} onSave={handleEditorSave} />
		</div>
	{/if}
</RightSidebar>

<!-- Orchestrator Layer (context menus, modals, drag preview) -->
<OrchestratorLayer
	bind:this={orchestratorLayer}
	{playlists}
	{tagCategories}
	{devices}
	{selectedPlaylistId}
	{allAvailableReleases}
	{tagController}
	{trackController}
	{deviceController}
	{exportController}
	{playlistController}
	onEditorSave={handleEditorSave}
/>
