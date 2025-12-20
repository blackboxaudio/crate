import type { ArtworkSource, BulkEditValue, BulkTrackInfo, Track } from '$lib/types'

/**
 * Placeholder text for mixed values in bulk edit mode
 */
export const MIXED_PLACEHOLDER = '---'

/**
 * Compute a BulkEditValue for a field from an array of tracks
 */
function computeBulkValue<T>(tracks: Track[], getter: (track: Track) => T | null): BulkEditValue<T> {
	const values = tracks.map(getter)
	const nonNullValues = values.filter((v): v is T => v !== null)

	if (nonNullValues.length === 0) {
		return { value: null, mixed: false, count: 0 }
	}

	// Check if all non-null values are the same
	const firstValue = nonNullValues[0]
	const allSame = nonNullValues.every((v) => v === firstValue)

	if (allSame && nonNullValues.length === tracks.length) {
		return { value: firstValue, mixed: false, count: nonNullValues.length }
	}

	return {
		value: allSame ? firstValue : null,
		mixed: !allSame || nonNullValues.length !== tracks.length,
		count: nonNullValues.length,
	}
}

/**
 * Compute bulk edit info from selected tracks
 */
export function computeBulkTrackInfo(tracks: Track[]): BulkTrackInfo {
	return {
		title: computeBulkValue(tracks, (t) => t.title),
		artist: computeBulkValue(tracks, (t) => t.artist),
		album: computeBulkValue(tracks, (t) => t.album),
		year: computeBulkValue(tracks, (t) => t.year),
		genre: computeBulkValue(tracks, (t) => t.genre),
		label: computeBulkValue(tracks, (t) => t.label),
		bpm: computeBulkValue(tracks, (t) => t.bpm),
		key: computeBulkValue(tracks, (t) => t.key),
		rating: computeBulkValue(tracks, (t) => t.rating),
		artworkPath: computeBulkValue(tracks, (t) => t.artwork_path),
		artworkSource: computeBulkValue(tracks, (t) => t.artwork_source as ArtworkSource | null),
	}
}
