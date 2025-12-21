import type { Track, SortConfig, TrackSortField, SortDirection, TrackColor } from '$lib/types'
import { COLOR_SORT_ORDER } from '$lib/types'

/**
 * Sort tracks by the given configuration
 */
export function sortTracks(tracks: Track[], config: SortConfig): Track[] {
	const { field, direction } = config
	const multiplier = direction === 'asc' ? 1 : -1

	return [...tracks].sort((a, b) => {
		const valueA = getTrackSortValue(a, field)
		const valueB = getTrackSortValue(b, field)

		// Handle nulls - push them to the end
		if (valueA === null && valueB === null) return 0
		if (valueA === null) return 1
		if (valueB === null) return -1

		// Compare values
		if (typeof valueA === 'string' && typeof valueB === 'string') {
			return valueA.localeCompare(valueB) * multiplier
		}

		if (valueA < valueB) return -1 * multiplier
		if (valueA > valueB) return 1 * multiplier
		return 0
	})
}

/**
 * Get the sortable value for a track field
 */
function getTrackSortValue(track: Track, field: TrackSortField): string | number | null {
	switch (field) {
		case 'title':
			return track.title?.toLowerCase() ?? track.file_path.toLowerCase()
		case 'artist':
			return track.artist?.toLowerCase() ?? null
		case 'album':
			return track.album?.toLowerCase() ?? null
		case 'bpm':
			return track.bpm
		case 'key':
			return track.key
		case 'duration_ms':
			return track.duration_ms
		case 'date_added':
			return track.date_added
		case 'rating':
			return track.rating
		case 'color':
			// No color goes to end (use 999), otherwise use ROYGBIV order
			if (!track.color) return 999
			return COLOR_SORT_ORDER[track.color as TrackColor] ?? 999
		default:
			return null
	}
}

/**
 * Toggle sort direction
 */
export function toggleSortDirection(direction: SortDirection): SortDirection {
	return direction === 'asc' ? 'desc' : 'asc'
}

/**
 * Get next sort config when clicking a column header
 */
export function getNextSortConfig(currentConfig: SortConfig, clickedField: TrackSortField): SortConfig {
	if (currentConfig.field === clickedField) {
		// Same field - toggle direction
		return {
			field: clickedField,
			direction: toggleSortDirection(currentConfig.direction),
		}
	}
	// Different field - sort ascending by default
	return {
		field: clickedField,
		direction: 'asc',
	}
}
