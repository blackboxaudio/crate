<script lang="ts">
	import type { DiscoveryReleaseCreate, DiscoverySourceType, DiscoveryTrackCreate, FetchedMetadata } from '$lib/types'
	import { Modal, Input, Select, Button, Text, Spinner } from '$lib/components/common'
	import { translate } from '$lib/i18n'
	import { autoFetchMetadata } from '$lib/stores/settings'
	import * as discoveryApi from '$lib/api/discovery'

	type Props = {
		open: boolean
		onClose: () => void
		onSubmit: (create: DiscoveryReleaseCreate) => Promise<void>
	}

	let { open, onClose, onSubmit }: Props = $props()

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

	const sourceOptions = [
		{ value: 'bandcamp', label: 'Bandcamp' },
		{ value: 'soundcloud', label: 'SoundCloud' },
		{ value: 'youtube', label: 'YouTube' },
		{ value: 'discogs', label: 'Discogs' },
		{ value: 'other', label: 'Other' },
	]

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
		if (fetching) return
		if (fetchUrl === lastFetchedUrl) return

		fetching = true
		fetchError = ''

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
				}))
			}
			if (data.source_type && data.source_type !== 'other') {
				sourceType = data.source_type as DiscoverySourceType
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
		if (fetchDebounceTimer) {
			clearTimeout(fetchDebounceTimer)
			fetchDebounceTimer = null
		}
	}

	function handleClose() {
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

		await onSubmit(create)
	}

	// Reset form when modal opens
	$effect(() => {
		if (open) resetForm()
	})
</script>

<Modal {open} title={$translate('discovery.addRelease')} onClose={handleClose} onSubmit={handleSubmit}>
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
			{:else if fetchError}
				<Text size="xs" color="danger" class="mt-2">{$translate('discovery.fetchError')}</Text>
			{/if}
		</div>

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
					{$translate('discovery.tracks')} ({$translate('discovery.trackCount', { values: { count: tracks.length } })})
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

	{#snippet footer()}
		<Button variant="ghost" onclick={handleClose}>
			{$translate('common.cancel')}
		</Button>
		<Button variant="primary" disabled={!url.trim()} onclick={handleSubmit}>
			{$translate('discovery.addRelease')}
		</Button>
	{/snippet}
</Modal>
