/**
 * Format duration in milliseconds to MM:SS or HH:MM:SS
 */
export function formatDuration(ms: number): string {
	const totalSeconds = Math.floor(ms / 1000)
	const hours = Math.floor(totalSeconds / 3600)
	const minutes = Math.floor((totalSeconds % 3600) / 60)
	const seconds = totalSeconds % 60

	if (hours > 0) {
		return `${hours}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`
	}
	return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

/**
 * Format duration in milliseconds to M:SS (compact format for track lists)
 */
export function formatDurationCompact(ms: number): string {
	const totalSeconds = Math.floor(ms / 1000)
	const minutes = Math.floor(totalSeconds / 60)
	const seconds = totalSeconds % 60
	return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

/**
 * Format BPM to display string
 */
export function formatBpm(bpm: number | null): string {
	if (bpm === null) return '-'
	return bpm.toFixed(1)
}

/**
 * Mapping from Standard notation to Camelot wheel notation
 * Keys are stored in Standard format and converted to Camelot for display when needed
 */
const STANDARD_TO_CAMELOT: Record<string, string> = {
	// Major keys
	C: '8B',
	G: '9B',
	D: '10B',
	A: '11B',
	E: '12B',
	B: '1B',
	'F#': '2B',
	Gb: '2B',
	'C#': '3B',
	Db: '3B',
	Ab: '4B',
	'G#': '4B',
	Eb: '5B',
	'D#': '5B',
	Bb: '6B',
	'A#': '6B',
	F: '7B',
	// Minor keys
	Am: '8A',
	Em: '9A',
	Bm: '10A',
	'F#m': '11A',
	Gbm: '11A',
	'C#m': '12A',
	Dbm: '12A',
	'G#m': '1A',
	Abm: '1A',
	'D#m': '2A',
	Ebm: '2A',
	'A#m': '3A',
	Bbm: '3A',
	Fm: '4A',
	Cm: '5A',
	Gm: '6A',
	Dm: '7A',
}

/**
 * Format key for display based on notation format preference
 * Keys are stored in Standard notation and converted to Camelot when that format is selected
 */
export function formatKey(key: string | null, format: 'standard' | 'camelot' = 'camelot'): string {
	if (!key) return '-'

	if (format === 'camelot') {
		return STANDARD_TO_CAMELOT[key] ?? key
	}

	return key
}

/**
 * Format file size in bytes to human-readable string
 */
export function formatFileSize(bytes: number): string {
	if (bytes === 0) return '0 B'
	const k = 1024
	const sizes = ['B', 'KB', 'MB', 'GB']
	const i = Math.floor(Math.log(bytes) / Math.log(k))
	return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`
}

/**
 * Format bytes to human-readable string (handles null/undefined)
 */
export function formatBytes(bytes: number | null | undefined): string {
	if (bytes === null || bytes === undefined) return '-'
	if (bytes === 0) return '0 B'
	const k = 1024
	const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
	const i = Math.floor(Math.log(bytes) / Math.log(k))
	return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`
}

/**
 * Format date string to localized display based on format preference.
 * Uses UTC getters for date-only strings (YYYY-MM-DD) to avoid timezone day-shift.
 */
export function formatDate(dateStr: string, format: 'locale' | 'iso' | 'us' | 'eu' | 'dot' = 'locale'): string {
	const date = new Date(dateStr)
	const isDateOnly = /^\d{4}-\d{2}-\d{2}$/.test(dateStr)
	const y = isDateOnly ? date.getUTCFullYear() : date.getFullYear()
	const m = isDateOnly ? date.getUTCMonth() : date.getMonth()
	const d = isDateOnly ? date.getUTCDate() : date.getDate()
	const mm = String(m + 1).padStart(2, '0')
	const dd = String(d).padStart(2, '0')

	switch (format) {
		case 'iso':
			return `${y}-${mm}-${dd}`
		case 'us':
			return `${mm}/${dd}/${y}`
		case 'eu':
			return `${dd}/${mm}/${y}`
		case 'dot':
			return `${dd}.${mm}.${y}`
		case 'locale':
		default:
			return isDateOnly ? new Date(Date.UTC(y, m, d)).toLocaleDateString() : date.toLocaleDateString()
	}
}

/**
 * Format date string to relative time (e.g., "2 days ago")
 */
export function formatRelativeDate(dateStr: string): string {
	const date = new Date(dateStr)
	const now = new Date()
	const diffMs = now.getTime() - date.getTime()
	const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))

	if (diffDays === 0) return 'Today'
	if (diffDays === 1) return 'Yesterday'
	if (diffDays < 7) return `${diffDays} days ago`
	if (diffDays < 30) return `${Math.floor(diffDays / 7)} weeks ago`
	if (diffDays < 365) return `${Math.floor(diffDays / 30)} months ago`
	return `${Math.floor(diffDays / 365)} years ago`
}

/**
 * Format bitrate for display
 */
export function formatBitrate(bitrate: number | null): string {
	if (bitrate === null) return '-'
	return `${bitrate} kbps`
}

/**
 * Get display name for a track (title or filename)
 */
export function getTrackDisplayName(track: { title: string | null; file_path: string }): string {
	if (track.title) return track.title
	// Extract filename from path
	const parts = track.file_path.split(/[/\\]/)
	const filename = parts[parts.length - 1]
	// Remove extension
	return filename.replace(/\.[^.]+$/, '')
}

/**
 * Get display artist (or "Unknown Artist")
 */
export function getTrackDisplayArtist(track: { artist: string | null }): string {
	return track.artist || 'Unknown Artist'
}
