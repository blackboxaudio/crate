<script lang="ts">
	import type { ScannedPage, BulkImportProgress, BulkImportResult, DiscoverySourceType, FollowType } from '$lib/types'
	import { Button, Checkbox, Text, Spinner, Icon } from '$lib/components/common'
	import { translate } from '$lib/i18n'
	import { followStore, followedSources } from '$lib/stores'
	import { autoFollowOnImport } from '$lib/stores/settings'
	import { looseUrlEq } from '$lib/utils'
	import { listen } from '@tauri-apps/api/event'
	import { onMount, untrack } from 'svelte'
	import * as discoveryApi from '$lib/api/discovery'
	import { SvelteSet } from 'svelte/reactivity'

	type Props = {
		scannedPage: ScannedPage
		onImportComplete: (result: BulkImportResult) => void
		onCancel: () => void
	}

	let { scannedPage, onImportComplete, onCancel }: Props = $props()

	let selectedUrls = $state(new Set<string>())
	let importing = $state(false)
	let progress = $state<BulkImportProgress | null>(null)
	let result = $state<BulkImportResult | null>(null)
	let unlisten: (() => void) | null = null

	// Initialize selected URLs (all non-existing releases)
	$effect(() => {
		const initial = new SvelteSet<string>()
		for (const release of scannedPage.releases) {
			if (!release.already_exists) {
				initial.add(release.url)
			}
		}
		selectedUrls = initial
	})

	let selectableCount = $derived(scannedPage.releases.filter((r) => !r.already_exists).length)
	let selectedCount = $derived(selectedUrls.size)

	// "Also follow" the scanned page (the label — or the artist, when an artist page was
	// scanned) for future releases. The page URL is the only reliably-followable entity in
	// bulk import; a release URL alone can't recover its label's page.
	const followUrl = $derived(scannedPage.page_url)
	const followName = $derived(scannedPage.page_label ?? scannedPage.page_artist)
	const followType = $derived<FollowType>(scannedPage.page_label ? 'label' : 'artist')
	const alreadyFollowing = $derived(!!followUrl && $followedSources.some((s) => looseUrlEq(s.url, followUrl)))
	// Seed the opt-in from the user's "auto-follow on import" preference (Off by default).
	// `untrack` because this is a one-time seed of mutable checkbox state — the user toggles
	// it afterward, so it should capture the initial value, not react to later changes.
	let followSource = $state(
		untrack(
			() => $autoFollowOnImport === 'both' || $autoFollowOnImport === (scannedPage.page_label ? 'label' : 'artist')
		)
	)

	onMount(() => {
		listen<BulkImportProgress>('bulk-import-progress', (event) => {
			progress = event.payload
		}).then((fn) => {
			unlisten = fn
		})

		return () => {
			unlisten?.()
			if (importing) {
				discoveryApi.cancelBulkImport()
			}
		}
	})

	function toggleUrl(url: string) {
		const next = new SvelteSet(selectedUrls)
		if (next.has(url)) {
			next.delete(url)
		} else {
			next.add(url)
		}
		selectedUrls = next
	}

	function selectAll() {
		const next = new SvelteSet<string>()
		for (const release of scannedPage.releases) {
			if (!release.already_exists) {
				next.add(release.url)
			}
		}
		selectedUrls = next
	}

	function deselectAll() {
		selectedUrls = new Set()
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
			// Follow before onImportComplete — that callback closes/unmounts this modal.
			if (followSource && followUrl && !alreadyFollowing) {
				await followStore.followEntity({
					url: followUrl,
					name: followName ?? null,
					sourceType: scannedPage.source_type as DiscoverySourceType,
					followType,
				})
			}
			onImportComplete(importResult)
		} catch (error) {
			// If cancelled or errored, still show partial results if we have progress
			const lastProgress = progress as BulkImportProgress | null
			if (lastProgress) {
				result = {
					succeeded: lastProgress.succeeded,
					failed: lastProgress.failed,
					failed_urls: [],
				}
			}
		} finally {
			importing = false
		}
	}

	async function handleCancel() {
		if (importing) {
			await discoveryApi.cancelBulkImport()
		}
		onCancel()
	}
</script>

