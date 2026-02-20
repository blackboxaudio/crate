<script lang="ts">
	import type { Theme, AccentColor, Font } from '$lib/types'
	import { Select, Text } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import { settingsStore, theme, accentColor, font } from '$lib/stores/settings'
	import { translate } from '$lib/i18n'

	// Theme options
	const themeOptions: { value: Theme; labelKey: string }[] = [
		{ value: 'light', labelKey: 'settings.appearance.themeLight' },
		{ value: 'dark', labelKey: 'settings.appearance.themeDark' },
		{ value: 'system', labelKey: 'settings.appearance.themeSystem' },
	]

	// Accent color options
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

	// Font options
	const fontOptions: { value: Font; label: string; style: string }[] = [
		{ value: 'open-sans', label: 'Open Sans', style: "font-family: 'Open Sans', sans-serif" },
		{ value: 'inter', label: 'Inter', style: "font-family: 'Inter', sans-serif" },
		{ value: 'fira-code', label: 'Fira Code', style: "font-family: 'Fira Code', monospace" },
		{ value: 'jetbrains-mono', label: 'JetBrains Mono', style: "font-family: 'JetBrains Mono', monospace" },
		{ value: 'ibm-plex-mono', label: 'IBM Plex Mono', style: "font-family: 'IBM Plex Mono', monospace" },
	]

	function handleThemeChange(newTheme: Theme) {
		settingsStore.setTheme(newTheme)
	}

	function handleAccentChange(newColor: AccentColor) {
		settingsStore.setAccentColor(newColor)
	}

	function handleFontChange(value: string) {
		settingsStore.setFont(value as Font)
	}
</script>

<div class="space-y-8">
	<!-- Font Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.appearance.font')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.appearance.fontDescription')}</Text>
		<div class="max-w-md">
			<Select
				value={$font}
				options={fontOptions}
				placeholder={$translate('settings.appearance.font')}
				onchange={handleFontChange}
			/>
		</div>
	</section>

	<!-- Theme Section -->
	<section>
		<Text variant="header-3" class="mb-4">{$translate('settings.appearance.theme')}</Text>
		<div class="flex gap-3">
			{#each themeOptions as option (option.value)}
				<button
					type="button"
					class="flex flex-1 flex-col items-center gap-2 rounded-lg border-2 p-4
					transition-colors {$theme === option.value
						? 'border-brand-primary bg-brand-muted'
						: 'border-stroke hover:cursor-pointer hover:border-text-tertiary'}"
					onclick={() => handleThemeChange(option.value)}
				>
					{#if option.value === 'light'}
						<Icon name="sun" class="h-6 w-6" />
					{:else if option.value === 'dark'}
						<Icon name="moon" class="h-6 w-6" />
					{:else}
						<Icon name="monitor" class="h-6 w-6" />
					{/if}
					<Text variant="body-2" as="span">{$translate(option.labelKey)}</Text>
				</button>
			{/each}
		</div>
	</section>

	<!-- Accent Color Section -->
	<section>
		<Text variant="header-3" class="mb-4">{$translate('settings.appearance.accentColor')}</Text>
		<div class="grid grid-cols-5 gap-3">
			{#each accentColors as color (color.value)}
				<button
					type="button"
					class="group flex flex-col items-center gap-2 rounded-lg p-3
					transition-colors hover:cursor-pointer hover:bg-surface-2"
					onclick={() => handleAccentChange(color.value)}
					title={$translate(color.labelKey)}
				>
					<div
						class="h-8 w-8 rounded-full transition-transform
						group-hover:scale-110 {$accentColor === color.value
							? 'ring-2 ring-text-primary ring-offset-2 ring-offset-surface-1'
							: ''}"
						style="background-color: {color.hex};"
					></div>
					<Text variant="caption" color="secondary">{$translate(color.labelKey)}</Text>
				</button>
			{/each}
		</div>
	</section>
</div>
