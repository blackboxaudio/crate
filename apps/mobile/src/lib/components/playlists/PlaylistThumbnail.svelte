<script lang="ts">
	import ArtworkPlaceholder from '$lib/components/common/ArtworkPlaceholder.svelte'

	// Spotify-style playlist thumbnail: a seamless 2x2 mosaic of the first four distinct release
	// covers, a single cover when there are fewer than four, or the artwork placeholder when the
	// playlist has none. Uses the release `artwork_url` (same field the feed/detail cards render), so
	// thumbnails match the rest of the UI without needing the app data dir. A cover URL that fails to
	// load is dropped from the candidate set (the mosaic degrades to fewer covers, never to WebKit's
	// broken-image icon). A smart playlist gets a small sparkle badge so it reads as rule-based at a glance.
	type Props = {
		urls: string[]
		smart?: boolean
		class?: string
	}
	let { urls, smart = false, class: className = 'h-11 w-11' }: Props = $props()

	// Dead URLs observed via <img onerror>, reset whenever the candidate set changes.
	let failedUrls = $state<ReadonlySet<string>>(new Set())
	let lastKey = ''
	$effect(() => {
		const key = urls.join('\n')
		if (key !== lastKey) {
			lastKey = key
			failedUrls = new Set()
		}
	})
	function markFailed(url: string) {
		failedUrls = new Set([...failedUrls, url])
	}

	let good = $derived(urls.filter((u) => !failedUrls.has(u)))
</script>

<div class="relative {className}">
	{#if good.length >= 4}
		<div class="grid h-full w-full grid-cols-2 grid-rows-2 overflow-hidden rounded">
			{#each good.slice(0, 4) as url (url)}
				<img src={url} alt="" class="h-full w-full object-cover" loading="lazy" onerror={() => markFailed(url)} />
			{/each}
		</div>
	{:else if good.length > 0}
		<img
			src={good[0]}
			alt=""
			class="h-full w-full rounded object-cover"
			loading="lazy"
			onerror={() => markFailed(good[0])}
		/>
	{:else}
		<ArtworkPlaceholder class="h-full w-full rounded" />
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
