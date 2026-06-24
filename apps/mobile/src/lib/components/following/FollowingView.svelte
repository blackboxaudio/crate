<script lang="ts">
	import { onMount } from 'svelte'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { translate } from '$shared/i18n'
	import type { FollowedSource } from '$shared/types'
	import { followStore, sortedFollowedSources } from '$shared/stores/follow'
	import { formatRelativeDate } from '$shared/utils'
	import { confirmDialog } from '$lib/utils/dialog'
	import { lightTap, rigidTap } from '$lib/utils/haptics'
	import { mobileUIStore } from '$lib/stores/mobileUI'
	import MobileList from '$lib/components/common/MobileList.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'
	import MobilePromptDialog from '$lib/components/common/MobilePromptDialog.svelte'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import ContextMenuItem from '$lib/components/common/ContextMenuItem.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'

	// The Following tab: a management roster of the artists/labels the user follows. Mirrors the desktop
	// Following manager (list + add-by-URL + check + unfollow) over the same shared `followStore` / Tauri IPC.
	// New releases from follows still surface in the Discovery feed (flagged new) — this tab is the roster, not
	// a second feed. Tap a row to drill into its releases; long-press for check / open / unfollow. Reloads on every
	// mount (the shell remounts tab views on switch via `{#key activeTab}`), so it's fresh each time it's opened.
	onMount(() => {
		followStore.load()
	})

	const sources = $derived($sortedFollowedSources)
	const hasSources = $derived(sources.length > 0)

	function domain(url: string): string {
		try {
			return new URL(url).host
		} catch {
			return url
		}
	}

	async function checkAll() {
		void lightTap()
		await followStore.checkAll()
	}

	function openSource(source: FollowedSource) {
		void lightTap()
		void openUrl(source.url).catch(() => {})
	}

	// Drill into a followed source: open its detail overlay (a feed of the releases from this artist/label).
	function openSourceDetail(source: FollowedSource) {
		void lightTap()
		mobileUIStore.openFollowSource(source.id)
	}

	// --- Add a source by URL (paste an artist/label page) -----------------------------------------------
	// Uses the centered prompt (not a bottom sheet) so the field stays above the iOS keyboard. Kept open
	// during the network scan (`addBusy` disables the confirm) so a failed URL keeps the user's input.
	let addOpen = $state(false)
	let addUrl = $state('')
	let addBusy = $state(false)

	function openAdd() {
		addUrl = ''
		addBusy = false
		addOpen = true
	}

	async function submitAdd() {
		const trimmed = addUrl.trim()
		if (!trimmed || addBusy) return
		addBusy = true
		const source = await followStore.followFromUrl(trimmed)
		addBusy = false
		if (source) {
			addUrl = ''
			addOpen = false
		}
	}

	// --- Long-press row actions (mirrors PlaylistsView) -------------------------------------------------
	let longPressTimer = 0
	let actionsOpen = $state(false)
	let actionTarget = $state<FollowedSource | null>(null)
	let longPressRect = $state<{ top: number; left: number; width: number; height: number } | null>(null)
	// A stationary long-press also synthesizes a click on release; latch so we can swallow that one click
	// (else opening the menu would also fire the row's openSource — MobileListItem is a real <button>).
	let suppressNextClick = false

	function startLongPress(e: PointerEvent, source: FollowedSource) {
		suppressNextClick = false
		if (longPressTimer) clearTimeout(longPressTimer)
		const el = e.currentTarget as HTMLElement
		longPressTimer = window.setTimeout(() => {
			longPressTimer = 0
			const r = el?.getBoundingClientRect()
			longPressRect = r ? { top: r.top, left: r.left, width: r.width, height: r.height } : null
			suppressNextClick = true
			void rigidTap()
			actionTarget = source
			actionsOpen = true
		}, 450)
		window.addEventListener('pointermove', cancelLongPress, { once: true, passive: true })
		window.addEventListener('pointerup', cancelLongPress, { once: true })
		window.addEventListener('pointercancel', cancelLongPress, { once: true })
	}

	function cancelLongPress() {
		if (longPressTimer) {
			clearTimeout(longPressTimer)
			longPressTimer = 0
		}
	}

	function onRowClickCapture(e: MouseEvent) {
		if (!suppressNextClick) return
		suppressNextClick = false
		e.preventDefault()
		e.stopPropagation()
	}

	function checkOne(source: FollowedSource) {
		actionsOpen = false
		void followStore.check(source.id)
	}

	async function unfollow(source: FollowedSource) {
		actionsOpen = false
		// Reuse the desktop copy that already explains unfollowing keeps existing releases, so it reads as
		// the dialog body rather than a bare "Are you sure?".
		const ok = await confirmDialog($translate('discovery.following.footerNote'), {
			title: $translate('discovery.following.unfollow'),
			confirmLabel: $translate('discovery.following.unfollow'),
		})
		if (!ok) return
		await followStore.unfollow(source.id)
	}
