<script lang="ts">
	import type { ScannedPage, BulkImportProgress, BulkImportResult } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { discoveryStore } from '$shared/stores/discovery'
	import { listen } from '@tauri-apps/api/event'
	import { onMount } from 'svelte'
	import * as discoveryApi from '$shared/api/discovery'
	import { SvelteSet } from 'svelte/reactivity'
	import ArtworkPlaceholder from '$lib/components/common/ArtworkPlaceholder.svelte'

	type Props = {
		scannedPage: ScannedPage
		onImportComplete: (result: BulkImportResult) => void
		onCancel: () => void
	}
	let { scannedPage, onImportComplete, onCancel }: Props = $props()

	let selectedUrls = $state(new Set<string>())
	// Scanned covers whose URL failed to load — show the placeholder, not a broken image.
	let failedArtworkUrls = $state<ReadonlySet<string>>(new Set())
	let importing = $state(false)
	let progress = $state<BulkImportProgress | null>(null)
	let result = $state<BulkImportResult | null>(null)
	let unlisten: (() => void) | null = null

	$effect(() => {
		const initial = new SvelteSet<string>()
		for (const release of scannedPage.releases) {
			if (!release.already_exists) initial.add(release.url)
		}
		selectedUrls = initial
	})

	let selectableCount = $derived(scannedPage.releases.filter((r) => !r.already_exists).length)
	let selectedCount = $derived(selectedUrls.size)
	const allSelected = $derived(selectedCount === selectableCount && selectableCount > 0)

	onMount(() => {
		listen<BulkImportProgress>('bulk-import-progress', (event) => {
			progress = event.payload
		}).then((fn) => {
			unlisten = fn
		})
		return () => {
			unlisten?.()
			if (importing) discoveryApi.cancelBulkImport()
		}
	})

	function toggleUrl(url: string) {
		const next = new SvelteSet(selectedUrls)
		if (next.has(url)) next.delete(url)
		else next.add(url)
		selectedUrls = next
	}

	function toggleAll() {
		if (allSelected) {
			selectedUrls = new Set()
		} else {
			const next = new SvelteSet<string>()
			for (const release of scannedPage.releases) {
				if (!release.already_exists) next.add(release.url)
			}
			selectedUrls = next
		}
	}

	async function handleImport() {
		if (selectedCount === 0) return
		importing = true
		progress = null
		result = null

		try {
			const selectedReleases = scannedPage.releases.filter((r) => selectedUrls.has(r.url))
			const importResult = await discoveryApi.bulkCreateReleases(
				[...selectedUrls],
				scannedPage.page_label,
				scannedPage.page_artist,
				selectedReleases,
				scannedPage.source_type,
				scannedPage.page_url
			)
			result = importResult
			await discoveryStore.loadReleases()
			onImportComplete(importResult)
		} catch {
			const lastProgress = progress as BulkImportProgress | null
			if (lastProgress) {
				result = { succeeded: lastProgress.succeeded, failed: lastProgress.failed, failed_urls: [] }
			}
		} finally {
			importing = false
		}
	}

	async function handleCancel() {
		if (importing) await discoveryApi.cancelBulkImport()
		onCancel()
	}

	const progressFraction = $derived(progress ? progress.current / Math.max(progress.total, 1) : 0)
</script>

<!-- Fills the form sheet's scroll body (min-h-full) so the action bar can pin to the sheet's bottom
     edge via sticky — the list scrolls behind it, and short result sets don't leave the buttons
     floating mid-screen. -->
