import type {
	DiscoveryRelease,
	DiscoverySortConfig,
	SortConfig,
	SortDirection,
	Track,
	TrackColor,
	TrackSortField,
} from '../types'
import { COLOR_SORT_ORDER } from '../types'

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
 * Sort discovery releases by the given configuration. ONE comparator shared by the discovery
 * feed's derived stores and the mobile per-view (playlist/tag/follow detail) sort controls, so
 * every surface orders identically: invalid/missing release dates always sink to the end
 * regardless of direction, and ties break by id so paginated re-renders stay stable.
 */
export function sortDiscoveryReleases(releases: DiscoveryRelease[], sort: DiscoverySortConfig): DiscoveryRelease[] {
	const { field, direction } = sort
	const dir = direction === 'asc' ? 1 : -1

	return [...releases].sort((a, b) => {
		let cmp = 0
		if (field === 'release_date') {
			const aDate = a.release_date ? new Date(a.release_date).getTime() : NaN
			const bDate = b.release_date ? new Date(b.release_date).getTime() : NaN
			const aValid = !isNaN(aDate)
			const bValid = !isNaN(bDate)
			if (!aValid && !bValid) cmp = 0
			else if (!aValid) return 1
			else if (!bValid) return -1
			else if (aDate < bDate) cmp = -1 * dir
			else if (aDate > bDate) cmp = 1 * dir
		} else if (field === 'track_count') {
			cmp = (a.tracks.length - b.tracks.length) * dir
		} else {
			const aVal = a[field] ?? ''
			const bVal = b[field] ?? ''
			if (aVal < bVal) cmp = -1 * dir
			else if (aVal > bVal) cmp = 1 * dir
		}
		if (cmp !== 0) return cmp
		// Grouping sorts (artist / label / platform / track count) tie constantly — order within a
		// group alphabetically by title so it reads intentionally instead of by opaque id.
		if (field !== 'title') {
			const aTitle = a.title ?? ''
			const bTitle = b.title ?? ''
			if (aTitle < bTitle) return -1
			if (aTitle > bTitle) return 1
		}
		return a.id < b.id ? -1 : a.id > b.id ? 1 : 0
	})
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
