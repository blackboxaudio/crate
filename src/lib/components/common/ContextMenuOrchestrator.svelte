<script lang="ts" module>
	import type { Track, Playlist, Tag, TagCategory, UsbDevice, TrackColor, DiscoveryRelease } from '$lib/types'

	// Tag context target discriminated union
	export type TagContextTarget =
		| { type: 'tag'; tag: Tag; category: TagCategory }
		| { type: 'category'; category: TagCategory }

	// Discriminated union for all context menu states
	export type ActiveContextMenu =
		| { type: 'none' }
		| { type: 'track'; x: number; y: number; tracks: Track[] }
		| { type: 'playlist'; x: number; y: number; playlists: Playlist[]; source: 'tree' | 'folder' }
		| { type: 'folderView'; x: number; y: number; folderId: string }
		| { type: 'playlistTree'; x: number; y: number }
		| { type: 'libraryView'; x: number; y: number }
		| { type: 'playlistView'; x: number; y: number; playlist: Playlist }
		| { type: 'tag'; x: number; y: number; target: TagContextTarget }
		| { type: 'tagsSidebar'; x: number; y: number }
		| { type: 'device'; x: number; y: number; device: UsbDevice }
		| { type: 'discoveryRelease'; x: number; y: number; releases: DiscoveryRelease[] }
		| { type: 'discoveryTrack'; x: number; y: number; release: DiscoveryRelease; trackIndex: number; canPlay: boolean }
		| { type: 'discoveryView'; x: number; y: number }
</script>

