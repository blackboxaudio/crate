import { writable } from 'svelte/store'
import type { ActiveView } from '$lib/types'
import type {
	TagController,
	TrackController,
	DeviceController,
	ExportController,
	PlaylistController,
} from '$lib/controllers'

export interface PageActions {
	tagController: TagController
	trackController: TrackController
	deviceController: DeviceController
	exportController: ExportController
	playlistController: PlaylistController
	handleViewChange: (view: ActiveView) => void
	handleToggleDevTools: () => void
	playNextTrack: () => void
	playPreviousTrack: () => void
	openAddReleaseModal: () => void
	getModalOrchestrator: () => Record<string, (...args: unknown[]) => void> | undefined
	getContextMenuOrchestrator: () => Record<string, (...args: unknown[]) => void> | undefined
}

export const pageActions = writable<PageActions | null>(null)
