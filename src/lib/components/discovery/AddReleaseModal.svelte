<script lang="ts">
	import type {
		DiscoveryRelease,
		DiscoveryReleaseCreate,
		DiscoverySourceType,
		DiscoveryTrackCreate,
		FetchedMetadata,
		ScannedPage,
		ScanPageProgress,
		BulkImportResult,
	} from '$lib/types'
	import { Modal, Input, Select, Button, Text, Spinner } from '$lib/components/common'
	import { BulkImportView } from '$lib/components/discovery'
	import { translate } from '$lib/i18n'
	import { autoFetchMetadata } from '$lib/stores/settings'
	import { listen } from '@tauri-apps/api/event'
	import { onMount } from 'svelte'
	import * as discoveryApi from '$lib/api/discovery'

	type Props = {
		open: boolean
		onClose: () => void
		onSubmit: (create: DiscoveryReleaseCreate) => Promise<void>
		onAddToExisting?: (releaseId: string, tracks: DiscoveryTrackCreate[]) => Promise<void>
		onBulkImportComplete?: () => void
	}

	let { open, onClose, onSubmit, onAddToExisting, onBulkImportComplete }: Props = $props()

	let url = $state('')
	let sourceType = $state<DiscoverySourceType>('other')
	let artist = $state('')
	let title = $state('')
	let label = $state('')
	let releaseDate = $state('')

	// Fetch state
	let fetching = $state(false)
	let fetchError = $state('')
	let fetchedData = $state<FetchedMetadata | null>(null)
	let artworkPreview = $state('')
	let tracks = $state<DiscoveryTrackCreate[]>([])
	let fetchDebounceTimer: ReturnType<typeof setTimeout> | null = null
	let lastFetchedUrl = ''

	// Match detection state
	let matchedRelease = $state<DiscoveryRelease | null>(null)
	let matchType = $state<'parent' | 'similar' | null>(null)

	// Bulk import state
	let isBulkMode = $state(false)
	let scanning = $state(false)
	let scanProgress = $state<ScanPageProgress | null>(null)
	let scannedPage = $state<ScannedPage | null>(null)
	let bulkImporting = $state(false)

	let unlistenScanProgress: (() => void) | null = null

	onMount(() => {
		listen<ScanPageProgress>('scan-page-progress', (event) => {
			scanProgress = event.payload
		}).then((fn) => {
			unlistenScanProgress = fn
		})

		return () => {
			unlistenScanProgress?.()
		}
	})

	const sourceOptions = [
		{ value: 'bandcamp', label: 'Bandcamp' },
		{ value: 'soundcloud', label: 'SoundCloud' },
		{ value: 'youtube', label: 'YouTube' },
		{ value: 'discogs', label: 'Discogs' },
		{ value: 'other', label: 'Other' },
	]

	function isPageUrl(input: string): boolean {
		const lower = input.toLowerCase()
		// Bandcamp: *.bandcamp.com without /album/ or /track/
		if (lower.includes('bandcamp.com') && !lower.includes('/album/') && !lower.includes('/track/')) {
			return true
		}
		// Discogs: /artist/ or /label/ paths
		if (lower.includes('discogs.com') && (lower.includes('/artist/') || lower.includes('/label/'))) {
			return true
		}
		return false
	}

	function detectSource(input: string) {
		const lower = input.toLowerCase()
		if (lower.includes('bandcamp.com')) sourceType = 'bandcamp'
		else if (lower.includes('soundcloud.com')) sourceType = 'soundcloud'
		else if (lower.includes('youtube.com') || lower.includes('youtu.be')) sourceType = 'youtube'
		else if (lower.includes('discogs.com')) sourceType = 'discogs'
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
		matchedRelease = null
		matchType = null
		isBulkMode = false
		scanning = false
		scanProgress = null
		scannedPage = null
	}

	function handleUrlInput() {
		detectSource(url)

		const trimmed = url.trim()

		// If URL changed from what we already fetched, clear old results
		if (lastFetchedUrl && trimmed !== lastFetchedUrl) {
			clearFetchResults()
			lastFetchedUrl = ''
		}

		// Debounced auto-fetch
		if (fetchDebounceTimer) clearTimeout(fetchDebounceTimer)

		if (trimmed.startsWith('http') && $autoFetchMetadata) {
			fetchDebounceTimer = setTimeout(() => {
				autoFetch(trimmed)
			}, 500)
		}
	}

	async function autoFetch(fetchUrl: string) {
		if (fetching || scanning) return
		if (fetchUrl === lastFetchedUrl) return

		// Branch: page URL → scan for releases
		if (isPageUrl(fetchUrl)) {
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

		// Single-release flow
		fetching = true
		fetchError = ''
		matchedRelease = null
		matchType = null

		try {
			const data = await discoveryApi.fetchMetadata(fetchUrl)
			lastFetchedUrl = fetchUrl

			fetchedData = data

			// Auto-populate fields from fetched data
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

			// Check for matching releases
			try {
				const matches = await discoveryApi.checkMatches(data.artist, data.title, data.parent_url)
				if (matches.length > 0) {
					// Prefer parent_url match over artist+title match
					if (data.parent_url) {
						const parentMatch = matches.find((m) => m.url === data.parent_url || m.parent_url === data.parent_url)
						if (parentMatch) {
							matchedRelease = parentMatch
							matchType = 'parent'
						} else {
							matchedRelease = matches[0]
							matchType = 'similar'
						}
					} else {
						matchedRelease = matches[0]
						matchType = 'similar'
					}
				}
			} catch {
				// Non-blocking: if match check fails, just continue without showing matches
			}
		} catch (error) {
			fetchError = typeof error === 'string' ? error : error instanceof Error ? error.message : 'Fetch failed'
		} finally {
			fetching = false
		}
	}

	function formatDuration(ms: number | undefined): string {
		if (!ms) return ''
		const totalSeconds = Math.floor(ms / 1000)
		const minutes = Math.floor(totalSeconds / 60)
		const seconds = totalSeconds % 60
		return `${minutes}:${seconds.toString().padStart(2, '0')}`
	}

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
		isBulkMode = false
		scanning = false
		scanProgress = null
		scannedPage = null
		bulkImporting = false
		if (fetchDebounceTimer) {
			clearTimeout(fetchDebounceTimer)
			fetchDebounceTimer = null
		}
	}

	function handleClose() {
		if (bulkImporting) {
			discoveryApi.cancelBulkImport()
		}
		if (scanning) {
			discoveryApi.cancelScanPage()
		}
		resetForm()
		onClose()
	}

	async function handleSubmit() {
		if (!url.trim()) return

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

		await onSubmit(create)
	}

	async function handleAddToExisting() {
		if (!matchedRelease || !onAddToExisting) return
		await onAddToExisting(matchedRelease.id, tracks)
		handleClose()
	}

	function handleBulkImportComplete(_result: BulkImportResult) {
		bulkImporting = false
		onBulkImportComplete?.()
		handleClose()
	}

	function handleBulkCancel() {
		handleClose()
	}

	// Reset form when modal opens
	$effect(() => {
		if (open) resetForm()
	})
