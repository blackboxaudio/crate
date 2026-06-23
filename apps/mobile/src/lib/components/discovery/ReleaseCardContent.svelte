<script lang="ts">
	import type { DiscoveryRelease } from '$shared/types'
	import { translate } from '$shared/i18n'

	// The visual interior of a discovery row — artwork + title/artist/label. Extracted so the live
	// `ReleaseCard` and the `ContextMenu` lifted preview render from one source of truth: the preview is a
	// real, data-driven copy (no DOM cloning, so the already-decoded artwork shows instantly with no flash).
	// Renders as two flex children (artwork, then the text column) so it drops straight into the card's
	// `flex items-center gap-3` row between the optional select checkbox and the trailing chevron.
	type Props = { release: DiscoveryRelease }
	let { release }: Props = $props()
</script>

{#if release.artwork_url}
	<img src={release.artwork_url} alt="" class="h-12 w-12 flex-shrink-0 rounded object-cover" loading="lazy" />
{:else}
	<div class="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded bg-surface-2 text-text-tertiary">
		<svg viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor">
			<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
		</svg>
	</div>
{/if}

<div class="flex min-w-0 flex-1 flex-col leading-tight">
	<span class="truncate text-sm font-medium text-text-primary">
		{release.title ?? $translate('common.untitled')}
	</span>
	<span class="truncate text-xs text-text-secondary">
		{release.artist ?? $translate('common.unknownArtist')}
	</span>
	<!-- Label line is always rendered (a non-breaking space when absent) so every row keeps the fixed
	     height the virtualizer estimates — a conditional line would desync row heights while scrolling. -->
	<span class="truncate text-xs text-text-tertiary" aria-hidden={!release.label}>
		{release.label ?? ' '}
	</span>
</div>
