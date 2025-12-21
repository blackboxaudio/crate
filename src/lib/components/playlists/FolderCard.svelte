<script lang="ts">
	import type { Playlist } from '$lib/types'
	import Icon from '$lib/components/common/Icon.svelte'
	import { translate } from '$lib/i18n'

	type Props = {
		playlist: Playlist
		childCount?: number
		onclick?: () => void
		oncontextmenu?: (e: MouseEvent) => void
	}

	let { playlist, childCount = 0, onclick, oncontextmenu }: Props = $props()
</script>

<button
	type="button"
	class="flex flex-col items-center gap-3 rounded-lg bg-surface-1 p-6 text-center transition-colors hover:cursor-pointer hover:bg-surface-2"
	{onclick}
	{oncontextmenu}
>
	<!-- Icon -->
	<div class="flex h-12 w-12 items-center justify-center rounded-lg bg-stroke">
		{#if playlist.is_folder}
			<Icon name="folder" class="h-6 w-6 text-text-secondary" />
		{:else if playlist.is_smart}
			<Icon name="bolt" class="h-6 w-6 text-yellow-400" />
		{:else}
			<Icon name="music-note" class="h-6 w-6 text-text-secondary" />
		{/if}
	</div>

	<!-- Name -->
	<span class="w-full truncate text-sm font-medium text-text-primary">
		{playlist.name}
	</span>

	<!-- Count -->
	<span class="text-xs text-text-tertiary">
		{#if playlist.is_folder}
			{childCount} {childCount === 1 ? $translate('library.item') : $translate('library.items')}
		{:else}
			{playlist.track_count} {playlist.track_count === 1 ? $translate('library.track') : $translate('library.tracks')}
		{/if}
	</span>
</button>
