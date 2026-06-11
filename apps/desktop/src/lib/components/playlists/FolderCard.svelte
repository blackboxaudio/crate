<script lang="ts">
	import type { Playlist } from '$shared/types'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import { translate } from '$shared/i18n'
	import { dragStore } from '$lib/stores'
	import { DRAG_THRESHOLD, getDistance } from '$shared/utils/drag'

	type Props = {
		playlist: Playlist
		childCount?: number
		onclick?: () => void
		oncontextmenu?: (e: MouseEvent) => void
	}

	let { playlist, childCount = 0, onclick, oncontextmenu }: Props = $props()

	// Track pointer state for drag detection
	let pointerStartPos: { x: number; y: number } | null = null
	let isDragStarted = false

	function handlePointerDown(e: PointerEvent) {
		// Only handle primary button (left click)
		if (e.button !== 0) return

		pointerStartPos = { x: e.clientX, y: e.clientY }
		isDragStarted = false
	}

	function handlePointerMove(e: PointerEvent) {
		if (!pointerStartPos) return

		const distance = getDistance(pointerStartPos.x, pointerStartPos.y, e.clientX, e.clientY)

		// Start drag if threshold exceeded
		if (!isDragStarted && distance >= DRAG_THRESHOLD) {
			isDragStarted = true
			dragStore.startPlaylistDrag(playlist.id, playlist.is_folder, e.clientX, e.clientY)
		}
	}

	function handlePointerUp() {
		pointerStartPos = null
		isDragStarted = false
	}
</script>

<button
	type="button"
	class="flex flex-col items-center gap-3 rounded-lg bg-surface-1 p-6 text-center transition-colors hover:cursor-pointer hover:bg-surface-2"
	{onclick}
	{oncontextmenu}
	onpointerdown={handlePointerDown}
	onpointermove={handlePointerMove}
	onpointerup={handlePointerUp}
	onpointercancel={handlePointerUp}
>
	<!-- Icon -->
	<div class="flex h-12 w-12 items-center justify-center rounded-lg bg-stroke">
		{#if playlist.is_folder}
			<Icon name="folder" class="h-6 w-6 text-text-secondary" />
		{:else if playlist.is_smart}
			<Icon name="bolt" class="h-6 w-6 text-text-secondary" />
		{:else}
			<Icon name="music-note" class="h-6 w-6 text-text-secondary" />
		{/if}
	</div>

	<!-- Name -->
	<Text as="span" variant="body-2" truncate class="w-full">
		{playlist.name}
	</Text>

	<!-- Count -->
	<Text variant="caption">
		{#if playlist.is_folder}
			{childCount} {childCount === 1 ? $translate('library.item') : $translate('library.items')}
		{:else}
			{playlist.track_count} {playlist.track_count === 1 ? $translate('library.track') : $translate('library.tracks')}
		{/if}
	</Text>
</button>
