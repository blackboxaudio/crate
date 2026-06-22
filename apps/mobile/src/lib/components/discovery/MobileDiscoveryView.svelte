<script lang="ts">
	import { onMount } from 'svelte'
	import { translate } from '$shared/i18n'
	import { discoveryStore, sortedReleases, isDiscoveryLoading } from '$shared/stores/discovery'
	import { previewInfo, previewLoadingReleaseId } from '$shared/stores/player'
	import { mobileUIStore } from '$lib/stores/mobileUI'
	import MobileList from '$lib/components/common/MobileList.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'

	// Real Discovery feed: loads releases and renders artist/title + remote artwork. Tapping a release
	// opens its detail screen (metadata, notes, tags, track list + preview playback). The full feed
	// (search, filters, sort, per-track selection, virtualization) is a later issue.
	onMount(() => {
		discoveryStore.loadReleases()
	})
</script>

{#if $isDiscoveryLoading && $sortedReleases.length === 0}
	<div class="flex h-full items-center justify-center text-sm text-text-secondary">
		{$translate('common.loading')}
	</div>
{:else if $sortedReleases.length === 0}
	<div class="flex h-full items-center justify-center text-sm text-text-secondary">
		{$translate('discovery.noReleasesYet')}
	</div>
{:else}
	<MobileList>
		{#each $sortedReleases as release (release.id)}
			<MobileListItem
				onclick={() => mobileUIStore.openDetail(release.id)}
				selected={$previewInfo?.releaseId === release.id}
				ariaLabel={`${release.artist ?? $translate('common.unknownArtist')} — ${release.title ?? $translate('common.untitled')}`}
			>
				{#snippet leading()}
					{#if release.artwork_url}
						<img src={release.artwork_url} alt="" class="h-12 w-12 rounded object-cover" loading="lazy" />
					{:else}
						<div class="flex h-12 w-12 items-center justify-center rounded bg-surface-2 text-text-tertiary">
							<svg viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor">
								<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
							</svg>
						</div>
					{/if}
				{/snippet}
				<div class="flex min-w-0 flex-col">
					<span class="truncate text-sm font-medium text-text-primary">
						{release.artist ?? $translate('common.unknownArtist')}
					</span>
					<span class="truncate text-xs text-text-secondary">
						{release.title ?? $translate('common.untitled')}
					</span>
				</div>
				{#snippet trailing()}
					{#if $previewLoadingReleaseId === release.id}
						<svg class="h-4 w-4 animate-spin" viewBox="0 0 24 24" fill="none">
							<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
							<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 0 1 8-8V0C5.4 0 0 5.4 0 12h4z" />
						</svg>
					{:else}
						<!-- Chevron: signals the row opens its detail screen (no hover affordance on touch). -->
						<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M9 18l6-6-6-6" stroke-linecap="round" stroke-linejoin="round" />
						</svg>
					{/if}
				{/snippet}
			</MobileListItem>
		{/each}
		{#if $previewInfo}
			<!-- Spacer so the fixed now-playing bar never covers the last row. -->
			<div class="h-20" aria-hidden="true"></div>
		{/if}
	</MobileList>
{/if}
