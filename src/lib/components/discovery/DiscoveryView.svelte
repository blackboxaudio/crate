<script lang="ts">
	import type { DiscoveryRelease, DiscoverySortConfig } from '$lib/types'
	import DiscoveryList from './DiscoveryList.svelte'
	import { IconButton } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { translate } from '$lib/i18n'
	import { SvelteSet } from 'svelte/reactivity'

	type Props = {
		releases: DiscoveryRelease[]
		releaseCount: number
		selectedIds: Set<string>
		sortConfig: DiscoverySortConfig
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		onSelectionChange?: (ids: Set<string>) => void
		onReleaseOpen?: (release: DiscoveryRelease) => void
		onReleaseImport?: (release: DiscoveryRelease) => void
		onSortChange?: (config: DiscoverySortConfig) => void
		onContextMenu?: (e: MouseEvent, release: DiscoveryRelease) => void
		onEmptySpaceContextMenu?: (e: MouseEvent) => void
		onUrlDrop?: (url: string) => void
	}

	let {
		releases,
		releaseCount,
		selectedIds,
		sortConfig,
		categoryColors,
		categorySortOrders,
		onSelectionChange,
		onReleaseOpen,
		onReleaseImport,
		onSortChange,
		onContextMenu,
		onEmptySpaceContextMenu,
		onUrlDrop,
	}: Props = $props()

	let isDragOver = $state(false)
	let expandedIds = $state(new Set<string>())

	const hasExpandableReleases = $derived(releases.some((r) => r.tracks.length > 0))

	function toggleExpand(id: string) {
		expandedIds = new SvelteSet(expandedIds)
		if (expandedIds.has(id)) {
			expandedIds.delete(id)
		} else {
			expandedIds.add(id)
		}
	}

	export function expandAll() {
		expandedIds = new SvelteSet(releases.filter((r) => r.tracks.length > 0).map((r) => r.id))
	}

	export function collapseAll() {
		expandedIds = new SvelteSet()
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
	<div class="flex items-center justify-between border-b border-stroke px-4 py-4">
		<div class="flex items-center gap-2 rounded px-2 py-1 text-sm font-medium text-text-primary">
			<Icon name="globe" class="h-4 w-4 shrink-0" />
			<span>{$translate('nav.discovery')}</span>
			<Text as="span" color="tertiary" class="ml-2">
				{releaseCount}
				{releaseCount === 1 ? $translate('discovery.release') : $translate('discovery.releases')}
			</Text>
		</div>
		{#if hasExpandableReleases}
			<div class="flex items-center gap-1">
				<Tooltip text={$translate('discovery.expandAll')} position="bottom" delay={250}>
					<IconButton icon="unfold-vertical" size="sm" onclick={expandAll} />
				</Tooltip>
				<Tooltip text={$translate('discovery.collapseAll')} position="bottom" delay={250}>
					<IconButton icon="fold-vertical" size="sm" onclick={collapseAll} />
				</Tooltip>
			</div>
		{/if}
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-hidden">
		<DiscoveryList
			{releases}
			{selectedIds}
			{expandedIds}
			{sortConfig}
			{categoryColors}
			{categorySortOrders}
			{isDragOver}
			{onSelectionChange}
			{onReleaseOpen}
			{onReleaseImport}
			{onSortChange}
			{onContextMenu}
			{onEmptySpaceContextMenu}
			onToggleExpand={toggleExpand}
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
