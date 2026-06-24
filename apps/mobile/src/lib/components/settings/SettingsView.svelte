<script lang="ts">
	import { onMount, tick } from 'svelte'
	import { get } from 'svelte/store'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { translate } from '$shared/i18n'
	import type { Theme, AccentColor } from '$shared/types'
	import { settingsStore, theme, accentColor } from '$shared/stores/settings'
	import * as discoveryApi from '$shared/api/discovery'
	import { formatFileSize } from '$shared/utils/format'
	import { confirmDialog } from '$lib/utils/dialog'
	import { mobileUIStore, settingsScrollTarget } from '$lib/stores/mobileUI'
	import SyncPanel from '$lib/components/cloud-sync/SyncPanel.svelte'
	// @ts-expect-error — PUBLIC_APP_VERSION is set dynamically by vite.config.ts
	import { PUBLIC_APP_VERSION } from '$env/static/public'

	const themeOptions: { value: Theme; key: string }[] = [
		{ value: 'light', key: 'settings.appearance.themeLight' },
		{ value: 'dark', key: 'settings.appearance.themeDark' },
		{ value: 'system', key: 'settings.appearance.themeSystem' },
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

	let cacheSize = $state(0)
	let clearing = $state(false)

	let scrollContainer: HTMLDivElement | undefined

	onMount(async () => {
		try {
			cacheSize = await discoveryApi.getAudioCacheSize()
		} catch {
			cacheSize = 0
		}

		const target = get(settingsScrollTarget)
		if (target) {
			mobileUIStore.consumeSettingsScrollTarget()
			await tick()
			const el = scrollContainer?.querySelector(`[data-section="${target}"]`)
			el?.scrollIntoView({ behavior: 'smooth', block: 'start' })
		}
	})

	async function handleClearCache() {
		const t = get(translate)
		const confirmed = await confirmDialog(t('settings.discovery.clearCacheConfirmMessage'), {
			title: t('settings.discovery.clearCache'),
			confirmLabel: t('settings.discovery.clearCache'),
			kind: 'warning',
		})
		if (!confirmed) return
		clearing = true
		try {
			await discoveryApi.clearAudioCache()
			cacheSize = 0
		} finally {
			clearing = false
		}
	}
</script>

<div
	bind:this={scrollContainer}
	class="h-full overflow-y-auto pt-2"
	style="padding-bottom: var(--mini-player-inset, 0px)"
>
	<!-- Appearance -->
	<div class="px-4 py-2">
		<h3 class="mb-1.5 text-sm font-medium text-text-secondary">
			{$translate('settings.tabs.appearance')}
		</h3>
		<div class="flex gap-2">
			{#each themeOptions as option (option.value)}
				<button
					type="button"
					class="flex flex-1 flex-col items-center gap-1.5 rounded-md px-3 py-2.5 text-sm font-medium transition-colors {$theme ===
					option.value
						? 'bg-brand-primary text-white'
						: 'bg-surface-2 text-text-secondary active:bg-surface-2'}"
					onclick={() => settingsStore.setTheme(option.value)}
				>
					<svg
						class="h-5 w-5"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					>
						{#if option.value === 'light'}
							<circle cx="12" cy="12" r="5" />
							<line x1="12" y1="1" x2="12" y2="3" />
							<line x1="12" y1="21" x2="12" y2="23" />
							<line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
							<line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
							<line x1="1" y1="12" x2="3" y2="12" />
							<line x1="21" y1="12" x2="23" y2="12" />
							<line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
							<line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
						{:else if option.value === 'dark'}
							<path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z" />
						{:else}
							<rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
							<line x1="8" y1="21" x2="16" y2="21" />
							<line x1="12" y1="17" x2="12" y2="21" />
						{/if}
					</svg>
					{$translate(option.key)}
				</button>
			{/each}
		</div>

		<h3 class="mt-4 mb-1.5 text-sm font-medium text-text-secondary">
			{$translate('settings.appearance.accentColor')}
		</h3>
		<div class="grid grid-cols-5 gap-3">
			{#each accentColors as color (color.value)}
				<button
					type="button"
					class="flex items-center justify-center py-1"
					onclick={() => settingsStore.setAccentColor(color.value)}
					title={$translate(color.labelKey)}
				>
					<div
						class="h-7 w-7 rounded-full {$accentColor === color.value
							? 'ring-2 ring-text-primary ring-offset-2 ring-offset-surface-1'
							: ''}"
						style="background-color: {color.hex};"
					></div>
				</button>
			{/each}
		</div>
	</div>

	<!-- Sync -->
	<div data-section="sync" class="mt-2 border-t border-stroke-subtle px-4 py-3">
		<h3 class="mb-3 text-sm font-medium text-text-secondary">
			{$translate('settings.tabs.cloudSync')}
		</h3>
		<SyncPanel />
	</div>

	<!-- Cache -->
	<div class="mt-2 border-t border-stroke-subtle px-4 py-3">
		<h3 class="mb-1.5 text-sm font-medium text-text-secondary">
			{$translate('settings.discovery.previewCache')}
		</h3>
		<div class="flex items-center justify-between">
			<p class="text-sm text-text-primary">{formatFileSize(cacheSize)}</p>
			<button
				type="button"
				class="rounded-md bg-surface-2 px-3 py-1.5 text-sm font-medium text-text-secondary active:opacity-70 disabled:opacity-50"
				onclick={handleClearCache}
				disabled={cacheSize === 0 || clearing}
			>
				{$translate('settings.discovery.clearCache')}
			</button>
		</div>
	</div>

	<!-- About -->
	<div class="mt-2 border-t border-stroke-subtle px-4 py-3">
		<h3 class="mb-1.5 text-sm font-medium text-text-secondary">
			{$translate('settings.tabs.about')}
		</h3>
		<div class="flex flex-col gap-2">
			<div class="flex items-center justify-between">
				<p class="text-sm text-text-secondary">{$translate('settings.about.version')}</p>
				<p class="text-sm text-text-primary">{PUBLIC_APP_VERSION}</p>
			</div>
			<div class="flex items-center justify-between">
				<p class="text-sm text-text-secondary">{$translate('settings.about.project')}</p>
				<button
					type="button"
					class="text-sm font-medium text-brand-primary active:opacity-70"
					onclick={() => void openUrl('https://github.com/blackboxaudio/crate')}
				>
					GitHub
				</button>
			</div>
		</div>
	</div>
</div>
