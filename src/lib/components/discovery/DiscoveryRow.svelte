<script lang="ts">
	import type { DiscoveryRelease } from '$lib/types'
	import {
		daysUntilRelease,
		deriveFollowUrl,
		formatDate,
		formatDuration,
		formatRelativeDate,
		looseUrlEq,
	} from '$lib/utils'
	import { TagChip } from '$lib/components/tags'
	import { AlbumArt, AlbumArtModal, Icon, IconButton, Spinner, Text, Tooltip } from '$lib/components/common'
	import {
		language,
		dateFormat,
		dragStore,
		isDraggingTag,
		refreshingReleaseIds,
		discoveryStore,
		contextMenuDiscoveryTrackId,
		followedSources,
	} from '$lib/stores'
	import { playbackSource, previewInfo, previewLoadingReleaseId } from '$lib/stores/player'
	import { DRAG_THRESHOLD, getDistance } from '$lib/utils/drag'
	import { translate } from '$lib/i18n'
	import * as discoveryApi from '$lib/api/discovery'
	import { FollowPopover } from '$lib/components/follow'

	type Props = {
		release: DiscoveryRelease
		selected?: boolean
		expanded?: boolean
		isPreviewable?: boolean
		dragReleaseIds?: string[]
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		onclick?: (e: MouseEvent) => void
		ondblclick?: (e: MouseEvent) => void
		oncontextmenu?: (e: MouseEvent) => void
		onimport?: () => void
		onopenurl?: () => void
		onToggleExpand?: () => void
		onTrackPlay?: (trackIndex: number) => void
		onTrackLikeToggle?: (trackId: string) => void
		onTrackContextMenu?: (trackIndex: number, canPlay: boolean, e: MouseEvent) => void
		likedOnly?: boolean
	}

	let {
		release,
		selected = false,
		expanded = false,
		isPreviewable = true,
		dragReleaseIds = [],
		categoryColors,
		categorySortOrders,
		onclick,
		ondblclick,
		oncontextmenu,
		onimport,
		onopenurl,
		onToggleExpand,
		onTrackPlay,
		onTrackLikeToggle,
		onTrackContextMenu,
		likedOnly = false,
	}: Props = $props()

	let isTagDragHovered = $state(false)
	let showArtworkModal = $state(false)

	// Days until release for the "Upcoming" badge + countdown (null once out / unknown).
	// Computed at render, so the badge clears automatically when the date passes.
	const upcomingDays = $derived(daysUntilRelease(release.release_date))

	// Follow button + quick-follow popover.
	let showFollowPopover = $state(false)
	let followTriggerEl: HTMLElement | undefined = $state()
	const followUrl = $derived(deriveFollowUrl(release))
	const rowFollowing = $derived(!!followUrl && $followedSources.some((s) => looseUrlEq(s.url, followUrl)))

	function handleArtworkClick() {
		if (release.artwork_path || release.artwork_url) {
			showArtworkModal = true
		}
	}

	// Clear hover when tag drag ends
	$effect(() => {
		if (!$isDraggingTag) isTagDragHovered = false
	})

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

	function isTrackPlaying(idx: number): boolean {
		return $playbackSource === 'preview' && $previewInfo?.releaseId === release.id && $previewInfo?.trackIndex === idx
	}

	function trackCanPlay(trackIndex: number): boolean {
		const track = release.tracks[trackIndex]
		if (!track?.duration_ms) return false
		if (release.source_type === 'discogs') return track.video_id !== null
		return isPreviewable
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

	function handleKeyDown(e: KeyboardEvent) {
		if (e.key === ' ') {
			e.preventDefault()
			onclick?.(e as unknown as MouseEvent)
		}
	}
</script>

<div
	role="row"
	tabindex="0"
	data-release-row
	data-release-id={release.id}
	class="grid cursor-pointer grid-cols-[24px_40px_1.25fr_0.6fr_1fr_90px_110px_100px_92px] items-center gap-2 border-b border-stroke-subtle px-3 py-1.5 text-sm transition-colors select-none {selected
		? 'bg-brand-muted'
		: 'hover:bg-surface-2/50'} {isTagDragHovered ? 'bg-brand-primary/10 ring-1 ring-brand-primary ring-inset' : ''}"
	{onclick}
	{ondblclick}
	{oncontextmenu}
	onkeydown={handleKeyDown}
	onpointerdown={handlePointerDown}
	onpointermove={handlePointerMove}
	onpointerup={handlePointerUp}
	onpointercancel={handlePointerUp}
	onpointerenter={() => $isDraggingTag && (isTagDragHovered = true)}
	onpointerleave={() => (isTagDragHovered = false)}
