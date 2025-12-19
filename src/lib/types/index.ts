// =============================================================================
// Track Types
// =============================================================================

export interface Track {
	id: string
	file_path: string
	file_hash: string | null

	// Metadata
	title: string | null
	artist: string | null
	album: string | null
	year: number | null
	genre: string | null
	label: string | null
	catalog_number: string | null

	// Audio properties
	duration_ms: number
	bpm: number | null
	key: string | null
	bitrate: number | null
	sample_rate: number | null
	format: string

	// Analysis metadata
	analysis_source: string | null
	waveform_data: number[] | null

	// User data
	rating: number
	play_count: number

	// Timestamps
	date_added: string
	date_modified: string
	last_played: string | null

	// External references
	rekordbox_id: string | null

	// Related data
	tags: Tag[]
}

export interface TrackFilter {
	search?: string
	tag_ids?: string[]
	playlist_id?: string
	bpm_min?: number
	bpm_max?: number
	key?: string
}

export interface TrackUpdate {
	title?: string
	artist?: string
	album?: string
	year?: number
	genre?: string
	label?: string
	bpm?: number
	key?: string
	rating?: number
}

export interface ImportResult {
	tracks: Track[]
	failed_count: number
	errors: string[]
}

// =============================================================================
// Tag Types
// =============================================================================

export interface TagCategory {
	id: string
	name: string
	color: string | null
	sort_order: number
	tags: Tag[]
}

export interface Tag {
	id: string
	category_id: string
	name: string
	color: string | null
	sort_order: number
}

// =============================================================================
// Playlist Types
// =============================================================================

export interface Playlist {
	id: string
	name: string
	parent_id: string | null
	is_folder: boolean
	is_smart: boolean
	smart_rules: string | null
	sort_order: number
	date_created: string
	date_modified: string
	track_count: number
}

// =============================================================================
// Playback Types
// =============================================================================

export interface PlaybackState {
	is_playing: boolean
	position_ms: number
	duration_ms: number
	volume: number
	current_track_id: string | null
	current_track_path: string | null
}

// =============================================================================
// Cue Types
// =============================================================================

export type CueType = 'memory' | 'hot' | 'loop'

export interface Cue {
	id: string
	track_id: string
	position_ms: number
	cue_type: CueType
	loop_end_ms: number | null
	hot_cue_index: number | null
	name: string | null
	color: string | null
}

// =============================================================================
// UI Types
// =============================================================================

export type SortDirection = 'asc' | 'desc'

export type TrackSortField = 'title' | 'artist' | 'album' | 'bpm' | 'key' | 'duration_ms' | 'date_added' | 'rating'

export interface SortConfig {
	field: TrackSortField
	direction: SortDirection
}

export interface ColumnConfig {
	id: TrackSortField | 'tags'
	label: string
	width: number
	visible: boolean
	sortable: boolean
}

export interface ContextMenuItem {
	id: string
	label: string
	icon?: string
	shortcut?: string
	disabled?: boolean
	divider?: boolean
	action?: () => void
	submenu?: ContextMenuItem[]
}

// =============================================================================
// Breadcrumb Types
// =============================================================================

export type BreadcrumbType = 'library' | 'folder' | 'playlist'

export interface BreadcrumbItem {
	id: string | null // null for Library root
	name: string
	type: BreadcrumbType
	playlist?: Playlist // Reference to playlist/folder for context menu
	count?: number // Track count or child count (last item only)
	countLabel?: string // "tracks" or "items"
}

// =============================================================================
// Sidebar View Types
// =============================================================================

export type SidebarView = 'library' | 'playlist' | 'tag' | 'folder'

export interface SidebarState {
	view: SidebarView
	selectedPlaylistId: string | null
	selectedFolderId: string | null
	selectedTagId: string | null
}

// =============================================================================
// Device Types
// =============================================================================

export interface UsbDevice {
	id: string
	name: string
	mount_point: string
	total_space_bytes: number
	available_space_bytes: number
	is_removable: boolean
}

// =============================================================================
// Settings Types
// =============================================================================

export type Theme = 'light' | 'dark' | 'system'

export type AccentColor =
	| 'blue'
	| 'indigo'
	| 'violet'
	| 'purple'
	| 'pink'
	| 'rose'
	| 'orange'
	| 'amber'
	| 'emerald'
	| 'teal'

export interface AppSettings {
	theme: Theme
	accentColor: AccentColor
}
