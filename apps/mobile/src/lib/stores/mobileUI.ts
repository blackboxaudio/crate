import { writable, derived } from 'svelte/store'

/** The app's primary navigation destinations, surfaced as bottom tabs. */
export type MobileTab = 'discovery' | 'playlists' | 'settings'

interface MobileUIState {
	/** Which bottom tab is active — selects the view rendered in the content area. */
	activeTab: MobileTab
	/** Discovery release whose detail screen is open (full-screen overlay), or null for the feed. */
	detailReleaseId: string | null
	/**
	 * Whether the detail screen is in its covering position (over the tab bar). Distinct from
	 * `detailReleaseId`, which stays set until the slide-out animation finishes (so `+page` keeps the
	 * drawer mounted): this flips false the moment a close *starts*, so the mini-player can begin sliding
	 * back up *as* the drawer slides out rather than after it.
	 */
	detailCovering: boolean
	/** Whether the preview player is expanded to the full-screen view (vs. the mini bar). */
	playerExpanded: boolean
	/**
	 * One-shot: release the discovery feed should scroll into view. Set when locating the playing
	 * release from the expanded player; the feed clears it via `consumeScrollTarget` once it scrolls.
	 */
	scrollTargetReleaseId: string | null
}

const initialState: MobileUIState = {
	activeTab: 'discovery',
	detailReleaseId: null,
	detailCovering: false,
	playerExpanded: false,
	scrollTargetReleaseId: null,
}

function createMobileUIStore() {
	const { subscribe, set, update } = writable<MobileUIState>(initialState)

	return {
		subscribe,
		/** Switch the active bottom tab. */
		setTab(tab: MobileTab) {
			update((s) => ({ ...s, activeTab: tab }))
		},
		/** Push the release detail screen (a full-screen overlay layered above the active tab). */
		openDetail(releaseId: string) {
			update((s) => ({ ...s, detailReleaseId: releaseId, detailCovering: true }))
		},
		/** Begin closing the detail screen: drop the covering flag so the mini-player rises as it slides out,
		 *  while leaving `detailReleaseId` set so the drawer stays mounted through its slide-out animation. */
		beginCloseDetail() {
			update((s) => ({ ...s, detailCovering: false }))
		},
		/** Finalize the close once the slide-out animation lands (clears the mount). */
		closeDetail() {
			update((s) => ({ ...s, detailReleaseId: null, detailCovering: false }))
		},
		/** Expand the mini-player to the full-screen player. */
		expandPlayer() {
			update((s) => ({ ...s, playerExpanded: true }))
		},
		collapsePlayer() {
			update((s) => ({ ...s, playerExpanded: false }))
		},
		/**
		 * Reveal the playing release in the discovery feed (desktop "locate" parity): switch to the
		 * discovery tab, collapse the full-screen player, ask the feed to scroll the release into view
		 * behind the overlay, and open its detail screen on top. The feed consumes `scrollTargetReleaseId`
		 * once it has scrolled.
		 */
		locateRelease(releaseId: string) {
			update((s) => ({
				...s,
				activeTab: 'discovery',
				playerExpanded: false,
				detailReleaseId: releaseId,
				detailCovering: true,
				scrollTargetReleaseId: releaseId,
			}))
		},
		/** Clear the one-shot scroll target once the feed has scrolled to it. */
		consumeScrollTarget() {
			update((s) => (s.scrollTargetReleaseId === null ? s : { ...s, scrollTargetReleaseId: null }))
		},
		reset() {
			set(initialState)
		},
	}
}

export const mobileUIStore = createMobileUIStore()

export const activeTab = derived(mobileUIStore, ($s) => $s.activeTab)
export const detailReleaseId = derived(mobileUIStore, ($s) => $s.detailReleaseId)
export const detailCovering = derived(mobileUIStore, ($s) => $s.detailCovering)
export const isPlayerExpanded = derived(mobileUIStore, ($s) => $s.playerExpanded)
export const scrollTargetReleaseId = derived(mobileUIStore, ($s) => $s.scrollTargetReleaseId)
