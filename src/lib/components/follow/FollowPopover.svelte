<script module lang="ts">
	import { writable } from 'svelte/store'

	/** The release id whose follow popover is open. Setting it opens that row's popover and
	 *  closes any other — only one quick-follow popover is visible at a time. */
	export const openFollowPopoverId = writable<string | null>(null)
</script>

<script lang="ts">
	import type { DiscoveryRelease } from '$lib/types'
	import { AlbumArt, Icon } from '$lib/components/common'
	import { followStore, followedSources } from '$lib/stores'
	import { deriveArtistUrl, deriveLabelUrl, isCompilation, looseUrlEq } from '$lib/utils'
	import { fetchSourceAvatar } from '$lib/api/discovery'
	import { translate } from '$lib/i18n'
	import { untrack } from 'svelte'
	import { scale } from 'svelte/transition'

	type Props = {
		release: DiscoveryRelease
		triggerEl: HTMLElement
		onClose: () => void
	}

	let { release, triggerEl, onClose }: Props = $props()

	const platformLabels: Record<string, string> = {
		bandcamp: 'Bandcamp',
		soundcloud: 'SoundCloud',
		discogs: 'Discogs',
		youtube: 'YouTube',
		other: 'Other',
	}

	// A release yields two independent follow targets: the artist (its own Bandcamp
	// subdomain / SoundCloud profile) and the label (the page it was discovered from,
	// `source_page_url`). Each is followed separately, so the toggle stays visible and the
	// user can follow both. The label is only known for releases imported from a label
	// page; otherwise its tab is a non-interactive indicator.
	// A Various-Artists compilation is a label release, not an artist's: disable the Artist
	// tab and treat the release's own page as the label when no separate label page is known.
	const isComp = $derived(isCompilation(release.artist))
	const artistUrl = $derived(isComp ? null : deriveArtistUrl(release))
	const labelUrl = $derived(isComp ? (deriveLabelUrl(release) ?? deriveArtistUrl(release)) : deriveLabelUrl(release))
	const labelAvailable = $derived(!!labelUrl)
	const artistAvailable = $derived(!!artistUrl)
	const hasFollowable = $derived(!!artistUrl || !!labelUrl)

	// Selected tab. Honors the user's pick when that side is available, else defaults to
	// Label (the common discovery case) when present, otherwise Artist.
	let selectedOverride = $state<'artist' | 'label' | null>(null)
	const selected = $derived(
		selectedOverride === 'artist' && artistAvailable
			? 'artist'
			: selectedOverride === 'label' && labelAvailable
				? 'label'
				: labelAvailable
					? 'label'
					: 'artist'
	)

	const artistFollow = $derived(artistUrl ? $followedSources.find((s) => looseUrlEq(s.url, artistUrl)) : undefined)
	const labelFollow = $derived(labelUrl ? $followedSources.find((s) => looseUrlEq(s.url, labelUrl)) : undefined)

	const currentUrl = $derived(selected === 'label' ? labelUrl : artistUrl)
	const currentFollow = $derived(selected === 'label' ? labelFollow : artistFollow)
	const currentName = $derived(
		selected === 'label' ? (release.label ?? release.artist) : (release.artist ?? release.label)
	)
	const displayName = $derived(currentFollow?.name ?? currentName ?? $translate('common.unknownArtist'))
	const typeLabel = $derived(
		selected === 'label' ? $translate('discovery.following.label') : $translate('discovery.following.artist')
	)

	// Profile-picture preview. Followed sources already carry an avatar; otherwise fetch the
	// page's og:image on the fly (session-cached on the backend) so the user sees who they're
	// about to follow without any extra click.
	let fetchedAvatars = $state<Record<string, string | null>>({})
	const avatarPath = $derived(currentFollow?.artworkPath ?? null)
	const avatarUrl = $derived(currentFollow?.artworkUrl ?? (currentUrl ? (fetchedAvatars[currentUrl] ?? null) : null))

	$effect(() => {
		const url = currentUrl
		const follow = currentFollow
		if (!url || follow?.artworkUrl || follow?.artworkPath) return
		untrack(() => {
			if (url in fetchedAvatars) return
			fetchedAvatars[url] = null
			fetchSourceAvatar(url)
				.then((a) => (fetchedAvatars[url] = a))
				.catch(() => {})
		})
	})

	let popoverEl: HTMLDivElement | undefined = $state()
	let style = $state('')
	let busy = $state(false)

	function portal(node: HTMLElement) {
		const dialog = triggerEl.closest('dialog')
		;(dialog ?? document.body).appendChild(node)
		return { destroy: () => node.remove() }
	}

	function computePosition() {
		if (!popoverEl) return
		const r = triggerEl.getBoundingClientRect()
		const w = popoverEl.offsetWidth
		const h = popoverEl.offsetHeight
		let top = r.bottom + 4
		if (top + h + 8 > window.innerHeight) top = Math.max(8, r.top - h - 4)
		let left = Math.max(8, Math.min(r.right - w, window.innerWidth - w - 8))
		style = `position:fixed;top:${top}px;left:${left}px;`
	}

	$effect(() => {
		computePosition()
		requestAnimationFrame(computePosition)
		const onScroll = () => computePosition()
		window.addEventListener('scroll', onScroll, true)
		window.addEventListener('resize', onScroll)
		return () => {
			window.removeEventListener('scroll', onScroll, true)
			window.removeEventListener('resize', onScroll)
		}
	})

	function handleClickOutside(e: MouseEvent) {
		const t = e.target as Node
		if (popoverEl?.contains(t) || triggerEl.contains(t)) return
		onClose()
	}

	$effect(() => {
		document.addEventListener('click', handleClickOutside)
		return () => document.removeEventListener('click', handleClickOutside)
	})

	async function toggle() {
		if (busy || !currentUrl) return
		busy = true
		if (currentFollow) {
			await followStore.unfollow(currentFollow.id)
		} else {
			await followStore.followEntity({
				url: currentUrl,
				name: currentName ?? null,
				sourceType: release.source_type,
				followType: selected,
				// Hand off the avatar we already fetched so it shows in the Following manager
				// immediately, instead of waiting on the background baseline scan to backfill it.
				artworkUrl: avatarUrl ?? undefined,
			})
		}
		busy = false
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<div
	bind:this={popoverEl}
	use:portal
	class="z-50 w-64 rounded-md border border-stroke bg-surface-1 p-2 shadow-lg"
	{style}
	transition:scale={{ start: 0.95, duration: 150 }}
>
	<div class="px-1 pb-1.5 text-[10px] font-semibold tracking-wide text-text-tertiary uppercase">
		{$translate('discovery.following.popoverTitle')}
	</div>
	{#if hasFollowable}
		<div class="flex items-center gap-2 rounded px-1 py-1.5">
			{#if avatarPath || avatarUrl}
				<AlbumArt artworkPath={avatarPath} artworkUrl={avatarUrl} size="xs" />
			{:else}
				<Icon name={selected === 'label' ? 'disc' : 'user'} class="h-4 w-4 shrink-0 text-text-tertiary" />
			{/if}
			<div class="min-w-0 flex-1">
				<div class="truncate text-sm text-text-primary">{displayName}</div>
				<div class="truncate text-[11px] text-text-tertiary">
					{typeLabel} · {platformLabels[release.source_type] ?? release.source_type}
				</div>
			</div>
			<button
				type="button"
				class="shrink-0 rounded-md px-2 py-1 text-xs font-medium transition-colors hover:cursor-pointer disabled:opacity-60 {currentFollow
					? 'bg-brand-muted text-brand-primary'
					: 'bg-surface-2 text-text-secondary hover:text-text-primary'}"
				onclick={toggle}
				disabled={busy || !currentUrl}
			>
				{currentFollow ? $translate('discovery.following.following') : $translate('discovery.following.follow')}
			</button>
		</div>
		<!-- Always-visible Label｜Artist toggle so the user can follow both. When the release
		     has no known label page, the Label side is a non-interactive indicator. -->
		<div
			class="relative mt-1 grid grid-cols-2 rounded-md border border-stroke bg-surface-2 p-0.5 text-[11px] font-medium"
		>
			<div
				class="absolute top-0.5 bottom-0.5 left-0.5 w-[calc(50%-2px)] rounded bg-surface-1 shadow-sm transition-transform duration-200 ease-out motion-reduce:transition-none"
				style="transform: translateX({selected === 'artist' ? '100%' : '0%'})"
			></div>
			<button
				type="button"
				disabled={!labelAvailable}
				class="relative z-10 rounded px-2 py-1 text-center transition-colors {!labelAvailable
					? 'cursor-default text-text-tertiary/40'
					: selected === 'label'
						? 'text-text-primary hover:cursor-pointer'
						: 'text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
				onclick={() => labelAvailable && (selectedOverride = 'label')}
			>
				{$translate('discovery.following.label')}
			</button>
			<button
				type="button"
				disabled={!artistAvailable}
				class="relative z-10 rounded px-2 py-1 text-center transition-colors {!artistAvailable
					? 'cursor-default text-text-tertiary/40'
					: selected === 'artist'
						? 'text-text-primary hover:cursor-pointer'
						: 'text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
				onclick={() => artistAvailable && (selectedOverride = 'artist')}
			>
				{$translate('discovery.following.artist')}
			</button>
		</div>
	{:else}
		<div class="px-1 py-1.5 text-xs text-text-tertiary">
			{$translate('discovery.following.followViaPaste')}
		</div>
	{/if}
</div>
