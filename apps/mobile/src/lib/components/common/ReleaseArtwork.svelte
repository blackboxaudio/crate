<script lang="ts">
	import type { Snippet } from 'svelte'
	import type { DiscoveryRelease } from '$shared/types'
	import { getArtworkUrl, getDiscoveryArtworkSrc, discoveryArtworkThumbPath } from '$shared/utils/artwork'
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
		/** 'thumb' renders the 160px cached thumbnail variant instead of the 500px full cover — for
		 *  the small slots (feed rows, mini player, queue rows) where decoding the full cover wastes
		 *  ~12× the pixels per cover, mid-scroll. Falls back to the full cover when the thumb file
		 *  doesn't exist yet (covers cached before thumbnails shipped) and heals it for next time. */
		size?: 'full' | 'thumb'
		fallback?: Snippet
	}
	let { release, class: className = '', alt = '', eager = false, size = 'full', fallback }: Props = $props()

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
	// The thumb file is derived (not DB-tracked), so it can be missing for covers cached before
	// thumbnails shipped — a load error drops this mount to the full cover and requests a heal.
	let thumbFailed = $state(false)
	$effect(() => {
		if (release.id !== lastId) {
			lastId = release.id
			cachePath = release.artwork_cache_path
			remoteLocked = false
			thumbFailed = false
		} else if (release.artwork_cache_path && !cachePath) {
			cachePath = release.artwork_cache_path
		}
	})

	const thumbSrc = $derived(
		size === 'thumb' && cachePath && !thumbFailed
			? getArtworkUrl(discoveryArtworkThumbPath(cachePath), $mobileAppDataDir)
			: undefined
	)
	let src = $derived(
		remoteLocked
			? (release.artwork_url ?? undefined)
			: (thumbSrc ??
					getDiscoveryArtworkSrc(
						{ artwork_url: release.artwork_url, artwork_cache_path: cachePath },
						$mobileAppDataDir
					))
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
	// Debounced: each call is a Tauri IPC round-trip plus a fetch+decode in Rust, and during a fast
	// fling through uncached territory every mounted row would fire one. Rows that scroll past
	// within the window unmount (or are recycled to a new release id) and cancel here — only rows
	// the user actually pauses on kick off a download.
	$effect(() => {
		if (!cachePath && release.artwork_url) {
			const id = release.id
			const timer = setTimeout(() => {
				void cacheReleaseArtwork(id).then((path) => {
					if (path && release.id === id) cachePath = path
				})
			}, 250)
			return () => clearTimeout(timer)
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
		onerror={() => {
			if (thumbSrc && src === thumbSrc) {
				// Missing thumb file (cover predates thumbnails): fall back to the full cover for
				// this mount, and let the idempotent cache command regenerate the thumb on disk so
				// the next mount gets the cheap decode.
				thumbFailed = true
				void cacheReleaseArtwork(release.id)
			} else {
				failed = true
			}
		}}
	/>
{:else if fallback}
	{@render fallback()}
{:else}
	<ArtworkPlaceholder class={className} />
{/if}
