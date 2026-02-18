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
	}: Props = $props()
</script>

<div class="flex h-full flex-col overflow-hidden bg-surface-0">
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
			{onSelectionChange}
			{onReleaseOpen}
			{onReleaseImport}
			{onSortChange}
			{onContextMenu}
			{onEmptySpaceContextMenu}
		/>
	</div>
</div>
