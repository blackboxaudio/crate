<script lang="ts">
	import type { DiscoveryRelease } from '$lib/types'
	import { Icon } from '$lib/components/common'
	import { followStore, followedSources } from '$lib/stores'
	import { deriveFollowUrl, looseUrlEq } from '$lib/utils'
	import { translate } from '$lib/i18n'
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

	const sourceUrl = $derived(deriveFollowUrl(release))
	const entityName = $derived(release.artist || $translate('common.unknownArtist'))

	const followed = $derived(sourceUrl ? $followedSources.find((s) => looseUrlEq(s.url, sourceUrl)) : undefined)

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
		if (busy || !sourceUrl) return
		busy = true
		if (followed) {
			await followStore.unfollow(followed.id)
		} else {
			await followStore.followEntity({
				url: sourceUrl,
				name: entityName,
				sourceType: release.source_type,
				followType: 'artist',
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
	{#if sourceUrl}
		<div class="flex items-center gap-2 rounded px-1 py-1.5">
			<Icon name="user" class="h-4 w-4 shrink-0 text-text-tertiary" />
			<div class="min-w-0 flex-1">
				<div class="truncate text-sm text-text-primary">{entityName}</div>
				<div class="truncate text-[11px] text-text-tertiary">
					{$translate('discovery.following.artist')} · {platformLabels[release.source_type] ?? release.source_type}
				</div>
			</div>
			<button
				type="button"
				class="shrink-0 rounded-md px-2 py-1 text-xs font-medium transition-colors hover:cursor-pointer disabled:opacity-60 {followed
					? 'bg-brand-muted text-brand-primary'
					: 'bg-surface-2 text-text-secondary hover:text-text-primary'}"
				onclick={toggle}
				disabled={busy}
			>
				{followed ? $translate('discovery.following.following') : $translate('discovery.following.follow')}
			</button>
		</div>
	{:else}
		<div class="px-1 py-1.5 text-xs text-text-tertiary">
			{$translate('discovery.following.followViaPaste')}
		</div>
	{/if}
</div>
