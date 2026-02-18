<script lang="ts">
	import type { DiscoveryRelease } from '$lib/types'
	import { formatDate, formatRelativeDate } from '$lib/utils'
	import { TagChip } from '$lib/components/tags'
	import { AlbumArt, IconButton, Text, Tooltip } from '$lib/components/common'
	import { dragStore } from '$lib/stores'
	import { DRAG_THRESHOLD, getDistance } from '$lib/utils/drag'
	import { translate } from '$lib/i18n'

	type Props = {
		release: DiscoveryRelease
		selected?: boolean
		dragReleaseIds?: string[]
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		onclick?: (e: MouseEvent) => void
		ondblclick?: (e: MouseEvent) => void
		oncontextmenu?: (e: MouseEvent) => void
		onimport?: () => void
	}

	let {
		release,
		selected = false,
		dragReleaseIds = [],
		categoryColors,
		categorySortOrders,
		onclick,
		ondblclick,
		oncontextmenu,
		onimport,
	}: Props = $props()

	// Track pointer state for drag detection
	let pointerStartPos: { x: number; y: number } | null = null
	let isDragStarted = false

	function handlePointerDown(e: PointerEvent) {
		if (e.button !== 0) return
		const target = e.target as HTMLElement
		if (target.closest('button, [role="button"]')) return
		pointerStartPos = { x: e.clientX, y: e.clientY }
		isDragStarted = false
	}

	function handlePointerMove(e: PointerEvent) {
		if (!pointerStartPos) return
		const distance = getDistance(pointerStartPos.x, pointerStartPos.y, e.clientX, e.clientY)
		if (!isDragStarted && distance >= DRAG_THRESHOLD) {
			isDragStarted = true
			const releaseIds = selected && dragReleaseIds.length > 0 ? dragReleaseIds : [release.id]
			dragStore.startReleaseDrag(releaseIds, e.clientX, e.clientY)
		}
	}

	const sourceLabels: Record<string, string> = {
		bandcamp: 'Bandcamp',
		soundcloud: 'SoundCloud',
		youtube: 'YouTube',
		discogs: 'Discogs',
		other: 'Other',
	}

	function handlePointerUp() {
		pointerStartPos = null
		isDragStarted = false
	}
</script>

<div
	role="row"
	tabindex="0"
	data-release-row
	class="grid cursor-pointer grid-cols-[40px_1.25fr_0.6fr_1fr_110px_110px_100px_40px] items-center gap-2 border-b border-stroke-subtle px-3 py-1.5 text-sm transition-colors select-none {selected
		? 'bg-brand-muted'
		: 'hover:bg-surface-2/50'}"
	{onclick}
	{ondblclick}
	{oncontextmenu}
	onpointerdown={handlePointerDown}
	onpointermove={handlePointerMove}
	onpointerup={handlePointerUp}
	onpointercancel={handlePointerUp}
	onkeydown={(e) => e.key === 'Enter' && ondblclick?.(e)}
>
	<!-- Artwork -->
	<div class="flex justify-center">
		<AlbumArt artworkPath={release.artwork_path} artworkUrl={release.artwork_url} size="xs" />
	</div>

	<!-- Artist / Title -->
	<div class="flex flex-col justify-center truncate">
		<Text as="span" weight="medium" truncate>
			{release.title || $translate('common.untitled')}
		</Text>
		<Text as="span" variant="caption" truncate>
			{release.artist || $translate('common.unknownArtist')}
		</Text>
	</div>

	<!-- Label -->
	<div class="truncate text-text-secondary">
		{release.label || ''}
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

	<!-- Release Date -->
	<div class="truncate text-text-tertiary">
		{release.release_date ? formatDate(release.release_date) : ''}
	</div>

	<!-- Source -->
	<div class="truncate text-text-tertiary">
		{sourceLabels[release.source_type] ?? release.source_type}
	</div>

	<!-- Date Added -->
	<div class="truncate text-text-tertiary">
		{formatRelativeDate(release.date_added)}
	</div>

	<!-- Import -->
	<div class="flex items-center justify-end pr-1">
		<Tooltip text={$translate('discovery.importToLibrary')} position="left" delay={250}>
			<IconButton
				icon="plus"
				size="sm"
				onclick={(e) => {
					e.stopPropagation()
					onimport?.()
				}}
			/>
		</Tooltip>
	</div>
</div>
