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
	const followed = $derived(sourceUrl ? $followedSources.find((s) => looseUrlEq(s.url, sourceUrl)) : undefined)

	// We can derive only one page from a release and can't reliably tell artist from label
	// (see #126 polish), so the user picks. Defaults to Label (the common case); a user pick
	// (`typeOverride`) wins. Kept as an override so the derived default stays reactive.
	let typeOverride = $state<'artist' | 'label' | null>(null)
	const selectedType = $derived(typeOverride ?? 'label')
	// Once followed, reflect the stored classification; otherwise the live selection.
	const effectiveType = $derived(followed ? (followed.followType === 'label' ? 'label' : 'artist') : selectedType)
	const rawName = $derived(
		effectiveType === 'label' ? (release.label ?? release.artist) : (release.artist ?? release.label)
	)
	const displayName = $derived(followed?.name ?? rawName ?? $translate('common.unknownArtist'))
	const typeLabel = $derived(
		effectiveType === 'label' ? $translate('discovery.following.label') : $translate('discovery.following.artist')
	)

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
				name: rawName ?? null,
				sourceType: release.source_type,
				followType: selectedType,
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
			<Icon name={effectiveType === 'label' ? 'disc' : 'user'} class="h-4 w-4 shrink-0 text-text-tertiary" />
			<div class="min-w-0 flex-1">
				<div class="truncate text-sm text-text-primary">{displayName}</div>
				<div class="truncate text-[11px] text-text-tertiary">
					{typeLabel} · {platformLabels[release.source_type] ?? release.source_type}
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
		{#if !followed}
			<div
				class="relative mt-1 grid grid-cols-2 rounded-md border border-stroke bg-surface-2 p-0.5 text-[11px] font-medium"
			>
				<div
					class="absolute top-0.5 bottom-0.5 left-0.5 w-[calc(50%-2px)] rounded bg-surface-1 shadow-sm transition-transform duration-200 ease-out motion-reduce:transition-none"
					style="transform: translateX({selectedType === 'artist' ? '100%' : '0%'})"
				></div>
				<button
					type="button"
					class="relative z-10 rounded px-2 py-1 text-center transition-colors hover:cursor-pointer {selectedType ===
					'label'
						? 'text-text-primary'
						: 'text-text-tertiary hover:text-text-secondary'}"
					onclick={() => (typeOverride = 'label')}
				>
					{$translate('discovery.following.label')}
				</button>
				<button
					type="button"
					class="relative z-10 rounded px-2 py-1 text-center transition-colors hover:cursor-pointer {selectedType ===
					'artist'
						? 'text-text-primary'
						: 'text-text-tertiary hover:text-text-secondary'}"
					onclick={() => (typeOverride = 'artist')}
				>
					{$translate('discovery.following.artist')}
				</button>
			</div>
		{/if}
	{:else}
		<div class="px-1 py-1.5 text-xs text-text-tertiary">
			{$translate('discovery.following.followViaPaste')}
		</div>
	{/if}
</div>
