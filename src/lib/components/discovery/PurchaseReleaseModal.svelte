<script lang="ts">
	import type { DiscoveryRelease, DiscoveryTrack, ImportResultWithDuplicates } from '$lib/types'
	import { Modal, Button, Text, Checkbox, Spinner, Icon } from '$lib/components/common'
	import { discoveryStore } from '$lib/stores/discovery'
	import { transferTagsOnImport, removeReleaseAfterImport } from '$lib/stores/settings'
	import { translate } from '$lib/i18n'
	import { open } from '@tauri-apps/plugin-dialog'
	import { withNativeDialog } from '$lib/utils'
	import { SvelteSet } from 'svelte/reactivity'

	type Props = {
		open: boolean
		release: DiscoveryRelease
		onClose: () => void
		onComplete: (result: ImportResultWithDuplicates) => void
	}

	let { open: isOpen, release, onClose, onComplete }: Props = $props()

	let filePaths = $state<string[]>([])
	let matchResults = $state<{ matched: { path: string; track: DiscoveryTrack | null }[]; unmatched: string[] }>({
		matched: [],
		unmatched: [],
	})
	let importAll = $state(false)
	let transferTags = $state($transferTagsOnImport)
	let importing = $state(false)

	let hasReleaseTags = $derived(release.tags.length > 0)
	let hasReleaseTracks = $derived(release.tracks.length > 0)

	let importableCount = $derived(importAll ? filePaths.length : matchResults.matched.length)
	let canImport = $derived(!importing && filePaths.length > 0 && importableCount > 0)

	// Auto-match when file paths change
	$effect(() => {
		if (filePaths.length > 0 && hasReleaseTracks) {
			matchResults = matchFilesToTracks(filePaths, release.tracks, release.artist, release.title)
		} else if (filePaths.length > 0) {
			// No tracks to match against - all go to unmatched
			matchResults = { matched: [], unmatched: filePaths }
		} else {
			matchResults = { matched: [], unmatched: [] }
		}
	})

	// When no tracks to match, default importAll to true
	$effect(() => {
		if (filePaths.length > 0 && !hasReleaseTracks) {
			importAll = true
		}
	})

	async function handleSelectFiles() {
		const selected = await withNativeDialog(() =>
			open({
				multiple: true,
				filters: [{ name: 'Audio Files', extensions: ['mp3', 'wav', 'aiff', 'aif', 'flac', 'm4a', 'aac'] }],
			})
		)
		if (selected && Array.isArray(selected) && selected.length > 0) {
			filePaths = selected
		}
	}

	async function handleImport() {
		if (!canImport) return
		importing = true

		const pathsToImport = importAll ? filePaths : matchResults.matched.map((m) => m.path)

		try {
			const result = await discoveryStore.purchaseRelease(
				release.id,
				pathsToImport,
				transferTags && hasReleaseTags,
				$removeReleaseAfterImport
			)
			if (result) {
				onComplete(result)
			}
		} finally {
			importing = false
		}
	}

	function handleClose() {
		if (importing) return
		onClose()
	}

	// =========================================================================
	// File-to-Track Matching
	// =========================================================================

	function normalizeName(name: string): string {
		return name
			.toLowerCase()
			.replace(/\.[^.]+$/, '') // strip extension
			.replace(/^\d+[\s._-]+/, '') // strip leading track number patterns
			.replace(/[_-]+/g, ' ')
			.trim()
	}

	function getFileName(path: string): string {
		return path.split('/').pop() ?? path
	}

	function stripReleasePrefix(filename: string, artist: string | null, title: string | null): string {
		const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
		const sep = '\\s*-\\s*'

		const patterns: string[] = []
		if (artist && title) patterns.push(escape(artist) + sep + escape(title) + sep)
		if (title) patterns.push(escape(title) + sep)
		if (artist) patterns.push(escape(artist) + sep)

		for (const pat of patterns) {
			const re = new RegExp('^' + pat, 'i')
			const result = filename.replace(re, '')
			if (result !== filename) return result
		}

		return filename
	}

	function matchFilesToTracks(
		paths: string[],
		tracks: DiscoveryTrack[],
		artist: string | null,
		title: string | null
	): { matched: { path: string; track: DiscoveryTrack | null }[]; unmatched: string[] } {
		const matched: { path: string; track: DiscoveryTrack | null }[] = []
		const unmatched: string[] = []
		const usedTrackIds = new SvelteSet<string>()

		const normalizedTracks = tracks.map((t) => ({ track: t, normalized: normalizeName(t.name) }))
		const fileEntries = paths.map((p) => ({
			path: p,
			normalized: normalizeName(stripReleasePrefix(getFileName(p), artist, title)),
		}))

		// Pass 1: Exact match on prefix-stripped, normalized names
		for (const entry of fileEntries) {
			const exact = normalizedTracks.find((t) => !usedTrackIds.has(t.track.id) && entry.normalized === t.normalized)
			if (exact) {
				matched.push({ path: entry.path, track: exact.track })
				usedTrackIds.add(exact.track.id)
			}
		}

		// Pass 2: Substring match for remaining files
		const matchedPaths = new Set(matched.map((m) => m.path))
		for (const entry of fileEntries) {
			if (matchedPaths.has(entry.path)) continue

			let bestMatch: DiscoveryTrack | null = null
			for (const t of normalizedTracks) {
				if (usedTrackIds.has(t.track.id)) continue
				if (entry.normalized.includes(t.normalized) || t.normalized.includes(entry.normalized)) {
					bestMatch = t.track
					break
				}
			}

			if (bestMatch) {
				matched.push({ path: entry.path, track: bestMatch })
				usedTrackIds.add(bestMatch.id)
			} else {
				unmatched.push(entry.path)
			}
		}

		// Pass 3: Position-based fallback if no matches and file count equals track count
		if (matched.length === 0 && paths.length === tracks.length) {
			const sorted = [...tracks].sort((a, b) => a.position - b.position)
			return {
				matched: paths.map((path, i) => ({ path, track: sorted[i] })),
				unmatched: [],
			}
		}

		return { matched, unmatched }
	}
