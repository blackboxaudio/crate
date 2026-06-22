import { writable, derived } from 'svelte/store'

/**
 * Which drawer is currently open. A single slot (rather than two booleans) makes it structurally
 * impossible to have both drawers open at once — opening one replaces the other.
 */
export type OpenDrawer = 'left' | 'right' | null

interface MobileUIState {
	openDrawer: OpenDrawer
	/** Discovery release whose detail screen is open (full-screen overlay), or null for the feed. */
	detailReleaseId: string | null
	/** Whether the preview player is expanded to the full-screen view (vs. the mini bar). */
	playerExpanded: boolean
}

const initialState: MobileUIState = {
	openDrawer: null,
	detailReleaseId: null,
	playerExpanded: false,
}

function createMobileUIStore() {
	const { subscribe, set, update } = writable<MobileUIState>(initialState)

	return {
		subscribe,
		openLeft() {
			update((s) => ({ ...s, openDrawer: 'left' }))
		},
		openRight() {
			update((s) => ({ ...s, openDrawer: 'right' }))
		},
		close() {
			update((s) => ({ ...s, openDrawer: null }))
		},
		toggleLeft() {
			update((s) => ({ ...s, openDrawer: s.openDrawer === 'left' ? null : 'left' }))
		},
		toggleRight() {
			update((s) => ({ ...s, openDrawer: s.openDrawer === 'right' ? null : 'right' }))
		},
		/** Push the release detail screen (closes any open drawer so it doesn't sit atop the overlay). */
		openDetail(releaseId: string) {
			update((s) => ({ ...s, detailReleaseId: releaseId, openDrawer: null }))
		},
		closeDetail() {
			update((s) => ({ ...s, detailReleaseId: null }))
		},
		/** Expand the mini-player to the full-screen player. */
		expandPlayer() {
			update((s) => ({ ...s, playerExpanded: true }))
		},
		collapsePlayer() {
			update((s) => ({ ...s, playerExpanded: false }))
		},
		reset() {
			set(initialState)
		},
	}
}

export const mobileUIStore = createMobileUIStore()

export const openDrawer = derived(mobileUIStore, ($s) => $s.openDrawer)
export const isLeftOpen = derived(mobileUIStore, ($s) => $s.openDrawer === 'left')
export const isRightOpen = derived(mobileUIStore, ($s) => $s.openDrawer === 'right')
export const detailReleaseId = derived(mobileUIStore, ($s) => $s.detailReleaseId)
export const isPlayerExpanded = derived(mobileUIStore, ($s) => $s.playerExpanded)
