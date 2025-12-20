<script lang="ts">
	import type { Tag, TagFilterMode } from '$lib/types'
	import { Button, IconButton } from '$lib/components/common'
	import { SearchBar } from '$lib/components/library'
	import Icon from '$lib/components/common/Icon.svelte'
	import { isDev } from '$lib/stores'

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

<div class="flex flex-1 items-center gap-4 rounded-bl-md px-6 py-2">
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
			Import
		</Button>
		{#if $isDev}
			<IconButton title="Developer Tools" onclick={onDevTools}>
				<Icon name="terminal" class="h-5 w-5" />
			</IconButton>
		{/if}
		<IconButton title="Settings" onclick={onSettings}>
			<Icon name="settings" class="h-5 w-5" />
		</IconButton>
	</div>
</div>
