<script lang="ts">
	import { toastStore } from '$lib/stores/toast'
	import { libraryStore } from '$lib/stores/library'
	import { syncStore } from '$lib/stores/sync'
	import { uiStore } from '$lib/stores/ui'
	import { computeBulkTrackInfo } from '$lib/utils'
	import * as libraryApi from '$lib/api/library'
	import type { Track, TrackUpdate } from '$lib/types'
	import Icon from '$lib/components/common/Icon.svelte'
	import IconButton from '$lib/components/common/IconButton.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import EditorField from './EditorField.svelte'
	import EditorArtwork from './EditorArtwork.svelte'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	type Props = {
		selectedTracks: Track[]
		onSave?: () => void
	}

	let { selectedTracks, onSave }: Props = $props()

	// Compute bulk info from selected tracks
	let bulkInfo = $derived(computeBulkTrackInfo(selectedTracks))

	// Resolved artwork path when multiple tracks share identical artwork
	let resolvedArtworkPath = $state<string | null>(null)

	// Form state - only track changed values
	let formData = $state<Partial<TrackUpdate>>({})
	let saving = $state(false)
	let pendingSave = $state(false)

	// Reset form when selection changes
	$effect(() => {
		// Accessing selectedTracks to trigger on change
		/* eslint-disable @typescript-eslint/no-unused-expressions */
		selectedTracks.length
		formData = {}
	})

	// Compare artworks when paths are mixed to check if they're actually identical
	$effect(() => {
		if (bulkInfo.artworkPath.mixed && selectedTracks.length > 1) {
			const trackIds = selectedTracks.map((t) => t.id)
			libraryApi
				.compareTrackArtworks(trackIds)
				.then((path) => {
					resolvedArtworkPath = path
				})
				.catch(() => {
					resolvedArtworkPath = null
				})
		} else {
			resolvedArtworkPath = null
		}
	})

	// Check if there are any changes
	let hasChanges = $derived(Object.keys(formData).length > 0)

	function handleFieldChange(field: keyof TrackUpdate) {
		return (value: string | number | null) => {
			if (value === null || value === '') {
				// If value is cleared, remove from formData (or set to empty string for clearing)
				formData = { ...formData, [field]: value === null ? undefined : '' }
			} else {
				formData = { ...formData, [field]: value }
			}
		}
	}

	async function handleSave() {
		if (!hasChanges) return
		if (saving) {
			pendingSave = true
			return
		}

		saving = true
		const snapshot = { ...formData }
		try {
			const ids = selectedTracks.map((t) => t.id)
			const update: TrackUpdate = {}

			// Only include fields that have actual values
			for (const [key, value] of Object.entries(snapshot)) {
				if (value !== undefined) {
					;(update as Record<string, unknown>)[key] = value === '' ? null : value
				}
			}

			const updatedTracks = await libraryApi.updateTracks(ids, update)

			// Update the library store with the new track data
			libraryStore.updateTracksInState(updatedTracks)

			// Notify sync store about track changes (for auto-sync)
			syncStore.notifyTrackChanges(ids)

			onSave?.()

			// Only clear snapshotted keys (preserve any new edits made during save)
			const updated = { ...formData }
			for (const key of Object.keys(snapshot)) {
				if (updated[key as keyof TrackUpdate] === snapshot[key as keyof TrackUpdate]) {
					delete updated[key as keyof TrackUpdate]
				}
			}
			formData = updated
		} catch (error) {
			console.error('Failed to update tracks:', error)
			toastStore.error(get(translate)('toast.failedToUpdateTracks'))
		} finally {
			saving = false
			if (pendingSave) {
				pendingSave = false
				handleSave()
			}
		}
	}

	async function handleArtworkAdd(filePath: string) {
		if (selectedTracks.length === 0) return

		try {
			// For bulk, we apply the same artwork to all selected tracks
			const updatedTracks: Track[] = []
			for (const track of selectedTracks) {
				const updatedTrack = await libraryApi.setTrackArtwork(track.id, filePath)
				updatedTracks.push(updatedTrack)
			}
			libraryStore.updateTracksInState(updatedTracks)

			// Notify sync store about track changes (for auto-sync)
			syncStore.notifyTrackChanges(updatedTracks.map((t) => t.id))
		} catch (error) {
			console.error('Failed to set artwork:', error)
			toastStore.error(get(translate)('toast.failedToSetArtwork'))
		}
	}

	async function handleArtworkRemove() {
		if (selectedTracks.length === 0) return

		try {
			const updatedTracks: Track[] = []
			for (const track of selectedTracks) {
				const updatedTrack = await libraryApi.deleteTrackArtwork(track.id)
				updatedTracks.push(updatedTrack)
			}
			libraryStore.updateTracksInState(updatedTracks)

			// Notify sync store about track changes (for auto-sync)
			syncStore.notifyTrackChanges(updatedTracks.map((t) => t.id))
		} catch (error) {
			console.error('Failed to remove artwork:', error)
			toastStore.error(get(translate)('toast.failedToRemoveArtwork'))
		}
	}

	async function handleArtworkReextract() {
		if (selectedTracks.length !== 1) return

		try {
			const updatedTrack = await libraryApi.reextractTrackArtwork(selectedTracks[0].id)
			libraryStore.updateTracksInState([updatedTrack])

			// Notify sync store about track changes (for auto-sync)
			syncStore.notifyTrackChanges([updatedTrack.id])
		} catch (error) {
			console.error('Failed to re-extract artwork:', error)
			toastStore.error(get(translate)('toast.noArtworkInFile'))
		}
	}

	function handleClose() {
		uiStore.setRightSidebarVisible(false)
	}