</script>

<Modal open={isOpen} title={$translate('discovery.import.title')} onClose={handleClose}>
	<div class="flex flex-col gap-4">
		<!-- Release info -->
		<div class="flex items-center gap-3">
			{#if release.artwork_url}
				<img src={release.artwork_url} alt="" class="h-12 w-12 rounded object-cover" />
			{:else}
				<div class="flex h-12 w-12 items-center justify-center rounded bg-surface-2">
					<Icon name="music-note" class="h-5 w-5 text-text-tertiary" />
				</div>
			{/if}
			<div class="min-w-0 flex-1">
				<Text size="sm" weight="medium" class="truncate">
					{release.artist ?? $translate('common.unknownArtist')} – {release.title ?? $translate('common.untitled')}
				</Text>
				<Text size="xs" color="tertiary">
					{#if hasReleaseTracks}
						{$translate('discovery.trackCount', { values: { count: release.tracks.length } })}
					{/if}
					{#if release.source_type && release.source_type !== 'other'}
						{#if hasReleaseTracks}&middot;{/if}
						{release.source_type.charAt(0).toUpperCase() + release.source_type.slice(1)}
					{/if}
				</Text>
			</div>
		</div>

		<div class="border-t border-stroke-subtle"></div>

		<!-- File selection -->
		<div class="flex items-center gap-3">
			<Button variant="outline" size="sm" onclick={handleSelectFiles} disabled={importing}>
				{$translate('discovery.import.selectFiles')}
			</Button>
			<Text size="xs" color="tertiary">
				{#if filePaths.length > 0}
					{$translate('discovery.import.filesSelected', { values: { count: filePaths.length } })}
				{:else}
					{$translate('discovery.import.noFilesSelected')}
				{/if}
			</Text>
		</div>

		<!-- Match results -->
		{#if filePaths.length > 0}
			<div class="max-h-52 overflow-y-auto rounded-md border border-stroke bg-surface-2">
				<!-- Matched files -->
				{#if matchResults.matched.length > 0}
					<div class="border-b border-stroke-subtle px-3 py-1.5">
						<Text size="xs" weight="medium" color="secondary">
							{$translate('discovery.import.matched')} ({matchResults.matched.length})
						</Text>
					</div>
					{#each matchResults.matched as match (match.path)}
						<div class="flex items-center gap-2 px-3 py-1.5">
							<Icon name="check" class="h-3 w-3 shrink-0 text-green-500" />
							<Text size="xs" class="min-w-0 truncate">
								{getFileName(match.path)}
							</Text>
							{#if match.track}
								<Text size="xs" color="tertiary" class="shrink-0">&rarr;</Text>
								<Text size="xs" color="secondary" class="min-w-0 truncate">{match.track.name}</Text>
							{/if}
						</div>
					{/each}
				{/if}

				<!-- Unmatched files -->
				{#if matchResults.unmatched.length > 0}
					<div class="border-b border-stroke-subtle px-3 py-1.5" class:border-t={matchResults.matched.length > 0}>
						<Text size="xs" weight="medium" color="secondary">
							{$translate('discovery.import.unmatched')} ({matchResults.unmatched.length})
						</Text>
					</div>
					{#each matchResults.unmatched as path (path)}
						<div class="flex items-center gap-2 px-3 py-1.5">
							<div class="h-3 w-3 shrink-0 rounded-full border border-stroke"></div>
							<Text size="xs" color="tertiary" class="min-w-0 truncate">
								{getFileName(path)}
							</Text>
						</div>
					{/each}
				{/if}
			</div>

			<!-- Options -->
			<div class="flex flex-col gap-2">
				{#if matchResults.unmatched.length > 0 || !hasReleaseTracks}
					<Checkbox
						checked={importAll}
						onchange={(v) => (importAll = v)}
						label={$translate('discovery.import.importAllFiles')}
						disabled={importing}
					/>
				{/if}
				<Checkbox
					checked={transferTags}
					onchange={(v) => (transferTags = v)}
					label={hasReleaseTags
						? $translate('discovery.import.transferTagsCount', { values: { count: release.tags.length } })
						: $translate('discovery.import.transferTags')}
					disabled={importing || !hasReleaseTags}
				/>
			</div>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={handleClose} disabled={importing}>
			{$translate('common.cancel')}
		</Button>
		<Button variant="primary" disabled={!canImport} onclick={handleImport}>
			{#if importing}
				<span class="flex items-center gap-2">
					<Spinner class="h-3.5 w-3.5" />
					{$translate('discovery.import.importing')}
				</span>
			{:else}
				{$translate('discovery.importToLibrary')}
			{/if}
		</Button>
	{/snippet}
</Modal>
