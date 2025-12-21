<script lang="ts" module>
	import type { Track, Playlist, Tag, TagCategory, UsbDevice, TrackColor } from '$lib/types'

	// Tag context target discriminated union
	export type TagContextTarget =
		| { type: 'tag'; tag: Tag; category: TagCategory }
		| { type: 'category'; category: TagCategory }

	// Discriminated union for all context menu states
	export type ActiveContextMenu =
		| { type: 'none' }
		| { type: 'track'; x: number; y: number; tracks: Track[] }
		| { type: 'playlist'; x: number; y: number; playlist: Playlist; source: 'tree' | 'folder' }
		| { type: 'folderView'; x: number; y: number; folderId: string }
		| { type: 'playlistTree'; x: number; y: number }
		| { type: 'libraryView'; x: number; y: number }
		| { type: 'playlistView'; x: number; y: number; playlist: Playlist }
		| { type: 'tag'; x: number; y: number; target: TagContextTarget }
		| { type: 'tagsSidebar'; x: number; y: number }
		| { type: 'device'; x: number; y: number; device: UsbDevice }
</script>

<script lang="ts">
	import TrackContextMenu from '$lib/components/library/TrackContextMenu.svelte'
	import PlaylistContextMenu from '$lib/components/playlists/PlaylistContextMenu.svelte'
	import TagContextMenu from '$lib/components/tags/TagContextMenu.svelte'
	import TagsSidebarContextMenu from '$lib/components/tags/TagsSidebarContextMenu.svelte'
	import DeviceContextMenu from '$lib/components/devices/DeviceContextMenu.svelte'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'

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

		// Playlist callbacks
		onPlaylistRename: (playlist: Playlist) => void
		onPlaylistDelete: (playlist: Playlist) => void
		onPlaylistMove: (playlist: Playlist, folderId: string | null) => void

		// FolderView callbacks
		onFolderViewCreatePlaylist: (folderId: string | null) => void
		onFolderViewCreateFolder: (folderId: string | null) => void

		// PlaylistTree callbacks
		onPlaylistTreeCreatePlaylist: () => void
		onPlaylistTreeCreateFolder: () => void

		// LibraryView callbacks
		onLibraryViewImport: () => void

		// PlaylistView callbacks
		onPlaylistViewImport: (playlist: Playlist) => void

		// Tag callbacks
		onTagRename: (tag: Tag) => void
		onTagDelete: (tag: Tag) => void
		onCategoryRename: (category: TagCategory) => void
		onCategoryDelete: (category: TagCategory) => void
		onCategoryChangeColor: (category: TagCategory, color: string | null) => void

		// TagsSidebar callbacks
		onTagsSidebarAddCategory: () => void
		onTagsSidebarAddTag: () => void

		// Device callbacks
		onDeviceViewInfo: (device: UsbDevice) => void
		onDeviceRevealInFinder: (device: UsbDevice) => void
		onDeviceEject: (device: UsbDevice) => void
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
		onPlaylistRename,
		onPlaylistDelete,
		onPlaylistMove,
		onFolderViewCreatePlaylist,
		onFolderViewCreateFolder,
		onPlaylistTreeCreatePlaylist,
		onPlaylistTreeCreateFolder,
		onLibraryViewImport,
		onPlaylistViewImport,
		onTagRename,
		onTagDelete,
		onCategoryRename,
		onCategoryDelete,
		onCategoryChangeColor,
		onTagsSidebarAddCategory,
		onTagsSidebarAddTag,
		onDeviceViewInfo,
		onDeviceRevealInFinder,
		onDeviceEject,
	}: Props = $props()

	// =========================================================================
	// Internal State
	// =========================================================================
	let activeMenu = $state<ActiveContextMenu>({ type: 'none' })

	// =========================================================================
	// Exported Functions - API for parent component
	// =========================================================================

	export function closeAll() {
		activeMenu = { type: 'none' }
	}

	export function openTrackMenu(e: MouseEvent, tracks: Track[]) {
		e.preventDefault()
		activeMenu = {
			type: 'track',
			x: e.clientX,
			y: e.clientY,
			tracks,
		}
	}

	export function openPlaylistMenu(e: MouseEvent, playlist: Playlist, source: 'tree' | 'folder') {
		e.preventDefault()
		activeMenu = {
			type: 'playlist',
			x: e.clientX,
			y: e.clientY,
			playlist,
			source,
		}
	}

	export function openFolderViewMenu(e: MouseEvent, folderId: string) {
		e.preventDefault()
		activeMenu = {
			type: 'folderView',
			x: e.clientX,
			y: e.clientY,
			folderId,
		}
	}

	export function openPlaylistTreeMenu(e: MouseEvent) {
		e.preventDefault()
		activeMenu = {
			type: 'playlistTree',
			x: e.clientX,
			y: e.clientY,
		}
	}

	export function openLibraryViewMenu(e: MouseEvent) {
		e.preventDefault()
		activeMenu = {
			type: 'libraryView',
			x: e.clientX,
			y: e.clientY,
		}
	}

	export function openPlaylistViewMenu(e: MouseEvent, playlist: Playlist) {
		e.preventDefault()
		activeMenu = {
			type: 'playlistView',
			x: e.clientX,
			y: e.clientY,
			playlist,
		}
	}

	export function openTagMenu(e: MouseEvent, target: TagContextTarget) {
		e.preventDefault()
		activeMenu = {
			type: 'tag',
			x: e.clientX,
			y: e.clientY,
			target,
		}
	}

	export function openTagsSidebarMenu(e: MouseEvent) {
		e.preventDefault()
		activeMenu = {
			type: 'tagsSidebar',
			x: e.clientX,
			y: e.clientY,
		}
	}

	export function openDeviceMenu(e: MouseEvent, device: UsbDevice) {
		e.preventDefault()
		activeMenu = {
			type: 'device',
			x: e.clientX,
			y: e.clientY,
			device,
		}
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

	// Playlist handlers
	function handlePlaylistRename(playlist: Playlist) {
		closeAll()
		onPlaylistRename(playlist)
	}

	function handlePlaylistDelete(playlist: Playlist) {
		closeAll()
		onPlaylistDelete(playlist)
	}

	function handlePlaylistMove(playlist: Playlist, folderId: string | null) {
		closeAll()
		onPlaylistMove(playlist, folderId)
	}

	// FolderView handlers
	function handleFolderViewCreatePlaylist() {
		const folderId = activeMenu.type === 'folderView' ? activeMenu.folderId : null
		closeAll()
		onFolderViewCreatePlaylist(folderId)
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

	function handlePlaylistTreeCreateFolder() {
		closeAll()
		onPlaylistTreeCreateFolder()
	}

	// LibraryView handlers
	function handleLibraryViewImport() {
		closeAll()
		onLibraryViewImport()
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
	function handleTagRename(tag: Tag) {
		closeAll()
		onTagRename(tag)
	}

	function handleTagDelete(tag: Tag) {
		closeAll()
		onTagDelete(tag)
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

	function handleTagsSidebarAddTag() {
		closeAll()
		onTagsSidebarAddTag()
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

	function handleDeviceEject(device: UsbDevice) {
		closeAll()
		onDeviceEject(device)
	}
</script>

<!-- Track Context Menu -->
{#if activeMenu.type === 'track'}
	<TrackContextMenu
		open={true}
		x={activeMenu.x}
		y={activeMenu.y}
		selectedTracks={activeMenu.tracks}
		{playlists}
		{currentPlaylistId}
		onClose={closeAll}
		onRevealInExplorer={handleTrackRevealInExplorer}
		onAddToPlaylist={handleTrackAddToPlaylist}
		onRemoveFromPlaylist={handleTrackRemoveFromPlaylist}
		onRemoveFromLibrary={handleTrackRemoveFromLibrary}
		onRelocate={handleTrackRelocate}
		onSetColor={handleTrackSetColor}
	/>
{/if}

<!-- Playlist Context Menu -->
{#if activeMenu.type === 'playlist'}
	<PlaylistContextMenu
		open={true}
		x={activeMenu.x}
		y={activeMenu.y}
		playlist={activeMenu.playlist}
		folders={playlistFolders}
		onClose={closeAll}
		onRename={handlePlaylistRename}
		onDelete={handlePlaylistDelete}
		onMove={handlePlaylistMove}
	/>
{/if}

<!-- Playlist Tree Context Menu (whitespace right-click) -->
{#if activeMenu.type === 'playlistTree'}
	<ContextMenu
		open={true}
		x={activeMenu.x}
		y={activeMenu.y}
		items={[
			{ id: 'add-folder', label: 'New Folder', icon: 'folder', action: handlePlaylistTreeCreateFolder },
			{ id: 'add-playlist', label: 'New Playlist', icon: 'playlist', action: handlePlaylistTreeCreatePlaylist },
		]}
		onClose={closeAll}
	/>
{/if}

<!-- Folder View Context Menu (empty space right-click) -->
{#if activeMenu.type === 'folderView'}
	<ContextMenu
		open={true}
		x={activeMenu.x}
		y={activeMenu.y}
		items={[
			{ id: 'add-folder', label: 'New Folder', icon: 'folder', action: handleFolderViewCreateFolder },
			{ id: 'add-playlist', label: 'New Playlist', icon: 'playlist', action: handleFolderViewCreatePlaylist },
		]}
		onClose={closeAll}
	/>
{/if}

<!-- Library View Context Menu (empty space right-click) -->
{#if activeMenu.type === 'libraryView'}
	<ContextMenu
		open={true}
		x={activeMenu.x}
		y={activeMenu.y}
		items={[{ id: 'import', label: 'Import track', icon: 'upload', action: handleLibraryViewImport }]}
		onClose={closeAll}
	/>
{/if}

<!-- Playlist View Context Menu (empty space right-click) -->
{#if activeMenu.type === 'playlistView'}
	<ContextMenu
		open={true}
		x={activeMenu.x}
		y={activeMenu.y}
		items={[{ id: 'import', label: 'Import track', icon: 'upload', action: handlePlaylistViewImport }]}
		onClose={closeAll}
	/>
{/if}

<!-- Tag Context Menu -->
{#if activeMenu.type === 'tag'}
	<TagContextMenu
		open={true}
		x={activeMenu.x}
		y={activeMenu.y}
		target={activeMenu.target}
		onClose={closeAll}
		onRenameTag={handleTagRename}
		onDeleteTag={handleTagDelete}
		onRenameCategory={handleCategoryRename}
		onDeleteCategory={handleCategoryDelete}
		onChangeColor={handleCategoryChangeColor}
	/>
{/if}

<!-- Tags Sidebar Context Menu (whitespace right-click) -->
{#if activeMenu.type === 'tagsSidebar'}
	<TagsSidebarContextMenu
		open={true}
		x={activeMenu.x}
		y={activeMenu.y}
		{categoryCount}
		onClose={closeAll}
		onAddCategory={handleTagsSidebarAddCategory}
		onAddTag={handleTagsSidebarAddTag}
	/>
{/if}

<!-- Device Context Menu -->
{#if activeMenu.type === 'device'}
	<DeviceContextMenu
		open={true}
		x={activeMenu.x}
		y={activeMenu.y}
		device={activeMenu.device}
		onClose={closeAll}
		onViewInfo={handleDeviceViewInfo}
		onRevealInFinder={handleDeviceRevealInFinder}
		onEject={handleDeviceEject}
	/>
{/if}
