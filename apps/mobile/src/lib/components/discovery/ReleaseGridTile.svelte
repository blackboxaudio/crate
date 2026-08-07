<script lang="ts">
	import type { DiscoveryRelease } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { previewInfo } from '$shared/stores/player'
	import { fullyOwnedReleaseIds, partiallyOwnedReleaseIds } from '$shared/stores/collection'
	import { mobileUIStore, selectMode, selectedReleaseIds } from '$lib/stores/mobileUI'
	import { fullyCachedIds } from '$lib/stores/offlineCache'
	import { lightTap } from '$lib/utils/haptics'
	import { longPress } from '$lib/actions/longPress'
	import ReleaseArtwork from '$lib/components/common/ReleaseArtwork.svelte'

	// One tile of the discovery feed's 3-column artwork grid. Tap opens the detail (or toggles the
	// selection in select mode); long-press (the shared `longPress` action, disabled in select mode)
	// opens the same release context menu the list rows use, anchored to the tile. No swipe actions in
	// grid mode — those stay a list-row affordance.
	type Props = { release: DiscoveryRelease }
	let { release }: Props = $props()

	const isSelectMode = $derived($selectMode)
	const isSelected = $derived($selectedReleaseIds.has(release.id))
	const isCurrentPreview = $derived($previewInfo?.releaseId === release.id)

	function onClick() {
		if (isSelectMode) {
			mobileUIStore.toggleReleaseSelected(release.id)
		} else {
			void lightTap()
			mobileUIStore.openDetail(release.id)
		}
	}
</script>

<button
	type="button"
	use:longPress={{
		enabled: !isSelectMode,
		onLongPress: (rect) => mobileUIStore.openActionsSheet(release.id, 'feed', rect),
	}}
	class="flex w-full flex-col gap-1 text-left"
	aria-label={`${release.artist ?? $translate('common.unknownArtist')} — ${release.title ?? $translate('common.untitled')}`}
	aria-pressed={isSelectMode ? isSelected : undefined}
	onclick={onClick}
>
	<div class="relative w-full">
		<ReleaseArtwork
			{release}
			class="aspect-square w-full rounded-md object-cover {isCurrentPreview ? 'ring-2 ring-brand-primary' : ''}"
		/>
		{#if release.is_new}
			<span class="absolute top-1 left-1 h-2 w-2 rounded-full bg-brand-primary"></span>
		{/if}
		{#if $fullyOwnedReleaseIds.has(release.id) || $partiallyOwnedReleaseIds.has(release.id)}
			<!-- Owned chip (bottom-left; bottom-right is the downloaded chip, top-right is select). -->
			<span
				class="absolute bottom-1 left-1 flex h-4 w-4 items-center justify-center rounded-full bg-black/55 text-white"
			>
				<svg
					class="h-2.5 w-2.5"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2.5"
					stroke-linecap="round"
					stroke-linejoin="round"
					aria-hidden="true"
				>
					<path d="M6 8h12l-1.2 12H7.2L6 8z" />
					<path d="M9 8V6a3 3 0 0 1 6 0v2" />
				</svg>
			</span>
		{/if}
		{#if $fullyCachedIds.has(release.id)}
			<span
				class="absolute right-1 bottom-1 flex h-4 w-4 items-center justify-center rounded-full bg-black/55 text-white"
			>
				<svg
					class="h-2.5 w-2.5"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2.5"
					stroke-linecap="round"
					stroke-linejoin="round"
					aria-hidden="true"
				>
					<path d="M12 6v10M8 12l4 4 4-4" />
				</svg>
			</span>
		{/if}
		{#if isSelectMode}
			<span
				class="absolute top-1 right-1 flex h-5 w-5 items-center justify-center rounded-full border {isSelected
					? 'border-brand-primary bg-brand-primary text-white'
					: 'border-white/70 bg-black/30 text-transparent'}"
			>
				<svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
					<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</span>
		{/if}
	</div>
	<span class="w-full truncate text-xs font-medium text-text-primary">
		{release.title ?? $translate('common.untitled')}
	</span>
	<span class="-mt-1 w-full truncate text-[11px] text-text-tertiary">
		{release.artist ?? $translate('common.unknownArtist')}
	</span>
</button>
