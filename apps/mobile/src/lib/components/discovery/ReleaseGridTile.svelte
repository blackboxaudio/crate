<script lang="ts">
	import type { DiscoveryRelease } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { previewInfo } from '$shared/stores/player'
	import { DRAG_THRESHOLD } from '$shared/utils/drag'
	import { mobileUIStore, selectMode, selectedReleaseIds } from '$lib/stores/mobileUI'
	import { fullyCachedIds } from '$lib/stores/offlineCache'
	import { lightTap, rigidTap } from '$lib/utils/haptics'
	import ReleaseArtwork from '$lib/components/common/ReleaseArtwork.svelte'

	// One tile of the discovery feed's 3-column artwork grid. Tap opens the detail (or toggles the
	// selection in select mode); long-press opens the same release context menu the list rows use,
	// anchored to the tile. No swipe actions in grid mode — those stay a list-row affordance. The
	// long-press is the simple timer pattern (not the list row's swipe FSM): start on pointerdown,
	// cancel on movement past the drag threshold, and latch `suppressNextClick` because a stationary
	// long-press on a real <button> also synthesizes a click on release.
	type Props = { release: DiscoveryRelease }
	let { release }: Props = $props()

	const isSelectMode = $derived($selectMode)
	const isSelected = $derived($selectedReleaseIds.has(release.id))
	const isCurrentPreview = $derived($previewInfo?.releaseId === release.id)

	let el = $state<HTMLElement | null>(null)
	let longPressTimer = 0
	let startX = 0
	let startY = 0
	let suppressNextClick = false

	function clearLongPress() {
		if (longPressTimer) {
			clearTimeout(longPressTimer)
			longPressTimer = 0
		}
	}

	function detach() {
		window.removeEventListener('pointermove', onMove)
		window.removeEventListener('pointerup', onEnd)
		window.removeEventListener('pointercancel', onEnd)
	}

	function onMove(e: PointerEvent) {
		if (Math.abs(e.clientX - startX) < DRAG_THRESHOLD && Math.abs(e.clientY - startY) < DRAG_THRESHOLD) return
		clearLongPress()
		detach()
	}

	function onEnd() {
		clearLongPress()
		detach()
	}

	function startLongPress(e: PointerEvent) {
		suppressNextClick = false
		if (isSelectMode) return
		startX = e.clientX
		startY = e.clientY
		clearLongPress()
		longPressTimer = window.setTimeout(() => {
			longPressTimer = 0
			suppressNextClick = true
			void rigidTap()
			const r = el?.getBoundingClientRect()
			mobileUIStore.openActionsSheet(
				release.id,
				'feed',
				r ? { top: r.top, left: r.left, width: r.width, height: r.height } : null
			)
		}, 450)
		window.addEventListener('pointermove', onMove)
		window.addEventListener('pointerup', onEnd)
		window.addEventListener('pointercancel', onEnd)
	}

	// Tear down if the tile unmounts mid-press (the virtualizer recycles rows).
	$effect(() => () => {
		clearLongPress()
		detach()
	})

	function onClick(e: MouseEvent) {
		if (suppressNextClick) {
			suppressNextClick = false
			e.preventDefault()
			e.stopPropagation()
			return
		}
		if (isSelectMode) {
			mobileUIStore.toggleReleaseSelected(release.id)
		} else {
			void lightTap()
			mobileUIStore.openDetail(release.id)
		}
	}
</script>

<button
	bind:this={el}
	type="button"
	class="flex w-full flex-col gap-1 text-left"
	aria-label={`${release.artist ?? $translate('common.unknownArtist')} — ${release.title ?? $translate('common.untitled')}`}
	aria-pressed={isSelectMode ? isSelected : undefined}
	onpointerdown={startLongPress}
	onclick={onClick}
>
	<div class="relative w-full">
		<ReleaseArtwork
			{release}
			class="aspect-square w-full rounded-md object-cover {isCurrentPreview ? 'ring-2 ring-brand-primary' : ''}"
		/>
		{#if release.is_new}
			<span class="absolute top-1 left-1 h-2 w-2 rounded-full bg-brand-primary"></span>
		{/if}
		{#if $fullyCachedIds.has(release.id)}
			<span
				class="absolute right-1 bottom-1 flex h-4 w-4 items-center justify-center rounded-full bg-black/55 text-white"
			>
				<svg
					class="h-2.5 w-2.5"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2.5"
					stroke-linecap="round"
					stroke-linejoin="round"
					aria-hidden="true"
				>
					<path d="M12 6v10M8 12l4 4 4-4" />
				</svg>
			</span>
		{/if}
		{#if isSelectMode}
			<span
				class="absolute top-1 right-1 flex h-5 w-5 items-center justify-center rounded-full border {isSelected
					? 'border-brand-primary bg-brand-primary text-white'
					: 'border-white/70 bg-black/30 text-transparent'}"
			>
				<svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
					<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</span>
		{/if}
	</div>
	<span class="w-full truncate text-xs font-medium text-text-primary">
		{release.title ?? $translate('common.untitled')}
	</span>
	<span class="-mt-1 w-full truncate text-[11px] text-text-tertiary">
		{release.artist ?? $translate('common.unknownArtist')}
	</span>
</button>
