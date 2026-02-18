import { invoke } from '@tauri-apps/api/core'
import type {
	DiscoveryRelease,
	DiscoveryReleaseCreate,
	DiscoveryReleaseUpdate,
	DiscoveryFilter,
	FetchedMetadata,
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

export async function fetchMetadata(url: string): Promise<FetchedMetadata> {
	return invoke<FetchedMetadata>('fetch_release_metadata', { url })
}

export async function refreshMetadata(id: string): Promise<DiscoveryRelease> {
	return invoke<DiscoveryRelease>('refresh_release_metadata', { id })
}
