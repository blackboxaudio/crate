<script lang="ts">
	import type { Track, Playlist } from '$lib/types'
	import type { DragData } from '$lib/stores/drag'
	import { getTrackDisplayName, getTrackDisplayArtist, formatDurationCompact, getPlaylistById } from '$lib/utils'
	import AlbumArt from './AlbumArt.svelte'
	import Icon from './Icon.svelte'
	import Text from './Text.svelte'

	type Props = {
		data: DragData | null
		tracks: Track[]
		playlists: Playlist[]
		x: number
		y: number
	}

	let { data, tracks, playlists, x, y }: Props = $props()

	// Look up the first track when dragging tracks
	const track = $derived.by(() => {
		if (data?.type !== 'tracks' || data.trackIds.length === 0) return null
		return tracks.find((t) => t.id === data.trackIds[0]) ?? null
	})

	// Look up the playlist when dragging a playlist/folder
	const playlist = $derived.by(() => {
		if (data?.type !== 'playlist') return null
		return getPlaylistById(playlists, data.playlistId)
	})

	// Count for multi-track badge
	const additionalCount = $derived(data?.type === 'tracks' ? data.trackIds.length - 1 : 0)
</script>

<div class="pointer-events-none fixed z-50" style="left: {x + 12}px; top: {y + 12}px; transform: translate(0, 0);">
	{#if data?.type === 'tracks' && track}
		<!-- Track Preview -->
		<div
			class="relative flex items-center gap-2.5 rounded bg-surface-2 px-2.5 py-2 shadow-lg ring-1 ring-stroke-subtle"
		>
			<AlbumArt artworkPath={track.artwork_path} size="sm" />
			<div class="flex flex-col gap-0.5">
				<Text as="span" class="max-w-48 truncate text-sm font-medium text-text-primary">
					{getTrackDisplayName(track)}
				</Text>
				<div class="flex items-center gap-2 text-xs text-text-secondary">
					<span class="max-w-32 truncate">{getTrackDisplayArtist(track)}</span>
					<span class="tabular-nums">{formatDurationCompact(track.duration_ms)}</span>
				</div>
			</div>

			<!-- Multi-track count badge -->
			{#if additionalCount > 0}
				<div
					class="absolute -top-2 -right-2 flex h-5 min-w-5 items-center justify-center rounded-full bg-brand-primary px-1.5 text-xs font-medium text-white"
				>
					+{additionalCount}
				</div>
			{/if}
		</div>
	{:else if data?.type === 'playlist' && playlist}
		<!-- Playlist/Folder Preview -->
		<div class="flex items-center gap-2 rounded bg-surface-2 px-3 py-2 shadow-lg ring-1 ring-stroke-subtle">
			<span class="flex-shrink-0 text-text-secondary">
				{#if playlist.is_folder}
					<Icon name="folder" class="h-4 w-4" />
				{:else if playlist.is_smart}
					<Icon name="bolt" class="h-4 w-4" />
				{:else}
					<Icon name="music-note" class="h-4 w-4" />
				{/if}
			</span>
			<Text as="span" class="max-w-48 truncate text-sm text-text-primary">
				{playlist.name}
			</Text>
		</div>
	{/if}
</div>
