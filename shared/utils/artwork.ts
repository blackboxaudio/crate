import { convertFileSrc } from '@tauri-apps/api/core'
import type { DiscoveryRelease } from '../types'

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

/**
 * Cache-first displayable src for a discovery release cover. Prefers the on-disk cached copy
 * (which renders offline / in airplane mode) when present, otherwise falls back to the remote
 * URL. `dataDir` is the app data directory (on mobile, from the mobileAppDataDir store).
 */
export function getDiscoveryArtworkSrc(
	release: Pick<DiscoveryRelease, 'artwork_cache_path' | 'artwork_url'>,
	dataDir: string | null | undefined
): string | undefined {
	return getArtworkUrl(release.artwork_cache_path, dataDir) ?? release.artwork_url ?? undefined
}
