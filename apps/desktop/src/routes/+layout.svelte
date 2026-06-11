<script lang="ts">
	import '../style.css'
	import type { Snippet } from 'svelte'
	import type { Language, Playlist, TagCategory, Tag, TagSelectionState, UsbDevice } from '$shared/types'
	import ToastContainer from '$lib/components/common/ToastContainer.svelte'
	import CrashScreen from '$lib/components/common/CrashScreen.svelte'
	import SplashScreen from '$lib/components/common/SplashScreen.svelte'
	import { AboutDialog, OnboardingWizard } from '$lib/components/onboarding'
	import { WizardTour } from '$lib/components/wizard'
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	// @ts-expect-error — PUBLIC_APP_VERSION is set dynamically by vite.config.ts
	import { PUBLIC_APP_VERSION } from '$env/static/public'
	import { isDev } from '$lib/stores/app'
	import { settingsStore, hasCompletedOnboarding, hasCompletedWizard } from '$shared/stores/settings'
	import { splashVisible } from '$lib/stores/splash'
	import { useGlobalErrorHandler, hasAudioDrag } from '$lib/hooks'
	import { initializeI18n, translate } from '$shared/i18n'
	import { Sidebar, Toolbar } from '$lib/components/layout'
	import { Player } from '$lib/components/player'
	import { ResizeHandle, Icon, Text } from '$lib/components/common'
	import {
		playlistsStore,
		tagsStore,
		uiStore,
		uiLayoutStore,
		activeView,
		selectedTrackIds,
		selectedReleaseIds,
		visibleDevices,
		computeTagStates,
		releaseCount,
		trackCount,
		sortedReleases,
		displayedTracks,
		pageActions,
	} from '$lib/stores'
	import { discoveryPlaylistStore } from '$shared/stores/discoveryPlaylist'
	import { listen } from '@tauri-apps/api/event'
	import { setMenuItemEnabled, setOnboardingItemsEnabled } from '$shared/api/app'
	import { computeDiscoveryTagStates } from '$shared/utils/tagComputation'

	interface Props {
		children: Snippet
	}

	let { children }: Props = $props()
	let i18nReady = $state(false)
	let splashVersion = PUBLIC_APP_VERSION
	let onboardingComplete = $state(false)
	let showOnboarding = $derived(!$splashVisible && !$hasCompletedOnboarding && !onboardingComplete)
	let showAboutDialog = $state(false)
	let showWizardTour = $state(false)

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
			selectedTagIds = state.viewFilters[state.activeView].selectedTagIds
		})
		const unsubLayout = uiLayoutStore.subscribe((state) => {
			sidebarWidth = state.sidebarWidth
		})
		const unsubDevices = visibleDevices.subscribe((visibleDevicesList) => {
			devices = visibleDevicesList
		})

		return () => {
			unsubPlaylists()
			unsubTags()
			unsubUI()
			unsubLayout()
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

	// Disable menu items during onboarding wizard and sync state to uiStore
	$effect(() => {
		uiStore.setOnboarding(showOnboarding)
		setOnboardingItemsEnabled(!showOnboarding)
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
			uiLayoutStore.clearSelectedTreeIds()
		}
	})

	// Prune discovery selection to only include visible releases
	$effect(() => {
		if ($activeView !== 'discovery') return
		const visibleIds = new Set($sortedReleases.map((r) => r.id))
		const currentSelection = $selectedReleaseIds
		if (currentSelection.size === 0) return
		const pruned = new Set([...currentSelection].filter((id) => visibleIds.has(id)))
		if (pruned.size < currentSelection.size) {
			uiStore.setSelectedReleases(pruned)
		}
	})

	// Prune library selection to only include visible tracks
	$effect(() => {
		if ($activeView === 'discovery') return
		const visibleIds = new Set($displayedTracks.map((t) => t.id))
		const currentSelection = $selectedTrackIds
		if (currentSelection.size === 0) return
		const pruned = new Set([...currentSelection].filter((id) => visibleIds.has(id)))
		if (pruned.size < currentSelection.size) {
			uiStore.setSelectedTracks(pruned)
		}
	})

	// Auto-start wizard tour after onboarding completes for new users
	$effect(() => {
		if (!$splashVisible && !showOnboarding && onboardingComplete && !$hasCompletedWizard) {
			setTimeout(() => {
				showWizardTour = true
			}, 600)
		}
	})

	// =========================================================================
	// Derived State
	// =========================================================================

	const contextPlaylists = $derived(playlists.filter((p) => p.context === $activeView))

	// =========================================================================
	// Handlers
	// =========================================================================

	function handleSidebarResize(delta: number) {
		uiLayoutStore.setSidebarWidth(sidebarWidth + delta)
	}

	function handlePlaylistItemClick(playlist: Playlist, newSelectedIds: Set<string>, isModifierClick: boolean) {
		uiLayoutStore.setSelectedTreeIds(newSelectedIds)
	}

	function handlePlaylistContextMenu(e: MouseEvent, playlist: Playlist) {
		uiLayoutStore.setContextMenuPlaylistId(playlist.id)
		$pageActions?.getContextMenuOrchestrator()?.openPlaylistMenu(e, playlist, 'tree')
	}

	function handlePlaylistMultiContextMenu(e: MouseEvent, playlists: Playlist[]) {
		uiLayoutStore.clearContextMenuPlaylistId()
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

		// Listen for menu actions handled at the layout level
		let unlistenMenuAction: (() => void) | null = null
		listen<string>('menu-action', (event) => {
			if (event.payload === 'about' && showOnboarding) {
				showAboutDialog = true
			}
			if (event.payload === 'feature_tour' && !showOnboarding) {
				showWizardTour = true
			}
		}).then((unlisten) => {
			unlistenMenuAction = unlisten
		})

		return () => {
			cleanupErrorHandler()
			unlistenMenuAction?.()
			window.removeEventListener('dragover', dragoverHandler)
			window.removeEventListener('drop', dropHandler)
			document.removeEventListener('contextmenu', contextMenuHandler)
		}
	})
