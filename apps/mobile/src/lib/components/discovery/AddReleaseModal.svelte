<script lang="ts">
	import type {
		DiscoveryReleaseCreate,
		DiscoverySourceType,
		DiscoveryTrackCreate,
		FetchedMetadata,
		ScannedPage,
		ScanPageProgress,
		BulkImportResult,
	} from '$shared/types'
	import { translate } from '$shared/i18n'
	import { autoFetchMetadata } from '$shared/stores/settings'
	import { discoveryStore } from '$shared/stores/discovery'
	import * as discoveryApi from '$shared/api/discovery'
	import { isSupportedDiscoveryUrl, isDiscoveryPageUrl, detectSourceType } from '$shared/utils/discoveryLinks'
	import { formatDurationCompact } from '$shared/utils/format'
	import { listen } from '@tauri-apps/api/event'
	import { onMount } from 'svelte'
	import { mobileUIStore, addReleaseOpen } from '$lib/stores/mobileUI'
	import { pendingReleasesStore } from '$lib/stores/pendingReleases'
	import MobileModal from '$lib/components/common/MobileModal.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import SourceIcon from './SourceIcon.svelte'
	import BulkImportView from './BulkImportView.svelte'

	let url = $state('')
	let sourceType = $state<DiscoverySourceType>('other')
	let artist = $state('')
	let title = $state('')
	let label = $state('')
	let releaseDate = $state('')

	let fetching = $state(false)
	let fetchError = $state('')
	let fetchedData = $state<FetchedMetadata | null>(null)
	let artworkPreview = $state('')
	let tracks = $state<DiscoveryTrackCreate[]>([])
	let fetchDebounceTimer: ReturnType<typeof setTimeout> | null = null
	let lastFetchedUrl = ''

	let matchFound = $state(false)
	let unsupportedUrl = $state(false)
	let submitting = $state(false)

	let isBulkMode = $state(false)
	let scanning = $state(false)
	let scanProgress = $state<ScanPageProgress | null>(null)
	let scannedPage = $state<ScannedPage | null>(null)

	let unlistenScanProgress: (() => void) | null = null

	onMount(() => {
		listen<ScanPageProgress>('scan-page-progress', (event) => {
			scanProgress = event.payload
		}).then((fn) => {
			unlistenScanProgress = fn
		})
		return () => unlistenScanProgress?.()
	})

	function resetForm() {
		url = ''
		sourceType = 'other'
		artist = ''
		title = ''
		label = ''
		releaseDate = ''
		fetching = false
		fetchError = ''
		fetchedData = null
		artworkPreview = ''
		tracks = []
		lastFetchedUrl = ''
		unsupportedUrl = false
		matchFound = false
		submitting = false
		isBulkMode = false
		scanning = false
		scanProgress = null
		scannedPage = null
		if (fetchDebounceTimer) {
			clearTimeout(fetchDebounceTimer)
			fetchDebounceTimer = null
		}
	}

	function clearFetchResults() {
		fetchedData = null
		fetchError = ''
		artworkPreview = ''
		tracks = []
		artist = ''
		title = ''
		label = ''
		releaseDate = ''
		matchFound = false
		isBulkMode = false
		scanning = false
		scanProgress = null
		scannedPage = null
	}

	function handleUrlInput() {
		sourceType = detectSourceType(url)
		unsupportedUrl = false

		const trimmed = url.trim()
		if (lastFetchedUrl && trimmed !== lastFetchedUrl) {
			clearFetchResults()
			lastFetchedUrl = ''
		}

		if (fetchDebounceTimer) clearTimeout(fetchDebounceTimer)

		if (trimmed.startsWith('http')) {
			if (!isSupportedDiscoveryUrl(trimmed)) {
				unsupportedUrl = true
				return
			}
			if ($autoFetchMetadata) {
				fetchDebounceTimer = setTimeout(() => autoFetch(trimmed), 500)
			}
		}
	}

	async function autoFetch(fetchUrl: string) {
		if (fetching || scanning) return
		if (fetchUrl === lastFetchedUrl) return

		if (isDiscoveryPageUrl(fetchUrl)) {
			scanning = true
			fetchError = ''
			try {
				const page = await discoveryApi.scanPage(fetchUrl)
				lastFetchedUrl = fetchUrl
				scannedPage = page
				isBulkMode = true
			} catch (error) {
				fetchError = typeof error === 'string' ? error : error instanceof Error ? error.message : 'Scan failed'
			} finally {
				scanning = false
			}
			return
		}

		fetching = true
		fetchError = ''
		matchFound = false

		try {
			const data = await discoveryApi.fetchMetadata(fetchUrl)
			lastFetchedUrl = fetchUrl
			fetchedData = data

			if (data.artist) artist = data.artist
			if (data.title) title = data.title
			if (data.label) label = data.label
			if (data.release_date) releaseDate = data.release_date
			if (data.artwork_url) artworkPreview = data.artwork_url
			if (data.tracks.length > 0) {
				tracks = data.tracks.map((t) => ({
					name: t.name,
					position: t.position,
					duration_ms: t.duration_ms ?? undefined,
					video_id: t.video_id ?? undefined,
				}))
			}
			if (data.source_type && data.source_type !== 'other') {
				sourceType = data.source_type as DiscoverySourceType
			}

			try {
				const matches = await discoveryApi.checkMatches(fetchUrl, data.artist, data.title, data.parent_url)
				if (matches.length > 0) matchFound = true
			} catch {
				// Non-blocking
			}
		} catch (error) {
			if (!navigator.onLine) {
				fetchError = 'offline'
			} else {
				fetchError = typeof error === 'string' ? error : error instanceof Error ? error.message : 'Fetch failed'
			}
		} finally {
			fetching = false
		}
	}

	function handleClose() {
		if (scanning) discoveryApi.cancelScanPage()
		resetForm()
		mobileUIStore.closeAddRelease()
	}

	async function handleSubmit() {
		if (!url.trim() || submitting) return
		submitting = true

		try {
			const create: DiscoveryReleaseCreate = {
				url: url.trim(),
				source_type: sourceType,
			}
			if (artist.trim()) create.artist = artist.trim()
			if (title.trim()) create.title = title.trim()
			if (label.trim()) create.label = label.trim()
			if (releaseDate.trim()) create.release_date = releaseDate.trim()
			if (artworkPreview) create.artwork_url = artworkPreview
			if (tracks.length > 0) create.tracks = tracks
			if (fetchedData?.parent_url) create.parent_url = fetchedData.parent_url

			await discoveryStore.createRelease(create)
			handleClose()
		} finally {
			submitting = false
		}
	}

	function handleAddToQueue() {
		const trimmed = url.trim()
		if (!trimmed) return
		pendingReleasesStore.enqueue(trimmed)
		handleClose()
	}

	function handleBulkImportComplete(_result: BulkImportResult) {
		handleClose()
	}

	// Reset + clipboard prefill on open
	$effect(() => {
		if ($addReleaseOpen) {
			resetForm()
			void tryClipboardPrefill()
		}
	})

	async function tryClipboardPrefill() {
		try {
			const { readText } = await import('@tauri-apps/plugin-clipboard-manager')
			const text = await readText()
			if (text && text.startsWith('http') && isSupportedDiscoveryUrl(text)) {
				url = text
				sourceType = detectSourceType(text)
				if ($autoFetchMetadata) {
					fetchDebounceTimer = setTimeout(() => autoFetch(text), 300)
				}
			}
		} catch {
			// Clipboard not available or empty
		}
	}

	const isOffline = $derived(fetchError === 'offline')
	const canSubmit = $derived(url.trim() && !unsupportedUrl && !scanning && !fetching && !submitting)
