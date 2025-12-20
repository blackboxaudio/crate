import { open } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import type { Track, TrackColor, Playlist } from '$lib/types'
import type { playerStore as PlayerStoreType } from '$lib/stores/player'
import type { libraryStore as LibraryStoreType } from '$lib/stores/library'
import type { playlistsStore as PlaylistsStoreType } from '$lib/stores/playlists'
import type { missingTracksStore as MissingTracksStoreType } from '$lib/stores/missingTracks'
import type { uiStore as UIStoreType } from '$lib/stores/ui'
import type { toastStore as ToastStoreType } from '$lib/stores/toast'
import * as libraryApi from '$lib/api/library'

// =============================================================================
// Types
// =============================================================================

export interface TrackControllerDeps {
	playerStore: typeof PlayerStoreType
	libraryStore: typeof LibraryStoreType
	playlistsStore: typeof PlaylistsStoreType
	missingTracksStore: typeof MissingTracksStoreType
	uiStore: typeof UIStoreType
	toastStore: typeof ToastStoreType
	// Getters for reactive state
	getSelectedPlaylistId: () => string | null
	getPlaylists: () => Playlist[]
	getMissingTrackIds: () => Set<string>
}

export interface TrackControllerModalActions {
	openRelocateModal: (track: Track) => void
	openRemoveFromPlaylistModal: (trackIds: string[], playlistId: string) => void
	openRemoveFromLibraryModal: (trackIds: string[]) => void
}

export interface TrackController {
	// Playback
	play: (track: Track) => void

	// Selection
	handleSelectionChange: (ids: Set<string>) => void

	// Playlist operations
	addToPlaylist: (playlistId: string, tracks: Track[]) => Promise<void>
	handleTracksDropOnPlaylist: (playlistId: string, trackIds: string[]) => Promise<void>

	// File operations
	revealInExplorer: (track: Track) => Promise<void>
	handleImport: () => Promise<void>
	handleExternalFileDrop: (audioPaths: string[]) => Promise<void>

	// Removal operations
	removeFromPlaylistClick: (tracks: Track[]) => void
	removeFromLibraryClick: (tracks: Track[]) => void

	// Color operations
	setColor: (trackIds: string[], color: TrackColor | null) => Promise<void>
	setColorFromContextMenu: (color: TrackColor | null, tracks: Track[]) => Promise<void>
}

// =============================================================================
// Controller Factory
// =============================================================================

