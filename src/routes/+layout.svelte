<script lang="ts">
	import '../style.css'
	import type { Snippet } from 'svelte'
	import type { Language, Playlist, TagCategory, Tag, TagSelectionState, UsbDevice } from '$lib/types'
	import ToastContainer from '$lib/components/common/ToastContainer.svelte'
	import CrashScreen from '$lib/components/common/CrashScreen.svelte'
	import SplashScreen from '$lib/components/common/SplashScreen.svelte'
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { getVersion } from '@tauri-apps/api/app'
	import { isDev } from '$lib/stores/app'
	import { settingsStore } from '$lib/stores/settings'
	import { splashVisible } from '$lib/stores/splash'
	import { useGlobalErrorHandler, hasAudioDrag } from '$lib/hooks'
	import { initializeI18n } from '$lib/i18n'
	import { Sidebar, Toolbar } from '$lib/components/layout'
	import { Player } from '$lib/components/player'
	import { ResizeHandle, Icon, Text } from '$lib/components/common'
	import {
		playlistsStore,
		tagsStore,
		uiStore,
		activeView,
		selectedTrackIds,
		selectedReleaseIds,
		tagFilterMode,
		visibleDevices,
		computeTagStates,
		releaseCount,
		trackCount,
		sortedReleases,
		displayedTracks,
		pageActions,
	} from '$lib/stores'
	import { discoveryPlaylistStore } from '$lib/stores/discoveryPlaylist'
	import { setMenuItemEnabled } from '$lib/api/app'
	import { computeDiscoveryTagStates } from '$lib/utils/tagComputation'

	interface Props {
		children: Snippet
	}

	let { children }: Props = $props()
	let i18nReady = $state(false)
	let splashVersion = $state('0.0.0')

	// =========================================================================
	// Layout State (subscribed from stores)
	// =========================================================================

	let playlists = $state<Playlist[]>([])
	let tagCategories = $state<TagCategory[]>([])
	let devices = $state<UsbDevice[]>([])
	let selectedPlaylistId = $state<string | null>(null)
	let selectedFolderId = $state<string | null>(null)
	let selectedTagIds = $state<string[]>([])
	let sidebarWidth = $state(240)

	// Tag toggle state
	let tagStates = $state<Map<string, TagSelectionState>>(new Map())
	let tagCounts = $state<Map<string, number>>(new Map())

	// =========================================================================
	// Store Subscriptions
	// =========================================================================

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
			sidebarWidth = state.sidebarWidth
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

	// =========================================================================
	// Effects (migrated from +page.svelte)
	// =========================================================================

	// Compute tag states when selection or tracks/releases change
	$effect(() => {
		if ($activeView === 'discovery') {
			const result = computeDiscoveryTagStates(tagCategories, $sortedReleases, $selectedReleaseIds)
			tagStates = result.states
			tagCounts = result.counts
		} else {
			const result = computeTagStates(tagCategories, $displayedTracks, $selectedTrackIds)
			tagStates = result.states
			tagCounts = result.counts
		}
	})

	// Clear recently toggled tags when selection changes
	let previousSelectedIds = $state<Set<string>>(new Set())
	$effect(() => {
		const currentIds = $activeView === 'discovery' ? $selectedReleaseIds : $selectedTrackIds
		if (currentIds.size !== previousSelectedIds.size || ![...currentIds].every((id) => previousSelectedIds.has(id))) {
			uiStore.clearAllRecentlyToggledTags()
			previousSelectedIds = new Set(currentIds)
		}
	})

	// Clear discovery playlist releases when no playlist is selected
	$effect(() => {
		if (!selectedPlaylistId) {
			discoveryPlaylistStore.clearReleases()
		}
	})

	// Enable/disable Refresh Metadata menu item based on view and selection
	$effect(() => {
		setMenuItemEnabled('refresh_metadata', $activeView === 'discovery' && $selectedReleaseIds.size > 0)
	})

	// Clear tree multi-selection when navigation changes
	let prevNavPlaylistId: string | null = null
	let prevNavFolderId: string | null = null
	$effect(() => {
		const pId = selectedPlaylistId
		const fId = selectedFolderId
		if (pId !== prevNavPlaylistId || fId !== prevNavFolderId) {
			prevNavPlaylistId = pId
			prevNavFolderId = fId
			uiStore.clearSelectedTreeIds()
		}
	})

	// =========================================================================
	// Derived State
	// =========================================================================

	const contextPlaylists = $derived(playlists.filter((p) => p.context === $activeView))

	const activeFilterTags = $derived.by(() => {
		if (selectedTagIds.length === 0) return []
		const tags: Tag[] = []
		for (const category of tagCategories) {
			for (const tag of category.tags) {
				if (selectedTagIds.includes(tag.id)) tags.push(tag)
			}
		}
		return tags
	})

	const tagColors = $derived(new Map(tagCategories.map((c) => [c.id, c.color])))

	// =========================================================================
	// Handlers
	// =========================================================================

	function handleSidebarResize(delta: number) {
		uiStore.setSidebarWidth(sidebarWidth + delta)
	}

	function handlePlaylistItemClick(playlist: Playlist, newSelectedIds: Set<string>, isModifierClick: boolean) {
		uiStore.setSelectedTreeIds(newSelectedIds)
	}

	function handlePlaylistContextMenu(e: MouseEvent, playlist: Playlist) {
		uiStore.setContextMenuPlaylistId(playlist.id)
		$pageActions?.getContextMenuOrchestrator()?.openPlaylistMenu(e, playlist, 'tree')
	}

	function handlePlaylistMultiContextMenu(e: MouseEvent, playlists: Playlist[]) {
		uiStore.clearContextMenuPlaylistId()
		$pageActions?.getContextMenuOrchestrator()?.openPlaylistMenu(e, playlists, 'tree')
	}

	function handleTagContextMenu(e: MouseEvent, tag: Tag, category: TagCategory) {
		$pageActions?.getContextMenuOrchestrator()?.openTagMenu(e, { type: 'tag', tag, category })
	}

	function handleCategoryContextMenu(e: MouseEvent, category: TagCategory) {
		$pageActions?.getContextMenuOrchestrator()?.openTagMenu(e, { type: 'category', category })
	}

	function handleDeviceContextMenu(e: MouseEvent, device: UsbDevice) {
		$pageActions?.getContextMenuOrchestrator()?.openDeviceMenu(e, device)
	}

	// =========================================================================
	// Initialization
	// =========================================================================

	onMount(() => {
		getVersion().then((v) => (splashVersion = v))

		async function init() {
			const cachedLanguage = localStorage.getItem('crate-language') as Language | null
			await initializeI18n(cachedLanguage)
			i18nReady = true
			await settingsStore.load()
		}
		init()

		const cleanupErrorHandler = useGlobalErrorHandler()

		const dragoverHandler = (e: DragEvent) => {
			if (hasAudioDrag) {
				e.preventDefault()
			}
			e.stopPropagation()
		}

		const dropHandler = (e: DragEvent) => {
			e.preventDefault()
			e.stopPropagation()
		}

		window.addEventListener('dragover', dragoverHandler)
		window.addEventListener('drop', dropHandler)

		const contextMenuHandler = (e: MouseEvent) => {
			if (!get(isDev)) {
				e.preventDefault()
			}
		}
		document.addEventListener('contextmenu', contextMenuHandler)

		return () => {
			cleanupErrorHandler()
			window.removeEventListener('dragover', dragoverHandler)
			window.removeEventListener('drop', dropHandler)
			document.removeEventListener('contextmenu', contextMenuHandler)
		}
	})
