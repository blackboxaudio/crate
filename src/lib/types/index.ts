// =============================================================================
// Track Color Types
// =============================================================================

export type TrackColor = 'pink' | 'red' | 'orange' | 'yellow' | 'green' | 'aqua' | 'blue' | 'purple'

export const TRACK_COLORS: { id: TrackColor; label: string; hex: string }[] = [
	{ id: 'pink', label: 'Pink', hex: '#FF6B9D' },
	{ id: 'red', label: 'Red', hex: '#FF5252' },
	{ id: 'orange', label: 'Orange', hex: '#FF9500' },
	{ id: 'yellow', label: 'Yellow', hex: '#FFCC00' },
	{ id: 'green', label: 'Green', hex: '#50C878' },
	{ id: 'aqua', label: 'Aqua', hex: '#00CED1' },
	{ id: 'blue', label: 'Blue', hex: '#1E90FF' },
	{ id: 'purple', label: 'Purple', hex: '#9370DB' },
]

// ROYGBIV sort order (no-color at end with 999)
export const COLOR_SORT_ORDER: Record<TrackColor, number> = {
	red: 0,
	orange: 1,
	yellow: 2,
	green: 3,
	aqua: 4,
	blue: 5,
	purple: 6,
	pink: 7,
}

// =============================================================================
// Tag Category Color Types
// =============================================================================

export type TagCategoryColor =
	| 'red'
	| 'orange'
	| 'amber'
	| 'green'
	| 'teal'
	| 'blue'
	| 'indigo'
	| 'violet'
	| 'pink'
	| 'rose'

export const TAG_CATEGORY_COLORS: { id: TagCategoryColor; label: string; hex: string }[] = [
	{ id: 'red', label: 'Red', hex: '#ef4444' },
	{ id: 'orange', label: 'Orange', hex: '#f97316' },
	{ id: 'amber', label: 'Amber', hex: '#f59e0b' },
	{ id: 'green', label: 'Green', hex: '#22c55e' },
	{ id: 'teal', label: 'Teal', hex: '#14b8a6' },
	{ id: 'blue', label: 'Blue', hex: '#3b82f6' },
	{ id: 'indigo', label: 'Indigo', hex: '#6366f1' },
	{ id: 'violet', label: 'Violet', hex: '#8b5cf6' },
	{ id: 'pink', label: 'Pink', hex: '#ec4899' },
	{ id: 'rose', label: 'Rose', hex: '#f43f5e' },
]

// =============================================================================
// Artwork Types
// =============================================================================

export type ArtworkSource = 'extracted' | 'user_provided'

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

	// Album artwork
	artwork_path: string | null
	artwork_source: ArtworkSource | null

	// Track color (Rekordbox-compatible)
	color: TrackColor | null

	// Related data
	tags: Tag[]
}

export interface TrackFilter {
	search?: string
	tag_ids?: string[]
	tag_filter_mode?: TagFilterMode
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
// Duplicate Track Detection Types
// =============================================================================

export interface DuplicateTrack {
	new_file_path: string
	new_file_hash: string
	existing_track: Track
}

export interface ImportResultWithDuplicates {
	tracks: Track[]
	failed_count: number
	errors: string[]
	duplicates: DuplicateTrack[]
}

export type DuplicateResolutionAction = 'skip' | 'update_path' | 'replace'

export type DuplicateResolution =
	| { action: 'skip' }
	| { action: 'update_path'; new_path: string }
	| { action: 'replace'; new_path: string; new_hash: string }

export interface FileMatchResult {
	matches: boolean
	original_hash: string | null
	new_hash: string
	format_valid: boolean
}

// =============================================================================
// Bulk Edit Types
// =============================================================================

export interface BulkEditValue<T> {
	value: T | null // The value if all tracks have the same value
	mixed: boolean // True if tracks have different values
	count: number // Number of tracks with non-null values
}

export interface BulkTrackInfo {
	title: BulkEditValue<string>
	artist: BulkEditValue<string>
	album: BulkEditValue<string>
	year: BulkEditValue<number>
	genre: BulkEditValue<string>
	label: BulkEditValue<string>
	bpm: BulkEditValue<number>
	key: BulkEditValue<string>
	rating: BulkEditValue<number>
	artworkPath: BulkEditValue<string>
	artworkSource: BulkEditValue<ArtworkSource>
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

export type TagSelectionState = 'active' | 'inactive' | 'mixed'

export type TagFilterMode = 'and' | 'or'

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

export type MoveConflictResolution = 'overwrite' | 'merge'

export interface MoveConflict {
	movingItem: Playlist
	existingItem: Playlist
}

export interface MovePlaylistResult {
	playlist: Playlist
	nestedConflicts: MoveConflict[]
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

export type TrackSortField =
	| 'title'
	| 'artist'
	| 'album'
	| 'bpm'
	| 'key'
	| 'duration_ms'
	| 'date_added'
	| 'rating'
	| 'color'

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
	colorDot?: string
	selected?: boolean
	variant?: 'default' | 'danger'
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
	file_system: string
	disk_kind: string
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

export type Font = 'ibm-plex-mono' | 'jetbrains-mono' | 'fira-code' | 'inter' | 'open-sans'

export interface AppSettings {
	theme: Theme
	accentColor: AccentColor
	font: Font
	audioDevice: string | null
}

export interface AudioDevice {
	name: string
	isDefault: boolean
	isBuiltIn: boolean
}
