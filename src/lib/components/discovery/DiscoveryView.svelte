<script lang="ts">
	import type { DiscoveryRelease, DiscoverySortConfig } from '$lib/types'
	import DiscoveryList from './DiscoveryList.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import { translate } from '$lib/i18n'

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
	<div class="flex items-center gap-1 border-b border-stroke px-6 py-4">
		<div class="flex items-center gap-2 rounded px-2 py-1 text-sm font-medium text-text-primary">
			<Icon name="globe" class="h-4 w-4 shrink-0" />
			<span>{$translate('nav.discovery')}</span>
			<Text as="span" color="tertiary" class="ml-2">
				{releaseCount}
				{releaseCount === 1 ? $translate('discovery.release') : $translate('discovery.releases')}
			</Text>
		</div>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-hidden">
		<DiscoveryList
			{releases}
			{selectedIds}
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
