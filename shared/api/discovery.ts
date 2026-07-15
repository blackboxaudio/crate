import { invoke } from '@tauri-apps/api/core'
import type {
	DiscoveryRelease,
	DiscoveryReleaseCreate,
	DiscoveryReleaseUpdate,
	DiscoveryTrackCreate,
	DiscoveryFilter,
	FetchedMetadata,
	ImportResultWithDuplicates,
	ScannedPage,
	ScannedRelease,
	BulkImportResult,
} from '../types'

export async function createRelease(create: DiscoveryReleaseCreate): Promise<DiscoveryRelease> {
	return invoke<DiscoveryRelease>('create_discovery_release', { create })
}

export async function getRelease(id: string): Promise<DiscoveryRelease> {
	return invoke<DiscoveryRelease>('get_discovery_release', { id })
}

export async function getReleases(filter?: DiscoveryFilter): Promise<DiscoveryRelease[]> {
	return invoke<DiscoveryRelease[]>('get_discovery_releases', { filter: filter ?? null })
}

export async function updateRelease(id: string, update: DiscoveryReleaseUpdate): Promise<DiscoveryRelease> {
	return invoke<DiscoveryRelease>('update_discovery_release', { id, update })
}

export async function deleteRelease(id: string): Promise<void> {
	return invoke<void>('delete_discovery_release', { id })
}

export async function deleteReleases(ids: string[]): Promise<void> {
	return invoke<void>('delete_discovery_releases', { ids })
}

export async function assignTags(releaseIds: string[], tagIds: string[]): Promise<void> {
	return invoke<void>('assign_discovery_tags', { releaseIds, tagIds })
}

export async function removeTags(releaseIds: string[], tagIds: string[]): Promise<void> {
	return invoke<void>('remove_discovery_tags', { releaseIds, tagIds })
}

export async function checkMatches(
	url?: string | null,
	artist?: string | null,
	title?: string | null,
	parentUrl?: string | null
): Promise<DiscoveryRelease[]> {
	return invoke<DiscoveryRelease[]>('check_discovery_matches', {
		url: url ?? null,
		artist: artist ?? null,
		title: title ?? null,
		parentUrl: parentUrl ?? null,
	})
}

export async function addTracksToRelease(releaseId: string, tracks: DiscoveryTrackCreate[]): Promise<DiscoveryRelease> {
	return invoke<DiscoveryRelease>('add_tracks_to_discovery_release', { releaseId, tracks })
}

export async function mergeReleases(targetId: string, sourceIds: string[]): Promise<DiscoveryRelease> {
	return invoke<DiscoveryRelease>('merge_discovery_releases', { targetId, sourceIds })
}

export async function fetchMetadata(url: string): Promise<FetchedMetadata> {
	return invoke<FetchedMetadata>('fetch_release_metadata', { url })
}

/** Fetch (session-cached) the profile/avatar image URL for an artist/label page. */
export async function fetchSourceAvatar(url: string): Promise<string | null> {
	return invoke<string | null>('fetch_source_avatar', { url })
}

export async function refreshMetadata(id: string): Promise<DiscoveryRelease> {
	return invoke<DiscoveryRelease>('refresh_release_metadata', { id })
}

/**
 * Resolve a track's proxied stream URL. `background: true` marks opportunistic resolution
 * (queue look-ahead, offline pre-caching) that throttles through a small global permit pool
 * in the backend so it never delays a user-initiated (foreground) fetch.
 */
export async function fetchPreviewStream(
	releaseId: string,
	trackPosition: number,
	background = false
): Promise<string> {
	return invoke<string>('fetch_preview_stream', { releaseId, trackPosition, background })
}

export async function invalidatePreviewStreamCache(releaseId: string): Promise<void> {
	return invoke<void>('invalidate_preview_stream_cache', { releaseId })
}

/** Delete a release's downloaded audio bytes + cached stream URLs ("Remove Download"). */
export async function purgeReleaseAudioCache(releaseId: string): Promise<void> {
	return invoke<void>('purge_release_audio_cache', { releaseId })
}

