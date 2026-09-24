import { SvelteMap } from 'svelte/reactivity'
import { playlistsStore } from '$shared/stores/playlists'

// Cache of playlist id → up to 4 distinct cover URLs, for the mosaic thumbnails in PlaylistsView.
// Module-level so it survives the view's remount-on-tab-return; a SvelteMap so rows update reactively
// when covers arrive. An entry of `[]` is a real cached result ("this playlist has no covers"), not a
// miss — it stops us refetching playlists that legitimately have no artwork.
const covers = new SvelteMap<string, string[]>()

// Ids with a fetch in flight, so overlapping `ensure` calls don't double-request.
const inFlight = new Set<string>()

/** Reactive read of a playlist's cached covers (empty array until loaded / when none). */
export function getPlaylistCovers(playlistId: string): string[] {
	return covers.get(playlistId) ?? []
}

/** Batch-fetch covers for any of the given playlists not already cached or in flight. */
export async function ensurePlaylistCovers(playlistIds: string[]): Promise<void> {
	const missing = playlistIds.filter((id) => !covers.has(id) && !inFlight.has(id))
	if (missing.length === 0) return
	for (const id of missing) inFlight.add(id)
	try {
		const results = await playlistsStore.getPlaylistCoverArt(missing)
		for (const r of results) covers.set(r.playlist_id, r.artwork_urls)
		// The backend echoes every requested id, but guard against a partial result so a failed
		// fetch caches `[]` rather than retrying forever.
		for (const id of missing) if (!covers.has(id)) covers.set(id, [])
	} finally {
		for (const id of missing) inFlight.delete(id)
	}
}

/**
 * Re-fetch one playlist's covers and update the cache in place. Call after its releases change
 * (add / remove / reorder) so the thumbnail reflects the edit immediately — unlike a plain delete,
 * this never leaves the row showing a placeholder while waiting for the next list refresh.
 */
export async function refreshPlaylistCovers(playlistId: string): Promise<void> {
	const results = await playlistsStore.getPlaylistCoverArt([playlistId])
	const match = results.find((r) => r.playlist_id === playlistId)
	covers.set(playlistId, match?.artwork_urls ?? [])
}
