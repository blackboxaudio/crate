<script lang="ts">
	import type { DiscoveryRelease, DiscoverySortConfig, Tag, TagCategory, TagFilterMode } from '$lib/types'
	import DiscoveryList from './DiscoveryList.svelte'
	import { SearchBar, FilterDropdown } from '$lib/components/library'
	import { IconButton } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { translate } from '$lib/i18n'
	import { expandedReleaseIds, newOnly, discoveryStore } from '$lib/stores'
	import { FollowingButton } from '$lib/components/follow'

	type Props = {
		releases: DiscoveryRelease[]
		releaseCount: number
		selectedIds: Set<string>
		sortConfig: DiscoverySortConfig
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		editorVisible?: boolean
		hasSelection?: boolean
		searchValue?: string
		onSearchChange?: (query: string) => void
		activeFilterTags?: Tag[]
		tagCategories?: TagCategory[]
		tagColors?: Map<string, string | null>
		tagFilterMode?: TagFilterMode
		onToggleTagFilter?: (tagId: string) => void
		onClearAllTagFilters?: () => void
		onToggleTagFilterMode?: () => void
		onSelectionChange?: (ids: Set<string>) => void
		onReleaseOpen?: (release: DiscoveryRelease) => void
		onReleaseOpenUrl?: (release: DiscoveryRelease) => void
		onReleaseImport?: (release: DiscoveryRelease) => void
		onTrackPlay?: (release: DiscoveryRelease, trackIndex: number) => void
		onTrackLikeToggle?: (releaseId: string, trackId: string) => void
		onTrackContextMenu?: (release: DiscoveryRelease, trackIndex: number, canPlay: boolean, e: MouseEvent) => void
		onSortChange?: (config: DiscoverySortConfig) => void
		onContextMenu?: (e: MouseEvent, release: DiscoveryRelease) => void
		onEmptySpaceContextMenu?: (e: MouseEvent) => void
		onUrlDrop?: (url: string) => void
		onToggleEditor?: () => void
		likedOnly?: boolean
		onToggleLikedFilter?: () => void
		scrollOffset?: number
		onScrollChange?: (offset: number) => void
	}

	let {
		releases,
		releaseCount,
		selectedIds,
		sortConfig,
		categoryColors,
		categorySortOrders,
		editorVisible = false,
		hasSelection = false,
		searchValue = '',
		onSearchChange,
		activeFilterTags = [],
		tagCategories = [],
		tagColors = new Map(),
		tagFilterMode = 'or',
		onToggleTagFilter,
		onClearAllTagFilters,
		onToggleTagFilterMode,
		onSelectionChange,
		onReleaseOpen,
		onReleaseOpenUrl,
		onReleaseImport,
		onTrackPlay,
		onTrackLikeToggle,
		onTrackContextMenu,
		onSortChange,
		onContextMenu,
		onEmptySpaceContextMenu,
		onUrlDrop,
		onToggleEditor,
		likedOnly = false,
		onToggleLikedFilter,
		scrollOffset = 0,
		onScrollChange,
	}: Props = $props()

	let isDragOver = $state(false)

	const hasExpandableReleases = $derived(releases.some((r) => r.tracks.length > 0))

	function handleExpandAll() {
		expandedReleaseIds.expandAll(releases.filter((r) => r.tracks.length > 0).map((r) => r.id))
	}

	function handleCollapseAll() {
		expandedReleaseIds.collapseAll()
	}

	function hasUrlData(e: DragEvent): boolean {
		if (!e.dataTransfer) return false
		return e.dataTransfer.types.includes('text/uri-list') || e.dataTransfer.types.includes('text/plain')
	}

	function extractUrl(e: DragEvent): string | null {
		if (!e.dataTransfer) return null
		const uriList = e.dataTransfer.getData('text/uri-list')
		if (uriList) {
			// text/uri-list can contain multiple URLs separated by newlines; comments start with #
			const firstUrl = uriList
				.split('\n')
				.map((line) => line.trim())
				.find((line) => line && !line.startsWith('#'))
			if (firstUrl) return firstUrl
		}
		const text = e.dataTransfer.getData('text/plain')?.trim()
		if (text && isValidUrl(text)) return text
		return null
	}

	function isValidUrl(text: string): boolean {
		return text.startsWith('http://') || text.startsWith('https://')
	}

	function handleDragOver(e: DragEvent) {
		if (hasUrlData(e)) {
			e.preventDefault()
			isDragOver = true
		}
	}

	function handleDragLeave(e: DragEvent) {
		const related = e.relatedTarget as Node | null
		const currentTarget = e.currentTarget as HTMLElement
		if (related && currentTarget.contains(related)) return
		isDragOver = false
	}

	function handleDrop(e: DragEvent) {
		e.preventDefault()
		isDragOver = false
		const url = extractUrl(e)
		if (url && onUrlDrop) {
			onUrlDrop(url)
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="relative flex h-full flex-col overflow-hidden bg-surface-0"
	ondragover={handleDragOver}
	ondragleave={handleDragLeave}
	ondrop={handleDrop}
>
	<!-- Header -->
	<div class="flex items-center border-b border-stroke px-4 py-4">
		<div class="flex items-center gap-2 rounded px-2 py-1 text-sm font-medium text-text-primary">
			<Icon name="globe" class="h-4 w-4 shrink-0" />
			<span>{$translate('nav.discovery')}</span>
			<Text as="span" color="tertiary" class="ml-2">
				{releaseCount}
				{releaseCount === 1 ? $translate('discovery.release') : $translate('discovery.releases')}
			</Text>
		</div>
		<div class="flex flex-1 items-center justify-end gap-2">
			{#if onSearchChange}
				<div class="w-64">
					<SearchBar
						{onSearchChange}
						initialValue={searchValue}
						placeholder={$translate('discovery.searchPlaceholder')}
					/>
				</div>
			{/if}
			<FollowingButton />
			<FilterDropdown
				{activeFilterTags}
				{tagCategories}
				{tagColors}
				{tagFilterMode}
				onToggleTagFilter={(tagId) => onToggleTagFilter?.(tagId)}
				onClearAll={() => onClearAllTagFilters?.()}
				onToggleTagFilterMode={() => onToggleTagFilterMode?.()}
				showLikedFilter
				{likedOnly}
				{onToggleLikedFilter}
				showNewFilter
				newOnly={$newOnly}
				onToggleNewFilter={() => discoveryStore.toggleNewFilter()}
			/>
			<Tooltip text={$translate('discovery.expandAll')} position="bottom" delay={250}>
				<IconButton icon="unfold-vertical" size="sm" disabled={!hasExpandableReleases} onclick={handleExpandAll} />
			</Tooltip>
			<Tooltip text={$translate('discovery.collapseAll')} position="bottom" delay={250}>
				<IconButton icon="fold-vertical" size="sm" disabled={!hasExpandableReleases} onclick={handleCollapseAll} />
			</Tooltip>
			<Tooltip
				text={editorVisible ? $translate('editor.hideEditor') : $translate('editor.showEditor')}
				position="bottom"
				delay={250}
			>
				<IconButton icon="panel-right" size="sm" disabled={!hasSelection} onclick={onToggleEditor} />
			</Tooltip>
		</div>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-hidden">
		<DiscoveryList
			{releases}
			{selectedIds}
			expandedIds={$expandedReleaseIds}
			{sortConfig}
			{categoryColors}
			{categorySortOrders}
			{isDragOver}
			{scrollOffset}
			{likedOnly}
			{onSelectionChange}
			{onReleaseOpen}
			{onReleaseOpenUrl}
			{onReleaseImport}
			{onSortChange}
			{onContextMenu}
			{onEmptySpaceContextMenu}
			onToggleExpand={(id) => expandedReleaseIds.toggle(id)}
			{onTrackPlay}
			{onTrackLikeToggle}
			{onTrackContextMenu}
			{onScrollChange}
		/>
	</div>

	<!-- Drop overlay -->
	{#if isDragOver}
		<div
			class="border-brand-primary/50 bg-brand-primary/5 pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-md border-2 border-dashed"
		>
			<div class="flex flex-col items-center gap-2">
				<Icon name="globe" class="text-brand-primary/70 h-10 w-10" />
				<Text weight="medium" color="secondary">{$translate('discovery.dropHint')}</Text>
			</div>
		</div>
	{/if}
</div>