</script>

<SplashScreen show={$splashVisible} version={splashVersion} />

{#if showOnboarding}
	<OnboardingWizard
		onComplete={() => {
			onboardingComplete = true
			settingsStore.completeOnboarding()
		}}
	/>
	<AboutDialog open={showAboutDialog} onClose={() => (showAboutDialog = false)} />
{/if}

<div class="flex h-screen w-screen flex-col overflow-hidden bg-surface-0 text-text-primary">
	{#if i18nReady}
		<div
			class="flex h-full flex-col transition-opacity duration-300"
			style="opacity: {$splashVisible || showOnboarding ? 0 : 1}; pointer-events: {$splashVisible || showOnboarding
				? 'none'
				: 'auto'}"
		>
			<!-- Unified Header: Logo + Toolbar -->
			<div class="relative flex rounded-br bg-surface-1">
				<div class="flex flex-shrink-0 items-center justify-center gap-2" style="width: {sidebarWidth}px">
					<div
						class="h-6 w-6 bg-brand-primary"
						style="-webkit-mask-image: url('/crate-logo.svg'); -webkit-mask-size: contain; -webkit-mask-repeat: no-repeat; -webkit-mask-position: center; mask-image: url('/crate-logo.svg'); mask-size: contain; mask-repeat: no-repeat; mask-position: center;"
					></div>
					<Text variant="header-1" as="span" weight="bold">Crate</Text>
					{#if $isDev}
						<span class="rounded bg-amber-500/20 px-1.5 py-0.5 text-xs font-medium text-amber-500">DEV</span>
					{/if}
				</div>

				<!-- Segmented control (absolutely centered in full window) -->
				<div class="pointer-events-none absolute inset-0 flex items-center justify-center">
					<div
						id="wizard-view-switcher"
						class="pointer-events-auto relative inline-grid grid-cols-2 items-center rounded-lg bg-surface-2 p-0.5"
					>
						<div
							class="absolute top-0.5 bottom-0.5 left-0.5 w-[calc(50%-2px)] rounded-md bg-surface-0 shadow-sm transition-transform duration-200 ease-out motion-reduce:transition-none"
							style="transform: translateX({$activeView === 'library' ? '100%' : '0%'})"
						></div>
						<button
							type="button"
							class="relative z-10 rounded-md px-3 py-1 text-center text-xs font-medium transition-colors {$activeView ===
							'discovery'
								? 'text-text-primary'
								: 'text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
							onclick={() => $pageActions?.handleViewChange('discovery')}
						>
							{$translate('nav.discovery')}
						</button>
						<button
							type="button"
							class="relative z-10 rounded-md px-3 py-1 text-center text-xs font-medium transition-colors {$activeView ===
							'library'
								? 'text-text-primary'
								: 'text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
							onclick={() => $pageActions?.handleViewChange('library')}
						>
							{$translate('nav.library')}
						</button>
					</div>
				</div>

				<Toolbar
					onImport={$activeView === 'library' ? () => $pageActions?.trackController.handleImport() : undefined}
					onAddRelease={$activeView === 'discovery' ? () => $pageActions?.openAddReleaseModal() : undefined}
					onSettings={() => $pageActions?.getModalOrchestrator()?.openSettingsModal()}
					onCloudSync={() => $pageActions?.getModalOrchestrator()?.openSettingsModal('cloudSync')}
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
						contextMenuPlaylistId={$uiLayoutStore.contextMenuPlaylistId}
						{selectedTagIds}
						selectedTrackIds={$activeView === 'discovery' ? $selectedReleaseIds : $selectedTrackIds}
						selectedTreeIds={$uiLayoutStore.selectedTreeIds}
						{tagStates}
						{tagCounts}
						trackCount={$activeView === 'discovery' ? $releaseCount : $trackCount}
						showHeader={false}
						onLibraryClick={() => {
							uiLayoutStore.clearSelectedTreeIds()
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

			<Player
				onNext={() => $pageActions?.playNextTrack()}
				onPrevious={() => $pageActions?.playPreviousTrack()}
				onLocateTrack={() => $pageActions?.locatePlayingTrack()}
			/>
		</div>
	{/if}
</div>

{#if showWizardTour}
	<WizardTour
		onComplete={() => {
			showWizardTour = false
			settingsStore.completeWizard()
		}}
		onSkip={() => {
			showWizardTour = false
			settingsStore.completeWizard()
		}}
	/>
{/if}

<ToastContainer />
<CrashScreen />
