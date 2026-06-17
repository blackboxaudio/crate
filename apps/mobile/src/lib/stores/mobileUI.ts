import { writable, derived } from 'svelte/store'

/**
 * Which drawer is currently open. A single slot (rather than two booleans) makes it structurally
 * impossible to have both drawers open at once — opening one replaces the other.
 */
export type OpenDrawer = 'left' | 'right' | null

interface MobileUIState {
	openDrawer: OpenDrawer
}

const initialState: MobileUIState = {
	openDrawer: null,
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
		reset() {
			set(initialState)
		},
	}
}

export const mobileUIStore = createMobileUIStore()

export const openDrawer = derived(mobileUIStore, ($s) => $s.openDrawer)
export const isLeftOpen = derived(mobileUIStore, ($s) => $s.openDrawer === 'left')
export const isRightOpen = derived(mobileUIStore, ($s) => $s.openDrawer === 'right')
export const isAnyDrawerOpen = derived(mobileUIStore, ($s) => $s.openDrawer !== null)
