import type { DiscoveryRelease, Track } from '../../types'

/**
 * Pick model + playability predicates for the playback queue.
 *
 * A session's queue is a flat list of picks of ONE kind — entirely preview or entirely library
 * (desktop stops preview audio when a library track starts and vice versa, so a mixed session
 * cannot exist). The pure resolvers here (key / group / playability) are what let the queue
 * machinery in `index.ts` run a single code path over both kinds.
 */

/**
 * One playable discovery-preview unit. `release` is held by reference (a live object from the
 * context list / a queued item), so resolution is synchronous; only ids are persisted. The field
 * names deliberately keep the pre-union flat shape (`{release, trackIndex}`) — the iOS native-window
 * plumbing in player.ts consumes them structurally.
 */
export type PreviewPick = { kind: 'preview'; release: DiscoveryRelease; trackIndex: number }

/** One playable library unit. Its "release" for repeat purposes is the track's album (see
 *  `libraryGroupKey`) — the library model has no release entity, only free-text album strings. */
export type LibraryPick = { kind: 'library'; track: Track }

export type Pick = PreviewPick | LibraryPick

/** Stable identity key for a pick. The `p:`/`l:` prefixes keep the namespaces provably disjoint. */
export function pickKey(p: Pick): string {
	return p.kind === 'preview' ? `p:${p.release.id}:${p.trackIndex}` : `l:${p.track.id}`
}

/**
 * Library "release" = the album, scoped to the visible list. There is no album entity in the library
 * model, only free-text strings, and no album-artist column — so pairing artist+album would shatter
 * every compilation (label comps, chart packs — the most common multi-artist shape in a DJ library)
 * into one group per track, exactly where repeat-release matters most. Album title alone can fuse two
 * same-titled albums that are both on screen; rarer, and its failure mode is milder (a slightly larger
 * loop). A track with NO album is its OWN group, so repeat-release on an untagged file degenerates to
 * a track loop. One-line change here if an `album_artist` column ever lands.
 */
export function libraryGroupKey(t: Track): string {
	const album = t.album?.trim()
	return album ? `a:${album.toLowerCase()}` : `t:${t.id}`
}

/** The repeat-release grouping key: the release for previews, the album for library tracks. */
export function pickGroupKey(p: Pick): string {
	return p.kind === 'preview' ? `r:${p.release.id}` : libraryGroupKey(p.track)
}

/**
 * THE preview-playability predicate, shared by every surface (queue advance, shuffle pools, Up Next,
 * desktop rows/double-click, enqueue filters). A track the source serves no preview for (a
 * pre-order's unreleased tracks) is invisible to the queue: never advanced into, never shuffled,
 * never enqueued — it would only fail to resolve.
 */
export function isPreviewPlayable(release: DiscoveryRelease, trackIndex: number): boolean {
	const track = release.tracks[trackIndex]
	// No duration means the source never exposed the track as playable (unreleased pre-order tracks,
	// unenriched rows); Discogs plays via YouTube video only.
	if (!track?.duration_ms) return false
	if (track.preview_unavailable) return false
	if (release.source_type === 'discogs') return track.video_id != null
	// 'other'-sourced releases only play via a scraped video; with none, resolution always fails —
	// the conservative union of the two predicates that existed before unification (desktop skipped
	// these, the shared queue advanced into them and toasted an error).
	if (release.source_type === 'other') return release.tracks.some((t) => t.video_id != null)
	return true
}

/** First playable track index at or after `from`, or -1 when the release has none left. */
export function firstPlayablePreviewIndex(release: DiscoveryRelease, from = 0): number {
	for (let i = from; i < release.tracks.length; i++) if (isPreviewPlayable(release, i)) return i
	return -1
}

// Inverted dependency (the shared/desktop boundary rule): desktop registers its missing-file check
// here at init so the queue never advances into a track whose file is gone, without this module
// importing the desktop-only missingTracks store. Unset (mobile, tests) means "everything present".
let libraryPlayableFilter: ((t: Track) => boolean) | null = null

/** Register (or clear) the desktop-only library playability veto — e.g. the missing-file set. */
export function setLibraryPlayableFilter(fn: ((t: Track) => boolean) | null) {
	libraryPlayableFilter = fn
}

export function isLibraryPlayable(t: Track): boolean {
	// A 0-duration track would never end: the position tracker's end test requires duration_ms > 0,
	// so advancing into one stalls playback instead of finishing.
	if (!t.duration_ms) return false
	return libraryPlayableFilter?.(t) ?? true
}

export function isPlayablePick(p: Pick): boolean {
	return p.kind === 'preview' ? isPreviewPlayable(p.release, p.trackIndex) : isLibraryPlayable(p.track)
}
