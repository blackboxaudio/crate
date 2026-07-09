import { get, type Readable } from 'svelte/store'
import { discoveryStore } from '$shared/stores/discovery'
import { discoveryPlaylistReleases } from '$shared/stores/discoveryPlaylist'
import { followStore } from '$shared/stores/follow'
import { playlistsStore } from '$shared/stores/playlists'
import { tagsStore } from '$shared/stores/tags'
import { mobileUIStore } from './mobileUI'

/**
 * Boot validation for the persisted navigation state (see `mobileUI.ts`): the restored folder trail /
 * detail-overlay IDs / scroll anchor may reference entities deleted since the last session (removed on
 * another device and cloud-synced away). Once the owning stores load, stale references are silently
 * cleared — the trail truncates at the first missing folder, a stale detail overlay closes back to its
 * tab, and a stale scroll anchor is dropped so the feed falls back to its raw offset.
 *
 * Called fire-and-forget from `+page.svelte`'s `onMount`, which runs AFTER the tab views' `onMount`
 * loads have synchronously flipped their store's `loading` flag — so "already loading" reliably means
 * the active tab kicked the load off and we only need to wait for it.
 */
export async function validateRestoredNavigation(): Promise<void> {
	const ui = get(mobileUIStore)
	const tasks: Promise<void>[] = []

	if (ui.playlistFolderTrail.length > 0 || ui.detailPlaylistId !== null) {
		tasks.push(waitUntilLoaded(playlistsStore, () => playlistsStore.load()).then(validatePlaylists))
	}
	if (ui.detailTagId !== null) {
		tasks.push(waitUntilLoaded(tagsStore, () => tagsStore.load()).then(validateTag))
	}
	if (ui.detailFollowSourceId !== null) {
		tasks.push(waitUntilLoaded(followStore, () => followStore.load()).then(validateFollowSource))
	}
	if (ui.detailReleaseId !== null) {
		tasks.push(waitForDiscoveryLoaded().then(validateRelease))
	}

	await Promise.all(tasks)
}

/** Wait for a lazily-loaded store: if its view already started the load, wait for it to finish;
 *  otherwise (the store's tab isn't active this boot) run the load ourselves. */
function waitUntilLoaded(store: Readable<{ loading: boolean }>, load: () => Promise<unknown>): Promise<void> {
	if (!get(store).loading) return load().then(() => undefined)
	return whenNotLoading(store)
}

function whenNotLoading(store: Readable<{ loading: boolean }>): Promise<void> {
	return new Promise((resolve) => {
		// The microtask defers past the synchronous first emission, so `unsubscribe` is always assigned
		// by the time it runs (subscribe callbacks fire immediately in Svelte stores).
		const unsubscribe = store.subscribe((s) => {
			if (s.loading) return
			queueMicrotask(() => {
				unsubscribe()
				resolve()
			})
		})
	})
}

/** Discovery streams the whole collection through one `loadReleases()` — only start it if nothing has
 *  (neither the Discovery view nor a detail view that self-loads); a duplicate call would needlessly
 *  cancel and restart the in-flight load via its generation token. */
async function waitForDiscoveryLoaded(): Promise<void> {
	const s = get(discoveryStore)
	if (s.releases.length === 0 && !s.loading) {
		await discoveryStore.loadReleases()
		return
	}
	if (get(discoveryStore).loading) await whenNotLoading(discoveryStore)
}

function validatePlaylists(): void {
	const playlists = get(playlistsStore).playlists.filter((p) => p.context === 'discovery')
	const trail = get(mobileUIStore).playlistFolderTrail
	const firstMissing = trail.findIndex((id) => !playlists.some((p) => p.id === id && p.is_folder))
	if (firstMissing !== -1) mobileUIStore.setPlaylistFolderTrail(trail.slice(0, firstMissing))
	const playlistId = get(mobileUIStore).detailPlaylistId
	if (playlistId !== null && !playlists.some((p) => p.id === playlistId)) mobileUIStore.closePlaylist()
}

function validateTag(): void {
	const tagId = get(mobileUIStore).detailTagId
	if (tagId === null) return
	const exists = get(tagsStore).categories.some((c) => c.tags.some((t) => t.id === tagId))
	if (!exists) mobileUIStore.closeTag()
}

function validateFollowSource(): void {
	const sourceId = get(mobileUIStore).detailFollowSourceId
	if (sourceId === null) return
	if (!get(followStore).sources.some((s) => s.id === sourceId)) mobileUIStore.closeFollowSource()
}

function validateRelease(): void {
	const state = get(mobileUIStore)
	if (state.detailReleaseId !== null) {
		const id = state.detailReleaseId
		const exists =
			get(discoveryStore).releases.some((r) => r.id === id) || get(discoveryPlaylistReleases).some((r) => r.id === id)
		if (!exists) mobileUIStore.closeDetail()
	}
	// Opportunistic: with the full collection loaded anyway, drop a stale scroll anchor here so a later
	// Discovery visit doesn't wait for it (the Discovery view's own fallback covers the on-tab case).
	const anchor = get(mobileUIStore).discoveryRestoreAnchor
	if (anchor && !get(discoveryStore).releases.some((r) => r.id === anchor.releaseId)) {
		mobileUIStore.consumeDiscoveryAnchor()
	}
}