export function createTrackController(
	deps: TrackControllerDeps,
	modalActions?: TrackControllerModalActions
): TrackController {
	const {
		playerStore,
		libraryStore,
		playlistsStore,
		missingTracksStore,
		uiStore,
		toastStore,
		getSelectedPlaylistId,
		getPlaylists,
		getMissingTrackIds,
	} = deps

	/**
	 * Play a track, or open relocate modal if the track file is missing
	 */
	function play(track: Track): void {
		if (getMissingTrackIds().has(track.id)) {
			if (modalActions) {
				modalActions.openRelocateModal(track)
			} else {
				console.warn('TrackController: modalActions not provided, cannot open relocate modal')
			}
			return
		}
		playerStore.play(track)
	}

	/**
	 * Handle track selection change and lazy-check for missing files
	 */
	function handleSelectionChange(ids: Set<string>): void {
		uiStore.setSelectedTracks(ids)

		// Check file existence for newly selected tracks (lazy load)
		// Only check when selecting a single track to avoid excessive checks
		if (ids.size === 1) {
			const trackId = [...ids][0]
			// Don't check if already known to be missing or currently checking
			if (!getMissingTrackIds().has(trackId) && !missingTracksStore.isChecking(trackId)) {
				missingTracksStore.checkTrack(trackId)
			}
		}
	}

	/**
	 * Add tracks to a playlist
	 */
	async function addToPlaylist(playlistId: string, tracks: Track[]): Promise<void> {
		const trackIds = tracks.map((t) => t.id)
		await playlistsStore.addTracks(playlistId, trackIds)
	}

	/**
	 * Handle dropping tracks onto a playlist
	 */
	async function handleTracksDropOnPlaylist(playlistId: string, trackIds: string[]): Promise<void> {
		try {
			await playlistsStore.addTracks(playlistId, trackIds)
			// Find playlist name for the toast message
			const playlists = getPlaylists()
			const playlist = playlists.find((p) => p.id === playlistId)
			const playlistName = playlist?.name || 'playlist'
			const count = trackIds.length
			toastStore.success(count === 1 ? `1 track added to ${playlistName}` : `${count} tracks added to ${playlistName}`)
		} catch (error) {
			toastStore.error('Failed to add tracks to playlist')
		}
	}

	/**
	 * Reveal a track in the system file explorer
	 */
	async function revealInExplorer(track: Track): Promise<void> {
		await revealItemInDir(track.file_path)
	}

	/**
	 * Open file dialog and import tracks to the library
	 */
	async function handleImport(): Promise<void> {
		const selected = await open({
			multiple: true,
			filters: [
				{
					name: 'Audio Files',
					extensions: ['mp3', 'wav', 'aiff', 'aif', 'flac', 'm4a', 'aac'],
				},
			],
		})

		if (selected && Array.isArray(selected)) {
			await libraryStore.importTracks(selected)
		}
	}

	/**
	 * Handle files dropped from the OS file explorer
	 */
	async function handleExternalFileDrop(audioPaths: string[]): Promise<void> {
		const selectedPlaylistId = getSelectedPlaylistId()
		const playlists = getPlaylists()

		if (selectedPlaylistId) {
			// Import and add to playlist with combined toast
			const result = await libraryApi.importTracks(audioPaths)

			// Update library store state
			libraryStore.addTracksToState(result.tracks)

			if (result.tracks.length > 0) {
				const trackIds = result.tracks.map((t) => t.id)
				const playlist = playlists.find((p) => p.id === selectedPlaylistId)
				const playlistName = playlist?.name || 'playlist'

				try {
					await playlistsStore.addTracks(selectedPlaylistId, trackIds)
					const count = result.tracks.length
					const trackWord = count === 1 ? 'track' : 'tracks'
					if (result.failed_count > 0) {
						toastStore.warning(
							`${count} ${trackWord} imported and added to ${playlistName}, ${result.failed_count} failed`
						)
					} else {
						toastStore.success(`${count} ${trackWord} imported and added to ${playlistName}`)
					}
				} catch {
					toastStore.warning(`Tracks imported but failed to add to ${playlistName}`)
				}
			} else if (result.failed_count > 0) {
				toastStore.error(`Failed to import tracks: ${result.errors[0] || 'Unknown error'}`)
			}
		} else {
			// Library/folder view - use standard import with its own toast
			await libraryStore.importTracks(audioPaths)
		}
	}

	/**
	 * Open the remove from playlist confirmation modal
	 */
	function removeFromPlaylistClick(tracks: Track[]): void {
		const selectedPlaylistId = getSelectedPlaylistId()
		if (selectedPlaylistId && modalActions) {
			const trackIds = tracks.map((t) => t.id)
			modalActions.openRemoveFromPlaylistModal(trackIds, selectedPlaylistId)
		} else if (!modalActions) {
			console.warn('TrackController: modalActions not provided, cannot open remove modal')
		}
	}

	/**
	 * Open the remove from library confirmation modal
	 */
	function removeFromLibraryClick(tracks: Track[]): void {
		if (modalActions) {
			const trackIds = tracks.map((t) => t.id)
			modalActions.openRemoveFromLibraryModal(trackIds)
		} else {
			console.warn('TrackController: modalActions not provided, cannot open remove modal')
		}
	}

	/**
	 * Set color for tracks
	 */
	async function setColor(trackIds: string[], color: TrackColor | null): Promise<void> {
		await libraryStore.setTrackColors(trackIds, color)
	}

	/**
	 * Set color from context menu
	 */
	async function setColorFromContextMenu(color: TrackColor | null, tracks: Track[]): Promise<void> {
		const trackIds = tracks.map((t) => t.id)
		await libraryStore.setTrackColors(trackIds, color)
	}

	return {
		play,
		handleSelectionChange,
		addToPlaylist,
		handleTracksDropOnPlaylist,
		revealInExplorer,
		handleImport,
		handleExternalFileDrop,
		removeFromPlaylistClick,
		removeFromLibraryClick,
		setColor,
		setColorFromContextMenu,
	}
}