/** Per-release audio-cache state for the "downloaded for offline" indicator. */
export interface ReleaseCacheState {
	cached_tracks: number
	total_tracks: number
	bytes: number
	/** Any track pinned via "Download for Offline" (excluded from LRU eviction). */
	pinned: boolean
}

export async function getReleaseCacheState(releaseId: string): Promise<ReleaseCacheState> {
	return invoke<ReleaseCacheState>('get_release_cache_state', { releaseId })
}

/**
 * Bulk cached-state for list badges / the Downloaded filter: one entry per release with at
 * least one cached track. Refetch on the `discovery-cache-changed` Tauri event.
 */
export interface CachedReleaseState {
	release_id: string
	cached_tracks: number
	total_tracks: number
	fully_cached: boolean
	pinned: boolean
}

export async function getCachedReleaseStates(): Promise<CachedReleaseState[]> {
	return invoke<CachedReleaseState[]>('get_cached_release_states')
}

/** Proactively download + cache one track's audio for offline playback. Idempotent. */
export async function precachePreviewStream(releaseId: string, trackPosition: number): Promise<void> {
	return invoke<void>('precache_preview_stream', { releaseId, trackPosition })
}

export async function getAudioCacheSize(): Promise<number> {
	return invoke<number>('get_discovery_audio_cache_size')
}

export async function clearAudioCache(): Promise<void> {
	return invoke<void>('clear_discovery_audio_cache')
}

/**
 * Download + cache a release's remote cover to disk (idempotent). Returns the relative cache
 * path ("discovery/artwork/{id}.ext") for cache-first offline rendering, or null when the
 * release has no artwork_url or the fetch fails.
 */
export async function cacheReleaseArtwork(releaseId: string): Promise<string | null> {
	return invoke<string | null>('cache_release_artwork', { releaseId })
}

export async function getArtworkCacheSize(): Promise<number> {
	return invoke<number>('get_discovery_artwork_cache_size')
}

export async function clearArtworkCache(): Promise<void> {
	return invoke<void>('clear_discovery_artwork_cache')
}

export async function setDiscoveryReleaseArtwork(id: string, filePath: string): Promise<DiscoveryRelease> {
	return invoke<DiscoveryRelease>('set_discovery_release_artwork', { releaseId: id, filePath })
}

export async function deleteDiscoveryReleaseArtwork(id: string): Promise<DiscoveryRelease> {
	return invoke<DiscoveryRelease>('delete_discovery_release_artwork', { releaseId: id })
}

export async function scanPage(url: string): Promise<ScannedPage> {
	return invoke<ScannedPage>('scan_discovery_page', { url })
}

export async function bulkCreateReleases(
	urls: string[],
	pageLabel?: string | null,
	pageArtist?: string | null,
	scannedReleases?: ScannedRelease[] | null,
	sourceType?: string | null,
	pageUrl?: string | null
): Promise<BulkImportResult> {
	return invoke<BulkImportResult>('bulk_create_discovery_releases', {
		urls,
		scannedReleases: scannedReleases ?? null,
		pageLabel: pageLabel ?? null,
		pageArtist: pageArtist ?? null,
		sourceType: sourceType ?? null,
		pageUrl: pageUrl ?? null,
	})
}

export async function cancelBulkImport(): Promise<void> {
	return invoke<void>('cancel_bulk_import')
}

export async function cancelScanPage(): Promise<void> {
	return invoke<void>('cancel_scan_page')
}

export async function skipEnrichment(id: string): Promise<void> {
	return invoke<void>('skip_enrichment', { id })
}

export async function toggleTrackLiked(trackId: string): Promise<boolean> {
	return invoke<boolean>('toggle_discovery_track_liked', { trackId })
}

export async function purchaseRelease(
	releaseId: string,
	filePaths: string[],
	transferTags: boolean,
	removeAfterImport: boolean
): Promise<ImportResultWithDuplicates> {
	return invoke<ImportResultWithDuplicates>('purchase_discovery_release', {
		releaseId,
		filePaths,
		transferTags,
		removeAfterImport,
	})
}
