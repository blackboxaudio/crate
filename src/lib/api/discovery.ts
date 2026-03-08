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
	BulkImportResult,
} from '$lib/types'

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
	artist?: string | null,
	title?: string | null,
	parentUrl?: string | null
): Promise<DiscoveryRelease[]> {
	return invoke<DiscoveryRelease[]>('check_discovery_matches', {
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

export async function refreshMetadata(id: string): Promise<DiscoveryRelease> {
	return invoke<DiscoveryRelease>('refresh_release_metadata', { id })
}

export async function fetchPreviewStream(releaseId: string, trackPosition: number): Promise<string> {
	return invoke<string>('fetch_preview_stream', { releaseId, trackPosition })
}

export async function invalidatePreviewStreamCache(releaseId: string): Promise<void> {
	return invoke<void>('invalidate_preview_stream_cache', { releaseId })
}

export async function getAudioCacheSize(): Promise<number> {
	return invoke<number>('get_discovery_audio_cache_size')
}

export async function clearAudioCache(): Promise<void> {
	return invoke<void>('clear_discovery_audio_cache')
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
	pageArtist?: string | null
): Promise<BulkImportResult> {
	return invoke<BulkImportResult>('bulk_create_discovery_releases', {
		urls,
		pageLabel: pageLabel ?? null,
		pageArtist: pageArtist ?? null,
	})
}

export async function cancelBulkImport(): Promise<void> {
	return invoke<void>('cancel_bulk_import')
}

export async function cancelScanPage(): Promise<void> {
	return invoke<void>('cancel_scan_page')
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
