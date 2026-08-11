<script lang="ts">
	import type { DiscoveryRelease } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { fullyOwnedReleaseIds, partiallyOwnedReleaseIds } from '$shared/stores/collection'
	import { fullyCachedIds } from '$shared/stores/offlineCache'
	import ReleaseArtwork from '$lib/components/common/ReleaseArtwork.svelte'

	// The visual interior of a discovery row — artwork + title/artist/label. Extracted so the live
	// `ReleaseCard` and the `ContextMenu` lifted preview render from one source of truth: the preview is a
	// real, data-driven copy (no DOM cloning, so the already-decoded artwork shows instantly with no flash).
	// Renders as two flex children (artwork, then the text column) so it drops straight into the card's
	// `flex items-center gap-3` row between the optional select checkbox and the trailing chevron.
	type Props = { release: DiscoveryRelease }
	let { release }: Props = $props()

	const fullyOwned = $derived($fullyOwnedReleaseIds.has(release.id))
	const partiallyOwned = $derived($partiallyOwnedReleaseIds.has(release.id))
</script>

<ReleaseArtwork {release} size="thumb" class="h-12 w-12 flex-shrink-0 rounded object-cover" />

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
	     height the virtualizer estimates — a conditional line would desync row heights while scrolling.
	     The downloaded badge leads it when the release's audio is fully cached (offline-ready); the icon
	     is smaller than the line height, so the row height never changes. -->
	<span class="flex min-w-0 items-center gap-1 text-xs text-text-tertiary">
		{#if fullyOwned || partiallyOwned}
			<!-- Owned badge: brand-tinted when the whole release is purchased, muted when only
			     some tracks are. Inline and smaller than the line height (fixed-row invariant). -->
			<svg
				class="h-3 w-3 flex-shrink-0 {fullyOwned ? 'text-brand-primary' : ''}"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
			>
				<path d="M6 8h12l-1.2 12H7.2L6 8z" />
				<path d="M9 8V6a3 3 0 0 1 6 0v2" />
			</svg>
		{/if}
		{#if $fullyCachedIds.has(release.id)}
			<svg
				class="h-3 w-3 flex-shrink-0"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
			>
				<circle cx="12" cy="12" r="9" />
				<path d="M12 8v7M8.5 12l3.5 3.5L15.5 12" />
			</svg>
		{/if}
		<span class="truncate" aria-hidden={!release.label}>
			{release.label ?? ' '}
		</span>
	</span>
</div>
