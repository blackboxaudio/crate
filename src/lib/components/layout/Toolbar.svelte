<script lang="ts">
	import type { Tag, TagFilterMode } from '$lib/types'
	import { Button, IconButton, Tooltip } from '$lib/components/common'
	import { SearchBar } from '$lib/components/library'
	import Icon from '$lib/components/common/Icon.svelte'
	import { isDev } from '$lib/stores'
	import { translate } from '$lib/i18n'

	type Props = {
		activeFilterTags?: Tag[]
		tagColors?: Map<string, string | null>
		tagFilterMode?: TagFilterMode
		onRemoveTagFilter?: (tagId: string) => void
		onClearAllTagFilters?: () => void
		onToggleTagFilterMode?: () => void
		onImport?: () => void
		onSettings?: () => void
		onDevTools?: () => void
	}

	let {
		activeFilterTags,
		tagColors,
		tagFilterMode,
		onRemoveTagFilter,
		onClearAllTagFilters,
		onToggleTagFilterMode,
		onImport,
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

	<!-- Spacer -->
	<div class="flex-1"></div>

	<!-- Actions -->
	<div class="flex items-center gap-2">
		<Button variant="primary" size="sm" onclick={onImport}>
			<Icon name="upload" class="mr-1.5 h-4 w-4" />
			{$translate('library.importTracks')}
		</Button>
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
