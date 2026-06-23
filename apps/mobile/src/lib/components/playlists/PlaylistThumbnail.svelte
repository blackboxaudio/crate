<script lang="ts">
	// Spotify-style playlist thumbnail: a seamless 2x2 mosaic of the first four distinct release
	// covers, a single cover when there are fewer than four, or the music-note placeholder when the
	// playlist has none. Uses the release `artwork_url` (same field the feed/detail cards render), so
	// thumbnails match the rest of the UI without needing the app data dir.
	type Props = {
		urls: string[]
		class?: string
	}
	let { urls, class: className = 'h-11 w-11' }: Props = $props()
</script>

{#if urls.length >= 4}
	<div class="{className} grid grid-cols-2 grid-rows-2 overflow-hidden rounded">
		{#each urls.slice(0, 4) as url (url)}
			<img src={url} alt="" class="h-full w-full object-cover" loading="lazy" />
		{/each}
	</div>
{:else if urls.length > 0}
	<img src={urls[0]} alt="" class="{className} rounded object-cover" loading="lazy" />
{:else}
	<div class="{className} flex items-center justify-center rounded bg-surface-2 text-text-tertiary">
		<svg viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor">
			<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
		</svg>
	</div>
{/if}
