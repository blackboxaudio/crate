<script lang="ts">
	import type { Snippet } from 'svelte'
	import type { DiscoveryRelease } from '$shared/types'
	import { getDiscoveryArtworkSrc } from '$shared/utils/artwork'
	import { cacheReleaseArtwork } from '$shared/api/discovery'
	import { mobileAppDataDir } from '$lib/stores/appData'
	import ArtworkPlaceholder from './ArtworkPlaceholder.svelte'

	// Cache-first cover for a discovery release: renders the on-disk cached copy when present
	// (so it shows offline / in airplane mode), otherwise the remote URL — and downloads the
	// remote cover to disk on first display so it's cached next time. The caller supplies the
	// image `class`; when there's no artwork at all OR the image fails to load (dead URL,
	// offline and uncached), a polished placeholder renders in its place — the optional
	// `fallback` snippet overrides it for callers that need a different shape.
	type Props = {
		release: Pick<DiscoveryRelease, 'id' | 'artwork_url' | 'artwork_cache_path'>
		class?: string
		alt?: string
		/** Decode immediately instead of lazily — for covers that must be ready before they scroll in
		 *  (the expanded player pre-mounts the neighboring tracks' covers for the swipe pager). */
		eager?: boolean
		fallback?: Snippet
	}
	let { release, class: className = '', alt = '', eager = false, fallback }: Props = $props()

	// Local cache-path state so the download can flip the src remote → local without a prop
	// round-trip. Reset when the release identity changes (the virtualized feed reuses
	// instances across rows); adopt a non-null prop value if it catches up, but never clobber
	// an already-resolved path back to null.
	// (Both seeds below are intentional initial snapshots — the sync $effect keeps them current.)
	// svelte-ignore state_referenced_locally
	let lastId = $state(release.id)
	// svelte-ignore state_referenced_locally
	let cachePath = $state<string | null>(release.artwork_cache_path)
	// Once the REMOTE url has decoded successfully, keep showing it for this mount: flipping `src`
	// to the freshly cached local copy would make WebKit reload + re-decode the same pixels (a
	// per-cover double decode and a visible swap, mid-scroll). The cached path still lands in
	// `cachePath` for the next mount / offline. The `onload` guard below only locks when the loaded
	// src IS the remote url, so a cached-copy load can never lock local → remote.
	let remoteLocked = $state(false)
	$effect(() => {
		if (release.id !== lastId) {
			lastId = release.id
			cachePath = release.artwork_cache_path
			remoteLocked = false
		} else if (release.artwork_cache_path && !cachePath) {
			cachePath = release.artwork_cache_path
		}
	})

	let src = $derived(
		remoteLocked
			? (release.artwork_url ?? undefined)
			: getDiscoveryArtworkSrc({ artwork_url: release.artwork_url, artwork_cache_path: cachePath }, $mobileAppDataDir)
	)

	// A failed load (dead/expired URL, offline and uncached) must not leave WebKit's
	// broken-image icon on screen. Reset whenever `src` changes — the remote → cached-copy
	// flip below can succeed even when the webview's own fetch failed, so the retry is free.
	let failed = $state(false)
	$effect(() => {
		void src
		failed = false
	})

	// On first display of an uncached-but-remote cover, cache it to disk (idempotent, soft-fail).
	$effect(() => {
		if (!cachePath && release.artwork_url) {
			const id = release.id
			void cacheReleaseArtwork(id).then((path) => {
				if (path && release.id === id) cachePath = path
			})
		}
	})
</script>

{#if src && !failed}
	<img
		{src}
		{alt}
		class={className}
		loading={eager ? 'eager' : 'lazy'}
		decoding="async"
		onload={() => {
			if (src === release.artwork_url) remoteLocked = true
		}}
		onerror={() => (failed = true)}
	/>
{:else if fallback}
	{@render fallback()}
{:else}
	<ArtworkPlaceholder class={className} />
{/if}