</script>

<div class="flex h-full flex-col border-l border-stroke bg-surface-1">
	<!-- Header -->
	<div class="flex items-center justify-between px-4 py-4.5">
		<Text variant="header-2" as="h2">
			{selectedTracks.length === 1
				? $translate('editor.trackInfo')
				: $translate('editor.tracksCount', { values: { count: selectedTracks.length } })}
		</Text>
		<Tooltip text={$translate('common.close')} position="bottom" delay={250}>
			<IconButton icon="x" size="sm" onclick={handleClose} />
		</Tooltip>
	</div>

	<!-- Scrollable content -->
	<div class="flex-1 space-y-6 overflow-y-auto p-4">
		<!-- Artwork section -->
		<EditorArtwork
			artworkPath={bulkInfo.artworkPath}
			artworkSource={bulkInfo.artworkSource}
			trackCount={selectedTracks.length}
			{resolvedArtworkPath}
			onAdd={handleArtworkAdd}
			onRemove={handleArtworkRemove}
			onReextract={handleArtworkReextract}
		/>

		<!-- Divider -->
		<div class="border-t border-stroke"></div>

		<!-- Fields section -->
		<div class="space-y-4">
			<EditorField
				label={$translate('editor.title')}
				value={formData.title ?? bulkInfo.title.value}
				mixed={bulkInfo.title.mixed && formData.title === undefined}
				onchange={handleFieldChange('title')}
				onsubmit={handleSave}
				onblur={handleSave}
			/>
			<EditorField
				label={$translate('editor.artist')}
				value={formData.artist ?? bulkInfo.artist.value}
				mixed={bulkInfo.artist.mixed && formData.artist === undefined}
				onchange={handleFieldChange('artist')}
				onsubmit={handleSave}
				onblur={handleSave}
			/>
			<EditorField
				label={$translate('editor.album')}
				value={formData.album ?? bulkInfo.album.value}
				mixed={bulkInfo.album.mixed && formData.album === undefined}
				onchange={handleFieldChange('album')}
				onsubmit={handleSave}
				onblur={handleSave}
			/>

			<div class="grid grid-cols-2 gap-3">
				<EditorField
					label={$translate('editor.year')}
					type="number"
					value={formData.year ?? bulkInfo.year.value}
					mixed={bulkInfo.year.mixed && formData.year === undefined}
					onchange={handleFieldChange('year')}
					onsubmit={handleSave}
					onblur={handleSave}
				/>
				<EditorField
					label={$translate('editor.genre')}
					value={formData.genre ?? bulkInfo.genre.value}
					mixed={bulkInfo.genre.mixed && formData.genre === undefined}
					onchange={handleFieldChange('genre')}
					onsubmit={handleSave}
					onblur={handleSave}
				/>
			</div>

			<EditorField
				label={$translate('editor.label')}
				value={formData.label ?? bulkInfo.label.value}
				mixed={bulkInfo.label.mixed && formData.label === undefined}
				onchange={handleFieldChange('label')}
				onsubmit={handleSave}
				onblur={handleSave}
			/>

			<div class="grid grid-cols-2 gap-3">
				<EditorField
					label={$translate('editor.bpm')}
					type="number"
					value={formData.bpm ?? bulkInfo.bpm.value}
					mixed={bulkInfo.bpm.mixed && formData.bpm === undefined}
					onchange={handleFieldChange('bpm')}
					onsubmit={handleSave}
					onblur={handleSave}
				/>
				<EditorField
					label={$translate('editor.key')}
					value={formData.key ?? bulkInfo.key.value}
					mixed={bulkInfo.key.mixed && formData.key === undefined}
					onchange={handleFieldChange('key')}
					onsubmit={handleSave}
					onblur={handleSave}
				/>
			</div>
		</div>
	</div>
</div>