<div class="flex min-h-full flex-col">
	<!-- Header: summary + toggle-all in a clean row -->
	<div class="flex items-center justify-between px-4 pt-4 pb-1">
		<div>
			<p class="text-sm font-medium text-text-primary">
				{$translate('discovery.bulkImport.releasesFound', { values: { total: scannedPage.total_found } })}
			</p>
			{#if scannedPage.already_in_discovery > 0}
				<p class="mt-0.5 text-xs text-text-tertiary">
					{$translate('discovery.bulkImport.alreadyInDiscovery', {
						values: { count: scannedPage.already_in_discovery },
					})}
				</p>
			{/if}
		</div>
		{#if !result && selectableCount > 0}
			<button
				type="button"
				class="rounded-md px-3 py-1.5 text-xs font-semibold text-brand-primary active:bg-surface-2 disabled:opacity-40"
				disabled={importing}
				onclick={toggleAll}
			>
				{allSelected ? $translate('discovery.bulkImport.deselectAll') : $translate('discovery.bulkImport.selectAll')}
			</button>
		{/if}
	</div>

	<!-- Release list: no inner border/container — blends into the modal's own scroll. Rows are tall enough
	     for comfortable tapping (64px min) with 48px artwork. -->
	{#if scannedPage.releases.length === 0}
		<div class="flex flex-1 items-center justify-center py-12">
			<p class="text-sm text-text-tertiary">{$translate('discovery.bulkImport.noReleasesFound')}</p>
		</div>
	{:else}
		<div class="flex flex-1 flex-col px-2 py-1">
			{#each scannedPage.releases as release (release.url)}
				{@const isSelected = selectedUrls.has(release.url)}
				{@const alreadyExists = release.already_exists}
				{@const disabled = alreadyExists || importing}
				<button
					type="button"
					class="flex min-h-[64px] w-full items-center gap-3 rounded-lg px-2 py-2 text-left
						{disabled ? '' : 'active:bg-surface-2'} {importing && !alreadyExists ? 'opacity-50' : ''}"
					{disabled}
					onclick={() => toggleUrl(release.url)}
				>
					<!-- Checkbox; already-imported rows get a muted check instead of a dead empty box -->
					{#if alreadyExists}
						<div class="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-md bg-surface-2">
							<svg
								class="h-3.5 w-3.5 text-text-tertiary"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="3"
							>
								<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						</div>
					{:else}
						<div
							class="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-md transition-colors
							{isSelected ? 'bg-brand-primary' : 'border-2 border-stroke'}"
						>
							{#if isSelected}
								<svg
									class="h-3.5 w-3.5 text-white"
									viewBox="0 0 24 24"
									fill="none"
									stroke="currentColor"
									stroke-width="3"
								>
									<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round" />
								</svg>
							{/if}
						</div>
					{/if}

					<!-- Artwork -->
					{#if release.artwork_url && !failedArtworkUrls.has(release.artwork_url)}
						<img
							src={release.artwork_url}
							alt=""
							class="h-12 w-12 flex-shrink-0 rounded-md object-cover {alreadyExists ? 'opacity-50' : ''}"
							loading="lazy"
							decoding="async"
							onerror={() => (failedArtworkUrls = new Set([...failedArtworkUrls, release.artwork_url!]))}
						/>
					{:else}
						<ArtworkPlaceholder class="h-12 w-12 flex-shrink-0 rounded-md {alreadyExists ? 'opacity-50' : ''}" />
					{/if}

					<!-- Text: two-line layout — title bold, artist secondary -->
					<div class="min-w-0 flex-1">
						<p class="truncate text-sm font-medium {alreadyExists ? 'text-text-tertiary' : 'text-text-primary'}">
							{release.title ?? $translate('common.untitled')}
						</p>
						{#if alreadyExists}
							<p class="truncate text-xs text-text-tertiary">
								{$translate('discovery.bulkImport.alreadyInDiscoveryBadge')}
							</p>
						{:else}
							<p class="truncate text-xs text-text-tertiary">
								{release.artist ?? ''}
								{#if release.release_date}
									{#if release.artist}
										·
									{/if}{release.release_date}
								{/if}
							</p>
						{/if}
					</div>
				</button>
			{/each}
		</div>
	{/if}

	<!-- Action bar: pinned to the sheet's bottom edge (sticky within the sheet's scroll body), so the
	     primary action never floats mid-screen and stays reachable while the list scrolls behind it.
	     No Cancel here — the sheet's nav bar already provides it. -->
	<div class="sticky bottom-0 mt-auto flex flex-col gap-3 border-t border-stroke-subtle bg-surface-0 px-4 pt-3 pb-2">
		{#if importing && progress}
			<!-- Progress bar + count -->
			<div class="h-1.5 overflow-hidden rounded-full bg-surface-2">
				<div
					class="h-full rounded-full bg-brand-primary transition-[width] duration-300 ease-out"
					style="width: {progressFraction * 100}%"
				></div>
			</div>
			<div class="flex items-center justify-between">
				<p class="text-xs text-text-tertiary">
					{$translate('discovery.bulkImport.importing', {
						values: { current: progress.current, total: progress.total },
					})}
				</p>
				<button
					type="button"
					class="rounded-md px-3 py-1 text-xs font-medium text-text-secondary active:bg-surface-2"
					onclick={handleCancel}
				>
					{$translate('discovery.bulkImport.cancelImport')}
				</button>
			</div>
		{:else if result}
			<p class="text-center text-sm text-text-secondary">
				{#if result.failed > 0}
					{$translate('discovery.bulkImport.completedWithFailures', {
						values: { succeeded: result.succeeded, failed: result.failed },
					})}
				{:else}
					{$translate('discovery.bulkImport.completed', { values: { succeeded: result.succeeded } })}
				{/if}
			</p>
			<button
				type="button"
				class="h-11 w-full rounded-xl bg-brand-primary text-sm font-semibold text-white active:opacity-90"
				onclick={onCancel}
			>
				{$translate('common.done')}
			</button>
		{:else if selectableCount === 0}
			<!-- Everything found is already in discovery — an "Add 0 releases" button would be a dead end -->
			<button
				type="button"
				class="h-11 w-full rounded-xl bg-surface-2 text-sm font-semibold text-text-primary active:opacity-90"
				onclick={onCancel}
			>
				{$translate('common.done')}
			</button>
		{:else}
			<button
				type="button"
				class="h-11 w-full rounded-xl bg-brand-primary text-sm font-semibold text-white active:opacity-90 disabled:opacity-40"
				disabled={selectedCount === 0}
				onclick={handleImport}
			>
				{$translate('discovery.bulkImport.addReleases', { values: { count: selectedCount } })}
			</button>
		{/if}
	</div>
</div>
