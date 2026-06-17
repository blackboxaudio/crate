<script lang="ts">
	import { onMount } from 'svelte'
	import { translate } from '$shared/i18n'
	import { discoveryStore, sortedReleases, isDiscoveryLoading } from '$shared/stores/discovery'
	import MobileList from '$lib/components/common/MobileList.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'

	// Minimal real Discovery feed: loads releases and renders artist/title + remote artwork. The full
	// feed (search, filters, sort, preview playback, virtualization, context menus) is a later issue.
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
			<MobileListItem>
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
			</MobileListItem>
		{/each}
	</MobileList>
{/if}
