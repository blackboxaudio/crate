<script lang="ts">
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease } from '$shared/types'
	import { discoveryStore } from '$shared/stores/discovery'
	import * as playbackQueue from '$shared/stores/playbackQueue'
	import { toastStore } from '$shared/stores/toast'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { getReleasePlatformName } from '$shared/utils/discoveryLinks'
	import { mobileUIStore, actionsReleaseId, actionsContext } from '$lib/stores/mobileUI'
	import { confirmDialog } from '$lib/utils/dialog'
	import { lightTap } from '$lib/utils/haptics'
	import MobileModal from '$lib/components/common/MobileModal.svelte'
	import SourceIcon from './SourceIcon.svelte'

	type Props = {
		releases: DiscoveryRelease[]
		playlistId?: string | null
		onAddToPlaylist?: (releaseId: string) => void
		onRemoveFromPlaylist?: (releaseId: string) => void
	}
	let { releases, playlistId = null, onAddToPlaylist, onRemoveFromPlaylist }: Props = $props()

	const releaseId = $derived($actionsReleaseId)
	const context = $derived($actionsContext)
	const release = $derived(releaseId ? (releases.find((r) => r.id === releaseId) ?? null) : null)
	const open = $derived(release != null)
	const platformName = $derived(release ? getReleasePlatformName(release.source_type) : null)

	function close() {
		mobileUIStore.closeActionsSheet()
	}

	function handleSelect() {
		if (!releaseId) return
		close()
		mobileUIStore.enterSelectMode(releaseId)
	}

	function handleAddToPlaylist() {
		if (!releaseId) return
		close()
		onAddToPlaylist?.(releaseId)
	}

	function handleRemoveFromPlaylist() {
		if (!releaseId) return
		close()
		onRemoveFromPlaylist?.(releaseId)
	}

	function handlePlayNext() {
		if (!release || release.tracks.length === 0) return
		void lightTap()
		playbackQueue.playNext(release, 0)
		toastStore.success(get(translate)('queue.playingNext'))
		close()
	}

	function handleAddToQueue() {
		if (!release || release.tracks.length === 0) return
		void lightTap()
		playbackQueue.addToQueue(release, 0)
		toastStore.success(get(translate)('queue.addedToQueue'))
		close()
	}

	function handleReorder() {
		close()
		mobileUIStore.toggleReorderMode()
	}

	async function handleDelete() {
		if (!releaseId) return
		const ok = await confirmDialog($translate('discovery.confirmDeleteMessage'), {
			title: $translate('discovery.confirmDeleteTitle', { values: { count: 1 } }),
			confirmLabel: $translate('common.delete'),
		})
		if (!ok) return
		close()
		await discoveryStore.deleteRelease(releaseId)
	}

	function handleOpenInSource() {
		if (!release) return
		void openUrl(release.url).catch(() => {})
		close()
	}
</script>

<MobileModal {open} onClose={close} title={release?.title ?? $translate('common.untitled')}>
	<div class="flex flex-col">
		<button
			type="button"
			class="flex items-center gap-3 rounded-md px-2 py-3 text-left text-sm text-text-primary active:bg-surface-2"
			onclick={handleAddToPlaylist}
		>
			<svg
				class="h-5 w-5 flex-shrink-0 text-text-secondary"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			>
				<path d="M12 5v14M5 12h14" stroke-linecap="round" />
			</svg>
			{$translate('contextMenu.addToPlaylist')}
		</button>

		<button
			type="button"
			class="flex items-center gap-3 rounded-md px-2 py-3 text-left text-sm text-text-primary active:bg-surface-2"
			onclick={handlePlayNext}
		>
			<svg class="h-5 w-5 flex-shrink-0 text-text-secondary" viewBox="0 0 24 24" fill="currentColor">
				<path d="M5 5l11 7-11 7z" />
				<rect x="17.5" y="5" width="2" height="14" rx="1" />
			</svg>
			{$translate('queue.playNext')}
		</button>

		<button
			type="button"
			class="flex items-center gap-3 rounded-md px-2 py-3 text-left text-sm text-text-primary active:bg-surface-2"
			onclick={handleAddToQueue}
		>
			<svg
				class="h-5 w-5 flex-shrink-0 text-text-secondary"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			>
				<path d="M4 6h11M4 12h11M4 18h7M19 14v6M16 17h6" stroke-linecap="round" />
			</svg>
			{$translate('queue.addToQueue')}
		</button>

		{#if context === 'playlist' && playlistId}
			<button
				type="button"
				class="flex items-center gap-3 rounded-md px-2 py-3 text-left text-sm text-text-primary active:bg-surface-2"
				onclick={handleReorder}
			>
				<svg
					class="h-5 w-5 flex-shrink-0 text-text-secondary"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
				>
					<path d="M7 15l5 5 5-5M7 9l5-5 5 5" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
				{$translate('queue.reorder')}
			</button>

			<button
				type="button"
				class="flex items-center gap-3 rounded-md px-2 py-3 text-left text-sm text-danger active:bg-surface-2"
				onclick={handleRemoveFromPlaylist}
			>
				<svg class="h-5 w-5 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M5 12h14" stroke-linecap="round" />
				</svg>
				{$translate('contextMenu.removeFromPlaylist')}
			</button>
		{/if}

		<button
			type="button"
			class="flex items-center gap-3 rounded-md px-2 py-3 text-left text-sm text-text-primary active:bg-surface-2"
			onclick={handleSelect}
		>
			<svg
				class="h-5 w-5 flex-shrink-0 text-text-secondary"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			>
				<path d="M9 11l3 3L22 4" stroke-linecap="round" stroke-linejoin="round" />
				<path
					d="M21 12v7a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2h11"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			{$translate('common.select')}
		</button>

		<div class="my-1 border-t border-stroke-subtle"></div>

		<button
			type="button"
			class="flex items-center gap-3 rounded-md px-2 py-3 text-left text-sm text-text-primary active:bg-surface-2"
			onclick={handleOpenInSource}
		>
			<span class="flex h-5 w-5 flex-shrink-0 items-center justify-center text-text-secondary">
				{#if release}<SourceIcon source={release.source_type} />{/if}
			</span>
			{platformName
				? $translate('discovery.openInApp', { values: { app: platformName } })
				: $translate('discovery.openInBrowser')}
		</button>

		<button
			type="button"
			class="flex items-center gap-3 rounded-md px-2 py-3 text-left text-sm text-danger active:bg-surface-2"
			onclick={handleDelete}
		>
			<svg class="h-5 w-5 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path
					d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m2 0v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			{$translate('common.delete')}
		</button>
	</div>
</MobileModal>