</script>

<SplashScreen show={$splashVisible} version={splashVersion} />

<div class="flex h-screen w-screen flex-col overflow-hidden bg-surface-0 text-text-primary">
	{#if i18nReady}
		<div
			class="flex h-full flex-col transition-opacity duration-300"
			style="opacity: {$splashVisible ? 0 : 1}; pointer-events: {$splashVisible ? 'none' : 'auto'}"
		>
			<!-- Unified Header: Logo + Toolbar -->
			<div class="flex rounded-br bg-surface-1">
				<div class="flex flex-shrink-0 items-center justify-center gap-2" style="width: {sidebarWidth}px">
					<Icon name="logo" class="h-6 w-6 text-brand-primary" fill />
					<Text variant="header-1" as="span" weight="bold">Crate</Text>
					{#if $isDev}
						<span class="rounded bg-amber-500/20 px-1.5 py-0.5 text-xs font-medium text-amber-500">DEV</span>
					{/if}
				</div>
				<Toolbar
					activeView={$activeView}
					{activeFilterTags}
					{tagColors}
					tagFilterMode={$tagFilterMode}
					onViewChange={(view) => $pageActions?.handleViewChange(view)}
					onRemoveTagFilter={(tagId) => $pageActions?.tagController.removeTagFilter(tagId)}
					onClearAllTagFilters={() => $pageActions?.tagController.clearTagFilters()}
					onToggleTagFilterMode={() => $pageActions?.tagController.toggleTagFilterMode()}
					onImport={$activeView === 'library' ? () => $pageActions?.trackController.handleImport() : undefined}
					onAddRelease={$activeView === 'discovery' ? () => $pageActions?.openAddReleaseModal() : undefined}
					onSettings={() => $pageActions?.getModalOrchestrator()?.openSettingsModal()}
					onDevTools={() => $pageActions?.handleToggleDevTools()}
				/>
			</div>

			<div class="relative flex flex-1 overflow-hidden bg-surface-1">
				<!-- Left: Sidebar -->
				<div class="flex-shrink-0" style="width: {sidebarWidth}px">
					<Sidebar
						playlists={contextPlaylists}
						{tagCategories}
						{devices}
						{selectedPlaylistId}
						{selectedFolderId}
						contextMenuPlaylistId={$uiStore.contextMenuPlaylistId}
						{selectedTagIds}
						selectedTrackIds={$activeView === 'discovery' ? $selectedReleaseIds : $selectedTrackIds}
						selectedTreeIds={$uiStore.selectedTreeIds}
						{tagStates}
						{tagCounts}
						trackCount={$activeView === 'discovery' ? $releaseCount : $trackCount}
						showHeader={false}
						onLibraryClick={() => {
							uiStore.clearSelectedTreeIds()
							$pageActions?.playlistController.handleLibraryClick()
						}}
						onPlaylistSelect={(p) => $pageActions?.playlistController.handlePlaylistSelect(p)}
						onPlaylistItemClick={handlePlaylistItemClick}
						onPlaylistContextMenu={handlePlaylistContextMenu}
						onPlaylistMultiContextMenu={handlePlaylistMultiContextMenu}
						onPlaylistTreeContextMenu={(e) => $pageActions?.getContextMenuOrchestrator()?.openPlaylistTreeMenu(e)}
						onDeviceContextMenu={handleDeviceContextMenu}
						onCancelExport={() => $pageActions?.exportController.handleExportCancel()}
						onTagSelect={(tagId) => $pageActions?.tagController.selectTag(tagId)}
						onTagToggle={(tagId, state) => $pageActions?.tagController.toggleTagOnTracks(tagId, state)}
						onTagContextMenu={handleTagContextMenu}
						onCategoryContextMenu={handleCategoryContextMenu}
						onCreatePlaylist={() => $pageActions?.playlistController.handleCreatePlaylist()}
						onCreateSmartPlaylist={() => $pageActions?.playlistController.handleCreateSmartPlaylist($activeView)}
						onCreateFolder={() => $pageActions?.playlistController.handleCreateFolder()}
						onCreateCategory={() => $pageActions?.getModalOrchestrator()?.openCreateCategoryModal()}
						onCreateTag={(categoryId) => $pageActions?.getModalOrchestrator()?.openCreateTagModal(categoryId)}
						onTagsWhitespaceContextMenu={(e) => $pageActions?.getContextMenuOrchestrator()?.openTagsSidebarMenu(e)}
						onTracksDrop={(playlistId, trackIds) =>
							$pageActions?.trackController.handleTracksDropOnPlaylist(playlistId, trackIds)}
						onPlaylistMove={(playlistId, targetFolderId) =>
							$pageActions?.playlistController.handlePlaylistDragMove(playlistId, targetFolderId)}
					/>
				</div>

				<ResizeHandle onResize={handleSidebarResize} />

				<!-- Right: Main Content -->
				<div class="flex flex-1 overflow-hidden rounded-tl-md border-t border-l border-stroke">
					{@render children()}
				</div>
			</div>

			<Player onNext={() => $pageActions?.playNextTrack()} onPrevious={() => $pageActions?.playPreviousTrack()} />
		</div>
	{/if}
</div>

<ToastContainer />
<CrashScreen />
