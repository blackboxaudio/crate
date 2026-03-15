import type {
	SmartCondition,
	SmartRules,
	TextOperator,
	NumericOperator,
	DateOperator,
	EnumOperator,
	TagOperator,
	TagCategory,
} from '../types'

// =============================================================================
// Field Definitions
// =============================================================================

export type FieldType = 'text' | 'numeric' | 'date' | 'enum' | 'tags'

export interface FieldDefinition {
	field: string
	labelKey: string
	type: FieldType
	enumValues?: { value: string; labelKey: string }[]
}

const LIBRARY_FIELDS: FieldDefinition[] = [
	{ field: 'title', labelKey: 'smartPlaylist.fields.title', type: 'text' },
	{ field: 'artist', labelKey: 'smartPlaylist.fields.artist', type: 'text' },
	{ field: 'album', labelKey: 'smartPlaylist.fields.album', type: 'text' },
	{ field: 'genre', labelKey: 'smartPlaylist.fields.genre', type: 'text' },
	{ field: 'label', labelKey: 'smartPlaylist.fields.label', type: 'text' },
	{ field: 'catalog_number', labelKey: 'smartPlaylist.fields.catalogNumber', type: 'text' },
	{ field: 'key', labelKey: 'smartPlaylist.fields.key', type: 'text' },
	{ field: 'bpm', labelKey: 'smartPlaylist.fields.bpm', type: 'numeric' },
	{ field: 'rating', labelKey: 'smartPlaylist.fields.rating', type: 'numeric' },
	{ field: 'play_count', labelKey: 'smartPlaylist.fields.playCount', type: 'numeric' },
	{ field: 'year', labelKey: 'smartPlaylist.fields.year', type: 'numeric' },
	{ field: 'duration_ms', labelKey: 'smartPlaylist.fields.durationMs', type: 'numeric' },
	{ field: 'bitrate', labelKey: 'smartPlaylist.fields.bitrate', type: 'numeric' },
	{ field: 'sample_rate', labelKey: 'smartPlaylist.fields.sampleRate', type: 'numeric' },
	{ field: 'date_added', labelKey: 'smartPlaylist.fields.dateAdded', type: 'date' },
	{ field: 'last_played', labelKey: 'smartPlaylist.fields.lastPlayed', type: 'date' },
	{ field: 'date_modified', labelKey: 'smartPlaylist.fields.dateModified', type: 'date' },
	{
		field: 'color',
		labelKey: 'smartPlaylist.fields.color',
		type: 'enum',
		enumValues: [
			{ value: 'pink', labelKey: 'smartPlaylist.colors.pink' },
			{ value: 'red', labelKey: 'smartPlaylist.colors.red' },
			{ value: 'orange', labelKey: 'smartPlaylist.colors.orange' },
			{ value: 'yellow', labelKey: 'smartPlaylist.colors.yellow' },
			{ value: 'green', labelKey: 'smartPlaylist.colors.green' },
			{ value: 'aqua', labelKey: 'smartPlaylist.colors.aqua' },
			{ value: 'blue', labelKey: 'smartPlaylist.colors.blue' },
			{ value: 'purple', labelKey: 'smartPlaylist.colors.purple' },
		],
	},
	{
		field: 'format',
		labelKey: 'smartPlaylist.fields.format',
		type: 'enum',
		enumValues: [
			{ value: 'mp3', labelKey: 'MP3' },
			{ value: 'flac', labelKey: 'FLAC' },
			{ value: 'wav', labelKey: 'WAV' },
			{ value: 'aiff', labelKey: 'AIFF' },
			{ value: 'aac', labelKey: 'AAC' },
			{ value: 'ogg', labelKey: 'OGG' },
		],
	},
	{ field: 'tags', labelKey: 'smartPlaylist.fields.tags', type: 'tags' },
]

