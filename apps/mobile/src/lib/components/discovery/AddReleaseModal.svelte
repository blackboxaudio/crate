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
	import {
		isSupportedDiscoveryUrl,
		isDiscoveryPageUrl,
		detectSourceType,
		extractFirstUrl,
	} from '$shared/utils/discoveryLinks'
	import { isAndroid } from '$shared/utils/platform'
	import { readText } from '@tauri-apps/plugin-clipboard-manager'
	import { formatDurationCompact } from '$shared/utils/format'
	import { listen } from '@tauri-apps/api/event'
	import { onMount, tick } from 'svelte'
	import { mobileUIStore, addReleaseOpen, addReleasePrefillUrl } from '$lib/stores/mobileUI'
	import { pendingReleasesStore } from '$lib/stores/pendingReleases'
	import Drawer from '$lib/components/common/Drawer.svelte'
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
	// Hide the preview instead of showing a broken image when the fetched URL is dead;
	// reset whenever the preview target changes.
	let artworkPreviewFailed = $state(false)
	$effect(() => {
		void artworkPreview
		artworkPreviewFailed = false
	})
	let tracks = $state<DiscoveryTrackCreate[]>([])
	let fetchDebounceTimer: ReturnType<typeof setTimeout> | null = null
	let lastFetchedUrl = ''
	// Bumped whenever the form is cleared (reset / URL changed). An in-flight metadata fetch captures the epoch
	// at its start and drops its result if the epoch has moved on — otherwise a fetch that resolves after the
	// sheet was closed (or the URL was retyped) would repopulate the form and leak stale state into the next open.
	let formEpoch = 0

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
		formEpoch++
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
		formEpoch++
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

		// Snapshot the epoch; if the form is cleared (close / retype) while a request is in flight, its result is
		// stale and must not be written back — that leak is what carried old state into the next open.
		const epoch = formEpoch

		if (isDiscoveryPageUrl(fetchUrl)) {
			scanning = true
			fetchError = ''
			try {
				const page = await discoveryApi.scanPage(fetchUrl)
				if (epoch !== formEpoch) return
				lastFetchedUrl = fetchUrl
				scannedPage = page
				isBulkMode = true
			} catch (error) {
				if (epoch !== formEpoch) return
				fetchError = typeof error === 'string' ? error : error instanceof Error ? error.message : 'Scan failed'
			} finally {
				if (epoch === formEpoch) scanning = false
			}
			return
		}

		fetching = true
		fetchError = ''
		matchFound = false

		try {
			const data = await discoveryApi.fetchMetadata(fetchUrl)
			if (epoch !== formEpoch) return
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
					url: t.url ?? undefined,
				}))
			}
			if (data.source_type && data.source_type !== 'other') {
				sourceType = data.source_type as DiscoverySourceType
			}

			try {
				const matches = await discoveryApi.checkMatches(fetchUrl, data.artist, data.title, data.parent_url)
				if (epoch === formEpoch && matches.length > 0) matchFound = true
			} catch {
				// Non-blocking
			}
		} catch (error) {
			if (epoch !== formEpoch) return
			if (!navigator.onLine) {
				fetchError = 'offline'
			} else {
				fetchError = typeof error === 'string' ? error : error instanceof Error ? error.message : 'Fetch failed'
			}
		} finally {
			if (epoch === formEpoch) fetching = false
		}
	}

	function handleClose() {
		if (scanning) discoveryApi.cancelScanPage()
		mobileUIStore.closeAddRelease()
		// The form is cleared by the drawer's `onClosed` (after the slide-out) so the content doesn't visibly
		// wipe mid-animation, and every dismiss path (button, scrim, swipe) ends up clean for the next open.
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

	// Reset on open so the sheet always starts blank. (The URL field focuses itself via `use:focusOnOpen` below.)
	$effect(() => {
		if ($addReleaseOpen) {
			resetForm()
			clipboardMiss = false
		}
	})

	// Prefill from a shared URL (Android share intent, #62). MUST come after the reset-on-open effect —
	// effects run in creation order, so the reset wipes the blank form first and the prefill then lands on
	// top of it. Reacting to `$addReleasePrefillUrl` (not just open) also handles a second share arriving
	// while the sheet is already open: it overwrites the form and refetches. Skips the typing debounce —
	// a shared URL is complete by definition.
	$effect(() => {
		if (!$addReleaseOpen || $addReleasePrefillUrl === null) return
		const shared = mobileUIStore.consumeAddReleasePrefill()
		if (!shared) return
		resetForm()
		url = shared
		sourceType = detectSourceType(shared)
		unsupportedUrl = shared.startsWith('http') && !isSupportedDiscoveryUrl(shared)
		if (!unsupportedUrl && $autoFetchMetadata) void autoFetch(shared)
	})

	// Clipboard intake. Explicit "Paste link" button everywhere (iOS 16+ shows a system paste-
	// permission prompt on ANY programmatic clipboard read, so tying the read to a user tap keeps
	// that prompt expected — never auto-read on open on iOS). A miss leaves the field untouched and
	// shows a quiet inline line instead of a toast.
	let clipboardMiss = $state(false)

	async function pasteFromClipboard() {
		clipboardMiss = false
		try {
			const text = await readText()
			const found = text ? extractFirstUrl(text) : null
			if (!found) {
				clipboardMiss = true
				return
			}
			url = found
			handleUrlInput()
		} catch {
			clipboardMiss = true
		}
	}

	// Android-only auto-prefill from the clipboard (no read prompt there; Android 12+ just shows the
	// passive "app pasted" notice). Ordered after the share-intent effect so a shared URL wins, and
	// it never overwrites a non-empty field. Silent on a miss — auto-detect shouldn't nag.
	$effect(() => {
		if (!$addReleaseOpen || !isAndroid()) return
		if ($addReleasePrefillUrl !== null || url.trim()) return
		void readText()
			.then((text) => {
				const found = text ? extractFirstUrl(text) : null
				if (found && !url.trim()) {
					url = found
					handleUrlInput()
				}
			})
			.catch(() => {})
	})

	// Focus the URL field as the sheet mounts. `preventScroll` is the crux: the panel starts off-screen and
	// slides up, so a plain focus makes iOS scroll the document to "reveal" the field — dragging the settled
	// form off the top. `preventScroll` suppresses exactly that reveal-scroll while still raising the keyboard.
	// We focus via `tick` (like MobilePromptDialog) rather than a timeout so the tap's activation is preserved
	// and the keyboard actually appears — a `setTimeout` loses that activation and the keyboard stays hidden.
	function focusOnOpen(node: HTMLInputElement) {
		void tick().then(() => node.focus({ preventScroll: true }))
	}

	const isOffline = $derived(fetchError === 'offline')
	const canSubmit = $derived(url.trim() && !unsupportedUrl && !scanning && !fetching && !submitting)
</script>

<!-- A full-height form sheet (not a compact bottom sheet). The action bar and the URL field live at the TOP,
     so when the keyboard opens from the bottom it never covers the primary controls, and the focused field sits
     high on screen — well clear of the keyboard — so iOS has no reason to scroll the page to reveal it (the root
     cause of the earlier "whole view shifts up" bug). Metadata that loads after fetch scrolls in the body below. -->
<Drawer
	open={$addReleaseOpen}
	onClose={handleClose}
	onClosed={resetForm}
	direction="bottom"
	positionSlide
	z={50}
	panelDrag={false}
	ariaLabel={$translate('discovery.addRelease')}
	class="pb-safe flex h-[92vh] flex-col overflow-hidden rounded-t-2xl border-t border-stroke bg-surface-0"
>
	{#snippet children({ drag, animating })}
		<!-- Grab handle + nav bar (drag either to dismiss). Cancel / title / primary action. -->
		<div use:drag>
			<div class="flex justify-center pt-2 pb-1">
				<span class="h-1 w-10 rounded-full bg-text-tertiary/50"></span>
			</div>
			<div class="grid grid-cols-[1fr_auto_1fr] items-center gap-2 border-b border-stroke-subtle px-4 py-3">
				<button
					type="button"
					class="justify-self-start text-sm font-medium text-text-secondary active:opacity-60"
					onclick={handleClose}
				>
					{$translate('common.cancel')}
				</button>
				<h2 class="text-base font-medium text-text-primary">{$translate('discovery.addRelease')}</h2>
				{#if isBulkMode}
					<span></span>
				{:else if isOffline && url.trim() && !unsupportedUrl}
					<button
						type="button"
						class="justify-self-end text-sm font-semibold text-brand-primary active:opacity-60"
						onclick={handleAddToQueue}
					>
						{$translate('discovery.addToQueue')}
					</button>
				{:else}
					<button
						type="button"
						class="justify-self-end text-sm font-semibold text-brand-primary active:opacity-60 disabled:opacity-40"
						disabled={!canSubmit}
						onclick={handleSubmit}
					>
						{$translate('common.add')}
					</button>
				{/if}
			</div>
		</div>

		<!-- Scrollable body: URL field pinned at the top; states, artwork, editable fields, and tracks below. -->
		<div class="min-h-0 flex-1 {animating ? 'overflow-hidden' : 'overflow-y-auto'}">
			{#if isBulkMode && scannedPage}
				<BulkImportView {scannedPage} onImportComplete={handleBulkImportComplete} onCancel={handleClose} />
			{:else}
				<div class="flex flex-col gap-4 px-4 py-4">
					<!-- URL input -->
					<div>
						<div class="mb-1.5 flex items-center justify-between">
							<label for="add-url" class="block text-xs font-medium text-text-secondary">
								{$translate('discovery.url')}
							</label>
							<button
								type="button"
								class="text-xs font-medium text-brand-primary active:opacity-70"
								onclick={pasteFromClipboard}
							>
								{$translate('discovery.pasteLink')}
							</button>
						</div>
						<input
							id="add-url"
							type="url"
							use:focusOnOpen
							bind:value={url}
							oninput={handleUrlInput}
							placeholder="https://..."
							class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
						/>
						{#if clipboardMiss}
							<p class="mt-1.5 text-xs text-text-tertiary">{$translate('discovery.clipboardNoUrl')}</p>
						{/if}
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
					{#if artworkPreview && !artworkPreviewFailed}
						<div class="flex justify-center">
							<img
								src={artworkPreview}
								alt=""
								class="h-36 w-36 rounded-lg object-cover shadow-md"
								decoding="async"
								onerror={() => (artworkPreviewFailed = true)}
							/>
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
										<div
											class="flex items-center justify-between border-b border-stroke-subtle px-3 py-2 last:border-b-0"
										>
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
			{/if}
		</div>
	{/snippet}
</Drawer>
