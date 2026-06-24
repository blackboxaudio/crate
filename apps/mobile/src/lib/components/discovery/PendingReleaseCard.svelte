<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { PendingRelease } from '$lib/stores/pendingReleases'
	import { pendingReleasesStore } from '$lib/stores/pendingReleases'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import SourceIcon from './SourceIcon.svelte'

	type Props = {
		pending: PendingRelease
	}
	let { pending }: Props = $props()

	const domain = $derived(() => {
		try {
			return new URL(pending.url).hostname.replace(/^www\./, '')
		} catch {
			return pending.url
		}
	})
</script>

<div class="flex h-[72px] items-center gap-3 px-4 opacity-50">
	<div class="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded bg-surface-2 text-text-tertiary">
		<SourceIcon source={pending.sourceType} />
	</div>
	<div class="min-w-0 flex-1">
		<p class="truncate text-sm text-text-secondary">{domain()}</p>
		<div class="flex items-center gap-1.5">
			{#if pending.status === 'fetching'}
				<Spinner class="h-3 w-3" />
				<span class="text-xs text-text-tertiary">{$translate('discovery.fetchingMetadata')}</span>
			{:else if pending.status === 'failed'}
				<span class="rounded bg-danger/15 px-1.5 py-0.5 text-[10px] font-medium text-danger">
					{$translate('discovery.fetchError')}
				</span>
			{:else}
				<span class="bg-brand-primary/15 rounded px-1.5 py-0.5 text-[10px] font-medium text-brand-primary">
					{$translate('discovery.pending')}
				</span>
			{/if}
		</div>
	</div>
	<button
		type="button"
		class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-md text-text-tertiary active:bg-surface-2"
		aria-label={$translate('common.remove')}
		onclick={() => pendingReleasesStore.remove(pending.id)}
	>
		<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
			<path d="M18 6L6 18M6 6l12 12" stroke-linecap="round" />
		</svg>
	</button>
</div>
