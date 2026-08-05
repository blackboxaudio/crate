import { writable, derived, get } from 'svelte/store'
import { sortedReleases, discoveryStore } from '$shared/stores/discovery'
import { discoveryPlaylistReleases } from '$shared/stores/discoveryPlaylist'
import { followedSources } from '$shared/stores/follow'
import { releasesFromSource } from '$shared/utils'
import {
	getStoredArray,
	getStoredNumber,
	getStoredString,
	removeStored,
	setStoredArray,
	setStoredNumber,
	setStoredString,
} from '$shared/utils/storage'
import type { DiscoveryRelease, SortDirection, TagFilterMode } from '$shared/types'
import { ownedReleaseIds } from '$shared/stores/collection'
import { fullyCachedIds } from './offlineCache'

/** The app's primary navigation destinations, surfaced as bottom tabs. Settings is intentionally NOT a
 *  tab — it opens as a right-side drawer from the Header's gear button (see `openSettings`). */
export type MobileTab = 'discovery' | 'following' | 'playlists' | 'tags'

/** Sort field for the Playlists tab's folder listing (applied within the folders-first grouping). */
export type PlaylistsSortField = 'name' | 'date_created' | 'date_modified'

/** Settings drawer pages: a root grouped list plus flat sub-pages. The IA is exactly two levels, so
 *  a single value (not a trail) is enough. */
export type SettingsPage =
	| 'root'
	| 'general'
	| 'appearance'
	| 'following'
	| 'collection'
	| 'cloudSync'
	| 'storage'
	| 'about'

/** Where a preview-playback session was started from — selects which list scopes next / shuffle, and
 *  whether the discovery feed's live filter changes should keep re-scoping it. */
