<script lang="ts">
	import { fly } from 'svelte/transition'
	import type { DiscoveryRelease, UpNextEntry } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { upNext, userQueueCount, recentlyPlayed } from '$shared/stores/playbackQueue'
	import * as playbackQueue from '$shared/stores/playbackQueue'
	import { discoveryStore } from '$shared/stores/discovery'
	import { formatRelativeDate, getTrackDisplayArtist, getTrackDisplayName } from '$shared/utils'
	import { pageActions } from '$lib/stores/pageActions'
	import { AlbumArt, Button, Icon, IconButton, Tooltip } from '$lib/components/common'

	// Desktop's queue panel — the counterpart of mobile's UpNextSheet, hung from the player bar
	// instead of a bottom sheet. Two segments: Queue (the explicit user queue, reorderable +
	// removable, followed by a read-only forecast of what the context plays next) and History (the
	// persistent recently-played log; click an entry to play it again). Reads the shared two-tier
	// queue; mutations flow straight back into it. Dismissed by a click outside (context menus
	// excepted, so queuing from one keeps it open and the row appearing is the feedback).
	type Props = {
		/** Offset from the player bar's right edge that right-aligns the panel to its toggle button. */
		right?: number
		/** The button that opened the panel: its clicks are the caller's toggle, not "outside". */
		trigger?: HTMLElement
		onClose: () => void
	}
	let { right = 16, trigger, onClose }: Props = $props()

	let panelEl: HTMLDivElement | undefined = $state()

	// Click-outside dismissal, primary button only: a right-click that opens a release's context
	// menu, and the click on its "Add to Queue" item (which lives in a separate fixed-position
	// `role="menu"` subtree), both leave the panel open, so the queued row can be seen landing.
	function handleWindowPointerDown(e: PointerEvent) {
		if (e.button !== 0) return
		const target = e.target as Node
		if (panelEl?.contains(target) || trigger?.contains(target)) return
		if (target instanceof Element && target.closest('[role="menu"]')) return
		onClose()
	}

	let tab = $state<'upcoming' | 'history'>('upcoming')

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
			if (!release) continue
			// Entries logged from a playlist carry a member-list index; the id resolves against the feed's copy.
			const byTrackId = e.trackId ? release.tracks.findIndex((t) => t.id === e.trackId) : -1
			const trackIndex = byTrackId >= 0 ? byTrackId : e.trackIndex
			if (trackIndex >= release.tracks.length) continue
			rows.push({ key: `${e.releaseId}:${trackIndex}:${e.at}`, release, trackIndex, at: e.at })
		}
		return rows
	})

	function entryTitle(entry: UpNextEntry): string {
		if (entry.kind === 'library') return getTrackDisplayName(entry.track)
		return entry.release.tracks[entry.trackIndex]?.name ?? entry.release.title ?? $translate('common.untitled')
	}

	function entrySubtitle(entry: UpNextEntry): string {
		if (entry.kind === 'library') return getTrackDisplayArtist(entry.track)
		return entry.release.artist ?? $translate('common.unknownArtist')
	}

	function playAgain(release: DiscoveryRelease, trackIndex: number) {
		$pageActions?.playPreview(release, trackIndex)
	}

	// --- Drag-to-reorder (user queue only) ----------------------------------------------------------
	// Fixed row height keeps the index math simple: as the dragged handle crosses a row boundary the
	// entry moves one step live and the anchor re-bases, so the list shuffles under the pointer. The
	// handle owns the pointer (capture) so the rest of the panel keeps scrolling normally.
	const ROW_H = 44
	let draggingId = $state<string | null>(null)
	let dragAnchorY = 0

	function onHandleDown(e: PointerEvent, entryId: string) {
		e.preventDefault()
		;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
		draggingId = entryId
		dragAnchorY = e.clientY
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
		draggingId = null
	}
</script>

<!-- Fixed size: the two tabs hold different row counts, and a panel that resized (or a header that
     reflowed as Clear came and went) made every tab switch jump. The height fits the header, a
     section heading and four 44px rows; anything longer scrolls. -->
<svelte:window onpointerdown={handleWindowPointerDown} />

<div
	bind:this={panelEl}
	class="absolute bottom-full z-30 mb-2 flex h-[260px] w-[360px] flex-col overflow-hidden rounded-md border border-stroke bg-surface-1 shadow-xl"
	style="right: {right}px"
	role="region"
	aria-label={$translate('queue.openQueue')}
	transition:fly={{ y: 8, duration: 150 }}
