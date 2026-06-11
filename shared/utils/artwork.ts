import { convertFileSrc } from '@tauri-apps/api/core'

/**
 * Converts an artwork relative path to a displayable URL using Tauri's asset protocol.
 * Returns undefined if no artwork path or app data dir is available.
 *
 * @param artworkPath - Relative path like "artwork/{track_id}.webp"
 * @param dataDir - Absolute app data directory supplied by the caller (e.g. the desktop app store)
 * @returns Asset URL for use in img src, or undefined
 */
export function getArtworkUrl(
	artworkPath: string | null | undefined,
	dataDir: string | null | undefined
): string | undefined {
	if (!artworkPath) return undefined
	if (!dataDir) return undefined

	const fullPath = `${dataDir}/${artworkPath}`
	return convertFileSrc(fullPath)
}
