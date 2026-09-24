import { get } from 'svelte/store'
import type { DiscoveryRelease, DiscoveryTrack } from '$shared/types'
import * as playbackQueue from '$shared/stores/playbackQueue'
import { discoveryStore } from '$shared/stores/discovery'
import { toastStore } from '$shared/stores/toast'
import { translate } from '$shared/i18n'
import { queuePanelVisible } from '$lib/stores/uiLayout'

/**
 * Desktop entry points for the explicit user queue (the "Play Next" / "Add to Queue" context-menu
 * items). They resolve what the menus hold — releases, or tracks that may span several releases —
 * into the shared queue's release + index calls, and confirm with a toast only while the Up Next
 * panel is closed: with the panel open, the row appearing IS the feedback.
 */

type Placement = 'next' | 'last'

function confirm(placement: Placement) {
	if (get(queuePanelVisible)) return
	toastStore.success(get(translate)(placement === 'next' ? 'queue.playingNext' : 'queue.addedToQueue'))
}

// A track row only knows its release id; the loaded discovery set holds the release (and thus the
// track's index, which the queue keys on). Tracks whose release isn't loaded are skipped.
function resolveTracks(tracks: DiscoveryTrack[]): Array<{ release: DiscoveryRelease; trackIndex: number }> {
	const byId = new Map(get(discoveryStore).releases.map((r) => [r.id, r]))
	const picks: Array<{ release: DiscoveryRelease; trackIndex: number }> = []
	for (const track of tracks) {
		const release = byId.get(track.release_id)
		if (!release) continue
		const trackIndex = release.tracks.findIndex((t) => t.id === track.id)
		if (trackIndex === -1 || !playbackQueue.isPreviewPlayable(release, trackIndex)) continue
		picks.push({ release, trackIndex })
	}
	return picks
}

/** Queue the given tracks (in the order given) to play right after the current one. */
export function playTracksNext(tracks: DiscoveryTrack[]) {
	const picks = resolveTracks(tracks)
	if (picks.length === 0) return
	// Each call front-inserts, so walk backwards to keep the selection's own order.
	for (let i = picks.length - 1; i >= 0; i--) playbackQueue.playNext(picks[i].release, picks[i].trackIndex)
	confirm('next')
}

/** Append the given tracks (in the order given) to the end of the user queue. */
export function addTracksToQueue(tracks: DiscoveryTrack[]) {
	const picks = resolveTracks(tracks)
	if (picks.length === 0) return
	for (const { release, trackIndex } of picks) playbackQueue.addToQueue(release, trackIndex)
	confirm('last')
}

/** Queue every playable track of the given releases, in order, to play right after the current one. */
export function playReleasesNext(releases: DiscoveryRelease[]) {
	const playable = releases.filter((r) => playbackQueue.firstPlayablePreviewIndex(r) !== -1)
	if (playable.length === 0) return
	for (let i = playable.length - 1; i >= 0; i--) playbackQueue.playReleaseNext(playable[i])
	confirm('next')
}

/** Append every playable track of the given releases, in order, to the end of the user queue. */
export function addReleasesToQueue(releases: DiscoveryRelease[]) {
	const playable = releases.filter((r) => playbackQueue.firstPlayablePreviewIndex(r) !== -1)
	if (playable.length === 0) return
	for (const release of playable) playbackQueue.addReleaseToQueue(release)
	confirm('last')
}