export type PlaybackContextOrigin = 'discovery' | 'playlist' | 'tag' | 'follow'

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
	/**
	 * Last scroll offset (px) of the discovery feed. Persisted here because the shell remounts the feed
	 * on every return to the Discovery tab (`{#key activeTab}`), recreating its scroll container — so the
	 * feed saves its offset as the user scrolls and restores it on mount, keeping their place after a
	 * long scroll through the releases.
	 */
	discoveryScrollTop: number
	/**
	 * Active tag-filter IDs for the discovery feed. Filtering is client-side (AND/OR over the already
	 * loaded set) — unlike desktop, which reloads from the DB — so it stays instant and never resets the
	 * feed's scroll position. Held here (not the shared `uiStore`) so it's self-contained to mobile.
	 */
	tagFilterIds: string[]
	/** Whether the tag filter requires ALL selected tags (`and`) or ANY (`or`). */
	tagFilterMode: TagFilterMode
	/** Downloaded-only feed filter: show just the releases whose audio is fully cached (offline-ready). */
	downloadedOnly: boolean
	/** Purchased-only feed filter: swap the feed for the linked Bandcamp collection (the Purchased view). */
	purchasedOnly: boolean
	/** Whether the feed is in multi-select mode (entered by long-pressing a release). */
	selectMode: boolean
	/** Releases selected while in multi-select mode (batch delete / batch tag). */
	selectedReleaseIds: Set<string>
	/** Whether the add-release sheet is open. The sheet is a placeholder this pass; #56 fills it in. */
	addReleaseOpen: boolean
	/**
	 * One-shot URL to prefill the add-release sheet with (Android share-intent intake, #62). Set by
	 * `openAddReleaseWithUrl`; the sheet consumes it after its reset-on-open effect. Ephemeral —
	 * deliberately not persisted, so a share never replays after a restart.
	 */
	addReleasePrefillUrl: string | null
	/** The one release row whose swipe actions are revealed — opening another closes it. */
	openRowId: string | null
	/** Which side of the open row is revealed: 'trail' = swipe-left (queue/delete), 'lead' =
	 *  swipe-right (play next). Meaningful only while `openRowId` is non-null. */
	openRowSide: 'lead' | 'trail'
	/** Whether the settings drawer is mounted (a full-width right-side overlay). Mirrors `detailReleaseId`
	 *  as the mount flag; stays set until the slide-out animation finishes so `+page` keeps it mounted. The
	 *  drawer is opaque and sits above the mini-player (z-45 > z-40), so — unlike the detail overlays that
	 *  the mini-player floats *above* — no `covering` flag is needed: it simply covers the mini-player. */
	settingsOpen: boolean
	/** The settings drawer's current page ('root' = the grouped list; sub-pages slide in-place). Not
	 *  persisted — the drawer itself never survives a restart, and `openSettings` always sets the page
	 *  explicitly (gear → root, sync chip → cloudSync), so the entry points stay deterministic. */
	settingsPage: SettingsPage
	/** Discovery playlist whose detail screen is open (full-screen overlay), or null. */
	detailPlaylistId: string | null
	/** Whether the playlist detail is in its covering position (mirrors detailCovering). */
	playlistDetailCovering: boolean
	/** Whether the playlist detail is in reorder mode (long-press-drag to reorder releases). */
	playlistReorderMode: boolean
	/** Tag whose detail screen (its filtered release feed) is open (full-screen overlay), or null. */
	detailTagId: string | null
	/** Whether the tag detail is in its covering position (mirrors detailCovering). */
	tagDetailCovering: boolean
	/** Followed source (artist/label) whose detail screen — its releases — is open (overlay), or null. */
	detailFollowSourceId: string | null
	/** Whether the follow detail is in its covering position (mirrors detailCovering). */
	followDetailCovering: boolean
	/** Release ID for which the actions sheet is open, or null. */
	actionsReleaseId: string | null
	/** Context in which the actions sheet was opened — determines available actions. */
	actionsContext: 'feed' | 'playlist' | 'tag' | 'follow' | null
	/**
	 * Viewport rect of the long-pressed row (captured at long-press fire-time), so the context menu can
	 * lift a preview of it in place and anchor the platter to it. Plain snapshot (not a live `DOMRect`).
	 */
	actionsAnchorRect: { top: number; left: number; width: number; height: number } | null
	/**
	 * Release whose inline follow-artist/label sheet is open, or null. Set from the release context menu's
	 * "Follow" action; a single `FollowSheet` (mounted in `+page`) opens for it, so the action works the same
	 * from the feed, playlist-detail, and tag-detail context menus without threading props through any of them.
	 */
	followReleaseId: string | null
	/**
	 * The origin of the ACTIVE preview-playback session (captured when playback starts), or null when
	 * nothing is playing. Only a `discovery`-origin queue keeps following the feed's live filter; tag /
	 * follow / playlist queues are fixed snapshots of the list they started from.
	 */
	queueOrigin: PlaybackContextOrigin | null
	/**
	 * Bumped when the active tab is re-tapped with nothing to pop — the mounted view scrolls to the top
	 * (iOS "tap the active tab to scroll to top"). A nonce rather than a boolean so repeated taps each fire.
	 */
	scrollTopNonce: number
	/**
	 * Bumped when the active tab is re-tapped while a drill-in overlay is open — the topmost overlay pops
	 * to root (iOS "tap the active tab to pop the navigation stack"). A nonce so repeated taps back out level
	 * by level.
	 */
	overlayPopNonce: number
	/**
	 * The Playlists tab's folder trail (root → deepest), lifted out of `PlaylistsView` so it survives both
	 * the tab-switch remount (`{#key activeTab}`) and — persisted — an app restart. The last entry is the
	 * folder currently shown; empty means the root level.
	 */
	playlistFolderTrail: string[]
	/**
	 * One-shot boot anchor for the discovery feed's scroll restore: the release that was topmost when the
	 * app was last killed, plus the scroll offset within its row. The feed consumes it on its first mount
	 * (via `consumeDiscoveryAnchor`) once the release streams into the progressively loading list —
	 * anchoring by ID survives new releases shifting the list, unlike the raw `discoveryScrollTop`.
	 */
	discoveryRestoreAnchor: { releaseId: string; offset: number } | null
	/** Sort for the Playlists tab's folder listing. Persisted. */
	playlistsSort: { field: PlaylistsSortField; direction: SortDirection }
	/** Discovery feed layout: classic rows or the 3-column artwork grid. Persisted. */
	discoveryViewMode: 'list' | 'grid'
	/**
	 * The displayed (sorted/filtered) list of the OPEN detail overlay (playlist/tag/follow), published
	 * by the overlay while mounted so `activePlaybackContext` scopes playback to exactly what's on
	 * screen — not the raw unsorted set. Null when no overlay is open (or it hasn't published yet).
	 * Ephemeral by design.
	 */
	overlayDisplayedReleases: DiscoveryRelease[] | null
}

const defaultState: MobileUIState = {
	activeTab: 'discovery',
	detailReleaseId: null,
	detailCovering: false,
	playerExpanded: false,
	scrollTargetReleaseId: null,
	discoveryScrollTop: 0,
	tagFilterIds: [],
	tagFilterMode: 'or',
	downloadedOnly: false,
	purchasedOnly: false,
	selectMode: false,
	selectedReleaseIds: new Set(),
	addReleaseOpen: false,
	addReleasePrefillUrl: null,
	openRowId: null,
	openRowSide: 'trail',
	settingsOpen: false,
	settingsPage: 'root',
	detailPlaylistId: null,
	playlistDetailCovering: false,
	playlistReorderMode: false,
	detailTagId: null,
	tagDetailCovering: false,
	detailFollowSourceId: null,
	followDetailCovering: false,
	actionsReleaseId: null,
	actionsContext: null,
	actionsAnchorRect: null,
	followReleaseId: null,
	queueOrigin: null,
	scrollTopNonce: 0,
	overlayPopNonce: 0,
	playlistFolderTrail: [],
	discoveryRestoreAnchor: null,
	playlistsSort: { field: 'name', direction: 'asc' },
	discoveryViewMode: 'list',
	overlayDisplayedReleases: null,
}

