<script lang="ts">
	import type { ActiveView, Tag, TagFilterMode } from '$lib/types'
	import { Button, IconButton, Tooltip } from '$lib/components/common'
	import { SearchBar } from '$lib/components/library'
	import Icon from '$lib/components/common/Icon.svelte'
	import { isDev } from '$lib/stores'
	import { translate } from '$lib/i18n'

	type Props = {
		activeView?: ActiveView
		activeFilterTags?: Tag[]
		tagColors?: Map<string, string | null>
		tagFilterMode?: TagFilterMode
		onViewChange?: (view: ActiveView) => void
		onRemoveTagFilter?: (tagId: string) => void
		onClearAllTagFilters?: () => void
		onToggleTagFilterMode?: () => void
		onImport?: () => void
		onAddRelease?: () => void
		onSettings?: () => void
		onDevTools?: () => void
	}

	let {
		activeView = 'library',
		activeFilterTags,
		tagColors,
		tagFilterMode,
		onViewChange,
		onRemoveTagFilter,
		onClearAllTagFilters,
		onToggleTagFilterMode,
		onImport,
		onAddRelease,
		onSettings,
		onDevTools,
	}: Props = $props()
</script>

<div class="flex flex-1 items-center gap-4 rounded-bl-md px-4 py-4">
	<!-- Search bar -->
	<div class="max-w-md flex-1">
		<SearchBar
			{activeFilterTags}
			{tagColors}
			{tagFilterMode}
			{onRemoveTagFilter}
			{onClearAllTagFilters}
			{onToggleTagFilterMode}
		/>
	</div>

	<!-- Segmented control -->
	<div class="flex flex-1 items-center justify-center">
		<div class="relative inline-grid grid-cols-2 items-center rounded-lg bg-surface-2 p-0.5">
			<!-- Sliding indicator -->
			<div
				class="absolute top-0.5 bottom-0.5 left-0.5 w-[calc(50%-2px)] rounded-md bg-surface-0 shadow-sm transition-transform duration-200 ease-out motion-reduce:transition-none"
				style="transform: translateX({activeView === 'discovery' ? '100%' : '0%'})"
			></div>
			<button
				type="button"
				class="relative z-10 rounded-md px-3 py-1 text-center text-xs font-medium transition-colors {activeView ===
				'library'
					? 'text-text-primary'
					: 'text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
				onclick={() => onViewChange?.('library')}
			>
				{$translate('nav.library')}
			</button>
			<button
				type="button"
				class="relative z-10 rounded-md px-3 py-1 text-center text-xs font-medium transition-colors {activeView ===
				'discovery'
					? 'text-text-primary'
					: 'text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
				onclick={() => onViewChange?.('discovery')}
			>
				{$translate('nav.discovery')}
			</button>
		</div>
	</div>

	<!-- Actions -->
	<div class="flex items-center gap-2">
		{#if onAddRelease}
			<Button variant="primary" size="sm" onclick={onAddRelease}>
				<Icon name="plus" class="mr-1.5 h-4 w-4" />
				{$translate('discovery.addRelease')}
			</Button>
		{:else}
			<Button variant="primary" size="sm" onclick={onImport}>
				<Icon name="upload" class="mr-1.5 h-4 w-4" />
				{$translate('library.importTracks')}
			</Button>
		{/if}
		{#if $isDev}
			<Tooltip text={$translate('common.developerTools')} position="bottom" delay={250}>
				<IconButton icon="terminal" iconClass="h-5 w-5" onclick={onDevTools} />
			</Tooltip>
		{/if}
		<Tooltip text={$translate('settings.title')} position="bottom" delay={250}>
			<IconButton icon="settings" iconClass="h-5 w-5" onclick={onSettings} />
		</Tooltip>
	</div>
</div>