</script>

{#snippet avatar(source: FollowedSource)}
	{#if source.artworkUrl}
		<img src={source.artworkUrl} alt="" class="h-11 w-11 flex-shrink-0 rounded object-cover" loading="lazy" />
	{:else}
		<div class="flex h-11 w-11 flex-shrink-0 items-center justify-center rounded bg-surface-2 text-text-tertiary">
			{#if source.followType === 'label'}
				<!-- disc — a label release -->
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<circle cx="12" cy="12" r="9" />
					<circle cx="12" cy="12" r="2.5" />
				</svg>
			{:else}
				<!-- user — an artist -->
				<svg
					class="h-5 w-5"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				>
					<circle cx="12" cy="8" r="4" />
					<path d="M4 20c0-4 4-6 8-6s8 2 8 6" />
				</svg>
			{/if}
		</div>
	{/if}
{/snippet}

{#snippet status(source: FollowedSource, checking: boolean)}
	{#if checking}
		<Spinner class="h-4 w-4" />
	{:else if source.health === 'error' || source.health === 'rate_limited'}
		<span class="inline-flex items-center gap-1 text-[11px] text-amber-500">
			<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<circle cx="12" cy="12" r="9" />
				<path d="M12 8v4M12 16h.01" stroke-linecap="round" />
			</svg>
			{$translate('discovery.following.sourceError')}
		</span>
	{:else if source.newCount > 0}
		<span class="rounded-full bg-brand-muted px-1.5 py-0.5 text-[10px] font-semibold text-brand-primary">
			{$translate('discovery.following.newCount', { values: { count: source.newCount } })}
		</span>
	{:else}
		<span class="text-[10px] font-semibold tracking-wide text-text-tertiary uppercase">
			{$translate('discovery.following.upToDate')}
		</span>
	{/if}
{/snippet}

<div class="flex h-full flex-col">
	<!-- Action row: Check all (left) + Add source (right). Hidden when empty — the empty state has its own CTA. -->
	{#if hasSources}
		<div class="flex items-center justify-between gap-1 px-2 py-2">
			<button
				type="button"
				class="flex h-10 items-center gap-1.5 rounded-md px-3 text-sm font-medium text-text-secondary active:bg-surface-2 disabled:opacity-50"
				disabled={$followStore.checkingAll}
				onclick={checkAll}
			>
				{#if $followStore.checkingAll}
					<Spinner class="h-3.5 w-3.5" />
				{/if}
				{$translate('discovery.following.checkAll')}
			</button>
			<button
				type="button"
				class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-md text-text-secondary active:bg-surface-2"
				aria-label={$translate('discovery.following.followSource')}
				onclick={openAdd}
			>
				<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 5v14M5 12h14" stroke-linecap="round" />
				</svg>
			</button>
		</div>
	{/if}

	<!-- Scroll container (the shell frame reserves header/tab-bar padding; this owns its own scroll). -->
	<div class="min-h-0 flex-1 overflow-y-auto" style="padding-bottom: var(--mini-player-inset, 0px)">
		<MobileList isEmpty={!hasSources} empty={emptyState}>
			{#each sources as source (source.id)}
				{@const checking = $followStore.checkingIds.has(source.id) || $followStore.checkingAll}
				<div onpointerdown={(e) => startLongPress(e, source)} onclickcapture={onRowClickCapture}>
					<MobileListItem onclick={() => openSourceDetail(source)}>
						{#snippet leading()}
							{@render avatar(source)}
						{/snippet}
						{#snippet trailing()}
							{@render status(source, checking)}
						{/snippet}
						<span class="block truncate text-sm font-medium text-text-primary">{source.name ?? domain(source.url)}</span
						>
						<span class="block truncate text-xs text-text-tertiary">
							{domain(source.url)}{source.lastCheckedAt
								? ` · ${$translate('discovery.following.checkedAgo', { values: { time: formatRelativeDate(source.lastCheckedAt, $translate) } })}`
								: ''}
						</span>
					</MobileListItem>
				</div>
			{/each}
		</MobileList>
	</div>
</div>

{#snippet emptyState()}
	<div class="flex flex-col items-center gap-2 py-10 text-center">
		<svg
			class="h-8 w-8 text-text-tertiary"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
		>
			<path d="M5 12a7 7 0 0 1 7 7" />
			<path d="M5 5a14 14 0 0 1 14 14" />
			<circle cx="5.5" cy="18.5" r="1.5" fill="currentColor" stroke="none" />
		</svg>
		<div class="text-sm font-medium text-text-primary">{$translate('discovery.following.empty.title')}</div>
		<div class="max-w-xs text-xs text-text-tertiary">{$translate('discovery.following.empty.hint')}</div>
		<button
			type="button"
			class="mt-2 rounded-lg bg-brand-muted px-4 py-2 text-sm font-medium text-brand-primary active:opacity-80"
			onclick={openAdd}
		>
			{$translate('discovery.following.followSource')}
		</button>
	</div>
{/snippet}

<!-- Add-source prompt: paste an artist/label page URL. -->
<MobilePromptDialog
	open={addOpen}
	bind:value={addUrl}
	title={$translate('discovery.following.addSource.title')}
	message={$translate('discovery.following.addSource.urlInfo')}
	placeholder={$translate('discovery.following.addSource.urlPlaceholder')}
	confirmLabel={$translate('discovery.following.follow')}
	confirmDisabled={!addUrl.trim() || addBusy}
	onConfirm={submitAdd}
	onCancel={() => (addOpen = false)}
/>

<!-- Row long-press menu: check / open / unfollow. -->
<ContextMenu
	open={actionsOpen}
	anchorRect={longPressRect}
	onClose={() => (actionsOpen = false)}
	onClosed={() => {
		actionTarget = null
		longPressRect = null
	}}
>
	{#snippet preview()}
		{#if actionTarget}
			{@render avatar(actionTarget)}
			<span class="min-w-0 flex-1 truncate text-sm text-text-primary">
				{actionTarget.name ?? domain(actionTarget.url)}
			</span>
		{/if}
	{/snippet}

	<ContextMenuItem onclick={() => actionTarget && checkOne(actionTarget)}>
		{$translate('discovery.following.checkNow')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<polyline points="23 4 23 10 17 10" stroke-linecap="round" stroke-linejoin="round" />
				<polyline points="1 20 1 14 7 14" stroke-linecap="round" stroke-linejoin="round" />
				<path
					d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={() => actionTarget && openSource(actionTarget)}>
		{$translate('discovery.openInBrowser')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path
					d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
				<polyline points="15 3 21 3 21 9" stroke-linecap="round" stroke-linejoin="round" />
				<line x1="10" y1="14" x2="21" y2="3" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem destructive onclick={() => actionTarget && unfollow(actionTarget)}>
		{$translate('discovery.following.unfollow')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M18 6L6 18M6 6l12 12" stroke-linecap="round" />
			</svg>
		{/snippet}
	</ContextMenuItem>
</ContextMenu>
