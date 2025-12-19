<script lang="ts">
	import type { Playlist } from '$lib/types'

	type Props = {
		playlist: Playlist
		selected?: boolean
		depth?: number
		expanded?: boolean
		hasChildren?: boolean
		onclick?: () => void
		onToggle?: () => void
		oncontextmenu?: (e: MouseEvent) => void
		onTracksDrop?: (trackIds: string[]) => void
	}

	let {
		playlist,
		selected = false,
		depth = 0,
		expanded = false,
		hasChildren = false,
		onclick,
		onToggle,
		oncontextmenu,
		onTracksDrop,
	}: Props = $props()

	let paddingLeft = $derived(`${depth * 12 + 8}px`)
	let isDragOver = $state(false)
	let dragEnterCounter = $state(0)

	// Check if dataTransfer contains our custom track mime type
	function hasTrackData(dataTransfer: DataTransfer | null): boolean {
		if (!dataTransfer?.types) return false
		// Always use Array.from for reliable cross-browser compatibility
		return Array.from(dataTransfer.types).includes('application/x-crate-tracks')
	}

	function handleDragOver(e: DragEvent) {
		// Only accept drops on playlists, not folders
		if (playlist.is_folder) return

		console.log('[DragOver]', playlist.name, {
			types: e.dataTransfer?.types ? Array.from(e.dataTransfer.types) : null,
			hasData: hasTrackData(e.dataTransfer),
		})

		if (hasTrackData(e.dataTransfer)) {
			e.preventDefault()
			e.stopPropagation()
			e.dataTransfer!.dropEffect = 'copy'
		}
	}

	function handleDragEnter(e: DragEvent) {
		if (playlist.is_folder) return

		if (hasTrackData(e.dataTransfer)) {
			e.preventDefault() // Required to indicate valid drop target
			e.stopPropagation()
			dragEnterCounter++
			isDragOver = true
		}
	}

	function handleDragLeave(e: DragEvent) {
		if (playlist.is_folder) return

		if (hasTrackData(e.dataTransfer)) {
			dragEnterCounter--
			if (dragEnterCounter <= 0) {
				dragEnterCounter = 0
				isDragOver = false
			}
		}
	}

	function handleDrop(e: DragEvent) {
		e.preventDefault()
		e.stopPropagation()
		dragEnterCounter = 0
		isDragOver = false
		if (playlist.is_folder) return

		const data = e.dataTransfer?.getData('application/x-crate-tracks')
		if (data) {
			try {
				const trackIds = JSON.parse(data) as string[]
				onTracksDrop?.(trackIds)
			} catch {
				// Invalid data
			}
		}
	}
</script>

<div
	role="treeitem"
	tabindex="0"
	aria-selected={selected}
	aria-expanded={playlist.is_folder ? expanded : undefined}
	class="flex cursor-pointer items-center gap-2 rounded py-1.5 pr-2 transition-colors {selected
		? 'bg-blue-600/20 text-zinc-100'
		: isDragOver
			? 'bg-blue-600/30 text-zinc-100 ring-1 ring-blue-500'
			: 'text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200'}"
	style="padding-left: {paddingLeft}"
	{onclick}
	{oncontextmenu}
	ondragover={handleDragOver}
	ondragenter={handleDragEnter}
	ondragleave={handleDragLeave}
	ondrop={handleDrop}
	onkeydown={(e) => e.key === 'Enter' && onclick?.()}
>
	<!-- Expand/Collapse toggle for folders -->
	{#if playlist.is_folder && hasChildren}
		<button
			type="button"
			aria-label={expanded ? 'Collapse' : 'Expand'}
			class="flex h-4 w-4 items-center justify-center text-zinc-500 hover:text-zinc-300"
			onclick={(e) => {
				e.stopPropagation()
				onToggle?.()
			}}
		>
			<svg class="h-3 w-3 transition-transform {expanded ? 'rotate-90' : ''}" fill="currentColor" viewBox="0 0 24 24">
				<path d="M8 5v14l11-7z" />
			</svg>
		</button>
	{:else}
		<span class="w-4"></span>
	{/if}

	<!-- Icon -->
	<span class="flex-shrink-0">
		{#if playlist.is_folder}
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
				/>
			</svg>
		{:else if playlist.is_smart}
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
			</svg>
		{:else}
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"
				/>
			</svg>
		{/if}
	</span>

	<!-- Name -->
	<span class="flex-1 truncate text-sm">
		{playlist.name}
	</span>

	<!-- Track count -->
	{#if !playlist.is_folder}
		<span class="text-xs text-zinc-500">
			{playlist.track_count}
		</span>
	{/if}
</div>
