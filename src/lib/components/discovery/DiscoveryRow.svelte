<script lang="ts">
	import type { DiscoveryRelease } from '$lib/types'
	import { formatRelativeDate } from '$lib/utils'
	import { TagChip } from '$lib/components/tags'
	import { AlbumArt, Text } from '$lib/components/common'
	import { translate } from '$lib/i18n'

	type Props = {
		release: DiscoveryRelease
		selected?: boolean
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		onclick?: (e: MouseEvent) => void
		ondblclick?: (e: MouseEvent) => void
		oncontextmenu?: (e: MouseEvent) => void
	}

	let {
		release,
		selected = false,
		categoryColors,
		categorySortOrders,
		onclick,
		ondblclick,
		oncontextmenu,
	}: Props = $props()

	const statusColors: Record<string, string> = {
		unlistened: 'bg-surface-2 text-text-tertiary',
		listened: 'bg-blue-500/15 text-blue-500',
		purchased: 'bg-green-500/15 text-green-500',
		dismissed: 'bg-red-500/15 text-red-500',
	}
</script>

<div
	role="row"
	tabindex="0"
	data-release-row
	class="grid cursor-pointer grid-cols-[40px_1fr_1fr_100px_1fr_100px] items-center gap-2 border-b border-stroke-subtle px-3 py-1.5 text-sm transition-colors select-none {selected
		? 'bg-brand-muted'
		: 'hover:bg-surface-2/50'}"
	{onclick}
	{ondblclick}
	{oncontextmenu}
	onkeydown={(e) => e.key === 'Enter' && ondblclick?.(e)}
>
	<!-- Artwork -->
	<div class="flex justify-center">
		<AlbumArt artworkPath={release.artwork_path} size="xs" />
	</div>

	<!-- Artist / Title -->
	<div class="flex flex-col justify-center truncate">
		<span class="truncate font-medium text-text-primary">
			{release.title || $translate('common.untitled')}
		</span>
		<span class="truncate text-xs text-text-tertiary">
			{release.artist || $translate('common.unknownArtist')}
		</span>
	</div>

	<!-- Label -->
	<div class="truncate text-text-secondary">
		{release.label || ''}
	</div>

	<!-- Status -->
	<div class="flex items-center justify-center">
		<span
			class="rounded-full px-2 py-0.5 text-xs font-medium {statusColors[release.status] || statusColors.unlistened}"
		>
			{$translate(`discovery.status.${release.status}`)}
		</span>
	</div>

	<!-- Tags -->
	<div class="flex h-6 items-center gap-1 overflow-hidden">
		{#each release.tags
			.toSorted((a, b) => {
				const orderA = categorySortOrders?.get(a.category_id) ?? 0
				const orderB = categorySortOrders?.get(b.category_id) ?? 0
				if (orderA !== orderB) return orderA - orderB
				return a.name.localeCompare(b.name)
			})
			.slice(0, 3) as tag (tag.id)}
			<TagChip {tag} size="sm" color={categoryColors?.get(tag.category_id)} />
		{/each}
		{#if release.tags.length > 3}
			<Text variant="caption">+{release.tags.length - 3}</Text>
		{/if}
	</div>

	<!-- Date Added -->
	<div class="text-right text-text-tertiary">
		{formatRelativeDate(release.date_added)}
	</div>
</div>
