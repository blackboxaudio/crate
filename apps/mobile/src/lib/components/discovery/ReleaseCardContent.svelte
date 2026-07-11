<script lang="ts">
	import type { DiscoveryRelease } from '$shared/types'
	import { translate } from '$shared/i18n'
	import ReleaseArtwork from '$lib/components/common/ReleaseArtwork.svelte'

	// The visual interior of a discovery row — artwork + title/artist/label. Extracted so the live
	// `ReleaseCard` and the `ContextMenu` lifted preview render from one source of truth: the preview is a
	// real, data-driven copy (no DOM cloning, so the already-decoded artwork shows instantly with no flash).
	// Renders as two flex children (artwork, then the text column) so it drops straight into the card's
	// `flex items-center gap-3` row between the optional select checkbox and the trailing chevron.
	type Props = { release: DiscoveryRelease }
	let { release }: Props = $props()
</script>

<ReleaseArtwork {release} class="h-12 w-12 flex-shrink-0 rounded object-cover" />

<div class="flex min-w-0 flex-1 flex-col leading-tight">
	<!-- Title line: the "new" status pill (unread until listened to) trails the title, matching the
	     followed-source rows' status indicator. The title truncates; the pill never shrinks. -->
	<span class="flex min-w-0 items-center gap-1.5">
		<span class="truncate text-sm font-medium text-text-primary">
			{release.title ?? $translate('common.untitled')}
		</span>
		{#if release.is_new}
			<span
				class="flex-shrink-0 rounded-full bg-brand-muted px-1.5 py-0.5 text-[10px] font-semibold text-brand-primary"
			>
				{$translate('filters.new')}
			</span>
		{/if}
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
