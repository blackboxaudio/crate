import type { Playlist } from '$lib/types'

/**
 * Find a playlist by its ID.
 */
export function getPlaylistById(playlists: Playlist[], id: string): Playlist | null {
	return playlists.find((p) => p.id === id) ?? null
}

/**
 * Check if a playlist/folder has any children.
 */
export function hasChildren(playlists: Playlist[], playlistId: string): boolean {
	return playlists.some((p) => p.parent_id === playlistId)
}

/**
 * Find a playlist/folder that would conflict with moving an item to a target folder.
 * A conflict exists when another item in the target folder has the same name.
 */
export function findConflictingItem(
	playlists: Playlist[],
	movingItem: Playlist,
	targetParentId: string | null
): Playlist | null {
	return (
		playlists.find((p) => p.parent_id === targetParentId && p.name === movingItem.name && p.id !== movingItem.id) ??
		null
	)
}
