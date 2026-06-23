<script lang="ts">
	import { translate } from '$shared/i18n'
	import { upNext, userQueueCount } from '$shared/stores/playbackQueue'
	import * as playbackQueue from '$shared/stores/playbackQueue'
	import { lightTap } from '$lib/utils/haptics'
	import MobileModal from '$lib/components/common/MobileModal.svelte'

	// "Up Next" bottom sheet: the explicit user queue (reorderable + removable) followed by a read-only
	// forecast of what the discovery-feed context will play next. Reads the two-tier model from
	// playbackQueue; mutations (remove / reorder / clear) flow straight back into it (and, on iOS,
	// re-feed the native window live). Opened from the full-screen player.
	type Props = {
		open: boolean
		onClose: () => void
	}
	let { open, onClose }: Props = $props()

	// Split the unified Up Next list by tier: user-added items are interactive, context items are a forecast.
	const userEntries = $derived($upNext.filter((e) => e.source === 'user'))
	const contextEntries = $derived($upNext.filter((e) => e.source === 'context'))

	function trackName(entry: { release: { tracks: { name: string }[]; title: string | null }; trackIndex: number }) {
		return entry.release.tracks[entry.trackIndex]?.name ?? entry.release.title ?? $translate('common.untitled')
	}

	function remove(entryId: string) {
		void lightTap()
		playbackQueue.removeEntry(entryId)
	}

	function clearAll() {
		void lightTap()
		playbackQueue.clearUserQueue()
	}

	// --- Drag-to-reorder (user queue only) ----------------------------------------------------------
	// A fixed row height makes the index math simple: as the dragged handle crosses a row boundary we
	// reorder one step live (matching iOS) and re-anchor, so the list shuffles under the finger. The
	// handle owns the pointer (capture + touch-action:none) so the sheet body keeps scrolling elsewhere.
	const ROW_H = 56
	let draggingId = $state<string | null>(null)
	let dragAnchorY = 0

	function onHandleDown(e: PointerEvent, entryId: string) {
		e.preventDefault()
		;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
		draggingId = entryId
		dragAnchorY = e.clientY
		void lightTap()
	}

	function onHandleMove(e: PointerEvent) {
		if (draggingId == null) return
		const delta = Math.round((e.clientY - dragAnchorY) / ROW_H)
		if (delta === 0) return
		const cur = userEntries.findIndex((x) => x.key === draggingId)
		if (cur === -1) return
		const target = Math.max(0, Math.min(userEntries.length - 1, cur + delta))
		if (target !== cur) {
			playbackQueue.moveEntry(draggingId, target)
			dragAnchorY = e.clientY
		}
	}

	function onHandleUp() {
		if (draggingId == null) return
		draggingId = null
		void lightTap()
	}
</script>

<MobileModal {open} {onClose} title={$translate('queue.upNext')}>
	{#snippet headerAction()}
		{#if $userQueueCount > 0}
			<button
				type="button"
				class="rounded-md px-2 py-1 text-sm font-medium text-brand-primary active:bg-surface-2"
				onclick={clearAll}
			>
				{$translate('queue.clearQueue')}
			</button>
		{/if}
	{/snippet}

	{#if userEntries.length === 0 && contextEntries.length === 0}
		<div class="flex flex-col items-center justify-center gap-1 py-10 text-center">
			<p class="text-sm font-medium text-text-secondary">{$translate('queue.empty')}</p>
			<p class="text-xs text-text-tertiary">{$translate('queue.emptyHint')}</p>
		</div>
	{:else}
		<!-- User queue: reorderable + removable -->
		{#if userEntries.length > 0}
			<h3 class="mb-1 px-1 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
				{$translate('queue.nextInQueue')}
			</h3>
			<div class="mb-4 flex flex-col">
				{#each userEntries as entry (entry.key)}
					<div
						class="flex items-center gap-2 rounded {draggingId === entry.key ? 'bg-surface-2' : ''}"
						style="height: {ROW_H}px"
					>
						<!-- Drag handle (owns the reorder gesture) -->
						<button
							type="button"
							class="flex h-11 w-8 flex-shrink-0 touch-none items-center justify-center text-text-tertiary active:text-text-primary"
							aria-label={$translate('queue.reorder')}
							onpointerdown={(e) => onHandleDown(e, entry.key)}
							onpointermove={onHandleMove}
							onpointerup={onHandleUp}
							onpointercancel={onHandleUp}
						>
							<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M4 8h16M4 16h16" stroke-linecap="round" />
							</svg>
						</button>

						{#if entry.release.artwork_url}
							<img src={entry.release.artwork_url} alt="" class="h-10 w-10 flex-shrink-0 rounded object-cover" />
						{:else}
							<div class="h-10 w-10 flex-shrink-0 rounded bg-surface-2"></div>
						{/if}

						<div class="flex min-w-0 flex-1 flex-col leading-tight">
							<span class="truncate text-sm text-text-primary">{trackName(entry)}</span>
							<span class="truncate text-xs text-text-tertiary">
								{entry.release.artist ?? $translate('common.unknownArtist')}
							</span>
						</div>

						<button
							type="button"
							class="flex h-11 w-9 flex-shrink-0 items-center justify-center text-text-tertiary active:text-text-primary"
							aria-label={$translate('queue.removeFromQueue')}
							onclick={() => remove(entry.key)}
						>
							<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M6 6l12 12M18 6L6 18" stroke-linecap="round" />
							</svg>
						</button>
					</div>
				{/each}
			</div>
		{/if}

		<!-- Context forecast: read-only -->
		{#if contextEntries.length > 0}
			<h3 class="mb-1 px-1 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
				{$translate('queue.upNextFromContext')}
			</h3>
			<div class="flex flex-col">
				{#each contextEntries as entry (entry.key)}
					<div class="flex items-center gap-2 py-1.5">
						{#if entry.release.artwork_url}
							<img src={entry.release.artwork_url} alt="" class="h-10 w-10 flex-shrink-0 rounded object-cover" />
						{:else}
							<div class="h-10 w-10 flex-shrink-0 rounded bg-surface-2"></div>
						{/if}
						<div class="flex min-w-0 flex-1 flex-col leading-tight">
							<span class="truncate text-sm text-text-secondary">{trackName(entry)}</span>
							<span class="truncate text-xs text-text-tertiary">
								{entry.release.artist ?? $translate('common.unknownArtist')}
							</span>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	{/if}
</MobileModal>