>
	<!-- Header: the same segmented control as the Discovery | Library switcher in the app header (a
	     context switch, not a setting toggle), plus Clear — always present, disabled when empty, so the
	     header never reflows. Closing is the player-bar button / ⌘U, like the editor sidebar. -->
	<div class="flex items-center border-b border-stroke px-3 py-2">
		<div class="relative inline-grid grid-cols-2 items-center rounded-lg bg-surface-2 p-0.5">
			<div
				class="absolute top-0.5 bottom-0.5 left-0.5 w-[calc(50%-2px)] rounded-md bg-surface-0 shadow-sm transition-transform duration-200 ease-out motion-reduce:transition-none"
				style="transform: translateX({tab === 'history' ? '100%' : '0%'})"
			></div>
			<button
				type="button"
				class="relative z-10 rounded-md px-3 py-1 text-center text-xs font-medium transition-colors {tab === 'upcoming'
					? 'text-text-primary'
					: 'text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
				onclick={() => (tab = 'upcoming')}
			>
				{$translate('queue.upcoming')}
			</button>
			<button
				type="button"
				class="relative z-10 rounded-md px-3 py-1 text-center text-xs font-medium transition-colors {tab === 'history'
					? 'text-text-primary'
					: 'text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
				onclick={() => (tab = 'history')}
			>
				{$translate('queue.history')}
			</button>
		</div>

		<Button
			variant="ghost-danger"
			size="sm"
			class="ml-auto"
			disabled={tab === 'upcoming' ? $userQueueCount === 0 : historyRows.length === 0}
			onclick={() => (tab === 'upcoming' ? playbackQueue.clearUserQueue() : playbackQueue.clearRecentlyPlayed())}
		>
			{$translate('queue.clearQueue')}
		</Button>
	</div>

	<div class="min-h-0 flex-1 overflow-y-auto p-2">
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
					<div class="mb-3 flex flex-col">
						{#each userEntries as entry (entry.key)}
							<div
								class="group flex items-center gap-2 rounded pr-1 {draggingId === entry.key
									? 'bg-surface-2'
									: 'hover:bg-surface-2'}"
								style="height: {ROW_H}px"
							>
								<button
									type="button"
									class="flex h-full w-6 flex-shrink-0 touch-none items-center justify-center text-text-tertiary hover:cursor-grab hover:text-text-secondary active:cursor-grabbing"
									aria-label={$translate('queue.reorder')}
									onpointerdown={(e) => onHandleDown(e, entry.key)}
									onpointermove={onHandleMove}
									onpointerup={onHandleUp}
									onpointercancel={onHandleUp}
								>
									<Icon name="grid" class="h-3.5 w-3.5" />
								</button>

								{#if entry.kind === 'preview'}
									<AlbumArt artworkPath={entry.release.artwork_path} artworkUrl={entry.release.artwork_url} size="sm" />
								{:else}
									<AlbumArt artworkPath={entry.track.artwork_path} size="sm" />
								{/if}

								<div class="flex min-w-0 flex-1 flex-col leading-tight">
									<span class="truncate text-sm text-text-primary">{entryTitle(entry)}</span>
									<span class="truncate text-xs text-text-tertiary">{entrySubtitle(entry)}</span>
								</div>

								<Tooltip text={$translate('queue.removeFromQueue')} position="left" delay={250}>
									<IconButton
										size="sm"
										icon="x"
										iconClass="h-3.5 w-3.5"
										class="opacity-0 group-hover:opacity-100"
										onclick={() => playbackQueue.removeEntry(entry.key)}
									/>
								</Tooltip>
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
							<div class="flex items-center gap-2 py-1 pl-1">
								{#if entry.kind === 'preview'}
									<AlbumArt artworkPath={entry.release.artwork_path} artworkUrl={entry.release.artwork_url} size="sm" />
								{:else}
									<AlbumArt artworkPath={entry.track.artwork_path} size="sm" />
								{/if}
								<div class="flex min-w-0 flex-1 flex-col leading-tight">
									<span class="truncate text-sm text-text-secondary">{entryTitle(entry)}</span>
									<span class="truncate text-xs text-text-tertiary">{entrySubtitle(entry)}</span>
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
						class="flex items-center gap-2 rounded px-1 py-1 text-left transition-colors hover:cursor-pointer hover:bg-surface-2"
						onclick={() => playAgain(row.release, row.trackIndex)}
					>
						<AlbumArt artworkPath={row.release.artwork_path} artworkUrl={row.release.artwork_url} size="sm" />
						<div class="flex min-w-0 flex-1 flex-col leading-tight">
							<span class="truncate text-sm text-text-primary">
								{row.release.tracks[row.trackIndex]?.name ?? row.release.title ?? $translate('common.untitled')}
							</span>
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
	</div>
</div>
