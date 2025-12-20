<script lang="ts">
	import { toastStore } from '$lib/stores/toast'
	import { libraryStore } from '$lib/stores/library'
	import { uiStore } from '$lib/stores/ui'
	import { computeBulkTrackInfo } from '$lib/utils'
	import * as libraryApi from '$lib/api/library'
	import type { Track, TrackUpdate } from '$lib/types'
	import Button from '$lib/components/common/Button.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import EditorField from './EditorField.svelte'
	import EditorArtwork from './EditorArtwork.svelte'

	type Props = {
		selectedTracks: Track[]
	}

	let { selectedTracks }: Props = $props()

	// Compute bulk info from selected tracks
	let bulkInfo = $derived(computeBulkTrackInfo(selectedTracks))

	// Form state - only track changed values
	let formData = $state<Partial<TrackUpdate>>({})
	let saving = $state(false)

	// Reset form when selection changes
	$effect(() => {
		// Accessing selectedTracks to trigger on change
		/* eslint-disable @typescript-eslint/no-unused-expressions */
		selectedTracks.length
		formData = {}
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
		if (!hasChanges || saving) return

		saving = true
		try {
			const ids = selectedTracks.map((t) => t.id)
			const update: TrackUpdate = {}

			// Only include fields that have actual values
			for (const [key, value] of Object.entries(formData)) {
				if (value !== undefined) {
					;(update as Record<string, unknown>)[key] = value === '' ? null : value
				}
			}

			const updatedTracks = await libraryApi.updateTracks(ids, update)

			// Update the library store with the new track data
			libraryStore.updateTracksInState(updatedTracks)

			formData = {}
		} catch (error) {
			console.error('Failed to update tracks:', error)
			toastStore.error('Failed to update tracks')
		} finally {
			saving = false
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
		} catch (error) {
			console.error('Failed to set artwork:', error)
			toastStore.error('Failed to set artwork')
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
		} catch (error) {
			console.error('Failed to remove artwork:', error)
			toastStore.error('Failed to remove artwork')
		}
	}

	async function handleArtworkReextract() {
		if (selectedTracks.length !== 1) return

		try {
			const updatedTrack = await libraryApi.reextractTrackArtwork(selectedTracks[0].id)
			libraryStore.updateTracksInState([updatedTrack])
		} catch (error) {
			console.error('Failed to re-extract artwork:', error)
			toastStore.error('No artwork found in audio file')
		}
	}

	function handleClose() {
		uiStore.setRightSidebarVisible(false)
	}
</script>

<div class="flex h-full flex-col border-l border-stroke bg-surface-1">
	<!-- Header -->
	<div class="flex items-center justify-between border-b border-stroke px-4 py-3">
		<h2 class="text-sm font-semibold text-text-primary">
			{selectedTracks.length === 1 ? 'Track Info' : `${selectedTracks.length} Tracks`}
		</h2>
		<button
			class="rounded p-1 text-text-secondary transition-colors hover:cursor-pointer hover:bg-surface-2 hover:text-text-primary"
			onclick={handleClose}
		>
			<Icon name="x" class="h-4 w-4" />
		</button>
	</div>

	<!-- Scrollable content -->
	<div class="flex-1 space-y-6 overflow-y-auto p-4">
		<!-- Artwork section -->
		<EditorArtwork
			artworkPath={bulkInfo.artworkPath}
			artworkSource={bulkInfo.artworkSource}
			trackCount={selectedTracks.length}
			onAdd={handleArtworkAdd}
			onRemove={handleArtworkRemove}
			onReextract={handleArtworkReextract}
		/>

		<!-- Divider -->
		<div class="border-t border-stroke"></div>

		<!-- Fields section -->
		<div class="space-y-4">
			<EditorField
				label="Title"
				value={formData.title ?? bulkInfo.title.value}
				mixed={bulkInfo.title.mixed && formData.title === undefined}
				onchange={handleFieldChange('title')}
				onsubmit={handleSave}
			/>
			<EditorField
				label="Artist"
				value={formData.artist ?? bulkInfo.artist.value}
				mixed={bulkInfo.artist.mixed && formData.artist === undefined}
				onchange={handleFieldChange('artist')}
				onsubmit={handleSave}
			/>
			<EditorField
				label="Album"
				value={formData.album ?? bulkInfo.album.value}
				mixed={bulkInfo.album.mixed && formData.album === undefined}
				onchange={handleFieldChange('album')}
				onsubmit={handleSave}
			/>

			<div class="grid grid-cols-2 gap-3">
				<EditorField
					label="Year"
					type="number"
					value={formData.year ?? bulkInfo.year.value}
					mixed={bulkInfo.year.mixed && formData.year === undefined}
					onchange={handleFieldChange('year')}
					onsubmit={handleSave}
				/>
				<EditorField
					label="Genre"
					value={formData.genre ?? bulkInfo.genre.value}
					mixed={bulkInfo.genre.mixed && formData.genre === undefined}
					onchange={handleFieldChange('genre')}
					onsubmit={handleSave}
				/>
			</div>

			<EditorField
				label="Label"
				value={formData.label ?? bulkInfo.label.value}
				mixed={bulkInfo.label.mixed && formData.label === undefined}
				onchange={handleFieldChange('label')}
				onsubmit={handleSave}
			/>

			<div class="grid grid-cols-2 gap-3">
				<EditorField
					label="BPM"
					type="number"
					value={formData.bpm ?? bulkInfo.bpm.value}
					mixed={bulkInfo.bpm.mixed && formData.bpm === undefined}
					onchange={handleFieldChange('bpm')}
					onsubmit={handleSave}
				/>
				<EditorField
					label="Key"
					value={formData.key ?? bulkInfo.key.value}
					mixed={bulkInfo.key.mixed && formData.key === undefined}
					onchange={handleFieldChange('key')}
					onsubmit={handleSave}
				/>
			</div>
		</div>
	</div>

	<!-- Footer with save button -->
	<div class="p-4">
		<Button variant="primary" class="w-full" onclick={handleSave} disabled={!hasChanges || saving}>
			{saving ? 'Saving...' : 'Save Changes'}
		</Button>
	</div>
</div>
