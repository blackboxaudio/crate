import { convertFileSrc } from '@tauri-apps/api/core'
import { get } from 'svelte/store'
import { appDataDir } from '$lib/stores/app'

/**
 * Converts an artwork relative path to a displayable URL using Tauri's asset protocol.
 * Returns undefined if no artwork path or app data dir is available.
 *
 * @param artworkPath - Relative path like "artwork/{track_id}.webp"
 * @returns Asset URL for use in img src, or undefined
 */
export function getArtworkUrl(artworkPath: string | null | undefined): string | undefined {
	if (!artworkPath) return undefined

	const dataDir = get(appDataDir)
	if (!dataDir) return undefined

	const fullPath = `${dataDir}/${artworkPath}`
	return convertFileSrc(fullPath)
}
