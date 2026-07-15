<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease } from '$shared/types'
	import { upNext, userQueueCount, recentlyPlayed } from '$shared/stores/playbackQueue'
	import * as playbackQueue from '$shared/stores/playbackQueue'
	import { discoveryStore } from '$shared/stores/discovery'
	import { playerStore } from '$shared/stores/player'
	import { formatRelativeDate } from '$shared/utils'
	import { mobileUIStore } from '$lib/stores/mobileUI'
	import { lightTap } from '$lib/utils/haptics'
	import MobileModal from '$lib/components/common/MobileModal.svelte'
	import ReleaseArtwork from '$lib/components/common/ReleaseArtwork.svelte'

	// "Up Next" bottom sheet with two segments: Upcoming — the explicit user queue (reorderable +
	// removable) followed by a read-only forecast of what the context will play next — and History,
	// the persistent recently-played log (tap an entry to play it again). Reads the two-tier model
	// from playbackQueue; mutations flow straight back into it (and, on iOS, re-feed the native
	// window live). Opened from the full-screen player.
	type Props = {
		open: boolean
		onClose: () => void
	}
	let { open, onClose }: Props = $props()

	let tab = $state<'upcoming' | 'history'>('upcoming')

	// Split the unified Up Next list by tier: user-added items are interactive, context items are a forecast.
	const userEntries = $derived($upNext.filter((e) => e.source === 'user'))
	const contextEntries = $derived($upNext.filter((e) => e.source === 'context'))

	// History entries persist ids only; resolve them against the loaded discovery set, silently
	// dropping releases that were deleted (or whose track list shrank) since. Newest first.
	const byId = $derived(new Map($discoveryStore.releases.map((r) => [r.id, r])))
	const historyRows = $derived.by(() => {
		const rows: Array<{ key: string; release: DiscoveryRelease; trackIndex: number; at: number }> = []
		const list = $recentlyPlayed
		for (let i = list.length - 1; i >= 0; i--) {
			const e = list[i]
			const release = byId.get(e.releaseId)
			if (!release || e.trackIndex >= release.tracks.length) continue
			rows.push({ key: `${e.releaseId}:${e.trackIndex}:${e.at}`, release, trackIndex: e.trackIndex, at: e.at })
		}
		return rows
	})

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

	function clearHistory() {
		void lightTap()
		playbackQueue.clearRecentlyPlayed()
	}

	// Play a history entry again: a fresh single-release session (the restorePreview semantics), so
	// next/shuffle scope to that release; the null queue origin keeps the feed's re-scope effect off.
	// The sheet stays open — the mini/expanded player updating in place is the feedback.
	function playAgain(release: DiscoveryRelease, trackIndex: number) {
		void lightTap()
		mobileUIStore.setQueueOrigin(null)
		void playerStore.playPreview(release, trackIndex, [release])
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
		{#if tab === 'upcoming' && $userQueueCount > 0}
			<button
				type="button"
				class="rounded-md px-2 py-1 text-sm font-medium text-brand-primary active:bg-surface-2"
				onclick={clearAll}
			>
				{$translate('queue.clearQueue')}
			</button>
		{:else if tab === 'history' && historyRows.length > 0}
			<button
				type="button"
				class="rounded-md px-2 py-1 text-sm font-medium text-brand-primary active:bg-surface-2"
				onclick={clearHistory}
			>
				{$translate('queue.clearQueue')}
			</button>
		{/if}
	{/snippet}

	<!-- Segment switch: Upcoming (queue + forecast) | History (the persistent listening log). -->
	<div class="mb-3 grid grid-cols-2 rounded-full border border-stroke bg-surface-2 p-0.5 text-xs font-medium">
		<button
			type="button"
			class="rounded-full px-3 py-1.5 text-center transition-colors {tab === 'upcoming'
				? 'bg-brand-primary text-white'
				: 'text-text-tertiary'}"
			aria-pressed={tab === 'upcoming'}
			onclick={() => (tab = 'upcoming')}
		>
			{$translate('queue.upcoming')}
		</button>
		<button
			type="button"
			class="rounded-full px-3 py-1.5 text-center transition-colors {tab === 'history'
				? 'bg-brand-primary text-white'
				: 'text-text-tertiary'}"
			aria-pressed={tab === 'history'}
			onclick={() => (tab = 'history')}
		>
			{$translate('queue.history')}
		</button>
	</div>

	{#if tab === 'upcoming'}
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

							<ReleaseArtwork release={entry.release} class="h-10 w-10 flex-shrink-0 rounded object-cover" />

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
							<ReleaseArtwork release={entry.release} class="h-10 w-10 flex-shrink-0 rounded object-cover" />
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
	{:else if historyRows.length === 0}
		<div class="py-10 text-center">
			<p class="text-sm font-medium text-text-secondary">{$translate('queue.historyEmpty')}</p>
		</div>
	{:else}
		<div class="flex flex-col">
			{#each historyRows as row (row.key)}
				<button
					type="button"
					class="flex items-center gap-2 rounded py-1.5 text-left active:bg-surface-2"
					onclick={() => playAgain(row.release, row.trackIndex)}
				>
					<ReleaseArtwork release={row.release} class="h-10 w-10 flex-shrink-0 rounded object-cover" />
					<div class="flex min-w-0 flex-1 flex-col leading-tight">
						<span class="truncate text-sm text-text-primary">{trackName(row)}</span>
						<span class="truncate text-xs text-text-tertiary">
							{row.release.artist ?? $translate('common.unknownArtist')}
						</span>
					</div>
					<span class="flex-shrink-0 text-xs text-text-tertiary">
						{formatRelativeDate(new Date(row.at).toISOString(), $translate)}
					</span>
				</button>
			{/each}
		</div>
	{/if}
</MobileModal>
