<script lang="ts">
	import type { DiscoveryRelease } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { discoveryStore } from '$shared/stores/discovery'
	import { previewInfo, previewLoadingReleaseId } from '$shared/stores/player'
	import { DRAG_THRESHOLD } from '$shared/utils/drag'
	import * as playbackQueue from '$shared/stores/playbackQueue'
	import { toastStore } from '$shared/stores/toast'
	import { mobileUIStore, selectMode, selectedReleaseIds, openRowId } from '$lib/stores/mobileUI'
	import { lightTap, rigidTap } from '$lib/utils/haptics'
	import { confirmDialog } from '$lib/utils/dialog'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import ReleaseCardContent from './ReleaseCardContent.svelte'

	// A discovery feed row. Shows artwork + title + artist + label and hosts ONE combined pointer gesture
	// that disambiguates between: vertical scroll (always wins for the list), a plain tap (open the detail,
	// or toggle selection in select mode), a long-press (enter multi-select), and a swipe-left that reveals
	// the trailing actions (Add-to-queue + Delete). We don't compose two Svelte actions for this — `swipe`
	// + a `longpress` would both
	// claim the node's `touch-action` and race on the same dx/dy decision — so it's a single state machine,
	// modelled on the axis-lock / velocity-flick approach in `$lib/actions/swipe.ts`.
	type Props = {
		release: DiscoveryRelease
		playlistId?: string | null
		context?: 'feed' | 'playlist' | 'tag' | 'follow'
	}
	let { release, playlistId = null, context = 'feed' }: Props = $props()

	const isSelectMode = $derived($selectMode)
	const isSelected = $derived($selectedReleaseIds.has(release.id))
	const isCurrentPreview = $derived($previewInfo?.releaseId === release.id)
	const isPreviewLoading = $derived($previewLoadingReleaseId === release.id)
	// In select mode only the selection should read as highlighted — applying the current-preview tint there
	// would make the playing release look selected when it isn't. Outside select mode the selection set is
	// empty, so this is just the current-preview highlight.
	const highlighted = $derived(isSelectMode ? isSelected : isCurrentPreview)

	// --- Gesture state --------------------------------------------------------------------------------
	const LONG_PRESS_MS = 450
	const ACTION_W = 88 // width of one revealed trailing action
	const REVEAL_PX = ACTION_W * 2 // swipe-left reveals two actions (Add-to-queue + Delete); fully-open offset
	const FLICK_VELOCITY = 0.4 // px/ms — a flick this fast commits in its direction regardless of distance

	let revealPx = $state(0) // how far the foreground is shifted left (0 = closed, REVEAL_PX = open)
	let dragging = $state(false) // true only while the finger is actively driving the swipe (disables the CSS ease)
	let pressed = $state(false) // tap press-highlight

	let mode: 'idle' | 'pending' | 'swipe' = 'idle'
	let pointerId: number | null = null
	let startX = 0
	let startY = 0
	let lastX = 0
	let lastT = 0
	let velocity = 0
	let openAtStart = 0
	let longPressTimer = 0
	let foregroundEl = $state<HTMLElement | null>(null) // the row's foreground div; its rect anchors the context menu

	const bgClass = $derived(highlighted ? 'bg-brand-muted' : pressed && !dragging ? 'bg-surface-2' : 'bg-surface-0')

	function clamp(v: number, min: number, max: number): number {
		return Math.min(max, Math.max(min, v))
	}

	// Keep the row in sync with the store's single-open invariant: when another row opens (or a scroll
	// closes everything), animate this one shut. Bails while the finger is dragging so it never fights the
	// live follow.
	$effect(() => {
		const openId = $openRowId
		if (dragging) return
		revealPx = openId === release.id ? REVEAL_PX : 0
	})

	// Safety net: tear down listeners / timer if the row unmounts mid-gesture (the virtualizer recycles rows).
	$effect(() => () => {
		clearLongPress()
		detachWindow()
	})

	function clearLongPress() {
		if (longPressTimer) {
			clearTimeout(longPressTimer)
			longPressTimer = 0
		}
	}

	function detachWindow() {
		window.removeEventListener('pointermove', onPointerMove)
		window.removeEventListener('pointerup', onPointerUp)
		window.removeEventListener('pointercancel', onPointerUp)
	}

	function abandon() {
		// Release the gesture without committing (e.g. the finger went vertical → let the list scroll).
		clearLongPress()
		mode = 'idle'
		pressed = false
		detachWindow()
		pointerId = null
	}

	function settle(open: boolean) {
		mode = 'idle'
		dragging = false
		if (open) {
			revealPx = REVEAL_PX
			mobileUIStore.setOpenRow(release.id)
		} else {
			revealPx = 0
			if ($openRowId === release.id) mobileUIStore.setOpenRow(null)
		}
	}

	function onLongPress() {
		longPressTimer = 0
		if (mode !== 'pending') return
		void rigidTap()
		// Snapshot the row's viewport rect so the context menu can lift a preview of it in place. `revealPx`
		// is 0 here (long-press only arms from a closed row), so the foreground sits at its resting position.
		const r = foregroundEl?.getBoundingClientRect()
		const rect = r ? { top: r.top, left: r.left, width: r.width, height: r.height } : null
		mobileUIStore.openActionsSheet(release.id, context, rect)
		mode = 'idle'
		pressed = false
		detachWindow()
		pointerId = null
	}

	function onPointerDown(e: PointerEvent) {
		if (pointerId !== null) return
		if (e.pointerType === 'mouse' && e.button !== 0) return
		pointerId = e.pointerId
		startX = lastX = e.clientX
		startY = e.clientY
		lastT = e.timeStamp
		velocity = 0
		openAtStart = revealPx
		mode = 'pending'
		pressed = true
		// Long-press only makes sense from a closed row outside select mode (in select mode a tap toggles).
		clearLongPress()
		if (!isSelectMode && revealPx === 0) longPressTimer = window.setTimeout(onLongPress, LONG_PRESS_MS)
		window.addEventListener('pointermove', onPointerMove, { passive: false })
		window.addEventListener('pointerup', onPointerUp)
		window.addEventListener('pointercancel', onPointerUp)
	}

	function onPointerMove(e: PointerEvent) {
		if (e.pointerId !== pointerId) return
		const dx = e.clientX - startX
		const dy = e.clientY - startY

		if (mode === 'pending') {
			if (Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return
			// Any real movement ends the long-press window.
			clearLongPress()
			pressed = false
			// Vertical intent → hand the gesture back to the scroll container.
			if (Math.abs(dy) > Math.abs(dx)) {
				abandon()
				return
			}
			// Horizontal: swipe-to-delete is left-only, and disabled in select mode. From an already-open
			// row a rightward drag is allowed (it closes the row).
			if (isSelectMode) {
				abandon()
				return
			}
			if (openAtStart === 0 && dx >= 0) {
				abandon()
				return
			}
			mode = 'swipe'
			dragging = true
		}

		if (mode === 'swipe') {
			if (e.cancelable) e.preventDefault()
			const now = e.timeStamp
			if (now > lastT) velocity = (e.clientX - lastX) / (now - lastT)
			lastX = e.clientX
			lastT = now
			revealPx = clamp(openAtStart - dx, 0, REVEAL_PX)
		}
	}

	function onPointerUp(e: PointerEvent) {
		if (e.pointerId !== pointerId) return
		const dx = e.clientX - startX
		const dy = e.clientY - startY
		const moved = Math.abs(dx) >= DRAG_THRESHOLD || Math.abs(dy) >= DRAG_THRESHOLD
		const wasSwipe = mode === 'swipe'

		clearLongPress()
		detachWindow()
		pointerId = null
		pressed = false

		if (wasSwipe) {
			// Commit: a flick wins by direction; otherwise snap to whichever side we're past the midpoint of.
			if (velocity < -FLICK_VELOCITY) settle(true)
			else if (velocity > FLICK_VELOCITY) settle(false)
			else settle(revealPx > REVEAL_PX / 2)
			return
		}

		mode = 'idle'
		if (moved) return // a drag the FSM didn't claim (e.g. released to scroll) — not a tap

		// Stationary release → a tap.
		if (isSelectMode) {
			mobileUIStore.toggleReleaseSelected(release.id)
		} else if (revealPx > 0) {
			settle(false) // tapping an open row just closes it
		} else {
			void lightTap()
			mobileUIStore.openDetail(release.id)
		}
	}

	function onKeyDown(e: KeyboardEvent) {
		if (e.key !== 'Enter' && e.key !== ' ') return
		e.preventDefault()
		if (isSelectMode) mobileUIStore.toggleReleaseSelected(release.id)
		else mobileUIStore.openDetail(release.id)
	}

	async function confirmDelete() {
		const ok = await confirmDialog($translate('discovery.confirmDeleteMessage'), {
			title: $translate('discovery.confirmDeleteTitle', { values: { count: 1 } }),
			confirmLabel: $translate('common.delete'),
		})
		if (!ok) return
		settle(false)
		await discoveryStore.deleteRelease(release.id)
	}

	// Release-level "Add to queue": enqueue all of the release's tracks, in order (the granular per-track
	// actions live in the detail screen). Closes the swipe row and confirms with a toast (queue isn't on screen).
	function queueRelease() {
		settle(false)
		if (release.tracks.length === 0) return
		void lightTap()
		playbackQueue.addReleaseToQueue(release)
		toastStore.success($translate('queue.addedToQueue'))
	}
</script>

<div class="relative h-full overflow-hidden">
	<!-- Revealed-on-swipe Delete action, anchored to the right edge behind the foreground. Only mounted
	     while the row is open or being swiped (revealPx > 0): the foreground uses a *translucent* highlight
	     (bg-brand-muted) when the release is selected or the current preview, so a Delete button left painted
	     behind a closed row would bleed through that tint and look perpetually half-revealed. -->
	{#if revealPx > 0}
		<div class="absolute inset-y-0 right-0 flex">
			<button
				type="button"
				class="flex flex-col items-center justify-center gap-1 bg-brand-primary px-1 text-center text-[10px] leading-tight font-semibold text-white"
				style="width: {ACTION_W}px"
				aria-label={$translate('queue.addToQueue')}
				onclick={queueRelease}
			>
				<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M4 6h11M4 12h11M4 18h7M19 14v6M16 17h6" stroke-linecap="round" />
				</svg>
				{$translate('queue.addToQueue')}
			</button>
			<button
				type="button"
				class="flex flex-col items-center justify-center gap-1 bg-danger text-xs font-semibold text-white"
				style="width: {ACTION_W}px"
				aria-label={$translate('discovery.deleteRelease')}
				onclick={confirmDelete}
			>
				<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path
						d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m2 0v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
				{$translate('common.delete')}
			</button>
		</div>
	{/if}

	<!-- Foreground: the card itself, translated left to reveal the Delete action. Owns the pointer FSM.
	     `touch-pan-y` lets the browser keep handling vertical scroll until we claim a horizontal swipe. -->
	<div
		bind:this={foregroundEl}
		role="button"
		tabindex="0"
		aria-label={`${release.artist ?? $translate('common.unknownArtist')} — ${release.title ?? $translate('common.untitled')}`}
		aria-pressed={isSelectMode ? isSelected : undefined}
		class="relative flex h-full w-full touch-pan-y items-center gap-3 px-4 text-left {bgClass} {dragging
			? ''
			: 'transition-transform duration-200 ease-out'}"
		style="transform: translateX(-{revealPx}px)"
		onpointerdown={onPointerDown}
		onkeydown={onKeyDown}
	>
		{#if isSelectMode}
			<span
				class="flex h-5 w-5 flex-shrink-0 items-center justify-center rounded-full border {isSelected
					? 'border-brand-primary bg-brand-primary text-white'
					: 'border-stroke text-transparent'}"
			>
				<svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
					<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</span>
		{/if}

		<ReleaseCardContent {release} />

		{#if !isSelectMode}
			{#if isPreviewLoading}
				<Spinner class="h-4 w-4 flex-shrink-0 text-text-tertiary" />
			{:else}
				<svg
					class="h-4 w-4 flex-shrink-0 text-text-tertiary"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
				>
					<path d="M9 18l6-6-6-6" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			{/if}
		{/if}
	</div>
</div>
