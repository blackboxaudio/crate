import { writable, derived } from 'svelte/store'
import { sortedReleases, discoveryStore } from '$shared/stores/discovery'
import { discoveryPlaylistReleases } from '$shared/stores/discoveryPlaylist'
import { followedSources } from '$shared/stores/follow'
import { releasesFromSource } from '$shared/utils'
import type { DiscoveryRelease, TagFilterMode } from '$shared/types'

/** The app's primary navigation destinations, surfaced as bottom tabs. */
export type MobileTab = 'discovery' | 'following' | 'playlists' | 'tags' | 'settings'

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
	/** Whether the feed is in multi-select mode (entered by long-pressing a release). */
	selectMode: boolean
	/** Releases selected while in multi-select mode (batch delete / batch tag). */
	selectedReleaseIds: Set<string>
	/** Whether the add-release sheet is open. The sheet is a placeholder this pass; #56 fills it in. */
	addReleaseOpen: boolean
	/** The one release row whose swipe-to-delete action is revealed — opening another closes it. */
	openRowId: string | null
	/** One-shot: settings section to scroll into view after switching to the Settings tab. */
	settingsScrollTarget: string | null
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
}

const initialState: MobileUIState = {
	activeTab: 'discovery',
	detailReleaseId: null,
	detailCovering: false,
	playerExpanded: false,
	scrollTargetReleaseId: null,
	discoveryScrollTop: 0,
	tagFilterIds: [],
	tagFilterMode: 'or',
	selectMode: false,
	selectedReleaseIds: new Set(),
	addReleaseOpen: false,
	openRowId: null,
	settingsScrollTarget: null,
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
		/** Remember the discovery feed's scroll offset so it survives the tab-switch remount. */
		setDiscoveryScrollTop(top: number) {
			update((s) => (s.discoveryScrollTop === top ? s : { ...s, discoveryScrollTop: top }))
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

		// --- Settings deep-link --------------------------------------------------------------------
		/** Switch to the Settings tab and request a scroll to a named section (e.g. 'sync'). */
		navigateToSettings(section: string) {
			update((s) => ({ ...s, activeTab: 'settings', settingsScrollTarget: section }))
		},
		/** Clear the one-shot settings scroll target once the view has scrolled to it. */
		consumeSettingsScrollTarget() {
			update((s) => (s.settingsScrollTarget === null ? s : { ...s, settingsScrollTarget: null }))
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

		// --- Swipe-to-delete single-open invariant --------------------------------------------------
		/** Record which row's delete action is revealed; opening one row closes any other. Pass null to
		 *  close the open row (e.g. on scroll). */
		setOpenRow(id: string | null) {
			update((s) => (s.openRowId === id ? s : { ...s, openRowId: id }))
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
export const settingsScrollTarget = derived(mobileUIStore, ($s) => $s.settingsScrollTarget)
export const tagFilterIds = derived(mobileUIStore, ($s) => $s.tagFilterIds)
export const tagFilterMode = derived(mobileUIStore, ($s) => $s.tagFilterMode)

/** Client-side tag filter over the loaded feed (AND = all selected tags, OR = any). */
function applyTagFilter(list: DiscoveryRelease[], ids: string[], mode: TagFilterMode): DiscoveryRelease[] {
	if (ids.length === 0) return list
	const set = new Set(ids)
	return mode === 'and'
		? list.filter((r) => ids.every((id) => r.tags.some((t) => t.id === id)))
		: list.filter((r) => r.tags.some((t) => set.has(t.id)))
}

/**
 * The discovery feed's displayed list: the shared `sortedReleases` (search + liked/new + sort) with the
 * mobile-only tag filter applied. Single source of truth for both the rendered feed and the playback
 * queue captured when a preview starts — so "play / shuffle the whole list" spans exactly what's on screen.
 */
export const mobileDisplayedReleases = derived(
	[sortedReleases, tagFilterIds, tagFilterMode],
	([$sorted, $ids, $mode]) => applyTagFilter($sorted, $ids, $mode)
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
		if ($ui.detailFollowSourceId) {
			const source = $follows.find((s) => s.id === $ui.detailFollowSourceId)
			if (source) return { origin: 'follow', releases: releasesFromSource($disc.releases, source.url) }
		}
		if ($ui.detailTagId) {
			const tagId = $ui.detailTagId
			return { origin: 'tag', releases: $disc.releases.filter((r) => r.tags.some((t) => t.id === tagId)) }
		}
		if ($ui.detailPlaylistId) return { origin: 'playlist', releases: $playlistReleases }
		return { origin: 'discovery', releases: $displayed }
	}
)
export const selectMode = derived(mobileUIStore, ($s) => $s.selectMode)
export const selectedReleaseIds = derived(mobileUIStore, ($s) => $s.selectedReleaseIds)
export const selectedReleaseCount = derived(mobileUIStore, ($s) => $s.selectedReleaseIds.size)
export const addReleaseOpen = derived(mobileUIStore, ($s) => $s.addReleaseOpen)
export const openRowId = derived(mobileUIStore, ($s) => $s.openRowId)
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
