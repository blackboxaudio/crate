<script lang="ts">
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import { settingsStore, audioCacheLimitMb, artworkCacheLimitMb } from '$shared/stores/settings'
	import * as discoveryApi from '$shared/api/discovery'
	import { formatFileSize } from '$shared/utils/format'
	import { confirmDialog } from '$lib/utils/dialog'

	// Storage: the preview audio + artwork caches (sizes, clear actions, LRU caps). Moved from the
	// old flat SettingsView's Cache section. Note "Clear" removes pinned (downloaded-for-offline)
	// audio too — it's the explicit bulk reclaim; per-release "Remove Download" lives in the release
	// detail menu. The clear commands emit `discovery-cache-changed`, so row badges refresh on their own.
	let cacheSize = $state(0)
	let clearing = $state(false)
	let artworkCacheSize = $state(0)
	let clearingArtwork = $state(false)

	// Cache-size cap presets (MB). Audio previews are large; artwork is small.
	const audioCachePresets = [250, 500, 1000, 2000]
	const artworkCachePresets = [100, 250, 500]

	function formatCap(mb: number): string {
		return mb >= 1000 ? `${mb / 1000} GB` : `${mb} MB`
	}

	onMount(async () => {
		try {
			cacheSize = await discoveryApi.getAudioCacheSize()
		} catch {
			cacheSize = 0
		}

		try {
			artworkCacheSize = await discoveryApi.getArtworkCacheSize()
		} catch {
			artworkCacheSize = 0
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

	async function handleClearArtworkCache() {
		const t = get(translate)
		const confirmed = await confirmDialog(t('settings.discovery.clearArtworkCacheConfirmMessage'), {
			title: t('settings.discovery.artworkCache'),
			confirmLabel: t('settings.discovery.clearCache'),
			kind: 'warning',
		})
		if (!confirmed) return
		clearingArtwork = true
		try {
			await discoveryApi.clearArtworkCache()
			artworkCacheSize = 0
		} finally {
			clearingArtwork = false
		}
	}
</script>

<div class="px-4 py-2">
	<h3 class="mb-2 text-sm font-medium text-text-secondary">
		{$translate('settings.discovery.previewCache')}
	</h3>

	<!-- Audio cache -->
	<div class="flex items-center justify-between">
		<p class="text-sm text-text-primary">
			{$translate('settings.discovery.audioCache')} · {formatFileSize(cacheSize)}
		</p>
		<button
			type="button"
			class="rounded-md bg-surface-2 px-3 py-1.5 text-sm font-medium text-text-secondary active:opacity-70 disabled:opacity-50"
			onclick={handleClearCache}
			disabled={cacheSize === 0 || clearing}
		>
			{$translate('settings.discovery.clearCache')}
		</button>
	</div>
	<div class="mt-2 flex items-center justify-between gap-3">
		<span class="text-xs text-text-tertiary">{$translate('settings.discovery.cacheLimit')}</span>
		<div class="inline-flex gap-1">
			{#each audioCachePresets as mb (mb)}
				<button
					type="button"
					class="rounded-md border px-2.5 py-1 text-xs font-medium transition-colors {$audioCacheLimitMb === mb
						? 'border-brand-primary bg-brand-primary text-white'
						: 'border-stroke-subtle bg-surface-2 text-text-secondary active:opacity-70'}"
					onclick={() => settingsStore.setAudioCacheLimitMb(mb)}
				>
					{formatCap(mb)}
				</button>
			{/each}
		</div>
	</div>

	<!-- Artwork cache -->
	<p class="mt-4 text-xs text-text-tertiary">
		{$translate('settings.discovery.artworkCacheDescription')}
	</p>
	<div class="mt-1.5 flex items-center justify-between">
		<p class="text-sm text-text-primary">
			{$translate('settings.discovery.artworkCache')} · {formatFileSize(artworkCacheSize)}
		</p>
		<button
			type="button"
			class="rounded-md bg-surface-2 px-3 py-1.5 text-sm font-medium text-text-secondary active:opacity-70 disabled:opacity-50"
			onclick={handleClearArtworkCache}
			disabled={artworkCacheSize === 0 || clearingArtwork}
		>
			{$translate('settings.discovery.clearCache')}
		</button>
	</div>
	<div class="mt-2 flex items-center justify-between gap-3">
		<span class="text-xs text-text-tertiary">{$translate('settings.discovery.cacheLimit')}</span>
		<div class="inline-flex gap-1">
			{#each artworkCachePresets as mb (mb)}
				<button
					type="button"
					class="rounded-md border px-2.5 py-1 text-xs font-medium transition-colors {$artworkCacheLimitMb === mb
						? 'border-brand-primary bg-brand-primary text-white'
						: 'border-stroke-subtle bg-surface-2 text-text-secondary active:opacity-70'}"
					onclick={() => settingsStore.setArtworkCacheLimitMb(mb)}
				>
					{formatCap(mb)}
				</button>
			{/each}
		</div>
	</div>
</div>
