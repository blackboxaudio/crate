<script lang="ts">
	import { open } from '@tauri-apps/plugin-dialog'
	import { withNativeDialog } from '$shared/utils'
	import { translate } from '$shared/i18n'
	import { get } from 'svelte/store'
	import Modal from '$lib/components/common/Modal.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import * as libraryApi from '$shared/api/library'
	import { missingTracksStore, cloudSyncStore } from '$lib/stores'
	import type { Track, FileMatchResult } from '$shared/types'

	type Props = {
		open: boolean
		track: Track | null
		onClose: () => void
		onRelocate: (track: Track) => void
	}

	let { open: isOpen, track, onClose, onRelocate }: Props = $props()

	let selectedPath: string | null = $state(null)
	let validationResult: FileMatchResult | null = $state(null)
	let validating = $state(false)
	let relocating = $state(false)
	let forceRelocate = $state(false)
	let error: string | null = $state(null)

	// Cloud-synced tracks (with library_root_id) locate by mapping the root folder —
	// fixing all sibling tracks at once. Device-local tracks keep the per-file flow.
	const isCloudSynced = $derived(track?.library_root_id != null)

	// Reset state when modal opens/closes
	$effect(() => {
		if (!isOpen) {
			selectedPath = null
			validationResult = null
			validating = false
			relocating = false
			forceRelocate = false
			error = null
		}
	})

	async function handleSelectFile() {
		const selected = await withNativeDialog(() =>
			open({
				multiple: false,
				title: get(translate)('modals.relocate.selectReplacementFile'),
				filters: [
					{
						name: get(translate)('common.audioFiles'),
						extensions: ['mp3', 'wav', 'aiff', 'aif', 'flac', 'm4a', 'aac'],
					},
				],
			})
		)

		if (selected && typeof selected === 'string') {
			selectedPath = selected
			error = null
			forceRelocate = false
			await validateFile()
		}
	}

	async function handleSelectFolder() {
		const selected = await withNativeDialog(() =>
			open({
				directory: true,
				multiple: false,
				title: get(translate)('modals.relocate.selectRootFolder'),
			})
		)

		if (selected && typeof selected === 'string') {
			selectedPath = selected
			error = null
		}
	}

	async function validateFile() {
		if (!track || !selectedPath) return

		validating = true
		validationResult = null
		error = null

		try {
			validationResult = await libraryApi.validateReplacementFile(track.id, selectedPath)
		} catch (e) {
			error = e instanceof Error ? e.message : get(translate)('modals.relocate.validateFailed')
		} finally {
			validating = false
		}
	}

	async function handleRelocate() {
		if (!track || !selectedPath) return

		relocating = true
		error = null

		try {
			if (isCloudSynced) {
				await cloudSyncStore.locateTrack(track.id, selectedPath)
				missingTracksStore.markFound(track.id)
				onRelocate(track)
				onClose()
			} else {
				const updatedTrack = await libraryApi.relocateTrack(
					track.id,
					selectedPath,
					forceRelocate || (validationResult?.matches ?? false)
				)
				missingTracksStore.markFound(track.id)
				onRelocate(updatedTrack)
				onClose()
			}
		} catch (e) {
			error = e instanceof Error ? e.message : get(translate)('modals.relocate.relocateFailed')
		} finally {
			relocating = false
		}
	}

	const canRelocate = $derived(() => {
		if (!selectedPath || validating || relocating) return false
		if (isCloudSynced) return true
		if (!validationResult) return false
		if (!validationResult.format_valid) return false
		return validationResult.matches || forceRelocate
	})

	const displayTitle = $derived(track?.title || $translate('common.untitled'))
	const displayArtist = $derived(track?.artist || $translate('common.unknownArtist'))
</script>

