<script lang="ts">
	import { translate, SUPPORTED_LANGUAGES } from '$shared/i18n'
	import type { DateFormat } from '$shared/types'
	import { settingsStore, language, dateFormat } from '$shared/stores/settings'

	// General: Language + Date format (desktop's General tab minus its desktop-only backup group).
	// Both write through the shared settings store, so they sync to other devices; the language list
	// is the same shared SUPPORTED_LANGUAGES source desktop renders.
	const dateFormats: { value: DateFormat; key: string }[] = [
		{ value: 'locale', key: 'settings.general.dateFormatLocale' },
		{ value: 'iso', key: 'settings.general.dateFormatIso' },
		{ value: 'us', key: 'settings.general.dateFormatUs' },
		{ value: 'eu', key: 'settings.general.dateFormatEu' },
		{ value: 'dot', key: 'settings.general.dateFormatDot' },
	]
</script>

<div class="px-4 py-2">
	<h3 class="mb-1 text-sm font-medium text-text-secondary">{$translate('settings.general.language')}</h3>
	<p class="mb-2 text-xs text-text-tertiary">{$translate('settings.general.languageDescription')}</p>
	<div class="divide-y divide-stroke-subtle overflow-hidden rounded-xl border border-stroke-subtle">
		{#each SUPPORTED_LANGUAGES as lang (lang.value)}
			<button
				type="button"
				class="flex min-h-[44px] w-full items-center justify-between gap-2 px-3 py-2 text-left active:bg-surface-2"
				aria-pressed={$language === lang.value}
				onclick={() => void settingsStore.setLanguage(lang.value)}
			>
				<span class="flex min-w-0 flex-col leading-tight">
					<span class="truncate text-sm text-text-primary">{lang.nativeLabel}</span>
					<span class="truncate text-xs text-text-tertiary">{lang.label}</span>
				</span>
				{#if $language === lang.value}
					<svg
						class="h-4 w-4 flex-shrink-0 text-brand-primary"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2.5"
					>
						<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				{/if}
			</button>
		{/each}
	</div>

	<h3 class="mt-5 mb-1 text-sm font-medium text-text-secondary">{$translate('settings.general.dateFormat')}</h3>
	<p class="mb-2 text-xs text-text-tertiary">{$translate('settings.general.dateFormatDescription')}</p>
	<div class="divide-y divide-stroke-subtle overflow-hidden rounded-xl border border-stroke-subtle">
		{#each dateFormats as format (format.value)}
			<button
				type="button"
				class="flex min-h-[44px] w-full items-center justify-between gap-2 px-3 py-2 text-left active:bg-surface-2"
				aria-pressed={$dateFormat === format.value}
				onclick={() => void settingsStore.setDateFormat(format.value)}
			>
				<span class="truncate text-sm text-text-primary">{$translate(format.key)}</span>
				{#if $dateFormat === format.value}
					<svg
						class="h-4 w-4 flex-shrink-0 text-brand-primary"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2.5"
					>
						<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				{/if}
			</button>
		{/each}
	</div>
</div>
