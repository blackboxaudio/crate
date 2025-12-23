<script lang="ts">
	import type { Language } from '$lib/types'
	import { Select, Text } from '$lib/components/common'
	import { settingsStore, language } from '$lib/stores/settings'
	import { SUPPORTED_LANGUAGES, translate } from '$lib/i18n'

	const languageOptions = SUPPORTED_LANGUAGES.map((lang) => ({
		value: lang.value,
		label: lang.nativeLabel,
		sublabel: lang.label,
	}))

	function handleLanguageChange(value: string) {
		settingsStore.setLanguage(value as Language)
	}
</script>

<div class="space-y-8">
	<!-- Language Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.general.language')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.general.languageDescription')}</Text>
		<div class="max-w-md">
			<Select
				value={$language}
				options={languageOptions}
				placeholder={$translate('settings.general.language')}
				onchange={handleLanguageChange}
			/>
		</div>
	</section>
</div>
