<script lang="ts">
	import { discoveryStore, refreshingReleaseIds } from '$lib/stores/discovery'
	import { dateFormat } from '$lib/stores/settings'
	import { uiStore } from '$lib/stores/ui'
	import { toastStore } from '$lib/stores/toast'
	import { formatDate } from '$lib/utils'
	import type { DiscoveryRelease, DiscoveryReleaseUpdate } from '$lib/types'
	import Button from '$lib/components/common/Button.svelte'
	import IconButton from '$lib/components/common/IconButton.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import EditorField from '$lib/components/editor/EditorField.svelte'
	import { EditorTextArea, EditorArtwork } from '$lib/components/editor'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	type Props = {
		selectedReleases: DiscoveryRelease[]
		onImport?: (release: DiscoveryRelease) => void
	}

	let { selectedReleases, onImport }: Props = $props()

	// Compute bulk info from selected releases
	let bulkInfo = $derived(computeBulkReleaseInfo(selectedReleases))

	// Form state - only track changed values
	let formData = $state<Partial<DiscoveryReleaseUpdate>>({})
	let saving = $state(false)
	let pendingSave = $state(false)
	let refreshing = $derived(selectedReleases.length === 1 && $refreshingReleaseIds.has(selectedReleases[0].id))

	async function handleRefreshMetadata() {
		if (refreshing || selectedReleases.length !== 1) return
		await discoveryStore.refreshMetadata(selectedReleases[0].id)
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

	async function handleArtworkAdd(filePath: string) {
		for (const release of selectedReleases) {
			await discoveryStore.setArtwork(release.id, filePath)
		}
	}

	async function handleArtworkRemove() {
		for (const release of selectedReleases) {
			await discoveryStore.deleteArtwork(release.id)
		}
	}

	// Helper to compute bulk release info
	function computeBulkReleaseInfo(releases: DiscoveryRelease[]) {
		function computeValue<T>(getter: (r: DiscoveryRelease) => T | null): {
			value: T | null
			mixed: boolean
			count: number
		} {
			const values = releases.map(getter)
			const nonNull = values.filter((v): v is T => v !== null)
			if (nonNull.length === 0) return { value: null, mixed: false, count: 0 }
			const first = nonNull[0]
			const allSame = nonNull.every((v) => v === first)
			if (allSame && nonNull.length === releases.length) return { value: first, mixed: false, count: nonNull.length }
			return {
				value: allSame ? first : null,
				mixed: !allSame || nonNull.length !== releases.length,
				count: nonNull.length,
			}
		}

		return {
			artist: computeValue((r) => r.artist),
			title: computeValue((r) => r.title),
			label: computeValue((r) => r.label),
			releaseDate: computeValue((r) => r.release_date),
			notes: computeValue((r) => r.notes),
			artworkUrl: computeValue((r) => r.artwork_url),
			artworkPath: computeValue((r) => r.artwork_path),
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
				<Tooltip text={$translate('discovery.refreshMetadata')} position="bottom" delay={250}>
					<IconButton
						icon="refresh"
						size="sm"
						iconClass={refreshing ? 'animate-spin h-4 w-4' : ''}
						onclick={handleRefreshMetadata}
					/>
				</Tooltip>
			{/if}
			<Tooltip text={$translate('common.close')} position="bottom" delay={250}>
				<IconButton icon="x" size="sm" onclick={handleClose} />
			</Tooltip>
		</div>
	</div>

	<!-- Scrollable content -->
	<div class="flex-1 space-y-6 overflow-y-auto p-4">
		<!-- Artwork -->
		<EditorArtwork
			artworkPath={bulkInfo.artworkPath}
			artworkSource={{ value: null, mixed: false, count: 0 }}
			trackCount={selectedReleases.length}
			artworkUrl={bulkInfo.artworkUrl.mixed ? null : bulkInfo.artworkUrl.value}
			onAdd={handleArtworkAdd}
			onRemove={handleArtworkRemove}
			onReextract={() => {}}
		/>

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
				value={(() => {
					const raw = formData.release_date ?? bulkInfo.releaseDate.value
					return raw ? formatDate(raw, $dateFormat) : null
				})()}
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

		<!-- Import button -->
		{#if selectedReleases.length === 1 && onImport}
			<div class="border-t border-stroke pt-4">
				<Button variant="outline" class="w-full" onclick={() => onImport?.(selectedReleases[0])}>
					{$translate('discovery.importToLibrary')}
				</Button>
			</div>
		{/if}
	</div>
</div>
