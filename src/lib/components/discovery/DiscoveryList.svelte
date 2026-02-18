<script lang="ts">
	import type { DiscoveryRelease, DiscoverySortConfig } from '$lib/types'
	import { handleSelection } from '$lib/utils'
	import { translate } from '$lib/i18n'
	import DiscoveryListHeader from './DiscoveryListHeader.svelte'
	import DiscoveryRow from './DiscoveryRow.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'

	type Props = {
		releases: DiscoveryRelease[]
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

	let lastClickedId: string | null = $state(null)

	function handleReleaseClick(release: DiscoveryRelease, e: MouseEvent) {
		const result = handleSelection(releases, selectedIds, release.id, lastClickedId, {
			shiftKey: e.shiftKey,
			metaKey: e.metaKey,
			ctrlKey: e.ctrlKey,
		})

		lastClickedId = result.lastClickedId
		onSelectionChange?.(result.selectedIds)
	}

	function handleReleaseDoubleClick(release: DiscoveryRelease) {
		onReleaseOpen?.(release)
	}

	function handleReleaseContextMenu(release: DiscoveryRelease, e: MouseEvent) {
		e.preventDefault()

		if (!selectedIds.has(release.id)) {
			onSelectionChange?.(new Set([release.id]))
		}

		onContextMenu?.(e, release)
	}

	function handleContainerClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onSelectionChange?.(new Set())
		}
	}

	function handleContainerContextMenu(e: MouseEvent) {
		const target = e.target as HTMLElement
		if (target.closest('[data-release-row]')) return

		if (onEmptySpaceContextMenu) {
			e.preventDefault()
			onEmptySpaceContextMenu(e)
		}
	}
</script>

<div class="flex h-full flex-col bg-surface-0">
	<DiscoveryListHeader {sortConfig} onSort={onSortChange} />

	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="relative flex-1 overflow-auto" onclick={handleContainerClick} oncontextmenu={handleContainerContextMenu}>
		{#if releases.length === 0}
			<div class="flex h-full flex-col items-center justify-center p-8 text-text-tertiary">
				<Icon name="globe" class="mb-4 h-16 w-16" />
				<Text variant="header-1" weight="medium" class="mb-2">{$translate('discovery.noReleasesYet')}</Text>
				<Text color="tertiary" class="max-w-sm text-center">{$translate('discovery.addReleaseHint')}</Text>
			</div>
		{:else}
			{#each releases as release (release.id)}
				<DiscoveryRow
					{release}
					selected={selectedIds.has(release.id)}
					dragReleaseIds={Array.from(selectedIds)}
					{categoryColors}
					{categorySortOrders}
					onclick={(e) => handleReleaseClick(release, e)}
					ondblclick={() => handleReleaseDoubleClick(release)}
					oncontextmenu={(e) => handleReleaseContextMenu(release, e)}
					onimport={() => onReleaseImport?.(release)}
				/>
			{/each}
		{/if}
	</div>
</div>
