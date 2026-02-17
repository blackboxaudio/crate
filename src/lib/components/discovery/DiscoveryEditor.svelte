<script lang="ts">
	import { discoveryStore } from '$lib/stores/discovery'
	import { uiStore } from '$lib/stores/ui'
	import { toastStore } from '$lib/stores/toast'
	import type { DiscoveryRelease, DiscoveryReleaseUpdate, DiscoveryStatus } from '$lib/types'
	import Button from '$lib/components/common/Button.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import Select from '$lib/components/common/Select.svelte'
	import EditorField from '$lib/components/editor/EditorField.svelte'
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

	// Reset form when selection changes
	$effect(() => {
		/* eslint-disable @typescript-eslint/no-unused-expressions */
		selectedReleases.length
		formData = {}
	})

	let hasChanges = $derived(Object.keys(formData).length > 0)

	let statusOptions = $derived(
		(['unlistened', 'listened', 'purchased', 'dismissed'] as const).map((s) => ({
			value: s,
			label: $translate(`discovery.status.${s}`),
		}))
	)

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
		if (!hasChanges || saving) return

		saving = true
		try {
			const update: DiscoveryReleaseUpdate = {}
			for (const [key, value] of Object.entries(formData)) {
				if (value !== undefined) {
					;(update as Record<string, unknown>)[key] = value === '' ? null : value
				}
			}

			for (const release of selectedReleases) {
				await discoveryStore.updateRelease(release.id, update)
			}

			formData = {}
		} catch (error) {
			console.error('Failed to update releases:', error)
			toastStore.error(get(translate)('errors.generic'))
		} finally {
			saving = false
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
			status: computeValue((r) => r.status),
			artworkUrl: computeValue((r) => r.artwork_url),
		}
	}
</script>

<div class="flex h-full flex-col border-l border-stroke bg-surface-1">
	<!-- Header -->
	<div class="flex items-center justify-between px-4 py-4.5">
		<h2 class="text-sm font-semibold text-text-primary">
			{selectedReleases.length === 1
				? $translate('discovery.editor.releaseInfo')
				: $translate('discovery.editor.releasesCount', { values: { count: selectedReleases.length } })}
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
			/>
			<EditorField
				label={$translate('discovery.editor.title')}
				value={formData.title ?? bulkInfo.title.value}
				mixed={bulkInfo.title.mixed && formData.title === undefined}
				onchange={handleFieldChange('title')}
				onsubmit={handleSave}
			/>
			<EditorField
				label={$translate('discovery.editor.label')}
				value={formData.label ?? bulkInfo.label.value}
				mixed={bulkInfo.label.mixed && formData.label === undefined}
				onchange={handleFieldChange('label')}
				onsubmit={handleSave}
			/>
			<EditorField
				label={$translate('discovery.editor.releaseDate')}
				value={formData.release_date ?? bulkInfo.releaseDate.value}
				mixed={bulkInfo.releaseDate.mixed && formData.release_date === undefined}
				onchange={handleFieldChange('release_date')}
				onsubmit={handleSave}
			/>
			<EditorField
				label={$translate('discovery.editor.notes')}
				value={formData.notes ?? bulkInfo.notes.value}
				mixed={bulkInfo.notes.mixed && formData.notes === undefined}
				onchange={handleFieldChange('notes')}
				onsubmit={handleSave}
			/>

			<!-- Status dropdown -->
			<label class="block space-y-1">
				<span class="block text-xs font-medium text-text-secondary">{$translate('discovery.editor.status')}</span>
				<Select
					value={formData.status ?? bulkInfo.status.value ?? 'unlistened'}
					options={statusOptions}
					onchange={(value) => {
						formData = { ...formData, status: value as DiscoveryStatus }
					}}
				/>
			</label>
		</div>
	</div>

	<!-- Footer -->
	<div class="p-4">
		<Button variant="primary" class="w-full" onclick={handleSave} disabled={!hasChanges || saving}>
			{saving ? $translate('discovery.editor.saving') : $translate('discovery.editor.saveChanges')}
		</Button>
	</div>
</div>
