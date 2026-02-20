<script lang="ts">
	import { Text, Checkbox } from '$lib/components/common'
	import {
		settingsStore,
		autoFetchMetadata,
		transferTagsOnImport,
		removeReleaseAfterImport,
	} from '$lib/stores/settings'
	import { translate } from '$lib/i18n'

	function handleAutoFetchMetadataChange(checked: boolean) {
		settingsStore.setAutoFetchMetadata(checked)
	}

	function handleTransferTagsOnImportChange(checked: boolean) {
		settingsStore.setTransferTagsOnImport(checked)
	}

	function handleRemoveReleaseAfterImportChange(checked: boolean) {
		settingsStore.setRemoveReleaseAfterImport(checked)
	}
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
</div>
