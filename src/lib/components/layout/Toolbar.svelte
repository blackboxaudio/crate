<script lang="ts">
	import type { Tag, TagFilterMode } from '$lib/types'
	import { Button, IconButton, Text } from '$lib/components/common'
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

<div class="relative z-20 flex items-center gap-4 border-b border-stroke bg-surface-1 px-4 py-2">
	<!-- Logo/Title -->
	<div class="flex items-center gap-2">
		<Icon name="logo" class="h-6 w-6 text-brand-primary" fill />
		<Text variant="header-1" as="span" weight="bold">Crate</Text>
		{#if $isDev}
			<span class="rounded bg-amber-500/20 px-1.5 py-0.5 text-xs font-medium text-amber-500"> DEV </span>
		{/if}
	</div>

	<!-- Search -->
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