<div class="flex flex-col gap-3">
	<!-- Summary header -->
	<div class="flex items-center justify-between">
		<div class="flex flex-col gap-0.5">
			<Text size="sm" weight="medium">
				{$translate('discovery.bulkImport.releasesFound', { values: { total: scannedPage.total_found } })}
			</Text>
			{#if scannedPage.already_in_discovery > 0}
				<Text size="xs" color="tertiary">
					{$translate('discovery.bulkImport.alreadyInDiscovery', {
						values: { count: scannedPage.already_in_discovery },
					})}
				</Text>
			{/if}
		</div>
		{#if !result && selectableCount > 0}
			<div class="flex gap-2">
				<Button variant="ghost" size="sm" onclick={selectAll} disabled={importing || selectedCount === selectableCount}>
					{$translate('discovery.bulkImport.selectAll')}
				</Button>
				<Button variant="ghost" size="sm" onclick={deselectAll} disabled={importing || selectedCount === 0}>
					{$translate('discovery.bulkImport.deselectAll')}
				</Button>
			</div>
		{/if}
	</div>

	<!-- Release checklist -->
	{#if scannedPage.releases.length === 0}
		<div class="flex items-center justify-center py-8">
			<Text size="sm" color="tertiary">{$translate('discovery.bulkImport.noReleasesFound')}</Text>
		</div>
	{:else}
		<div class="max-h-80 overflow-y-auto rounded-md border border-stroke bg-surface-2">
			{#each scannedPage.releases as release (release.url)}
				<button
					type="button"
					class="flex w-full items-center gap-3 border-b border-stroke-subtle px-3 py-2 text-left last:border-b-0
						{release.already_exists ? 'cursor-not-allowed opacity-50' : 'hover:bg-surface-3 cursor-pointer'}"
					disabled={release.already_exists || importing}
					onclick={() => toggleUrl(release.url)}
				>
					<Checkbox
						checked={selectedUrls.has(release.url)}
						disabled={release.already_exists || importing}
						onchange={() => toggleUrl(release.url)}
					/>
					{#if release.artwork_url}
						<img src={release.artwork_url} alt="" class="h-8 w-8 shrink-0 rounded object-cover" />
					{:else}
						<div class="bg-surface-3 flex h-8 w-8 shrink-0 items-center justify-center rounded">
							<Text size="xs" color="tertiary">--</Text>
						</div>
					{/if}
					<div class="min-w-0 flex-1">
						<Text size="sm" class="block truncate">
							{#if release.artist}
								<span class="text-text-secondary">{release.artist}</span>
								<span class="text-text-tertiary"> - </span>
							{/if}
							{release.title ?? 'Untitled'}
						</Text>
						<div class="flex items-center gap-2">
							{#if release.release_date}
								<Text size="xs" color="tertiary">{release.release_date}</Text>
							{/if}
							{#if release.already_exists}
								<span class="rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] text-amber-400">
									{$translate('discovery.bulkImport.alreadyInDiscoveryBadge')}
								</span>
							{/if}
						</div>
					</div>
				</button>
			{/each}
		</div>
	{/if}

	<!-- Footer area -->
	<div class="flex items-center justify-between">
		{#if result}
			<Text size="sm" color="secondary">
				{#if result.failed > 0}
					{$translate('discovery.bulkImport.completedWithFailures', {
						values: { succeeded: result.succeeded, failed: result.failed },
					})}
				{:else}
					{$translate('discovery.bulkImport.completed', { values: { succeeded: result.succeeded } })}
				{/if}
			</Text>
			<Button variant="ghost" onclick={onCancel}>
				{$translate('common.close')}
			</Button>
		{:else if importing && progress}
			<div class="flex items-center gap-2">
				<Spinner class="h-3.5 w-3.5" />
				<Text size="sm" color="tertiary">
					{$translate('discovery.bulkImport.importing', {
						values: { current: progress.current, total: progress.total },
					})}
				</Text>
			</div>
			<Button variant="ghost" onclick={handleCancel}>
				{$translate('discovery.bulkImport.cancelImport')}
			</Button>
		{:else}
			{#if followUrl}
				{#if alreadyFollowing}
					<div class="flex items-center gap-2 text-sm text-text-tertiary">
						<Icon name="rss" class="h-3.5 w-3.5 text-brand-primary" />
						{followType === 'label'
							? $translate('discovery.following.label')
							: $translate('discovery.following.artist')} · {$translate('discovery.following.following')}
					</div>
				{:else}
					<Checkbox
						checked={followSource}
						onchange={(v) => (followSource = v)}
						label={$translate('discovery.following.alsoFollow')}
						disabled={importing}
					/>
				{/if}
			{:else}
				<div></div>
			{/if}
			<div class="flex gap-2">
				<Button variant="ghost" onclick={onCancel}>
					{$translate('common.cancel')}
				</Button>
				<Button variant="primary" disabled={selectedCount === 0} onclick={handleImport}>
					{$translate('discovery.bulkImport.addReleases', { values: { count: selectedCount } })}
				</Button>
			</div>
		{/if}
	</div>
</div>