</script>

<MobileModal open={$addReleaseOpen} onClose={handleClose} title={$translate('discovery.addRelease')}>
	{#if isBulkMode && scannedPage}
		<BulkImportView {scannedPage} onImportComplete={handleBulkImportComplete} onCancel={handleClose} />
	{:else}
		<div class="flex flex-col gap-4 py-1">
			<!-- URL input -->
			<div>
				<label for="add-url" class="mb-1.5 block text-xs font-medium text-text-secondary">
					{$translate('discovery.url')}
				</label>
				<input
					id="add-url"
					type="url"
					bind:value={url}
					oninput={handleUrlInput}
					placeholder="https://..."
					autofocus
					class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
				/>
				{#if fetching}
					<div class="mt-2 flex items-center gap-2">
						<Spinner class="h-3.5 w-3.5" />
						<span class="text-xs text-text-tertiary">{$translate('discovery.fetchingMetadata')}</span>
					</div>
				{:else if scanning}
					<div class="mt-2 flex items-center gap-2">
						<Spinner class="h-3.5 w-3.5" />
						<span class="text-xs text-text-tertiary">
							{#if scanProgress?.total_pages}
								{$translate('discovery.scanningReleasesProgress', {
									values: {
										current: scanProgress.current_page,
										total: scanProgress.total_pages,
										found: scanProgress.releases_found,
									},
								})}
							{:else if scanProgress?.entity_name}
								{$translate('discovery.scanningReleasesEntity', { values: { name: scanProgress.entity_name } })}
							{:else}
								{$translate('discovery.scanningReleases')}
							{/if}
						</span>
					</div>
				{:else if unsupportedUrl}
					<p class="mt-2 text-xs text-danger">{$translate('discovery.unsupportedUrl')}</p>
				{:else if isOffline}
					<p class="mt-2 text-xs text-text-tertiary">{$translate('discovery.offlineQueueNotice')}</p>
				{:else if fetchError}
					<p class="mt-2 text-xs text-danger">{$translate('discovery.fetchError')}</p>
				{/if}
			</div>

			<!-- Match notice -->
			{#if matchFound}
				<div class="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2.5">
					<p class="text-sm text-text-secondary">
						{$translate('discovery.similarFound', { values: { title: title || '', artist: artist || '' } })}
					</p>
				</div>
			{/if}

			<!-- Artwork preview -->
			{#if artworkPreview}
				<div class="flex justify-center">
					<img src={artworkPreview} alt="" class="h-36 w-36 rounded-lg object-cover shadow-md" />
				</div>
			{/if}

			<!-- Source badge (read-only; auto-detected from URL) -->
			{#if sourceType !== 'other' && !fetchedData}
				<div class="flex items-center gap-2 text-sm text-text-secondary">
					<SourceIcon source={sourceType} />
					<span class="capitalize">{sourceType}</span>
				</div>
			{/if}

			<!-- Editable fields (shown after metadata fetch) -->
			{#if fetchedData}
				<div>
					<label for="add-artist" class="mb-1.5 block text-xs font-medium text-text-secondary">
						{$translate('discovery.editor.artist')}
					</label>
					<input
						id="add-artist"
						type="text"
						bind:value={artist}
						placeholder={$translate('discovery.editor.artist')}
						class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
					/>
				</div>

				<div>
					<label for="add-title" class="mb-1.5 block text-xs font-medium text-text-secondary">
						{$translate('discovery.editor.title')}
					</label>
					<input
						id="add-title"
						type="text"
						bind:value={title}
						placeholder={$translate('discovery.editor.title')}
						class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
					/>
				</div>

				<div>
					<label for="add-label" class="mb-1.5 block text-xs font-medium text-text-secondary">
						{$translate('discovery.editor.label')}
					</label>
					<input
						id="add-label"
						type="text"
						bind:value={label}
						placeholder={$translate('discovery.editor.label')}
						class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
					/>
				</div>

				<!-- Track list preview -->
				{#if tracks.length > 0}
					<div>
						<p class="mb-1.5 text-xs font-medium text-text-secondary">
							{$translate('discovery.tracks')} ({$translate('discovery.trackCount', {
								values: { count: tracks.length },
							})})
						</p>
						<div class="max-h-48 overflow-y-auto rounded-lg border border-stroke bg-surface-1">
							{#each tracks as track (track.position)}
								<div class="flex items-center justify-between border-b border-stroke-subtle px-3 py-2 last:border-b-0">
									<span class="min-w-0 flex-1 truncate text-sm text-text-primary">
										<span class="mr-2 text-xs text-text-tertiary">{track.position}.</span>{track.name}
									</span>
									{#if track.duration_ms}
										<span class="ml-2 flex-shrink-0 text-xs text-text-tertiary tabular-nums"
											>{formatDurationCompact(track.duration_ms)}</span
										>
									{/if}
								</div>
							{/each}
						</div>
					</div>
				{/if}
			{/if}
		</div>

		{#snippet footer()}
			<div class="flex w-full justify-end gap-2">
				<button
					type="button"
					class="rounded-lg px-4 py-2 text-sm font-medium text-text-secondary active:bg-surface-2"
					onclick={handleClose}
				>
					{$translate('common.cancel')}
				</button>
				{#if isOffline && url.trim() && !unsupportedUrl}
					<button
						type="button"
						class="rounded-lg bg-brand-primary px-4 py-2 text-sm font-semibold text-white active:opacity-90"
						onclick={handleAddToQueue}
					>
						{$translate('discovery.addToQueue')}
					</button>
				{:else}
					<button
						type="button"
						class="rounded-lg bg-brand-primary px-4 py-2 text-sm font-semibold text-white active:opacity-90 disabled:opacity-40"
						disabled={!canSubmit}
						onclick={handleSubmit}
					>
						{$translate('discovery.addRelease')}
					</button>
				{/if}
			</div>
		{/snippet}
	{/if}
</MobileModal>