const DISCOVERY_FIELDS: FieldDefinition[] = [
	{ field: 'title', labelKey: 'smartPlaylist.fields.title', type: 'text' },
	{ field: 'artist', labelKey: 'smartPlaylist.fields.artist', type: 'text' },
	{ field: 'label', labelKey: 'smartPlaylist.fields.label', type: 'text' },
	{ field: 'notes', labelKey: 'smartPlaylist.fields.notes', type: 'text' },
	{
		field: 'source_type',
		labelKey: 'smartPlaylist.fields.sourceType',
		type: 'enum',
		enumValues: [
			{ value: 'bandcamp', labelKey: 'Bandcamp' },
			{ value: 'soundcloud', labelKey: 'SoundCloud' },
			{ value: 'youtube', labelKey: 'YouTube' },
			{ value: 'discogs', labelKey: 'Discogs' },
			{ value: 'other', labelKey: 'Other' },
		],
	},
	{ field: 'release_date', labelKey: 'smartPlaylist.fields.releaseDate', type: 'date' },
	{ field: 'date_added', labelKey: 'smartPlaylist.fields.dateAdded', type: 'date' },
	{ field: 'date_modified', labelKey: 'smartPlaylist.fields.dateModified', type: 'date' },
	{ field: 'tags', labelKey: 'smartPlaylist.fields.tags', type: 'tags' },
]

export function getFieldsForContext(context: string): FieldDefinition[] {
	return context === 'discovery' ? DISCOVERY_FIELDS : LIBRARY_FIELDS
}

export function getFieldDefinition(field: string, context: string): FieldDefinition | undefined {
	return getFieldsForContext(context).find((f) => f.field === field)
}

// =============================================================================
// Operator Definitions
// =============================================================================

export interface OperatorDefinition {
	value: string
	labelKey: string
}

const TEXT_OPERATORS: OperatorDefinition[] = [
	{ value: 'contains', labelKey: 'smartPlaylist.operators.contains' },
	{ value: 'not_contains', labelKey: 'smartPlaylist.operators.notContains' },
	{ value: 'equals', labelKey: 'smartPlaylist.operators.equals' },
	{ value: 'not_equals', labelKey: 'smartPlaylist.operators.notEquals' },
	{ value: 'starts_with', labelKey: 'smartPlaylist.operators.startsWith' },
	{ value: 'ends_with', labelKey: 'smartPlaylist.operators.endsWith' },
	{ value: 'is_empty', labelKey: 'smartPlaylist.operators.isEmpty' },
	{ value: 'is_not_empty', labelKey: 'smartPlaylist.operators.isNotEmpty' },
]

const NUMERIC_OPERATORS: OperatorDefinition[] = [
	{ value: 'equals', labelKey: 'smartPlaylist.operators.equals' },
	{ value: 'not_equals', labelKey: 'smartPlaylist.operators.notEquals' },
	{ value: 'greater_than', labelKey: 'smartPlaylist.operators.greaterThan' },
	{ value: 'less_than', labelKey: 'smartPlaylist.operators.lessThan' },
	{ value: 'in_range', labelKey: 'smartPlaylist.operators.inRange' },
]

const DATE_OPERATORS: OperatorDefinition[] = [
	{ value: 'in_last_days', labelKey: 'smartPlaylist.operators.inLastDays' },
	{ value: 'not_in_last_days', labelKey: 'smartPlaylist.operators.notInLastDays' },
	{ value: 'before', labelKey: 'smartPlaylist.operators.before' },
	{ value: 'after', labelKey: 'smartPlaylist.operators.after' },
	{ value: 'is_empty', labelKey: 'smartPlaylist.operators.isEmpty' },
	{ value: 'is_not_empty', labelKey: 'smartPlaylist.operators.isNotEmpty' },
]

const ENUM_OPERATORS: OperatorDefinition[] = [
	{ value: 'equals', labelKey: 'smartPlaylist.operators.equals' },
	{ value: 'not_equals', labelKey: 'smartPlaylist.operators.notEquals' },
	{ value: 'is_empty', labelKey: 'smartPlaylist.operators.isEmpty' },
	{ value: 'is_not_empty', labelKey: 'smartPlaylist.operators.isNotEmpty' },
]

const TAG_OPERATORS: OperatorDefinition[] = [
	{ value: 'has_any', labelKey: 'smartPlaylist.operators.hasAny' },
	{ value: 'has_all', labelKey: 'smartPlaylist.operators.hasAll' },
	{ value: 'has_none', labelKey: 'smartPlaylist.operators.hasNone' },
]

