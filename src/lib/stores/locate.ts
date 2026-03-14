import { writable, derived } from 'svelte/store'

interface LocateState {
	pendingTrackId: string | null
	pendingReleaseId: string | null
}

const initialState: LocateState = {
	pendingTrackId: null,
	pendingReleaseId: null,
}

const { subscribe, set, update } = writable<LocateState>(initialState)

export const locateStore = {
	subscribe,
	scrollToTrack(id: string) {
		update((s) => ({ ...s, pendingTrackId: id }))
	},
	scrollToRelease(id: string) {
		update((s) => ({ ...s, pendingReleaseId: id }))
	},
	clear() {
		set(initialState)
	},
}

export const pendingScrollTrackId = derived(locateStore, ($s) => $s.pendingTrackId)
export const pendingScrollReleaseId = derived(locateStore, ($s) => $s.pendingReleaseId)
