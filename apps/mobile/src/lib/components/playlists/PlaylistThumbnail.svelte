<script lang="ts">
	// Spotify-style playlist thumbnail: a seamless 2x2 mosaic of the first four distinct release
	// covers, a single cover when there are fewer than four, or the music-note placeholder when the
	// playlist has none. Uses the release `artwork_url` (same field the feed/detail cards render), so
	// thumbnails match the rest of the UI without needing the app data dir. A smart playlist gets a small
	// sparkle badge so it reads as rule-based at a glance.
	type Props = {
		urls: string[]
		smart?: boolean
		class?: string
	}
	let { urls, smart = false, class: className = 'h-11 w-11' }: Props = $props()
</script>

<div class="relative {className}">
	{#if urls.length >= 4}
		<div class="grid h-full w-full grid-cols-2 grid-rows-2 overflow-hidden rounded">
			{#each urls.slice(0, 4) as url (url)}
				<img src={url} alt="" class="h-full w-full object-cover" loading="lazy" />
			{/each}
		</div>
	{:else if urls.length > 0}
		<img src={urls[0]} alt="" class="h-full w-full rounded object-cover" loading="lazy" />
	{:else}
		<div class="flex h-full w-full items-center justify-center rounded bg-surface-2 text-text-tertiary">
			<svg viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor">
				<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
			</svg>
		</div>
	{/if}

	{#if smart}
		<span
			class="absolute -right-1 -bottom-1 flex h-4 w-4 items-center justify-center rounded-full bg-brand-primary text-white ring-2 ring-surface-0"
			aria-hidden="true"
		>
			<svg viewBox="0 0 24 24" class="h-2.5 w-2.5" fill="currentColor">
				<path d="M13 2L15.5 8.5L22 11L15.5 13.5L13 20L10.5 13.5L4 11L10.5 8.5L13 2Z" />
			</svg>
		</span>
	{/if}
</div>