</script>

<Modal
	{open}
	title={$translate('discovery.addRelease')}
	size={isBulkMode ? 'lg' : 'sm'}
	onClose={handleClose}
	onSubmit={isBulkMode ? undefined : handleSubmit}
>
	{#if isBulkMode && scannedPage}
		<!-- Bulk import mode -->
		<BulkImportView {scannedPage} onImportComplete={handleBulkImportComplete} onCancel={handleBulkCancel} />
	{:else}
		<div class="flex flex-col gap-4">
			<div>
				<Text as="label" for="release-url" size="sm" weight="medium" color="secondary" class="mb-1.5 block">
					{$translate('discovery.url')}
				</Text>
				<Input bind:value={url} placeholder="https://..." autofocus oninput={handleUrlInput} />
				{#if fetching}
					<div class="mt-2 flex items-center gap-2">
						<Spinner class="h-3.5 w-3.5" />
						<Text size="xs" color="tertiary">{$translate('discovery.fetchingMetadata')}</Text>
					</div>
				{:else if scanning}
					<div class="mt-2 flex items-center gap-2">
						<Spinner class="h-3.5 w-3.5" />
						<Text size="xs" color="tertiary">
							{#if scanProgress?.total_pages}
								{$translate('discovery.scanningReleasesProgress', {
									values: {
										current: scanProgress.current_page,
										total: scanProgress.total_pages,
										found: scanProgress.releases_found,
									},
								})}
							{:else if scanProgress?.entity_name}
								{$translate('discovery.scanningReleasesEntity', {
									values: { name: scanProgress.entity_name },
								})}
							{:else}
								{$translate('discovery.scanningReleases')}
							{/if}
						</Text>
					</div>
				{:else if fetchError}
					<Text size="xs" color="danger" class="mt-2">{$translate('discovery.fetchError')}</Text>
				{/if}
			</div>

			<!-- Match detection notice -->
			{#if matchedRelease && matchType}
				<div class="rounded-md border border-amber-500/30 bg-amber-500/10 p-3">
					<Text size="sm" color="secondary">
						{#if matchType === 'parent' && fetchedData?.parent_album_title}
							{$translate('discovery.matchFound', {
								values: { albumTitle: fetchedData.parent_album_title },
							})}
						{:else}
							{$translate('discovery.similarFound', {
								values: {
									title: matchedRelease.title ?? '',
									artist: matchedRelease.artist ?? '',
								},
							})}
						{/if}
					</Text>
					<div class="mt-2 flex gap-2">
						{#if onAddToExisting}
							<Button variant="ghost" size="sm" onclick={handleAddToExisting}>
								{$translate('discovery.addToExisting')}
							</Button>
						{/if}
						<Button variant="ghost" size="sm" onclick={handleSubmit}>
							{$translate('discovery.addAsNew')}
						</Button>
					</div>
				</div>
			{/if}

			<!-- Artwork preview -->
			{#if artworkPreview}
				<div class="flex justify-center">
					<img src={artworkPreview} alt="Release artwork" class="h-40 w-40 rounded-md object-cover" />
				</div>
			{/if}

			<div>
				<Text as="label" for="release-source" size="sm" weight="medium" color="secondary" class="mb-1.5 block">
					{$translate('discovery.source')}
				</Text>
				<Select bind:value={sourceType} options={sourceOptions} />
			</div>

			<div>
				<Text as="label" for="release-artist" size="sm" weight="medium" color="secondary" class="mb-1.5 block">
					{$translate('library.columns.artist')}
				</Text>
				<Input bind:value={artist} placeholder={$translate('library.columns.artist')} />
			</div>

			<div>
				<Text as="label" for="release-title" size="sm" weight="medium" color="secondary" class="mb-1.5 block">
					{$translate('library.columns.title')}
				</Text>
				<Input bind:value={title} placeholder={$translate('library.columns.title')} />
			</div>

			<div>
				<Text as="label" for="release-label" size="sm" weight="medium" color="secondary" class="mb-1.5 block">
					{$translate('editor.label')}
				</Text>
				<Input bind:value={label} placeholder={$translate('editor.label')} />
			</div>

			<!-- Track list preview -->
			{#if tracks.length > 0}
				<div>
					<Text size="sm" weight="medium" color="secondary" class="mb-1.5 block">
						{$translate('discovery.tracks')} ({$translate('discovery.trackCount', {
							values: { count: tracks.length },
						})})
					</Text>
					<div class="max-h-48 overflow-y-auto rounded-md border border-stroke bg-surface-2 p-2">
						{#each tracks as track (track.name)}
							<div class="flex items-center justify-between px-2 py-1">
								<Text size="xs" color="secondary">
									<span class="mr-2 text-text-tertiary">{track.position}.</span>{track.name}
								</Text>
								{#if track.duration_ms}
									<Text size="xs" color="tertiary" class="ml-2 shrink-0">{formatDuration(track.duration_ms)}</Text>
								{/if}
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	{/if}

	{#snippet footer()}
		{#if !isBulkMode}
			<Button variant="ghost" onclick={handleClose}>
				{$translate('common.cancel')}
			</Button>
			<Button variant="primary" disabled={!url.trim() || scanning || fetching} onclick={handleSubmit}>
				{$translate('discovery.addRelease')}
			</Button>
		{/if}
	{/snippet}
</Modal>
