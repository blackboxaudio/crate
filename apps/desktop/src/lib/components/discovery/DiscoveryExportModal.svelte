<script lang="ts">
	import { save } from '@tauri-apps/plugin-dialog'
	import { writeTextFile } from '@tauri-apps/plugin-fs'
	import { get } from 'svelte/store'
	import type { DiscoveryRelease } from '$shared/types'
	import type { DiscoveryExportScope, DiscoveryExportShape, DiscoveryExportTrackDetail } from '$shared/utils'
	import {
		buildDiscoveryExport,
		buildDiscoveryExportFilename,
		estimateDiscoveryExportBytes,
		formatFileSize,
		withNativeDialog,
	} from '$shared/utils'
	import { translate } from '$shared/i18n'
	import { tagsStore } from '$shared/stores/tags'
	import * as appApi from '$shared/api/app'
	import * as discoveryApi from '$shared/api/discovery'
	import { Modal, Button, Select, Spinner, Text } from '$lib/components/common'

	type Props = {
		open: boolean
		scope: DiscoveryExportScope
		/** The releases to export for the `selection` scope; ignored for `collection`, which refetches. */
		releases?: DiscoveryRelease[]
		onClose: () => void
	}
	let { open, scope, releases = [], onClose }: Props = $props()

	let shape = $state<DiscoveryExportShape>('curated')
	let trackDetail = $state<DiscoveryExportTrackDetail>('all')
	let fetched = $state.raw<DiscoveryRelease[]>([])
	let fetching = $state(false)
	let exporting = $state(false)
	let failed = $state(false)
	let appVersion = $state<string | undefined>(undefined)

	// The collection scope refetches unfiltered: the discovery store only ever holds the
	// current tag/search/playlist view, never the whole collection.
	$effect(() => {
		if (!open) return
		failed = false
		exporting = false
		appApi
			.getAppInfo()
			.then((info) => (appVersion = info.version))
			.catch(() => (appVersion = undefined))
		if (scope !== 'collection') return
		fetching = true
		discoveryApi
			.getReleases({})
			.then((result) => (fetched = result))
			.catch(() => {
				fetched = []
				failed = true
			})
			.finally(() => (fetching = false))
	})

	const sourceReleases = $derived(scope === 'collection' ? fetched : releases)

	const shapeOptions = $derived([
		{ value: 'curated', label: $translate('discovery.export.formatCurated') },
		{ value: 'raw', label: $translate('discovery.export.formatRaw') },
	])
	const trackOptions = $derived([
		{ value: 'all', label: $translate('discovery.export.tracksAll') },
		{ value: 'liked', label: $translate('discovery.export.tracksLiked') },
		{ value: 'none', label: $translate('discovery.export.tracksNone') },
	])
	const shapeHint = $derived(
		shape === 'raw' ? $translate('discovery.export.formatRawHint') : $translate('discovery.export.formatCuratedHint')
	)

	const estimatedBytes = $derived(
		estimateDiscoveryExportBytes(sourceReleases, $tagsStore.categories, {
			shape,
			trackDetail,
			scope,
			generatedAt: '',
		})
	)

	const canExport = $derived(!exporting && !fetching && sourceReleases.length > 0)

	async function handleExport() {
		if (!canExport) return
		exporting = true
		failed = false
		try {
			const path = await withNativeDialog(() =>
				save({
					defaultPath: buildDiscoveryExportFilename(scope),
					filters: [{ name: 'JSON', extensions: ['json'] }],
				})
			)
			if (!path) return
			const payload = buildDiscoveryExport(sourceReleases, get(tagsStore).categories, {
				shape,
				trackDetail,
				scope,
				appVersion,
				generatedAt: new Date().toISOString(),
			})
			await writeTextFile(path, JSON.stringify(payload, null, 2))
			onClose()
		} catch (error) {
			console.error('Discovery export failed:', error)
			failed = true
		} finally {
			exporting = false
		}
	}
</script>

<Modal {open} {onClose} title={$translate('discovery.export.title')} size="md" onSubmit={handleExport}>
	<div class="flex flex-col gap-5">
		<Text variant="caption" as="p" class="text-text-tertiary">
			{$translate('discovery.export.description')}
		</Text>

		<div class="flex flex-col gap-2">
			<Text variant="body-2" as="label" weight="medium">{$translate('discovery.export.format')}</Text>
			<Select
				value={shape}
				options={shapeOptions}
				onchange={(v) => (shape = v as DiscoveryExportShape)}
				disabled={exporting}
				class="w-full"
			/>
			<Text variant="caption" as="p" class="text-text-tertiary">{shapeHint}</Text>
		</div>

		<div class="flex flex-col gap-2">
			<Text variant="body-2" as="label" weight="medium">{$translate('discovery.export.tracks')}</Text>
			<Select
				value={trackDetail}
				options={trackOptions}
				onchange={(v) => (trackDetail = v as DiscoveryExportTrackDetail)}
				disabled={exporting}
				class="w-full"
			/>
		</div>

		<div class="flex min-h-5 items-center gap-2">
			{#if fetching}
				<Spinner />
				<Text variant="caption" class="text-text-tertiary">{$translate('discovery.export.preparing')}</Text>
			{:else if failed}
				<Text variant="caption" color="danger">{$translate('discovery.export.failed')}</Text>
			{:else}
				<Text variant="caption" class="text-text-tertiary">
					{$translate('discovery.export.summary', {
						values: { count: sourceReleases.length, size: formatFileSize(estimatedBytes) },
					})}
				</Text>
			{/if}
		</div>
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose} disabled={exporting}>{$translate('common.cancel')}</Button>
		<Button variant="primary" onclick={handleExport} disabled={!canExport}>
			{exporting ? $translate('export.exporting') : $translate('export.export')}
		</Button>
	{/snippet}
</Modal>
