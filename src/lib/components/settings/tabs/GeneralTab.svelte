<script lang="ts">
	import type { Language, DateFormat } from '$lib/types'
	import { Select, Text } from '$lib/components/common'
	import { settingsStore, language, dateFormat } from '$lib/stores/settings'
	import { SUPPORTED_LANGUAGES, translate } from '$lib/i18n'

	const languageOptions = SUPPORTED_LANGUAGES.map((lang) => ({
		value: lang.value,
		label: lang.nativeLabel,
		sublabel: lang.label,
	}))

	let dateFormatOptions = $derived([
		{ value: 'locale', label: $translate('settings.general.dateFormatLocale') },
		{ value: 'iso', label: $translate('settings.general.dateFormatIso') },
		{ value: 'us', label: $translate('settings.general.dateFormatUs') },
		{ value: 'eu', label: $translate('settings.general.dateFormatEu') },
		{ value: 'dot', label: $translate('settings.general.dateFormatDot') },
	])

	function handleLanguageChange(value: string) {
		settingsStore.setLanguage(value as Language)
	}

	function handleDateFormatChange(value: string) {
		settingsStore.setDateFormat(value as DateFormat)
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

	<!-- Date Format Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.general.dateFormat')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.general.dateFormatDescription')}</Text>
		<div class="max-w-md">
			<Select
				value={$dateFormat}
				options={dateFormatOptions}
				placeholder={$translate('settings.general.dateFormat')}
				onchange={handleDateFormatChange}
			/>
		</div>
	</section>
</div>