<Modal open={isOpen} title={$translate('modals.relocate.title')} {onClose}>
	<div class="space-y-4">
		<!-- Track Info -->
		<div class="rounded-md bg-surface-2 p-3">
			<Text variant="body-2" truncate>{displayTitle}</Text>
			<Text color="secondary" truncate>{displayArtist}</Text>
			{#if track}
				<Text variant="caption" color="tertiary" truncate class="mt-2" title={track.file_path}>
					{track.file_path}
				</Text>
			{/if}
		</div>

		<!-- Warning Banner -->
		<div class="flex gap-2 rounded-md border border-warning/20 bg-warning/10 p-3">
			<Icon name="warning" class="h-5 w-5 flex-shrink-0 text-warning" />
			<Text color="warning">
				{#if isCloudSynced}
					{$translate('modals.relocate.cloudSyncedMessage')}
				{:else}
					{$translate('modals.relocate.missingFileMessage')}
				{/if}
			</Text>
		</div>

		<!-- File / Folder Picker -->
		<div class="space-y-2">
			{#if isCloudSynced}
				<Button variant="secondary" onclick={handleSelectFolder} class="w-full">
					<Icon name="folder-open" class="mr-2 h-4 w-4" />
					{$translate('modals.relocate.selectRootFolder')}
				</Button>
			{:else}
				<Button variant="secondary" onclick={handleSelectFile} class="w-full">
					<Icon name="folder" class="mr-2 h-4 w-4" />
					{$translate('modals.relocate.selectReplacementFile')}
				</Button>
			{/if}

			{#if selectedPath}
				<Text color="secondary" truncate title={selectedPath}>
					{selectedPath}
				</Text>
			{/if}
		</div>

		<!-- Validation Status (device-local file flow only) -->
		{#if !isCloudSynced && validating}
			<div class="flex items-center gap-2 text-sm text-text-secondary">
				<div class="h-4 w-4 animate-spin rounded-full border-2 border-brand-primary border-t-transparent"></div>
				{$translate('modals.relocate.validating')}
			</div>
		{:else if !isCloudSynced && validationResult}
			{#if !validationResult.format_valid}
				<div class="flex gap-2 rounded-md border border-red-500/20 bg-red-500/10 p-3">
					<Icon name="close" class="h-5 w-5 flex-shrink-0 text-red-500" />
					<Text color="danger">{$translate('modals.relocate.unsupportedFormat')}</Text>
				</div>
			{:else if validationResult.matches}
				<div class="flex gap-2 rounded-md border border-green-500/20 bg-green-500/10 p-3">
					<Icon name="check" class="h-5 w-5 flex-shrink-0 text-green-500" />
					<Text color="success">{$translate('modals.relocate.hashMatch')}</Text>
				</div>
			{:else}
				<div class="space-y-3">
					<div class="flex gap-2 rounded-md border border-amber-500/20 bg-amber-500/10 p-3">
						<Icon name="warning" class="h-5 w-5 flex-shrink-0 text-amber-500" />
						<div>
							<Text weight="medium" color="warning">{$translate('modals.relocate.hashMismatch')}</Text>
							{#if !validationResult.original_hash}
								<Text size="xs" color="warning" class="mt-1">{$translate('modals.relocate.noOriginalHash')}</Text>
							{/if}
						</div>
					</div>

					<label class="flex cursor-pointer items-center gap-2">
						<input
							type="checkbox"
							bind:checked={forceRelocate}
							class="h-4 w-4 rounded border-stroke bg-surface-2 text-brand-primary focus:ring-brand-primary focus:ring-offset-0"
						/>
						<Text color="secondary" as="span">{$translate('modals.relocate.useAnyway')}</Text>
					</label>
				</div>
			{/if}
		{/if}

		<!-- Error Message -->
		{#if error}
			<div class="flex gap-2 rounded-md border border-red-500/20 bg-red-500/10 p-3">
				<Icon name="close" class="h-5 w-5 flex-shrink-0 text-red-500" />
				<Text color="danger">{error}</Text>
			</div>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onClose} disabled={relocating}>{$translate('common.cancel')}</Button>
		<Button variant="primary" onclick={handleRelocate} disabled={!canRelocate()}>
			{#if relocating}
				{$translate('modals.relocate.relocating')}
			{:else}
				{$translate('modals.relocate.relocate')}
			{/if}
		</Button>
	{/snippet}
</Modal>
