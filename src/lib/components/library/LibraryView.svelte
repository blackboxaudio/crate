<script lang="ts">
	import type { Track, TrackColor, SortConfig, Tag, TagFilterMode } from '$lib/types'
	import TrackList from './TrackList.svelte'
	import SearchBar from './SearchBar.svelte'
	import FilterDropdown from './FilterDropdown.svelte'
	import { IconButton } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { translate } from '$lib/i18n'

	type Props = {
		tracks: Track[]
		trackCount: number
		selectedIds: Set<string>
		playingTrackId?: string | null
		sortConfig: SortConfig
		isDragOver?: boolean
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		editorVisible?: boolean
		hasSelection?: boolean
		searchValue?: string
		onSearchChange?: (query: string) => void
		activeFilterTags?: Tag[]
		tagColors?: Map<string, string | null>
		tagFilterMode?: TagFilterMode
		onRemoveTagFilter?: (tagId: string) => void
		onClearAllTagFilters?: () => void
		onToggleTagFilterMode?: () => void
		onSelectionChange?: (ids: Set<string>) => void
		onTrackPlay?: (track: Track) => void
		onSortChange?: (config: SortConfig) => void
		onContextMenu?: (e: MouseEvent, track: Track) => void
		onEmptySpaceContextMenu?: (e: MouseEvent) => void
		onTrackColorChange?: (trackIds: string[], color: TrackColor | null) => void
		onCancelAnalysis?: (trackId: string) => void
		onToggleEditor?: () => void
		scrollOffset?: number
		onScrollChange?: (offset: number) => void
	}

	let {
		tracks,
		trackCount,
		selectedIds,
		playingTrackId = null,
		sortConfig,
		isDragOver = false,
		categoryColors,
		categorySortOrders,
		editorVisible = false,
		hasSelection = false,
		searchValue = '',
		onSearchChange,
		activeFilterTags = [],
		tagColors = new Map(),
		tagFilterMode = 'or',
		onRemoveTagFilter,
		onClearAllTagFilters,
		onToggleTagFilterMode,
		onSelectionChange,
		onTrackPlay,
		onSortChange,
		onContextMenu,
		onEmptySpaceContextMenu,
		onTrackColorChange,
		onCancelAnalysis,
		onToggleEditor,
		scrollOffset = 0,
		onScrollChange,
	}: Props = $props()
</script>

<div class="flex h-full flex-col overflow-hidden bg-surface-0">
	<!-- Header (matches Breadcrumbs styling) -->
	<div class="flex items-center border-b border-stroke px-4 py-4">
		<div class="flex items-center gap-2 rounded px-2 py-1 text-sm font-medium text-text-primary">
			<Icon name="library" class="h-4 w-4 shrink-0" />
			<span>{$translate('nav.library')}</span>
			<Text as="span" color="tertiary" class="ml-2">
				{trackCount}
				{trackCount === 1 ? $translate('library.track') : $translate('library.tracks')}
			</Text>
		</div>
		<div class="flex flex-1 items-center justify-end gap-2">
			{#if onSearchChange}
				<div class="w-64">
					<SearchBar {onSearchChange} initialValue={searchValue} />
				</div>
			{/if}
			<FilterDropdown
				{activeFilterTags}
				{tagColors}
				{tagFilterMode}
				onRemoveTagFilter={(tagId) => onRemoveTagFilter?.(tagId)}
				onClearAll={() => onClearAllTagFilters?.()}
				onToggleTagFilterMode={() => onToggleTagFilterMode?.()}
			/>
			<Tooltip
				text={editorVisible ? $translate('editor.hideEditor') : $translate('editor.showEditor')}
				position="bottom"
				delay={250}
			>
				<IconButton
					icon="panel-right"
					size="sm"
					active={editorVisible && hasSelection}
					disabled={!hasSelection}
					onclick={onToggleEditor}
				/>
			</Tooltip>
		</div>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-hidden">
		<TrackList
			{tracks}
			{selectedIds}
			{playingTrackId}
			{sortConfig}
			{isDragOver}
			{categoryColors}
			{categorySortOrders}
			{scrollOffset}
			{onSelectionChange}
			{onTrackPlay}
			{onSortChange}
			{onContextMenu}
			{onEmptySpaceContextMenu}
			{onTrackColorChange}
			{onCancelAnalysis}
			{onScrollChange}
		/>
	</div>
</div>
