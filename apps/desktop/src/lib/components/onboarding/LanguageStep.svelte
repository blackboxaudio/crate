<script lang="ts">
	import { Text } from '$lib/components/common'
	import { settingsStore, language } from '$shared/stores/settings'
	import { SUPPORTED_LANGUAGES, translate } from '$shared/i18n'
	import type { Language } from '$shared/types'
</script>

<div class="flex w-full max-w-lg flex-col gap-6">
	<div class="text-center">
		<Text variant="header-2" weight="bold" class="mb-2">{$translate('onboarding.language.title')}</Text>
		<Text variant="body-2" color="secondary">{$translate('onboarding.language.description')}</Text>
	</div>

	<div class="grid grid-cols-3 gap-2">
		{#each SUPPORTED_LANGUAGES as lang (lang.value)}
			<button
				type="button"
				class="flex flex-col items-center gap-1 rounded-lg border-2 px-3 py-3 transition-colors
				{$language === lang.value
					? 'border-brand-primary bg-brand-muted'
					: 'border-stroke hover:cursor-pointer hover:border-text-tertiary'}"
				onclick={() => settingsStore.setLanguage(lang.value as Language)}
			>
				<Text variant="body-2" weight="medium">{lang.nativeLabel}</Text>
				<Text variant="caption" color="tertiary">{lang.label}</Text>
			</button>
		{/each}
	</div>
</div>
