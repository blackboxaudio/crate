<script lang="ts">
	import type { Playlist } from '$lib/types'

	type Props = {
		playlist: Playlist
		childCount?: number
		onclick?: () => void
	}

	let { playlist, childCount = 0, onclick }: Props = $props()
</script>

<button
	type="button"
	class="flex flex-col items-center gap-3 rounded-lg bg-surface-2 p-6 text-center transition-colors hover:bg-stroke"
	{onclick}
>
	<!-- Icon -->
	<div class="flex h-12 w-12 items-center justify-center rounded-lg bg-stroke">
		{#if playlist.is_folder}
			<svg class="h-6 w-6 text-text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
				/>
			</svg>
		{:else if playlist.is_smart}
			<svg class="h-6 w-6 text-yellow-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
			</svg>
		{:else}
			<svg class="h-6 w-6 text-text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"
				/>
			</svg>
		{/if}
	</div>

	<!-- Name -->
	<span class="w-full truncate text-sm font-medium text-text-primary">
		{playlist.name}
	</span>

	<!-- Count -->
	<span class="text-xs text-text-tertiary">
		{#if playlist.is_folder}
			{childCount} {childCount === 1 ? 'item' : 'items'}
		{:else}
			{playlist.track_count} {playlist.track_count === 1 ? 'track' : 'tracks'}
		{/if}
	</span>
</button>