>
	<!-- Expand toggle -->
	<div class="flex items-center justify-center">
		{#if $previewLoadingReleaseId === release.id}
			<Spinner class="h-3.5 w-3.5" />
		{:else if $refreshingReleaseIds.has(release.id)}
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="group flex h-6 w-6 cursor-pointer items-center justify-center rounded"
				onclick={(e) => {
					e.stopPropagation()
					discoveryStore.cancelRefresh(release.id)
				}}
			>
				<Spinner class="h-3.5 w-3.5 group-hover:hidden" />
				<Icon name="x" class="hidden h-3.5 w-3.5 text-text-tertiary group-hover:block hover:text-text-primary" />
			</div>
		{:else if release.tracks.length > 0}
			<IconButton
				icon="chevron-right"
				iconClass="h-3.5 w-3.5 text-text-tertiary transition-transform duration-200 {expanded ? 'rotate-90' : ''}"
				size="sm"
				onclick={(e) => {
					e.stopPropagation()
					onToggleExpand?.()
				}}
			/>
		{/if}
	</div>

	<!-- Artwork -->
	<div class="flex items-center justify-center">
		<AlbumArt
			artworkPath={release.artwork_path}
			artworkUrl={release.artwork_url}
			size="xs"
			onclick={handleArtworkClick}
			class={release.artwork_path || release.artwork_url ? 'cursor-zoom-in' : ''}
		/>
	</div>

	<!-- Artist / Title -->
	<div class="flex flex-col justify-center truncate">
		<div class="flex items-center gap-2">
			<Text as="span" weight="medium" truncate>
				{release.title || $translate('common.untitled')}
			</Text>
			{#if release.tracks.length > 0}
				<Text as="span" size="xs" color="tertiary" class="shrink-0">
					{$translate('discovery.trackCount', { values: { count: release.tracks.length } })}
				</Text>
			{/if}
			{#if upcomingDays !== null}
				<span
					class="shrink-0 rounded-full bg-orange-500/15 px-1.5 py-0.5 text-[10px] leading-none font-medium text-orange-500"
				>
					{$translate('discovery.following.upcoming')}
				</span>
			{/if}
		</div>
		<Text as="span" variant="caption" truncate>
			{release.artist || $translate('common.unknownArtist')}
		</Text>
	</div>

	<!-- Label -->
	<div class="truncate text-left text-text-secondary">
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

	<!-- Source -->
	<div class="truncate text-left text-text-tertiary">
		{sourceLabels[release.source_type] ?? release.source_type}
	</div>

	<!-- Release Date -->
	<div class="truncate text-left text-text-tertiary">
		{#if upcomingDays !== null}
			<span class="text-orange-500"
				>{$translate('discovery.following.daysUntil', { values: { days: upcomingDays } })}</span
			>
		{:else if release.release_date}
			{formatDate(release.release_date, $dateFormat, $language)}
		{/if}
	</div>

	<!-- Date Added -->
	<div class="truncate text-left text-text-tertiary">
		{formatRelativeDate(release.date_added, $translate)}
	</div>

	<!-- Actions -->
	<div class="flex items-center justify-end gap-1 pr-1">
		<Tooltip text={$translate('discovery.following.followForNewReleases')} position="left" delay={250}>
			<IconButton
				icon="rss"
				size="sm"
				active={rowFollowing}
				onclick={(e) => {
					e.stopPropagation()
					followTriggerEl = e.currentTarget as HTMLElement
					showFollowPopover = !showFollowPopover
				}}
			/>
		</Tooltip>
		<Tooltip text={$translate('discovery.importToLibrary')} position="left" delay={250}>
			<IconButton
				icon="upload"
				size="sm"
				onclick={(e) => {
					e.stopPropagation()
					onimport?.()
				}}
			/>
		</Tooltip>
		<Tooltip text={$translate('discovery.openInBrowser')} position="left" delay={250}>
			<IconButton
				icon="external-link"
				size="sm"
				onclick={(e) => {
					e.stopPropagation()
					onopenurl?.()
				}}
			/>
		</Tooltip>
	</div>
</div>

{#if showArtworkModal}
	<AlbumArtModal
		open={showArtworkModal}
		artworkPath={release.artwork_path}
		artworkUrl={release.artwork_url}
		trackTitle={release.title ?? ''}
		onClose={() => (showArtworkModal = false)}
	/>
{/if}

{#if showFollowPopover && followTriggerEl}
	<FollowPopover {release} triggerEl={followTriggerEl} onClose={() => (showFollowPopover = false)} />
{/if}

<!-- Track sub-rows (CSS grid-template-rows transition for smooth expand/collapse) -->
{#if release.tracks.length > 0}
	<div class="grid overflow-hidden" style="grid-template-rows: {expanded ? '1fr' : '0fr'}">
		<div class="min-h-0 overflow-hidden">
			<div class="border-b border-stroke-subtle bg-surface-1/30">
				{#each release.tracks as track, idx (track.id)}
					{#if !likedOnly || track.is_liked}
						{@const canPlay = trackCanPlay(idx)}
						{@const playing = canPlay && isTrackPlaying(idx)}
						{@const isContextActive = track.id === $contextMenuDiscoveryTrackId}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							class="group/track grid grid-cols-[24px_40px_1fr_80px] items-center gap-2 px-3 py-1 {canPlay
								? 'cursor-pointer hover:bg-surface-2/50'
								: 'cursor-default opacity-60'} {isContextActive ? 'bg-surface-2/50' : ''} {track.position > 1
								? 'border-t border-stroke-subtle/50'
								: ''}"
							ondblclick={canPlay
								? (e) => {
										e.stopPropagation()
										onTrackPlay?.(idx)
									}
								: undefined}
							onmouseenter={canPlay
								? () => {
										discoveryApi.fetchPreviewStream(release.id, track.position).catch(() => {})
									}
								: undefined}
							oncontextmenu={(e) => {
								e.preventDefault()
								e.stopPropagation()
								onTrackContextMenu?.(idx, canPlay, e)
							}}
						>
							<div class="flex items-center justify-center">
								<button
									class="flex h-5 w-5 cursor-pointer items-center justify-center rounded transition-colors {track.is_liked
										? 'text-brand-primary'
										: isContextActive
											? 'text-text-tertiary opacity-100'
											: 'text-text-tertiary opacity-0 group-hover/track:opacity-100 hover:opacity-100'}"
									onclick={(e) => {
										e.stopPropagation()
										e.currentTarget.animate(
											[{ transform: 'scale(1)' }, { transform: 'scale(1.35)' }, { transform: 'scale(1)' }],
											{ duration: 300, easing: 'ease-out' }
										)
										onTrackLikeToggle?.(track.id)
									}}
									ondblclick={(e) => e.stopPropagation()}
								>
									<Icon name="heart" class="h-3 w-3" fill={track.is_liked} />
								</button>
							</div>
							<div
								class="text-center text-xs {playing
									? 'text-brand-primary'
									: canPlay
										? 'text-text-tertiary'
										: 'text-text-tertiary/50'}"
							>
								{track.position}
							</div>
							<div
								class="truncate text-xs {playing
									? 'font-medium text-brand-primary'
									: canPlay
										? 'text-text-secondary'
										: 'text-text-tertiary'}"
							>
								{track.name}
							</div>
							<div
								class="mr-1 text-right text-xs {playing
									? 'text-brand-primary'
									: canPlay
										? 'text-text-tertiary'
										: 'text-text-tertiary/50'}"
							>
								{track.duration_ms ? formatDuration(track.duration_ms) : ''}
							</div>
						</div>
					{/if}
				{/each}
			</div>
		</div>
	</div>
{/if}