// --- Persisted navigation state (localStorage via shared/utils/storage) -------------------------
// The persisted slice of the UI state: active tab, open detail overlays, the Playlists folder trail,
// and the discovery feed's scroll position (raw offset + release-ID anchor) — so a killed app reopens
// where the user left off. Ephemeral state (multi-select, expanded player, sheets, tag filters, nonces)
// intentionally resets. Stale IDs (entities deleted from another device) are validated and cleared
// after the stores load — see `navRestore.ts`.
const STORAGE_KEYS = {
	activeTab: 'mobile.nav.activeTab',
	detailReleaseId: 'mobile.nav.detailReleaseId',
	detailPlaylistId: 'mobile.nav.detailPlaylistId',
	detailTagId: 'mobile.nav.detailTagId',
	detailFollowSourceId: 'mobile.nav.detailFollowSourceId',
	playlistFolderTrail: 'mobile.nav.playlistFolderTrail',
	scrollTop: 'mobile.discovery.scrollTop',
	anchorReleaseId: 'mobile.discovery.anchorReleaseId',
	anchorOffset: 'mobile.discovery.anchorOffset',
	playlistsSort: 'mobile.playlists.sort',
	discoveryViewMode: 'mobile.discovery.viewMode',
} as const

/** Row height (px) of the discovery feed's release cards (list mode) — the feed passes it to
 *  `ReleaseFeedList` and the scroll persistence derives the anchor row from it, so the two can't
 *  drift. Grid mode passes its own geometry via `setDiscoveryScrollTop`'s `geom` parameter. */
export const DISCOVERY_ROW_HEIGHT = 72

/** Feed scroll geometry: virtual row height + releases per row (1 in list mode, 3 in grid mode).
 *  The scroll persistence uses it to translate a pixel offset into a release-ID anchor. */
export interface ScrollGeom {
	rowHeight: number
	cols: number
}

const LIST_SCROLL_GEOM: ScrollGeom = { rowHeight: DISCOVERY_ROW_HEIGHT, cols: 1 }

const PLAYLISTS_SORT_FIELDS: PlaylistsSortField[] = ['name', 'date_created', 'date_modified']

function readStoredPlaylistsSort(): { field: PlaylistsSortField; direction: SortDirection } {
	try {
		const raw = getStoredString(STORAGE_KEYS.playlistsSort, '')
		if (raw) {
			const parsed = JSON.parse(raw) as { field?: string; direction?: string }
			if (
				PLAYLISTS_SORT_FIELDS.includes(parsed.field as PlaylistsSortField) &&
				(parsed.direction === 'asc' || parsed.direction === 'desc')
			) {
				return { field: parsed.field as PlaylistsSortField, direction: parsed.direction }
			}
		}
	} catch {
		// Malformed stored value — fall through to the default.
	}
	return { field: 'name', direction: 'asc' }
}

function readStoredId(key: string): string | null {
	return getStoredString(key, '') || null
}

/** Overlay kinds restored from storage at boot — each detail view consumes its marker once (via
 *  `consumeBootRestoredOverlay`) to skip the slide-in animation on the restored mount. */
const bootRestoredOverlays = new Set<'release' | 'playlist' | 'tag' | 'follow'>()

function seedInitialState(): MobileUIState {
	const detailReleaseId = readStoredId(STORAGE_KEYS.detailReleaseId)
	const detailPlaylistId = readStoredId(STORAGE_KEYS.detailPlaylistId)
	const detailTagId = readStoredId(STORAGE_KEYS.detailTagId)
	const detailFollowSourceId = readStoredId(STORAGE_KEYS.detailFollowSourceId)
	if (detailReleaseId) bootRestoredOverlays.add('release')
	if (detailPlaylistId) bootRestoredOverlays.add('playlist')
	if (detailTagId) bootRestoredOverlays.add('tag')
	if (detailFollowSourceId) bootRestoredOverlays.add('follow')
	const anchorReleaseId = readStoredId(STORAGE_KEYS.anchorReleaseId)
	return {
		...defaultState,
		activeTab: getStoredString<MobileTab>(STORAGE_KEYS.activeTab, 'discovery', [
			'discovery',
			'following',
			'playlists',
			'tags',
		]),
		detailReleaseId,
		detailCovering: detailReleaseId !== null,
		detailPlaylistId,
		playlistDetailCovering: detailPlaylistId !== null,
		detailTagId,
		tagDetailCovering: detailTagId !== null,
		detailFollowSourceId,
		followDetailCovering: detailFollowSourceId !== null,
		playlistFolderTrail: getStoredArray(STORAGE_KEYS.playlistFolderTrail),
		discoveryScrollTop: getStoredNumber(STORAGE_KEYS.scrollTop, 0),
		discoveryRestoreAnchor: anchorReleaseId
			? { releaseId: anchorReleaseId, offset: getStoredNumber(STORAGE_KEYS.anchorOffset, 0) }
			: null,
		playlistsSort: readStoredPlaylistsSort(),
		discoveryViewMode: getStoredString<'list' | 'grid'>(STORAGE_KEYS.discoveryViewMode, 'list', ['list', 'grid']),
	}
}

