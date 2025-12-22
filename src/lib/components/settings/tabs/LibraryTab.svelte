<script lang="ts">
	import type { KeyNotationFormat } from '$lib/types'
	import { Text, Checkbox } from '$lib/components/common'
	import { settingsStore, keyNotationFormat, autoAnalyzeOnImport } from '$lib/stores/settings'
	import { translate } from '$lib/i18n'

	function handleKeyNotationFormatChange(format: KeyNotationFormat) {
		settingsStore.setKeyNotationFormat(format)
	}

	function handleAutoAnalyzeOnImportChange(checked: boolean) {
		settingsStore.setAutoAnalyzeOnImport(checked)
	}
</script>

<div class="space-y-8">
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
</div>
