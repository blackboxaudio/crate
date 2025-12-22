<script lang="ts">
	import type { KeyNotationFormat } from '$lib/types'
	import { Text, Checkbox } from '$lib/components/common'
	import {
		settingsStore,
		keyNotationFormat,
		autoAnalyzeOnImport,
		autoSyncOnConnect,
		autoSyncOnChange,
	} from '$lib/stores/settings'
	import { translate } from '$lib/i18n'

	function handleKeyNotationFormatChange(format: KeyNotationFormat) {
		settingsStore.setKeyNotationFormat(format)
	}

	function handleAutoAnalyzeOnImportChange(checked: boolean) {
		settingsStore.setAutoAnalyzeOnImport(checked)
	}

	function handleAutoSyncOnConnectChange(checked: boolean) {
		settingsStore.setAutoSyncOnConnect(checked)
	}

	function handleAutoSyncOnChangeChange(checked: boolean) {
		settingsStore.setAutoSyncOnChange(checked)
	}
</script>

<div class="space-y-8">
	<!-- Key Notation Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.library.keyNotation')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.library.keyNotationDescription')}</Text>
		<div class="flex gap-3">
			<button
				type="button"
				class="flex flex-1 flex-col items-center gap-2 rounded-lg border-2 p-4
				transition-colors {$keyNotationFormat === 'camelot'
					? 'border-brand-primary bg-brand-muted'
					: 'border-stroke hover:cursor-pointer hover:border-text-tertiary'}"
				onclick={() => handleKeyNotationFormatChange('camelot')}
			>
				<Text variant="body-2" as="span">{$translate('settings.library.keyNotationCamelot')}</Text>
				<Text variant="caption" color="secondary">8A, 8B, 11A</Text>
			</button>
			<button
				type="button"
				class="flex flex-1 flex-col items-center gap-2 rounded-lg border-2 p-4
				transition-colors {$keyNotationFormat === 'standard'
					? 'border-brand-primary bg-brand-muted'
					: 'border-stroke hover:cursor-pointer hover:border-text-tertiary'}"
				onclick={() => handleKeyNotationFormatChange('standard')}
			>
				<Text variant="body-2" as="span">{$translate('settings.library.keyNotationStandard')}</Text>
				<Text variant="caption" color="secondary">Am, C, F#m</Text>
			</button>
		</div>
	</section>

	<!-- Analysis Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.library.analysis')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.library.autoAnalyzeOnImportDescription')}</Text>

		<!-- Auto-analyze on Import -->
		<Checkbox
			checked={$autoAnalyzeOnImport}
			onchange={handleAutoAnalyzeOnImportChange}
			label={$translate('settings.library.autoAnalyzeOnImport')}
		/>
	</section>

	<!-- Sync Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.library.sync')}</Text>
		<Text variant="caption" as="p" class="mb-4">{$translate('settings.library.syncDescription')}</Text>

		<div class="space-y-3">
			<div>
				<Checkbox
					checked={$autoSyncOnConnect}
					onchange={handleAutoSyncOnConnectChange}
					label={$translate('settings.library.autoSyncOnConnect')}
				/>
				<Text variant="caption" as="p" class="mt-1 ml-6 text-text-tertiary">
					{$translate('settings.library.autoSyncOnConnectDescription')}
				</Text>
			</div>

			<div>
				<Checkbox
					checked={$autoSyncOnChange}
					onchange={handleAutoSyncOnChangeChange}
					label={$translate('settings.library.autoSyncOnChange')}
				/>
				<Text variant="caption" as="p" class="mt-1 ml-6 text-text-tertiary">
					{$translate('settings.library.autoSyncOnChangeDescription')}
				</Text>
			</div>
		</div>
	</section>
</div>