const initialState: MobileUIState = seedInitialState()

// Debounced scroll persistence: `setDiscoveryScrollTop` fires on every (rAF-coalesced) scroll event, so
// the localStorage writes trail behind. The anchor is derived at write time from the displayed list:
// the release whose row spans the saved offset, plus the offset within that row.
let persistScrollTimer: ReturnType<typeof setTimeout> | null = null
let pendingScrollTop: number | null = null
let pendingScrollGeom: ScrollGeom = LIST_SCROLL_GEOM

function persistDiscoveryScroll(top: number, geom: ScrollGeom) {
	// While a boot restore is still pending (anchor unconsumed), last session's stored values remain the
	// truth — don't let pre-restore scroll events (often a spurious 0 at mount) wipe them before the feed
	// has scrolled back. Persistence resumes once the feed consumes the anchor (or validation drops it).
	if (get(mobileUIStore).discoveryRestoreAnchor !== null) return
	setStoredNumber(STORAGE_KEYS.scrollTop, top)
	const list = get(mobileDisplayedReleases)
	const rowIndex = Math.floor(top / geom.rowHeight)
	// Anchor = the first release of the top visible virtual row (grid rows span `cols` releases).
	const index = Math.min(list.length - 1, rowIndex * geom.cols)
	const anchor = index >= 0 ? list[index] : undefined
	setStoredString(STORAGE_KEYS.anchorReleaseId, anchor?.id ?? '')
	setStoredNumber(STORAGE_KEYS.anchorOffset, anchor ? top - rowIndex * geom.rowHeight : 0)
}

function schedulePersistDiscoveryScroll(top: number, geom: ScrollGeom) {
	pendingScrollTop = top
	pendingScrollGeom = geom
	if (persistScrollTimer !== null) clearTimeout(persistScrollTimer)
	persistScrollTimer = setTimeout(() => {
		persistScrollTimer = null
		if (pendingScrollTop !== null) persistDiscoveryScroll(pendingScrollTop, pendingScrollGeom)
		pendingScrollTop = null
	}, 300)
}

function cancelPendingScrollPersist() {
	if (persistScrollTimer !== null) clearTimeout(persistScrollTimer)
	persistScrollTimer = null
	pendingScrollTop = null
}

/** Flush any pending (debounced) scroll persistence immediately — called when the app is backgrounded,
 *  so a backgrounded-then-killed app still keeps its very latest scroll position. */
export function flushNavPersistence() {
	if (persistScrollTimer === null) return
	clearTimeout(persistScrollTimer)
	persistScrollTimer = null
	if (pendingScrollTop !== null) persistDiscoveryScroll(pendingScrollTop, pendingScrollGeom)
	pendingScrollTop = null
}