export function getOperatorsForType(type: FieldType): OperatorDefinition[] {
	switch (type) {
		case 'text':
			return TEXT_OPERATORS
		case 'numeric':
			return NUMERIC_OPERATORS
		case 'date':
			return DATE_OPERATORS
		case 'enum':
			return ENUM_OPERATORS
		case 'tags':
			return TAG_OPERATORS
	}
}

export function operatorRequiresValue(operator: string): boolean {
	return !['is_empty', 'is_not_empty'].includes(operator)
}

export function operatorRequiresSecondValue(operator: string): boolean {
	return operator === 'in_range'
}

// =============================================================================
// Sort Field Definitions
// =============================================================================

export interface SortFieldDefinition {
	value: string
	labelKey: string
}

const LIBRARY_SORT_FIELDS: SortFieldDefinition[] = [
	{ value: 'date_added', labelKey: 'smartPlaylist.sortFields.dateAdded' },
	{ value: 'rating', labelKey: 'smartPlaylist.sortFields.rating' },
	{ value: 'play_count', labelKey: 'smartPlaylist.sortFields.playCount' },
	{ value: 'bpm', labelKey: 'smartPlaylist.sortFields.bpm' },
	{ value: 'title', labelKey: 'smartPlaylist.sortFields.title' },
	{ value: 'artist', labelKey: 'smartPlaylist.sortFields.artist' },
	{ value: 'random', labelKey: 'smartPlaylist.sortFields.random' },
]

const DISCOVERY_SORT_FIELDS: SortFieldDefinition[] = [
	{ value: 'date_added', labelKey: 'smartPlaylist.sortFields.dateAdded' },
	{ value: 'title', labelKey: 'smartPlaylist.sortFields.title' },
	{ value: 'artist', labelKey: 'smartPlaylist.sortFields.artist' },
	{ value: 'release_date', labelKey: 'smartPlaylist.sortFields.releaseDate' },
	{ value: 'random', labelKey: 'smartPlaylist.sortFields.random' },
]

export function getSortFieldsForContext(context: string): SortFieldDefinition[] {
	return context === 'discovery' ? DISCOVERY_SORT_FIELDS : LIBRARY_SORT_FIELDS
}

// =============================================================================
// Condition Factory
// =============================================================================

export function createDefaultCondition(fieldDef: FieldDefinition): SmartCondition {
	switch (fieldDef.type) {
		case 'text':
			return { type: 'text', field: fieldDef.field, operator: 'contains' as TextOperator, value: '' }
		case 'numeric':
			return { type: 'numeric', field: fieldDef.field, operator: 'equals' as NumericOperator, value: 0 }
		case 'date':
			return { type: 'date', field: fieldDef.field, operator: 'in_last_days' as DateOperator, value: '30' }
		case 'enum':
			return {
				type: 'enum',
				field: fieldDef.field,
				operator: 'equals' as EnumOperator,
				value: fieldDef.enumValues?.[0]?.value ?? '',
			}
		case 'tags':
			return { type: 'tags', operator: 'has_any' as TagOperator, tag_ids: [] }
	}
}

// =============================================================================
// Condition Completeness
// =============================================================================

export function conditionHasValue(condition: SmartCondition): boolean {
	if (!operatorRequiresValue(condition.operator)) return true
	switch (condition.type) {
		case 'text':
		case 'date':
			return condition.value != null && condition.value !== ''
		case 'numeric':
			if (condition.operator === 'in_range') {
				return condition.value != null && condition.value2 != null
			}
			return condition.value != null
		case 'enum':
			return condition.value != null && condition.value !== ''
		case 'tags':
			return condition.tag_ids.length > 0
	}
}

// =============================================================================
// Parse / Serialize
// =============================================================================

export function parseSmartRules(json: string | null): SmartRules | null {
	if (!json) return null
	try {
		return JSON.parse(json) as SmartRules
	} catch {
		return null
	}
}

export function serializeSmartRules(rules: SmartRules): string {
	return JSON.stringify(rules)
}

// =============================================================================
// Tag Helpers
// =============================================================================

export function findDeletedTagIds(tagIds: string[], categories: TagCategory[]): string[] {
	const allTagIds = new Set(categories.flatMap((c) => c.tags.map((t) => t.id)))
	return tagIds.filter((id) => !allTagIds.has(id))
}
