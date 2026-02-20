<script lang="ts">
	import type { DiscoveryRelease, DiscoverySortConfig, DiscoverySourceType } from '$lib/types'
	import { handleSelection } from '$lib/utils'
	import { translate } from '$lib/i18n'
	import DiscoveryListHeader from './DiscoveryListHeader.svelte'
	import DiscoveryRow from './DiscoveryRow.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'

	const BASE_PREVIEWABLE: Set<DiscoverySourceType> = new Set(['bandcamp', 'soundcloud', 'youtube'])

	function isReleasePreviewable(release: DiscoveryRelease): boolean {
		if (BASE_PREVIEWABLE.has(release.source_type)) return true
		return release.tracks.some((t) => t.video_id !== null)
	}

	type Props = {
		releases: DiscoveryRelease[]
		selectedIds: Set<string>
		expandedIds?: Set<string>
		sortConfig: DiscoverySortConfig
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		isDragOver?: boolean
		onSelectionChange?: (ids: Set<string>) => void
		onReleaseOpen?: (release: DiscoveryRelease) => void
		onReleaseOpenUrl?: (release: DiscoveryRelease) => void
		onReleaseImport?: (release: DiscoveryRelease) => void
		onSortChange?: (config: DiscoverySortConfig) => void
		onContextMenu?: (e: MouseEvent, release: DiscoveryRelease) => void
		onEmptySpaceContextMenu?: (e: MouseEvent) => void
		onToggleExpand?: (id: string) => void
		onTrackPlay?: (release: DiscoveryRelease, trackIndex: number) => void
	}

	let {
		releases,
		selectedIds,
		expandedIds = new Set<string>(),
		sortConfig,
		categoryColors,
		categorySortOrders,
		isDragOver = false,
		onSelectionChange,
		onReleaseOpen,
		onReleaseOpenUrl,
		onReleaseImport,
		onSortChange,
		onContextMenu,
		onEmptySpaceContextMenu,
		onToggleExpand,
		onTrackPlay,
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
		onToggleExpand?.(release.id)
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
	<div
		class="relative flex-1 overflow-auto"
		data-drop-target="releaselist-main"
		onclick={handleContainerClick}
		oncontextmenu={handleContainerContextMenu}
	>
		{#if releases.length === 0}
			<div
				class="flex h-full flex-col items-center justify-center p-8 text-text-tertiary {isDragOver
					? 'border-brand-primary/50 bg-brand-primary/5 rounded-md border-2 border-dashed'
					: ''}"
			>
				<Icon name="globe" class="mb-4 h-16 w-16" />
				{#if isDragOver}
					<Text variant="header-1" weight="medium" class="mb-2">{$translate('discovery.dropHint')}</Text>
				{:else}
					<Text variant="header-1" weight="medium" class="mb-2">{$translate('discovery.noReleasesYet')}</Text>
					<Text color="tertiary" class="max-w-sm text-center">
						{$translate('discovery.addReleaseHint', { values: { shortcut: '⌘D' } })}
					</Text>
				{/if}
			</div>
		{:else}
			{#each releases as release (release.id)}
				<DiscoveryRow
					{release}
					selected={selectedIds.has(release.id)}
					expanded={expandedIds.has(release.id)}
					isPreviewable={isReleasePreviewable(release)}
					dragReleaseIds={Array.from(selectedIds)}
					{categoryColors}
					{categorySortOrders}
					onclick={(e) => handleReleaseClick(release, e)}
					ondblclick={() => handleReleaseDoubleClick(release)}
					oncontextmenu={(e) => handleReleaseContextMenu(release, e)}
					onimport={() => onReleaseImport?.(release)}
					onopenurl={() => onReleaseOpenUrl?.(release)}
					onToggleExpand={() => onToggleExpand?.(release.id)}
					onTrackPlay={(idx) => onTrackPlay?.(release, idx)}
				/>
			{/each}
		{/if}
	</div>
</div>
