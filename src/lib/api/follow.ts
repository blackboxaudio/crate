import { invoke } from '@tauri-apps/api/core'
import type { FollowedReleasesFound, FollowedSource, FollowedSourceCreate, SourceCheckResult } from '$lib/types'

/** Follow a pasted artist/label page URL (backend scans to detect type + baseline). */
export async function followSource(url: string): Promise<FollowedSource> {
	return invoke<FollowedSource>('follow_source', { url })
}

/** Follow a known entity (from a release's popover); baseline scanned in background. */
export async function followFromEntity(create: FollowedSourceCreate): Promise<FollowedSource> {
	return invoke<FollowedSource>('follow_from_entity', { create })
}

export async function unfollowSource(id: string): Promise<void> {
	return invoke<void>('unfollow_source', { id })
}

/** Re-link a followed source to existing releases (backfills source_page_url). Returns count linked. */
export async function relinkSource(id: string): Promise<number> {
	return invoke<number>('relink_followed_source', { id })
}

export async function setFollowEnabled(id: string, enabled: boolean): Promise<FollowedSource> {
	return invoke<FollowedSource>('set_follow_enabled', { id, enabled })
}

/** Correct a follow's artist-vs-label classification after following. */
export async function setFollowType(id: string, followType: 'artist' | 'label'): Promise<FollowedSource> {
	return invoke<FollowedSource>('set_follow_type', { id, followType })
}

export async function getFollowedSources(): Promise<FollowedSource[]> {
	return invoke<FollowedSource[]>('get_followed_sources')
}

export async function checkFollowedSource(id: string): Promise<SourceCheckResult> {
	return invoke<SourceCheckResult>('check_followed_source', { id })
}

export async function checkAllFollowedSources(): Promise<FollowedReleasesFound> {
	return invoke<FollowedReleasesFound>('check_all_followed_sources')
}

export async function setReleaseNewFlag(releaseId: string, isNew: boolean): Promise<void> {
	return invoke<void>('set_release_new_flag', { releaseId, isNew })
}
