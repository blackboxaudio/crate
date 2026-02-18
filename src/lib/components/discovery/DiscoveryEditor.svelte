<script lang="ts">
	import { discoveryStore } from '$lib/stores/discovery'
	import { uiStore } from '$lib/stores/ui'
	import { toastStore } from '$lib/stores/toast'
	import type { DiscoveryRelease, DiscoveryReleaseUpdate } from '$lib/types'
	import IconButton from '$lib/components/common/IconButton.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import EditorField from '$lib/components/editor/EditorField.svelte'
	import { EditorTextArea } from '$lib/components/editor'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	type Props = {
		selectedReleases: DiscoveryRelease[]
	}

	let { selectedReleases }: Props = $props()

	// Compute bulk info from selected releases
	let bulkInfo = $derived(computeBulkReleaseInfo(selectedReleases))

	// Form state - only track changed values
	let formData = $state<Partial<DiscoveryReleaseUpdate>>({})
	let saving = $state(false)
	let pendingSave = $state(false)
	let refreshing = $state(false)

	async function handleRefreshMetadata() {
		if (refreshing || selectedReleases.length !== 1) return
		refreshing = true
		try {
			await discoveryStore.refreshMetadata(selectedReleases[0].id)
		} finally {
			refreshing = false
		}
	}

	// Reset form when selection changes
	$effect(() => {
		/* eslint-disable @typescript-eslint/no-unused-expressions */
		selectedReleases.length
		formData = {}
	})

	let hasChanges = $derived(Object.keys(formData).length > 0)

	function handleFieldChange(field: keyof DiscoveryReleaseUpdate) {
		return (value: string | number | null) => {
			if (value === null || value === '') {
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
			const update: DiscoveryReleaseUpdate = {}
			for (const [key, value] of Object.entries(snapshot)) {
				if (value !== undefined) {
					;(update as Record<string, unknown>)[key] = value === '' ? null : value
				}
			}

			for (const release of selectedReleases) {
				await discoveryStore.updateRelease(release.id, update)
			}

			// Only clear snapshotted keys (preserve any new edits made during save)
			const updated = { ...formData }
			for (const key of Object.keys(snapshot)) {
				if (updated[key as keyof DiscoveryReleaseUpdate] === snapshot[key as keyof DiscoveryReleaseUpdate]) {
					delete updated[key as keyof DiscoveryReleaseUpdate]
				}
			}
			formData = updated
		} catch (error) {
			console.error('Failed to update releases:', error)
			toastStore.error(get(translate)('errors.generic'))
		} finally {
			saving = false
			if (pendingSave) {
				pendingSave = false
				handleSave()
			}
		}
	}

	function handleClose() {
		uiStore.setRightSidebarVisible(false)
	}

	// Helper to compute bulk release info
	function computeBulkReleaseInfo(releases: DiscoveryRelease[]) {
		function computeValue<T>(getter: (r: DiscoveryRelease) => T | null): {
			value: T | null
			mixed: boolean
		} {
			const values = releases.map(getter)
			const nonNull = values.filter((v): v is T => v !== null)
			if (nonNull.length === 0) return { value: null, mixed: false }
			const first = nonNull[0]
			const allSame = nonNull.every((v) => v === first)
			if (allSame && nonNull.length === releases.length) return { value: first, mixed: false }
			return { value: allSame ? first : null, mixed: !allSame || nonNull.length !== releases.length }
		}

		return {
			artist: computeValue((r) => r.artist),
			title: computeValue((r) => r.title),
			label: computeValue((r) => r.label),
			releaseDate: computeValue((r) => r.release_date),
			notes: computeValue((r) => r.notes),
			artworkUrl: computeValue((r) => r.artwork_url),
		}
	}
</script>

<div class="flex h-full flex-col border-l border-stroke bg-surface-1">
	<!-- Header -->
	<div class="flex items-center justify-between px-4 py-4.5">
		<Text variant="header-2" as="h2">
			{selectedReleases.length === 1
				? $translate('discovery.editor.releaseInfo')
				: $translate('discovery.editor.releasesCount', { values: { count: selectedReleases.length } })}
		</Text>
		<div class="flex items-center gap-1">
			{#if selectedReleases.length === 1}
				{#if refreshing}
					<Spinner class="mx-1.5 h-3.5 w-3.5" />
				{:else}
					<Tooltip text={$translate('discovery.refreshMetadata')} position="bottom" delay={250}>
						<IconButton icon="refresh" size="sm" onclick={handleRefreshMetadata} />
					</Tooltip>
				{/if}
			{/if}
			<Tooltip text={$translate('common.close')} position="bottom" delay={250}>
				<IconButton icon="x" size="sm" onclick={handleClose} />
			</Tooltip>
		</div>
	</div>

	<!-- Scrollable content -->
	<div class="flex-1 space-y-6 overflow-y-auto p-4">
		<!-- Artwork (read-only, from URL) -->
		{#if selectedReleases.length === 1 && bulkInfo.artworkUrl.value}
			<div class="flex justify-center">
				<img src={bulkInfo.artworkUrl.value} alt="Release artwork" class="h-48 w-48 rounded-md object-cover" />
			</div>
		{/if}

		<!-- Divider -->
		<div class="border-t border-stroke"></div>

		<!-- Fields -->
		<div class="space-y-4">
			<EditorField
				label={$translate('discovery.editor.artist')}
				value={formData.artist ?? bulkInfo.artist.value}
				mixed={bulkInfo.artist.mixed && formData.artist === undefined}
				onchange={handleFieldChange('artist')}
				onsubmit={handleSave}
				onblur={handleSave}
			/>
			<EditorField
				label={$translate('discovery.editor.title')}
				value={formData.title ?? bulkInfo.title.value}
				mixed={bulkInfo.title.mixed && formData.title === undefined}
				onchange={handleFieldChange('title')}
				onsubmit={handleSave}
				onblur={handleSave}
			/>
			<EditorField
				label={$translate('discovery.editor.label')}
				value={formData.label ?? bulkInfo.label.value}
				mixed={bulkInfo.label.mixed && formData.label === undefined}
				onchange={handleFieldChange('label')}
				onsubmit={handleSave}
				onblur={handleSave}
			/>
			<EditorField
				label={$translate('discovery.editor.releaseDate')}
				value={formData.release_date ?? bulkInfo.releaseDate.value}
				mixed={bulkInfo.releaseDate.mixed && formData.release_date === undefined}
				disabled={true}
			/>
			<EditorTextArea
				label={$translate('discovery.editor.notes')}
				value={formData.notes ?? bulkInfo.notes.value}
				mixed={bulkInfo.notes.mixed && formData.notes === undefined}
				onchange={handleFieldChange('notes')}
				onblur={handleSave}
			/>
		</div>
	</div>
</div>
