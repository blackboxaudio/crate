<script lang="ts">
	import type { Track } from '$lib/types'
	import { formatDurationCompact, formatBpm, formatKey, getTrackDisplayName, getTrackDisplayArtist } from '$lib/utils'
	import { TagChip } from '$lib/components/tags'

	type Props = {
		track: Track
		selected?: boolean
		playing?: boolean
		dragTrackIds?: string[]
		categoryColors?: Map<string, string | null>
		onclick?: (e: MouseEvent) => void
		ondblclick?: (e: MouseEvent) => void
		oncontextmenu?: (e: MouseEvent) => void
	}

	let {
		track,
		selected = false,
		playing = false,
		dragTrackIds = [],
		categoryColors,
		onclick,
		ondblclick,
		oncontextmenu,
	}: Props = $props()

	function handleDragStart(e: DragEvent) {
		console.log('[DragStart]', track.title, { dataTransfer: !!e.dataTransfer })
		if (!e.dataTransfer) return

		// If this track is selected, drag all selected tracks; otherwise just this track
		const trackIds = selected && dragTrackIds.length > 0 ? dragTrackIds : [track.id]

		e.dataTransfer.effectAllowed = 'copy'
		e.dataTransfer.setData('application/x-crate-tracks', JSON.stringify(trackIds))
		e.dataTransfer.setData('text/plain', getTrackDisplayName(track))
		console.log('[DragStart] Set data:', { trackIds, types: Array.from(e.dataTransfer.types) })
	}
</script>

<div
	role="row"
	tabindex="0"
	draggable="true"
	class="grid cursor-pointer grid-cols-[1fr_1fr_80px_60px_80px_1fr] gap-2 border-b border-zinc-800 px-3 py-2 text-sm transition-colors {selected
		? 'bg-blue-600/20'
		: 'hover:bg-zinc-800/50'} {playing ? 'text-blue-400' : 'text-zinc-300'}"
	{onclick}
	{ondblclick}
	{oncontextmenu}
	ondragstart={handleDragStart}
	onkeydown={(e) => e.key === 'Enter' && ondblclick?.(e)}
>
	<!-- Title -->
	<div class="truncate font-medium {playing ? 'text-blue-400' : 'text-zinc-100'}">
		{#if playing}
			<span class="mr-1 inline-block w-4">
				<svg class="h-3 w-3 animate-pulse" fill="currentColor" viewBox="0 0 24 24">
					<path d="M8 5v14l11-7z" />
				</svg>
			</span>
		{/if}
		{getTrackDisplayName(track)}
	</div>

	<!-- Artist -->
	<div class="truncate text-zinc-400">
		{getTrackDisplayArtist(track)}
	</div>

	<!-- BPM -->
	<div class="text-right text-zinc-400 tabular-nums">
		{formatBpm(track.bpm)}
	</div>

	<!-- Key -->
	<div class="text-center text-zinc-400">
		{formatKey(track.key)}
	</div>

	<!-- Duration -->
	<div class="text-right text-zinc-400 tabular-nums">
		{formatDurationCompact(track.duration_ms)}
	</div>

	<!-- Tags -->
	<div class="flex gap-1 overflow-hidden">
		{#each track.tags.slice(0, 3) as tag (tag.id)}
			<TagChip {tag} size="sm" color={categoryColors?.get(tag.category_id)} />
		{/each}
		{#if track.tags.length > 3}
			<span class="text-xs text-zinc-500">+{track.tags.length - 3}</span>
		{/if}
	</div>
</div>
