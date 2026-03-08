import { writable } from 'svelte/store'
import type { ActiveView } from '$lib/types'
import type {
	TagController,
	TrackController,
	DeviceController,
	ExportController,
	PlaylistController,
} from '$lib/controllers'
import type ModalOrchestrator from '$lib/components/common/ModalOrchestrator.svelte'
import type ContextMenuOrchestrator from '$lib/components/common/ContextMenuOrchestrator.svelte'

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
	getModalOrchestrator: () => ReturnType<typeof ModalOrchestrator> | undefined
	getContextMenuOrchestrator: () => ReturnType<typeof ContextMenuOrchestrator> | undefined
}

export const pageActions = writable<PageActions | null>(null)
