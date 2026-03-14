<script lang="ts">
	import type { Theme, AccentColor, Font } from '$lib/types'
	import { Select, Text } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import { settingsStore, theme, accentColor, font } from '$lib/stores/settings'
	import { translate } from '$lib/i18n'

	const themeOptions: { value: Theme; labelKey: string }[] = [
		{ value: 'light', labelKey: 'settings.appearance.themeLight' },
		{ value: 'dark', labelKey: 'settings.appearance.themeDark' },
		{ value: 'system', labelKey: 'settings.appearance.themeSystem' },
	]

	const accentColors: { value: AccentColor; hex: string; labelKey: string }[] = [
		{ value: 'blue', hex: '#3b82f6', labelKey: 'colors.blue' },
		{ value: 'indigo', hex: '#6366f1', labelKey: 'colors.indigo' },
		{ value: 'violet', hex: '#8b5cf6', labelKey: 'colors.violet' },
		{ value: 'purple', hex: '#a855f7', labelKey: 'colors.purple' },
		{ value: 'pink', hex: '#ec4899', labelKey: 'colors.pink' },
		{ value: 'rose', hex: '#f43f5e', labelKey: 'colors.rose' },
		{ value: 'orange', hex: '#f97316', labelKey: 'colors.orange' },
		{ value: 'amber', hex: '#f59e0b', labelKey: 'colors.amber' },
		{ value: 'emerald', hex: '#10b981', labelKey: 'colors.emerald' },
		{ value: 'teal', hex: '#14b8a6', labelKey: 'colors.teal' },
	]

	const fontOptions: { value: Font; label: string; style: string }[] = [
		{ value: 'inter', label: 'Inter', style: "font-family: 'Inter', sans-serif" },
		{ value: 'nunito', label: 'Nunito', style: "font-family: 'Nunito', sans-serif" },
		{ value: 'open-sans', label: 'Open Sans', style: "font-family: 'Open Sans', sans-serif" },
		{ value: 'fira-code', label: 'Fira Code', style: "font-family: 'Fira Code', monospace" },
		{ value: 'ibm-plex-mono', label: 'IBM Plex Mono', style: "font-family: 'IBM Plex Mono', monospace" },
		{ value: 'source-code-pro', label: 'Source Code Pro', style: "font-family: 'Source Code Pro', monospace" },
	]
</script>

<div class="flex w-full max-w-lg flex-col gap-6">
	<div class="text-center">
		<Text variant="header-2" weight="bold" class="mb-2">{$translate('onboarding.appearance.title')}</Text>
		<Text variant="body-2" color="secondary">{$translate('onboarding.appearance.description')}</Text>
	</div>

	<!-- Theme -->
	<section>
		<Text variant="header-4" class="mb-3">{$translate('settings.appearance.theme')}</Text>
		<div class="flex gap-3">
			{#each themeOptions as option (option.value)}
				<button
					type="button"
					class="flex flex-1 flex-col items-center gap-2 rounded-lg border-2 p-3
					transition-colors {$theme === option.value
						? 'border-brand-primary bg-brand-muted'
						: 'border-stroke hover:cursor-pointer hover:border-text-tertiary'}"
					onclick={() => settingsStore.setTheme(option.value)}
				>
					{#if option.value === 'light'}
						<Icon name="sun" class="h-5 w-5" />
					{:else if option.value === 'dark'}
						<Icon name="moon" class="h-5 w-5" />
					{:else}
						<Icon name="monitor" class="h-5 w-5" />
					{/if}
					<Text variant="caption">{$translate(option.labelKey)}</Text>
				</button>
			{/each}
		</div>
	</section>

	<!-- Accent Color -->
	<section>
		<Text variant="header-4" class="mb-3">{$translate('settings.appearance.accentColor')}</Text>
		<div class="grid grid-cols-5 gap-2">
			{#each accentColors as color (color.value)}
				<button
					type="button"
					class="group flex flex-col items-center gap-1.5 rounded-lg p-2
					transition-colors hover:cursor-pointer hover:bg-surface-2"
					onclick={() => settingsStore.setAccentColor(color.value)}
					title={$translate(color.labelKey)}
				>
					<div
						class="h-7 w-7 rounded-full transition-transform
						group-hover:scale-110 {$accentColor === color.value
							? 'ring-2 ring-text-primary ring-offset-2 ring-offset-surface-0'
							: ''}"
						style="background-color: {color.hex};"
					></div>
					<Text variant="caption" color="secondary" class="text-[10px]">{$translate(color.labelKey)}</Text>
				</button>
			{/each}
		</div>
	</section>

	<!-- Font -->
	<section>
		<Text variant="header-4" class="mb-3">{$translate('settings.appearance.font')}</Text>
		<Select
			value={$font}
			options={fontOptions}
			placeholder={$translate('settings.appearance.font')}
			onchange={(value) => settingsStore.setFont(value as Font)}
			class="[&>button]:border-1 [&>button]:bg-surface-0"
		/>
	</section>
</div>