<script lang="ts">
	import TrackContextMenu from '$lib/components/library/TrackContextMenu.svelte'
	import PlaylistContextMenu from '$lib/components/playlists/PlaylistContextMenu.svelte'
	import TagContextMenu from '$lib/components/tags/TagContextMenu.svelte'
	import TagsSidebarContextMenu from '$lib/components/tags/TagsSidebarContextMenu.svelte'
	import DeviceContextMenu from '$lib/components/devices/DeviceContextMenu.svelte'
	import DiscoveryContextMenu from '$lib/components/discovery/DiscoveryContextMenu.svelte'
	import DiscoveryTrackContextMenu from '$lib/components/discovery/DiscoveryTrackContextMenu.svelte'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { get } from 'svelte/store'
	import { translate } from '$lib/i18n'
	import { devices, reformattingDeviceId } from '$lib/stores/devices'
	import { activeDeviceId } from '$lib/stores/export'
	import { syncingDeviceIds } from '$lib/stores/sync'
	import { isAnalyzing } from '$lib/stores/analysis'

	// =========================================================================
	// Props - Callback handlers passed from parent
	// =========================================================================
	type Props = {
		// Data needed by context menus
		playlists: Playlist[]
		currentPlaylistId: string | null
		playlistFolders: Playlist[]
		categoryCount: number

		// Track callbacks (receives tracks from menu state)
		onTrackAddToPlaylist: (playlistId: string, tracks: Track[]) => void
		onTrackRevealInExplorer: (track: Track) => void
		onTrackRemoveFromPlaylist: (tracks: Track[]) => void
		onTrackRemoveFromLibrary: (tracks: Track[]) => void
		onTrackRelocate: (track: Track) => void
		onTrackSetColor: (color: TrackColor | null, tracks: Track[]) => void
		onTrackAnalyze: (tracks: Track[]) => void

		// Playlist callbacks
		onPlaylistCreatePlaylist: (playlist: Playlist) => void
		onPlaylistCreateSmartPlaylist: (playlist: Playlist) => void
		onPlaylistCreateFolder: (playlist: Playlist) => void
		onPlaylistEditSmartPlaylist: (playlist: Playlist) => void
		onPlaylistRename: (playlist: Playlist) => void
		onPlaylistDelete: (playlist: Playlist) => void
		onPlaylistBulkDelete: (playlists: Playlist[]) => void
		onPlaylistMove: (playlist: Playlist, folderId: string | null) => void

		// Playlist bulk move
		onPlaylistBulkMove?: (playlists: Playlist[], folderId: string | null) => void

		// FolderView callbacks
		onFolderViewCreatePlaylist: (folderId: string | null) => void
		onFolderViewCreateSmartPlaylist: (folderId: string | null) => void
		onFolderViewCreateFolder: (folderId: string | null) => void

		// PlaylistTree callbacks
		onPlaylistTreeCreatePlaylist: () => void
		onPlaylistTreeCreateSmartPlaylist: () => void
		onPlaylistTreeCreateFolder: () => void

		// LibraryView callbacks
		onLibraryViewImport: () => void

		// PlaylistView callbacks
		onPlaylistViewImport: (playlist: Playlist) => void

		// Tag callbacks
		onTagAddTag: (categoryId: string) => void
		onTagRename: (tag: Tag) => void
		onTagDelete: (tag: Tag) => void
		onTagMove?: (tag: Tag, targetCategoryId: string) => void
		onCategoryRename: (category: TagCategory) => void
		onCategoryDelete: (category: TagCategory) => void
		onCategoryChangeColor: (category: TagCategory, color: string | null) => void

		// Tag categories for move submenu
		tagCategories?: TagCategory[]

		// TagsSidebar callbacks
		onTagsSidebarAddCategory: () => void

		// Device callbacks
		onDeviceViewInfo: (device: UsbDevice) => void
		onDeviceRevealInFinder: (device: UsbDevice) => void
		onDeviceReformat: (device: UsbDevice) => void
		onDeviceEject: (device: UsbDevice) => void
		onDeviceExport: (device: UsbDevice) => void
		onDeviceIgnore: (device: UsbDevice) => void

		// Playlist export callback
		onPlaylistExport: (playlist: Playlist) => void

		// DiscoveryView callbacks
		onDiscoveryViewAddRelease: () => void

		// Discovery callbacks
		onDiscoveryReleaseOpenInBrowser: (release: DiscoveryRelease) => void
		onDiscoveryReleaseRefreshMetadata: (releases: DiscoveryRelease[]) => void
		onDiscoveryReleaseImport: (release: DiscoveryRelease) => void
		onDiscoveryReleaseDelete: (releaseIds: string[]) => void
		onDiscoveryReleaseRemoveFromPlaylist?: (playlistId: string, releaseIds: string[]) => void
		onDiscoveryReleaseMerge?: (releases: DiscoveryRelease[]) => void
		onDiscoveryReleaseAddToPlaylist?: (playlistId: string, releases: DiscoveryRelease[]) => void

		// Discovery track callbacks
		onDiscoveryTrackLikeToggle: (release: DiscoveryRelease, trackIndex: number) => void
		onDiscoveryTrackPlayPreview: (release: DiscoveryRelease, trackIndex: number) => void

		// Close callback
		onClose?: () => void
	}

	let {
		playlists,
		currentPlaylistId,
		playlistFolders,
		categoryCount,
		onTrackAddToPlaylist,
		onTrackRevealInExplorer,
		onTrackRemoveFromPlaylist,
		onTrackRemoveFromLibrary,
		onTrackRelocate,
		onTrackSetColor,
		onTrackAnalyze,
		onPlaylistCreatePlaylist,
		onPlaylistCreateSmartPlaylist,
		onPlaylistCreateFolder,
		onPlaylistEditSmartPlaylist,
		onPlaylistRename,
		onPlaylistDelete,
		onPlaylistBulkDelete,
		onPlaylistBulkMove,
		onPlaylistMove,
		onFolderViewCreatePlaylist,
		onFolderViewCreateSmartPlaylist,
		onFolderViewCreateFolder,
		onPlaylistTreeCreatePlaylist,
		onPlaylistTreeCreateSmartPlaylist,
		onPlaylistTreeCreateFolder,
		onLibraryViewImport,
		onPlaylistViewImport,
		onTagAddTag,
		onTagRename,
		onTagDelete,
		onTagMove,
		onCategoryRename,
		onCategoryDelete,
		onCategoryChangeColor,
		tagCategories = [],
		onTagsSidebarAddCategory,
		onDeviceViewInfo,
		onDeviceRevealInFinder,
		onDeviceReformat,
		onDeviceEject,
		onDeviceExport,
		onDeviceIgnore,
		onPlaylistExport,
		onDiscoveryViewAddRelease,
		onDiscoveryReleaseOpenInBrowser,
		onDiscoveryReleaseRefreshMetadata,
		onDiscoveryReleaseImport,
		onDiscoveryReleaseDelete,
		onDiscoveryReleaseRemoveFromPlaylist,
		onDiscoveryReleaseMerge,
		onDiscoveryReleaseAddToPlaylist,
		onDiscoveryTrackLikeToggle,
		onDiscoveryTrackPlayPreview,
		onClose,
	}: Props = $props()

	// =========================================================================
	// Internal State
	// =========================================================================
	// activeMenu: The desired state (what should be shown/hidden)
	// visibleMenu: What's currently rendered (stays during out-transition)
	let activeMenu = $state<ActiveContextMenu>({ type: 'none' })
	let visibleMenu = $state<ActiveContextMenu | null>(null)

	// Close device context menu if the device is disconnected
	$effect(() => {
		const menu = activeMenu
		if (menu.type === 'device') {
			const deviceStillExists = $devices.some((d) => d.id === menu.device.id)
			if (!deviceStillExists) {
				closeAll()
			}
		}
	})

	// =========================================================================
	// Exported Functions - API for parent component
	// =========================================================================

	export function closeAll() {
		activeMenu = { type: 'none' }
		onClose?.()
	}

	// Called after out-transition completes
	function handleMenuClosed() {
		visibleMenu = null
	}

	export function openTrackMenu(e: MouseEvent, tracks: Track[]) {
		e.preventDefault()
		const menu = {
			type: 'track' as const,
			x: e.clientX,
			y: e.clientY,
			tracks,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openPlaylistMenu(
		e: MouseEvent,
		playlistOrPlaylists: Playlist | Playlist[],
		source: 'tree' | 'folder'
	) {
		e.preventDefault()
		const playlists = Array.isArray(playlistOrPlaylists) ? playlistOrPlaylists : [playlistOrPlaylists]
		const menu = {
			type: 'playlist' as const,
			x: e.clientX,
			y: e.clientY,
			playlists,
			source,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openFolderViewMenu(e: MouseEvent, folderId: string) {
		e.preventDefault()
		const menu = {
			type: 'folderView' as const,
			x: e.clientX,
			y: e.clientY,
			folderId,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openPlaylistTreeMenu(e: MouseEvent) {
		e.preventDefault()
		const menu = {
			type: 'playlistTree' as const,
			x: e.clientX,
			y: e.clientY,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openLibraryViewMenu(e: MouseEvent) {
		e.preventDefault()
		const menu = {
			type: 'libraryView' as const,
			x: e.clientX,
			y: e.clientY,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openDiscoveryViewMenu(e: MouseEvent) {
		e.preventDefault()
		const menu = {
			type: 'discoveryView' as const,
			x: e.clientX,
			y: e.clientY,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openPlaylistViewMenu(e: MouseEvent, playlist: Playlist) {
		e.preventDefault()
		const menu = {
			type: 'playlistView' as const,
			x: e.clientX,
			y: e.clientY,
			playlist,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openTagMenu(e: MouseEvent, target: TagContextTarget) {
		e.preventDefault()
		const menu = {
			type: 'tag' as const,
			x: e.clientX,
			y: e.clientY,
			target,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openTagsSidebarMenu(e: MouseEvent) {
		e.preventDefault()
		const menu = {
			type: 'tagsSidebar' as const,
			x: e.clientX,
			y: e.clientY,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openDeviceMenu(e: MouseEvent, device: UsbDevice) {
		e.preventDefault()
		const menu = {
			type: 'device' as const,
			x: e.clientX,
			y: e.clientY,
			device,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openDiscoveryReleaseMenu(e: MouseEvent, releases: DiscoveryRelease[]) {
		e.preventDefault()
		const menu = {
			type: 'discoveryRelease' as const,
			x: e.clientX,
			y: e.clientY,
			releases,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	export function openDiscoveryTrackMenu(
		e: MouseEvent,
		release: DiscoveryRelease,
		trackIndex: number,
		canPlay: boolean
	) {
		e.preventDefault()
		const menu = {
			type: 'discoveryTrack' as const,
			x: e.clientX,
			y: e.clientY,
			release,
			trackIndex,
			canPlay,
		}
		activeMenu = menu
		visibleMenu = menu
	}

	// =========================================================================
	// Internal Handlers - Close menu and invoke parent callback
	// =========================================================================

	// Track handlers
	function handleTrackAddToPlaylist(playlistId: string) {
		if (activeMenu.type === 'track') {
			const tracks = activeMenu.tracks
			closeAll()
			onTrackAddToPlaylist(playlistId, tracks)
		}
	}

	function handleTrackRevealInExplorer() {
		if (activeMenu.type === 'track' && activeMenu.tracks.length === 1) {
			const track = activeMenu.tracks[0]
			closeAll()
			onTrackRevealInExplorer(track)
		}
	}

	function handleTrackRemoveFromPlaylist() {
		if (activeMenu.type === 'track') {
			const tracks = activeMenu.tracks
			closeAll()
			onTrackRemoveFromPlaylist(tracks)
		}
	}

	function handleTrackRemoveFromLibrary() {
		if (activeMenu.type === 'track') {
			const tracks = activeMenu.tracks
			closeAll()
			onTrackRemoveFromLibrary(tracks)
		}
	}

	function handleTrackRelocate(track: Track) {
		closeAll()
		onTrackRelocate(track)
	}

	function handleTrackSetColor(color: TrackColor | null) {
		if (activeMenu.type === 'track') {
			const tracks = activeMenu.tracks
			closeAll()
			onTrackSetColor(color, tracks)
		}
	}

	function handleTrackAnalyze() {
		if (activeMenu.type === 'track') {
			const tracks = activeMenu.tracks
			closeAll()
			onTrackAnalyze(tracks)
		}
	}

	// Playlist handlers
	function handlePlaylistCreatePlaylist(playlist: Playlist) {
		closeAll()
		onPlaylistCreatePlaylist(playlist)
	}

	function handlePlaylistCreateSmartPlaylist(playlist: Playlist) {
		closeAll()
		onPlaylistCreateSmartPlaylist(playlist)
	}

	function handlePlaylistCreateFolder(playlist: Playlist) {
		closeAll()
		onPlaylistCreateFolder(playlist)
	}

	function handlePlaylistEditSmartPlaylist(playlist: Playlist) {
		closeAll()
		onPlaylistEditSmartPlaylist(playlist)
	}

	function handlePlaylistRename(playlist: Playlist) {
		closeAll()
		onPlaylistRename(playlist)
	}

	function handlePlaylistDelete(playlist: Playlist) {
		closeAll()
		onPlaylistDelete(playlist)
	}

	function handlePlaylistBulkDelete(playlists: Playlist[]) {
		closeAll()
		onPlaylistBulkDelete(playlists)
	}

	function handlePlaylistMove(playlist: Playlist, folderId: string | null) {
		closeAll()
		onPlaylistMove(playlist, folderId)
	}

	function handlePlaylistBulkMove(playlists: Playlist[], folderId: string | null) {
		closeAll()
		onPlaylistBulkMove?.(playlists, folderId)
	}

	// FolderView handlers
	function handleFolderViewCreatePlaylist() {
		const folderId = activeMenu.type === 'folderView' ? activeMenu.folderId : null
		closeAll()
		onFolderViewCreatePlaylist(folderId)
	}

	function handleFolderViewCreateSmartPlaylist() {
		const folderId = activeMenu.type === 'folderView' ? activeMenu.folderId : null
		closeAll()
		onFolderViewCreateSmartPlaylist(folderId)
	}

	function handleFolderViewCreateFolder() {
		const folderId = activeMenu.type === 'folderView' ? activeMenu.folderId : null
		closeAll()
		onFolderViewCreateFolder(folderId)
	}

	// PlaylistTree handlers
	function handlePlaylistTreeCreatePlaylist() {
		closeAll()
		onPlaylistTreeCreatePlaylist()
	}

	function handlePlaylistTreeCreateSmartPlaylist() {
		closeAll()
		onPlaylistTreeCreateSmartPlaylist()
	}

	function handlePlaylistTreeCreateFolder() {
		closeAll()
		onPlaylistTreeCreateFolder()
	}

	// LibraryView handlers
	function handleLibraryViewImport() {
		closeAll()
		onLibraryViewImport()
	}

	// DiscoveryView handlers
	function handleDiscoveryViewAddRelease() {
		closeAll()
		onDiscoveryViewAddRelease()
	}

	// PlaylistView handlers
	function handlePlaylistViewImport() {
		if (activeMenu.type === 'playlistView') {
			const playlist = activeMenu.playlist
			closeAll()
			onPlaylistViewImport(playlist)
		}
	}

	// Tag handlers
	function handleTagAddTag(categoryId: string) {
		closeAll()
		onTagAddTag(categoryId)
	}

	function handleTagRename(tag: Tag) {
		closeAll()
		onTagRename(tag)
	}

	function handleTagDelete(tag: Tag) {
		closeAll()
		onTagDelete(tag)
	}

	function handleTagMove(tag: Tag, targetCategoryId: string) {
		closeAll()
		onTagMove?.(tag, targetCategoryId)
	}

	function handleCategoryRename(category: TagCategory) {
		closeAll()
		onCategoryRename(category)
	}

	function handleCategoryDelete(category: TagCategory) {
		closeAll()
		onCategoryDelete(category)
	}

	function handleCategoryChangeColor(category: TagCategory, color: string | null) {
		closeAll()
		onCategoryChangeColor(category, color)
	}

	// TagsSidebar handlers
	function handleTagsSidebarAddCategory() {
		closeAll()
		onTagsSidebarAddCategory()
	}

	// Device handlers
	function handleDeviceViewInfo(device: UsbDevice) {
		closeAll()
		onDeviceViewInfo(device)
	}

	function handleDeviceRevealInFinder(device: UsbDevice) {
		closeAll()
		onDeviceRevealInFinder(device)
	}

	function handleDeviceReformat(device: UsbDevice) {
		closeAll()
		onDeviceReformat(device)
	}

	function handleDeviceEject(device: UsbDevice) {
		closeAll()
		onDeviceEject(device)
	}

	function handleDeviceExport(device: UsbDevice) {
		closeAll()
		onDeviceExport(device)
	}

	function handleDeviceIgnore(device: UsbDevice) {
		closeAll()
		onDeviceIgnore(device)
	}

	// Playlist export handler
	function handlePlaylistExport(playlist: Playlist) {
		closeAll()
		onPlaylistExport(playlist)
	}

	// Discovery handlers
	function handleDiscoveryReleaseOpenInBrowser() {
		if (activeMenu.type === 'discoveryRelease' && activeMenu.releases.length === 1) {
			const release = activeMenu.releases[0]
			closeAll()
			onDiscoveryReleaseOpenInBrowser(release)
		}
	}

	function handleDiscoveryReleaseRefreshMetadata() {
		if (activeMenu.type === 'discoveryRelease') {
			const releases = [...activeMenu.releases]
			closeAll()
			onDiscoveryReleaseRefreshMetadata(releases)
		}
	}

	function handleDiscoveryReleaseImport() {
		if (activeMenu.type === 'discoveryRelease' && activeMenu.releases.length === 1) {
			const release = activeMenu.releases[0]
			closeAll()
			onDiscoveryReleaseImport(release)
		}
	}

	function handleDiscoveryReleaseDelete() {
		if (activeMenu.type === 'discoveryRelease') {
			const releaseIds = activeMenu.releases.map((r) => r.id)
			closeAll()
			onDiscoveryReleaseDelete(releaseIds)
		}
	}

	function handleDiscoveryReleaseRemoveFromPlaylist() {
		if (activeMenu.type === 'discoveryRelease' && currentPlaylistId) {
			const releaseIds = activeMenu.releases.map((r) => r.id)
			closeAll()
			onDiscoveryReleaseRemoveFromPlaylist?.(currentPlaylistId, releaseIds)
		}
	}

	function handleDiscoveryReleaseMerge() {
		if (activeMenu.type === 'discoveryRelease' && activeMenu.releases.length >= 2) {
			const releases = [...activeMenu.releases]
			closeAll()
			onDiscoveryReleaseMerge?.(releases)
		}
	}

	function handleDiscoveryReleaseAddToPlaylist(playlistId: string) {
		if (activeMenu.type === 'discoveryRelease') {
			const releases = activeMenu.releases
			closeAll()
			onDiscoveryReleaseAddToPlaylist?.(playlistId, releases)
		}
	}

	// Discovery track handlers
	function handleDiscoveryTrackLikeToggle() {
		if (activeMenu.type === 'discoveryTrack') {
			const { release, trackIndex } = activeMenu
			closeAll()
			onDiscoveryTrackLikeToggle(release, trackIndex)
		}
	}

	function handleDiscoveryTrackPlayPreview() {
		if (activeMenu.type === 'discoveryTrack') {
			const { release, trackIndex } = activeMenu
			closeAll()
			onDiscoveryTrackPlayPreview(release, trackIndex)
		}
	}

	// Discovery playlists for the context menu submenu
	const discoveryPlaylists = $derived(playlists.filter((p) => p.context === 'discovery'))
</script>

<!-- Track Context Menu -->
{#if visibleMenu?.type === 'track'}
	<TrackContextMenu
		open={activeMenu.type === 'track'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		selectedTracks={visibleMenu.tracks}
		{playlists}
		{currentPlaylistId}
		isAnalyzing={$isAnalyzing}
		onClose={closeAll}
		onClosed={handleMenuClosed}
		onRevealInExplorer={handleTrackRevealInExplorer}
		onAddToPlaylist={handleTrackAddToPlaylist}
		onRemoveFromPlaylist={handleTrackRemoveFromPlaylist}
		onRemoveFromLibrary={handleTrackRemoveFromLibrary}
		onRelocate={handleTrackRelocate}
		onSetColor={handleTrackSetColor}
		onAnalyze={handleTrackAnalyze}
	/>
{/if}

<!-- Playlist Context Menu -->
{#if visibleMenu?.type === 'playlist'}
	<PlaylistContextMenu
		open={activeMenu.type === 'playlist'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		playlists={visibleMenu.playlists}
		folders={playlistFolders}
		onClose={closeAll}
		onClosed={handleMenuClosed}
		onCreatePlaylist={handlePlaylistCreatePlaylist}
		onCreateSmartPlaylist={handlePlaylistCreateSmartPlaylist}
		onCreateFolder={handlePlaylistCreateFolder}
		onEditSmartPlaylist={handlePlaylistEditSmartPlaylist}
		onRename={handlePlaylistRename}
		onDelete={handlePlaylistDelete}
		onBulkDelete={handlePlaylistBulkDelete}
		onBulkMove={handlePlaylistBulkMove}
		onMove={handlePlaylistMove}
		onExport={handlePlaylistExport}
	/>
{/if}

<!-- Playlist Tree Context Menu (whitespace right-click) -->
{#if visibleMenu?.type === 'playlistTree'}
	<ContextMenu
		open={activeMenu.type === 'playlistTree'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		items={[
			{ id: 'add-folder', label: 'New Folder', icon: 'folder', action: handlePlaylistTreeCreateFolder },
			{ id: 'add-playlist', label: 'New Playlist', icon: 'playlist', action: handlePlaylistTreeCreatePlaylist },
			{
				id: 'add-smart-playlist',
				label: get(translate)('playlists.newSmartPlaylist'),
				icon: 'bolt',
				action: handlePlaylistTreeCreateSmartPlaylist,
			},
		]}
		onClose={closeAll}
		onClosed={handleMenuClosed}
	/>
{/if}

<!-- Folder View Context Menu (empty space right-click) -->
{#if visibleMenu?.type === 'folderView'}
	<ContextMenu
		open={activeMenu.type === 'folderView'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		items={[
			{ id: 'add-folder', label: 'New Folder', icon: 'folder', action: handleFolderViewCreateFolder },
			{ id: 'add-playlist', label: 'New Playlist', icon: 'playlist', action: handleFolderViewCreatePlaylist },
			{
				id: 'add-smart-playlist',
				label: get(translate)('playlists.newSmartPlaylist'),
				icon: 'bolt',
				action: handleFolderViewCreateSmartPlaylist,
			},
		]}
		onClose={closeAll}
		onClosed={handleMenuClosed}
	/>
{/if}

<!-- Library View Context Menu (empty space right-click) -->
{#if visibleMenu?.type === 'libraryView'}
	<ContextMenu
		open={activeMenu.type === 'libraryView'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		items={[{ id: 'import', label: 'Import track', icon: 'upload', action: handleLibraryViewImport }]}
		onClose={closeAll}
		onClosed={handleMenuClosed}
	/>
{/if}

<!-- Discovery View Context Menu (empty space right-click) -->
{#if visibleMenu?.type === 'discoveryView'}
	<ContextMenu
		open={activeMenu.type === 'discoveryView'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		items={[{ id: 'add-release', label: 'Add release', icon: 'globe', action: handleDiscoveryViewAddRelease }]}
		onClose={closeAll}
		onClosed={handleMenuClosed}
	/>
{/if}

<!-- Playlist View Context Menu (empty space right-click) -->
{#if visibleMenu?.type === 'playlistView'}
	<ContextMenu
		open={activeMenu.type === 'playlistView'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		items={[{ id: 'import', label: 'Import track', icon: 'upload', action: handlePlaylistViewImport }]}
		onClose={closeAll}
		onClosed={handleMenuClosed}
	/>
{/if}

<!-- Tag Context Menu -->
{#if visibleMenu?.type === 'tag'}
	<TagContextMenu
		open={activeMenu.type === 'tag'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		target={visibleMenu.target}
		categories={tagCategories}
		{categoryCount}
		onClose={closeAll}
		onClosed={handleMenuClosed}
		onAddCategory={handleTagsSidebarAddCategory}
		onAddTag={handleTagAddTag}
		onRenameTag={handleTagRename}
		onDeleteTag={handleTagDelete}
		onMoveTag={handleTagMove}
		onRenameCategory={handleCategoryRename}
		onDeleteCategory={handleCategoryDelete}
		onChangeColor={handleCategoryChangeColor}
	/>
{/if}

<!-- Tags Sidebar Context Menu (whitespace right-click) -->
{#if visibleMenu?.type === 'tagsSidebar'}
	<TagsSidebarContextMenu
		open={activeMenu.type === 'tagsSidebar'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		{categoryCount}
		onClose={closeAll}
		onClosed={handleMenuClosed}
		onAddCategory={handleTagsSidebarAddCategory}
	/>
{/if}

<!-- Device Context Menu -->
{#if visibleMenu?.type === 'device'}
	<DeviceContextMenu
		open={activeMenu.type === 'device'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		device={visibleMenu.device}
		isReformatting={visibleMenu.device.id === $reformattingDeviceId}
		isExporting={visibleMenu.device.id === $activeDeviceId || $syncingDeviceIds.includes(visibleMenu.device.id)}
		onClose={closeAll}
		onClosed={handleMenuClosed}
		onExport={handleDeviceExport}
		onViewInfo={handleDeviceViewInfo}
		onRevealInFinder={handleDeviceRevealInFinder}
		onReformat={handleDeviceReformat}
		onEject={handleDeviceEject}
		onIgnore={handleDeviceIgnore}
	/>
{/if}

<!-- Discovery Release Context Menu -->
{#if visibleMenu?.type === 'discoveryRelease'}
	<DiscoveryContextMenu
		open={activeMenu.type === 'discoveryRelease'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		selectedReleases={visibleMenu.releases}
		playlists={discoveryPlaylists}
		{currentPlaylistId}
		onClose={closeAll}
		onClosed={handleMenuClosed}
		onOpenInBrowser={handleDiscoveryReleaseOpenInBrowser}
		onRefreshMetadata={handleDiscoveryReleaseRefreshMetadata}
		onImport={handleDiscoveryReleaseImport}
		onMerge={handleDiscoveryReleaseMerge}
		onDelete={handleDiscoveryReleaseDelete}
		onAddToPlaylist={handleDiscoveryReleaseAddToPlaylist}
		onRemoveFromPlaylist={currentPlaylistId ? handleDiscoveryReleaseRemoveFromPlaylist : undefined}
	/>
{/if}

<!-- Discovery Track Context Menu -->
{#if visibleMenu?.type === 'discoveryTrack'}
	<DiscoveryTrackContextMenu
		open={activeMenu.type === 'discoveryTrack'}
		x={visibleMenu.x}
		y={visibleMenu.y}
		release={visibleMenu.release}
		track={visibleMenu.release.tracks[visibleMenu.trackIndex]}
		canPlay={visibleMenu.canPlay}
		onClose={closeAll}
		onClosed={handleMenuClosed}
		onLikeToggle={handleDiscoveryTrackLikeToggle}
		onPlayPreview={handleDiscoveryTrackPlayPreview}
	/>
{/if}