function createMobileUIStore() {
	const { subscribe, set, update } = writable<MobileUIState>(initialState)

	return {
		subscribe,
		/** Switch the active bottom tab. No-op when already there, so navigating from both the pointerdown
		 *  and the trailing click of a single touch tap (see TabBar) collapses to one state update. */
		setTab(tab: MobileTab) {
			update((s) => (s.activeTab === tab ? s : { ...s, activeTab: tab }))
		},
		/**
		 * Activate a bottom tab from the tab bar. Switching to a *different* tab just navigates. Re-tapping
		 * the *active* tab follows the iOS convention: pop the topmost drill-in to root, else exit multi-select,
		 * else scroll the view to the top. The nonces let the mounted view / overlay react (see TabBar and the
		 * tab views / detail overlays). Guard against the double fire of a touch tap in the TabBar, not here.
		 */
		activateTab(tab: MobileTab) {
			update((s) => {
				if (s.activeTab !== tab) return { ...s, activeTab: tab }
				const hasOverlay =
					s.detailReleaseId !== null ||
					s.detailPlaylistId !== null ||
					s.detailTagId !== null ||
					s.detailFollowSourceId !== null
				if (hasOverlay) return { ...s, overlayPopNonce: s.overlayPopNonce + 1 }
				if (s.selectMode) return { ...s, selectMode: false, selectedReleaseIds: new Set() }
				return { ...s, scrollTopNonce: s.scrollTopNonce + 1 }
			})
		},
		/** Push the release detail screen (a full-screen overlay layered above the active tab). Closes any
		 *  swipe-open delete row so it isn't left revealed when the user returns to the feed. */
		openDetail(releaseId: string) {
			update((s) => ({ ...s, detailReleaseId: releaseId, detailCovering: true, openRowId: null }))
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
		/** Remember the discovery feed's scroll offset so it survives the tab-switch remount. Also persists
		 *  it (debounced, with the release-ID anchor) so it survives an app restart. Grid mode passes its
		 *  own geometry so the anchor maps pixel offsets to the right release. Call at COMMIT points
		 *  (feed unmount, layout toggle) — live scrolling goes through `stageDiscoveryScrollTop`. */
		setDiscoveryScrollTop(top: number, geom: ScrollGeom = LIST_SCROLL_GEOM) {
			update((s) => (s.discoveryScrollTop === top ? s : { ...s, discoveryScrollTop: top }))
			schedulePersistDiscoveryScroll(top, geom)
		},
		/** Per-scroll-frame variant of the above: feeds the debounced restart persistence WITHOUT touching
		 *  the reactive store. Nothing subscribes to `discoveryScrollTop` live (restores read it via `get()`
		 *  at mount), but a store update per scrolled frame would still notify every derived selector —
		 *  `safe_not_equal` treats objects as always-changed — for nothing. */
		stageDiscoveryScrollTop(top: number, geom: ScrollGeom = LIST_SCROLL_GEOM) {
			schedulePersistDiscoveryScroll(top, geom)
		},
		/** Clear the one-shot boot scroll anchor once the feed has applied (or abandoned) it. */
		consumeDiscoveryAnchor() {
			update((s) => (s.discoveryRestoreAnchor === null ? s : { ...s, discoveryRestoreAnchor: null }))
		},

		// --- Playlists folder trail (the Playlists tab's drill-down path) ---------------------------
		/** Drill into a folder (append it to the trail). */
		pushPlaylistFolder(folderId: string) {
			update((s) => ({ ...s, playlistFolderTrail: [...s.playlistFolderTrail, folderId] }))
		},
		/** Back out one folder level. */
		popPlaylistFolder() {
			update((s) => ({ ...s, playlistFolderTrail: s.playlistFolderTrail.slice(0, -1) }))
		},
		/** Replace the trail wholesale (boot validation truncation, folder deleted mid-trail). */
		setPlaylistFolderTrail(trail: string[]) {
			update((s) => ({ ...s, playlistFolderTrail: trail }))
		},
		/** Set the Playlists tab's folder-listing sort (persisted via the nav-persistence subscribe). */
		setPlaylistsSort(sort: { field: PlaylistsSortField; direction: SortDirection }) {
			update((s) => ({ ...s, playlistsSort: sort }))
		},
		/** Switch the discovery feed between list rows and the 3-column artwork grid (persisted). */
		setDiscoveryViewMode(mode: 'list' | 'grid') {
			update((s) => (s.discoveryViewMode === mode ? s : { ...s, discoveryViewMode: mode }))
		},
		/** Publish (or clear, with null) the open detail overlay's displayed list — see the state doc. */
		setOverlayReleases(releases: DiscoveryRelease[] | null) {
			update((s) => (s.overlayDisplayedReleases === releases ? s : { ...s, overlayDisplayedReleases: releases }))
		},

		/** Whether this overlay kind was restored from storage at boot — consumed once, so the restored
		 *  mount skips its slide-in animation while later in-session opens animate normally. */
		consumeBootRestoredOverlay(kind: 'release' | 'playlist' | 'tag' | 'follow'): boolean {
			return bootRestoredOverlays.delete(kind)
		},

		// --- Tag filtering (client-side over the loaded feed) ---------------------------------------
		/** Add a tag to the feed filter, or remove it if already active. */
		toggleTagFilter(id: string) {
			update((s) => ({
				...s,
				tagFilterIds: s.tagFilterIds.includes(id)
					? s.tagFilterIds.filter((tid) => tid !== id)
					: [...s.tagFilterIds, id],
			}))
		},
		/** Flip the filter between AND (all selected tags) and OR (any). */
		toggleTagFilterMode() {
			update((s) => ({ ...s, tagFilterMode: s.tagFilterMode === 'or' ? 'and' : 'or' }))
		},
		/** Clear every active tag filter. */
		clearTagFilters() {
			update((s) => (s.tagFilterIds.length === 0 ? s : { ...s, tagFilterIds: [] }))
		},
		/** Show only fully-downloaded (offline-ready) releases in the feed. Ephemeral, like tag filters. */
		toggleDownloadedFilter() {
			update((s) => ({ ...s, downloadedOnly: !s.downloadedOnly }))
		},
		/** Swap the feed for the Purchased view (the linked Bandcamp collection). Ephemeral. */
		togglePurchasedFilter() {
			update((s) => ({ ...s, purchasedOnly: !s.purchasedOnly }))
		},

		// --- Multi-select -------------------------------------------------------------------------
		/** Enter multi-select mode, seeding the selection with the long-pressed release (one update,
		 *  so the seed row is selected immediately with no flash). Closes any open swipe row. */
		enterSelectMode(seedId: string) {
			update((s) => ({ ...s, selectMode: true, selectedReleaseIds: new Set([seedId]), openRowId: null }))
		},
		/** Toggle a release's membership in the multi-select set. */
		toggleReleaseSelected(id: string) {
			update((s) => {
				const next = new Set(s.selectedReleaseIds)
				if (next.has(id)) next.delete(id)
				else next.add(id)
				return { ...s, selectedReleaseIds: next }
			})
		},
		/** Leave multi-select mode and drop the selection. */
		exitSelectMode() {
			update((s) => ({ ...s, selectMode: false, selectedReleaseIds: new Set() }))
		},

		// --- Add release (entry point only; the functional modal is issue #56) ----------------------
		openAddRelease() {
			update((s) => ({ ...s, addReleaseOpen: true }))
		},
		closeAddRelease() {
			update((s) => ({ ...s, addReleaseOpen: false }))
		},
		/**
		 * Open the add-release sheet prefilled with a shared URL (Android share intent, #62) in one
		 * update: the sheet only mounts inside the Discovery tab, and the expanded player would paint
		 * over it, so both are forced alongside the open.
		 */
		openAddReleaseWithUrl(url: string) {
			update((s) => ({
				...s,
				activeTab: 'discovery',
				playerExpanded: false,
				addReleaseOpen: true,
				addReleasePrefillUrl: url,
			}))
		},
		/** Take (and clear) the one-shot prefill URL — called by the sheet once it has applied it. */
		consumeAddReleasePrefill(): string | null {
			let url: string | null = null
			update((s) => {
				url = s.addReleasePrefillUrl
				return url === null ? s : { ...s, addReleasePrefillUrl: null }
			})
			return url
		},

		// --- Settings drawer (right-side overlay; mirrors the release detail mount pattern) ------------
		/** Open the settings drawer on a page ('root' from the gear; 'cloudSync' from the sync chip).
		 *  Closes any swipe-open action row so it isn't left revealed behind the overlay. */
		openSettings(page: SettingsPage = 'root') {
			update((s) => ({
				...s,
				settingsOpen: true,
				settingsPage: page,
				openRowId: null,
			}))
		},
		/** Finalize the close once the slide-out animation lands (clears the mount). */
		closeSettings() {
			update((s) => ({ ...s, settingsOpen: false }))
		},
		/** Navigate within the settings drawer (root ⇄ sub-page). */
		setSettingsPage(page: SettingsPage) {
			update((s) => (s.settingsPage === page ? s : { ...s, settingsPage: page }))
		},

		// --- Playlist detail overlay (mirrors release detail pattern) ---------------------------------
		openPlaylist(playlistId: string) {
			update((s) => ({
				...s,
				detailPlaylistId: playlistId,
				playlistDetailCovering: true,
				playlistReorderMode: false,
				selectMode: false,
				selectedReleaseIds: new Set(),
				openRowId: null,
			}))
		},
		beginClosePlaylist() {
			update((s) => ({ ...s, playlistDetailCovering: false, playlistReorderMode: false }))
		},
		closePlaylist() {
			update((s) => ({
				...s,
				detailPlaylistId: null,
				playlistDetailCovering: false,
				playlistReorderMode: false,
			}))
		},
		toggleReorderMode() {
			update((s) => ({ ...s, playlistReorderMode: !s.playlistReorderMode }))
		},
		exitReorderMode() {
			update((s) => ({ ...s, playlistReorderMode: false }))
		},

		// --- Tag detail overlay (its filtered release feed; mirrors the playlist detail pattern) -------
		/** Open the tag detail screen — a full-screen feed of the releases carrying the tag. Drops any
		 *  active multi-select / open swipe row so it isn't left dangling behind the overlay. */
		openTag(tagId: string) {
			update((s) => ({
				...s,
				detailTagId: tagId,
				tagDetailCovering: true,
				selectMode: false,
				selectedReleaseIds: new Set(),
				openRowId: null,
			}))
		},
		/** Begin closing the tag detail (drop the covering flag so the mini-player rises as it slides out). */
		beginCloseTag() {
			update((s) => ({ ...s, tagDetailCovering: false }))
		},
		/** Finalize the close once the slide-out animation lands. */
		closeTag() {
			update((s) => ({ ...s, detailTagId: null, tagDetailCovering: false }))
		},

		// --- Follow source detail overlay (a followed artist/label's releases; mirrors the tag detail) -
		/** Open the follow-source detail screen — a full-screen feed of the releases from this artist/label.
		 *  Drops any active multi-select / open swipe row so it isn't left dangling behind the overlay. */
		openFollowSource(sourceId: string) {
			update((s) => ({
				...s,
				detailFollowSourceId: sourceId,
				followDetailCovering: true,
				selectMode: false,
				selectedReleaseIds: new Set(),
				openRowId: null,
			}))
		},
		/** Begin closing the follow-source detail (drop the covering flag as it slides out). */
		beginCloseFollowSource() {
			update((s) => ({ ...s, followDetailCovering: false }))
		},
		/** Finalize the close once the slide-out animation lands. */
		closeFollowSource() {
			update((s) => ({ ...s, detailFollowSourceId: null, followDetailCovering: false }))
		},

		// --- Release context menu ---------------------------------------------------------------------
		openActionsSheet(
			releaseId: string,
			context: 'feed' | 'playlist' | 'tag' | 'follow',
			anchorRect: { top: number; left: number; width: number; height: number } | null
		) {
			update((s) => ({
				...s,
				actionsReleaseId: releaseId,
				actionsContext: context,
				actionsAnchorRect: anchorRect,
				openRowId: null,
			}))
		},
		closeActionsSheet() {
			update((s) => ({ ...s, actionsReleaseId: null, actionsContext: null, actionsAnchorRect: null }))
		},

		// --- Inline follow sheet (follow a release's artist / label) ----------------------------------
		/** Open the follow-artist/label sheet for a release (from its context menu's "Follow" action). */
		openFollowSheet(releaseId: string) {
			update((s) => ({ ...s, followReleaseId: releaseId }))
		},
		closeFollowSheet() {
			update((s) => ({ ...s, followReleaseId: null }))
		},

		// --- Playback context origin ------------------------------------------------------------------
		/** Record where the active preview session was started from (set when playback begins), so the feed's
		 *  live filter only re-scopes a discovery-origin queue. Pass null when playback stops. */
		setQueueOrigin(origin: PlaybackContextOrigin | null) {
			update((s) => (s.queueOrigin === origin ? s : { ...s, queueOrigin: origin }))
		},

		// --- Swipe-action single-open invariant -----------------------------------------------------
		/** Record which row's swipe actions are revealed (and which side); opening one row closes any
		 *  other. Pass null to close the open row (e.g. on scroll). */
		setOpenRow(id: string | null, side: 'lead' | 'trail' = 'trail') {
			update((s) => (s.openRowId === id && s.openRowSide === side ? s : { ...s, openRowId: id, openRowSide: side }))
		},

		/** Back to a pristine state (NOT the storage-seeded boot state), wiping the persisted keys too. */
		reset() {
			cancelPendingScrollPersist()
			for (const key of Object.values(STORAGE_KEYS)) removeStored(key)
			set(defaultState)
		},
	}
}

export const mobileUIStore = createMobileUIStore()

// Persist the navigation slice on change. One targeted diff-subscribe (rather than a write in every
// setter) so no transition — present or future — can forget to persist; the diff touches only these
// six fields, so the Set-valued ephemeral state costs nothing. The scroll offset/anchor persist
// separately (debounced) from `setDiscoveryScrollTop`.
let prevPersisted = initialState
mobileUIStore.subscribe((s) => {
	if (s.activeTab !== prevPersisted.activeTab) setStoredString(STORAGE_KEYS.activeTab, s.activeTab)
	if (s.detailReleaseId !== prevPersisted.detailReleaseId)
		setStoredString(STORAGE_KEYS.detailReleaseId, s.detailReleaseId ?? '')
	if (s.detailPlaylistId !== prevPersisted.detailPlaylistId)
		setStoredString(STORAGE_KEYS.detailPlaylistId, s.detailPlaylistId ?? '')
	if (s.detailTagId !== prevPersisted.detailTagId) setStoredString(STORAGE_KEYS.detailTagId, s.detailTagId ?? '')
	if (s.detailFollowSourceId !== prevPersisted.detailFollowSourceId)
		setStoredString(STORAGE_KEYS.detailFollowSourceId, s.detailFollowSourceId ?? '')
	if (s.playlistFolderTrail !== prevPersisted.playlistFolderTrail)
		setStoredArray(STORAGE_KEYS.playlistFolderTrail, s.playlistFolderTrail)
	if (s.playlistsSort !== prevPersisted.playlistsSort)
		setStoredString(STORAGE_KEYS.playlistsSort, JSON.stringify(s.playlistsSort))
	if (s.discoveryViewMode !== prevPersisted.discoveryViewMode)
		setStoredString(STORAGE_KEYS.discoveryViewMode, s.discoveryViewMode)
	prevPersisted = s
})

export const activeTab = derived(mobileUIStore, ($s) => $s.activeTab)
export const detailReleaseId = derived(mobileUIStore, ($s) => $s.detailReleaseId)
export const detailCovering = derived(mobileUIStore, ($s) => $s.detailCovering)
export const isPlayerExpanded = derived(mobileUIStore, ($s) => $s.playerExpanded)
export const scrollTargetReleaseId = derived(mobileUIStore, ($s) => $s.scrollTargetReleaseId)
export const settingsOpen = derived(mobileUIStore, ($s) => $s.settingsOpen)
export const settingsPage = derived(mobileUIStore, ($s) => $s.settingsPage)
export const tagFilterIds = derived(mobileUIStore, ($s) => $s.tagFilterIds)
export const tagFilterMode = derived(mobileUIStore, ($s) => $s.tagFilterMode)

/** Client-side tag filter over the loaded feed (AND = all selected tags, OR = any). Exported so the
 *  detail views' per-view filters (see `utils/listControls.ts`) apply the exact same semantics. */
export function applyTagFilter(list: DiscoveryRelease[], ids: string[], mode: TagFilterMode): DiscoveryRelease[] {
	if (ids.length === 0) return list
	const set = new Set(ids)
	return mode === 'and'
		? list.filter((r) => ids.every((id) => r.tags.some((t) => t.id === id)))
		: list.filter((r) => r.tags.some((t) => set.has(t.id)))
}

export const downloadedOnly = derived(mobileUIStore, ($s) => $s.downloadedOnly)
export const purchasedOnly = derived(mobileUIStore, ($s) => $s.purchasedOnly)

/**
 * The discovery feed's displayed list: the shared `sortedReleases` (search + liked/new + sort) with the
 * mobile-only tag + downloaded + purchased filters applied. Single source of truth for both the rendered
 * feed and the playback queue captured when a preview starts — so "play / shuffle the whole list" spans
 * exactly what's on screen. (When the Purchased filter is active, the feed component ALSO appends the
 * unmatched collection items as its own separate section — those aren't releases and never enter this
 * list or the playback context.)
 */
export const mobileDisplayedReleases = derived(
	[sortedReleases, tagFilterIds, tagFilterMode, downloadedOnly, purchasedOnly, fullyCachedIds, ownedReleaseIds],
	([$sorted, $ids, $mode, $downloadedOnly, $purchasedOnly, $cached, $owned]) => {
		let list = applyTagFilter($sorted, $ids, $mode)
		if ($downloadedOnly) list = list.filter((r) => $cached.has(r.id))
		if ($purchasedOnly) list = list.filter((r) => $owned.has(r.id))
		return list
	}
)

/**
 * The release list + origin a preview started *right now* would use as its playback context: the topmost
 * open overlay's list (follow / tag / playlist detail), or — with no detail open — the discovery feed's
 * on-screen list. Read once at play time (`ReleaseDetail`) so next / shuffle scope to the view the user is
 * in. The three detail overlays are mutually exclusive, so the precedence order here never conflicts.
 */
export const activePlaybackContext = derived(
	[mobileUIStore, discoveryStore, discoveryPlaylistReleases, followedSources, mobileDisplayedReleases],
	([$ui, $disc, $playlistReleases, $follows, $displayed]): {
		origin: PlaybackContextOrigin
		releases: DiscoveryRelease[]
	} => {
		// An open overlay that has published its displayed (sorted/filtered) list wins — playback
		// should span exactly what the user sees, not the raw unsorted set.
		if ($ui.detailFollowSourceId) {
			const source = $follows.find((s) => s.id === $ui.detailFollowSourceId)
			if (source)
				return {
					origin: 'follow',
					releases: $ui.overlayDisplayedReleases ?? releasesFromSource($disc.releases, source.url),
				}
		}
		if ($ui.detailTagId) {
			const tagId = $ui.detailTagId
			return {
				origin: 'tag',
				releases: $ui.overlayDisplayedReleases ?? $disc.releases.filter((r) => r.tags.some((t) => t.id === tagId)),
			}
		}
		if ($ui.detailPlaylistId) return { origin: 'playlist', releases: $ui.overlayDisplayedReleases ?? $playlistReleases }
		return { origin: 'discovery', releases: $displayed }
	}
)
export const selectMode = derived(mobileUIStore, ($s) => $s.selectMode)
export const selectedReleaseIds = derived(mobileUIStore, ($s) => $s.selectedReleaseIds)
export const selectedReleaseCount = derived(mobileUIStore, ($s) => $s.selectedReleaseIds.size)
export const addReleaseOpen = derived(mobileUIStore, ($s) => $s.addReleaseOpen)
export const addReleasePrefillUrl = derived(mobileUIStore, ($s) => $s.addReleasePrefillUrl)
export const openRowId = derived(mobileUIStore, ($s) => $s.openRowId)
export const openRowSide = derived(mobileUIStore, ($s) => $s.openRowSide)
export const playlistsSort = derived(mobileUIStore, ($s) => $s.playlistsSort)
export const discoveryViewMode = derived(mobileUIStore, ($s) => $s.discoveryViewMode)
export const detailPlaylistId = derived(mobileUIStore, ($s) => $s.detailPlaylistId)
export const playlistDetailCovering = derived(mobileUIStore, ($s) => $s.playlistDetailCovering)
export const playlistReorderMode = derived(mobileUIStore, ($s) => $s.playlistReorderMode)
export const detailTagId = derived(mobileUIStore, ($s) => $s.detailTagId)
export const tagDetailCovering = derived(mobileUIStore, ($s) => $s.tagDetailCovering)
export const detailFollowSourceId = derived(mobileUIStore, ($s) => $s.detailFollowSourceId)
export const followDetailCovering = derived(mobileUIStore, ($s) => $s.followDetailCovering)
export const actionsReleaseId = derived(mobileUIStore, ($s) => $s.actionsReleaseId)
export const actionsContext = derived(mobileUIStore, ($s) => $s.actionsContext)
export const actionsAnchorRect = derived(mobileUIStore, ($s) => $s.actionsAnchorRect)
export const followReleaseId = derived(mobileUIStore, ($s) => $s.followReleaseId)
export const queueOrigin = derived(mobileUIStore, ($s) => $s.queueOrigin)
export const scrollTopNonce = derived(mobileUIStore, ($s) => $s.scrollTopNonce)
export const overlayPopNonce = derived(mobileUIStore, ($s) => $s.overlayPopNonce)
export const playlistFolderTrail = derived(mobileUIStore, ($s) => $s.playlistFolderTrail)
