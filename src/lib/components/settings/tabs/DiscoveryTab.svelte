<script lang="ts">
	import { Text, Checkbox, Button, ConfirmModal } from '$lib/components/common'
	import {
		settingsStore,
		autoFetchMetadata,
		transferTagsOnImport,
		removeReleaseAfterImport,
	} from '$lib/stores/settings'
	import { translate } from '$lib/i18n'
	import * as discoveryApi from '$lib/api/discovery'
	import { formatFileSize } from '$lib/utils/format'

	let cacheSize = $state(0)
	let clearing = $state(false)
	let showClearConfirm = $state(false)

	async function loadCacheSize() {
		try {
			cacheSize = await discoveryApi.getAudioCacheSize()
		} catch {
			cacheSize = 0
		}
	}

	async function handleClearCache() {
		showClearConfirm = false
		clearing = true
		try {
			await discoveryApi.clearAudioCache()
			cacheSize = 0
		} finally {
			clearing = false
		}
	}

	function handleAutoFetchMetadataChange(checked: boolean) {
		settingsStore.setAutoFetchMetadata(checked)
	}

	function handleTransferTagsOnImportChange(checked: boolean) {
		settingsStore.setTransferTagsOnImport(checked)
	}

	function handleRemoveReleaseAfterImportChange(checked: boolean) {
		settingsStore.setRemoveReleaseAfterImport(checked)
	}

	$effect(() => {
		loadCacheSize()
	})
</script>

<div class="space-y-8">
	<!-- Metadata Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.discovery.metadata')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.discovery.autoFetchMetadataDescription')}</Text>

		<Checkbox
			checked={$autoFetchMetadata}
			onchange={handleAutoFetchMetadataChange}
			label={$translate('settings.discovery.autoFetchMetadata')}
		/>
	</section>

	<!-- Import Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.discovery.import')}</Text>

		<div class="space-y-4">
			<div>
				<Text variant="caption" as="p" class="mb-2 text-text-tertiary">
					{$translate('settings.discovery.transferTagsOnImportDescription')}
				</Text>
				<Checkbox
					checked={$transferTagsOnImport}
					onchange={handleTransferTagsOnImportChange}
					label={$translate('settings.discovery.transferTagsOnImport')}
				/>
			</div>

			<div>
				<Text variant="caption" as="p" class="mb-2 text-text-tertiary">
					{$translate('settings.discovery.removeReleaseAfterImportDescription')}
				</Text>
				<Checkbox
					checked={$removeReleaseAfterImport}
					onchange={handleRemoveReleaseAfterImportChange}
					label={$translate('settings.discovery.removeReleaseAfterImport')}
				/>
			</div>
		</div>
	</section>

	<!-- Preview Cache Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.discovery.previewCache')}</Text>
		<Text variant="caption" as="p" class="mb-2 text-text-tertiary">
			{$translate('settings.discovery.previewCacheDescription')}
		</Text>

		<div class="flex items-center gap-4">
			<Text>{$translate('settings.discovery.cacheSize')}: {formatFileSize(cacheSize)}</Text>
			<Button variant="secondary" onclick={() => (showClearConfirm = true)} disabled={cacheSize === 0 || clearing}>
				{clearing ? $translate('common.loading') : $translate('settings.discovery.clearCache')}
			</Button>
		</div>
	</section>
</div>

<ConfirmModal
	open={showClearConfirm}
	title={$translate('settings.discovery.clearCache')}
	message={$translate('settings.discovery.clearCacheConfirmMessage')}
	warnings={[$translate('settings.discovery.clearCacheWarning')]}
	destructive={true}
	onConfirm={handleClearCache}
	onCancel={() => (showClearConfirm = false)}
/>
