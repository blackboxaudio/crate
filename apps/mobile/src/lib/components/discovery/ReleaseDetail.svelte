<script lang="ts">
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease } from '$shared/types'
	import { discoveryStore } from '$shared/stores/discovery'
	import { playerStore, previewInfo, previewLoadingReleaseId } from '$shared/stores/player'
	import { formatDate, formatDurationCompact } from '$shared/utils/format'
	import { getReleasePlatformName } from '$shared/utils/discoveryLinks'
	import { mobileUIStore } from '$lib/stores/mobileUI'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import MarqueeText from '$lib/components/common/MarqueeText.svelte'
	import MobileTagPicker from './MobileTagPicker.svelte'
	import SourceIcon from './SourceIcon.svelte'

	// Full-screen release detail: large artwork, metadata, editable notes (auto-save on blur),
	// assignable tags (via the bottom-sheet picker), and the track list with per-track preview playback.
	// Reads the release from the discovery store (passed in by +page) so notes/tag edits reflect live.
	// Dismissed by the back chevron or an iOS-style left-edge swipe (finger-follow); the discovery feed
	// shows through a fading scrim behind it as it slides — matching the drawers and the player.
	type Props = {
		release: DiscoveryRelease
	}
	let { release }: Props = $props()

	// Notes: local editing state, synced from the release only when switching to a different release so
	// in-progress typing isn't clobbered by store refreshes. Auto-saves on blur (mirrors desktop's
	// DiscoveryEditor onblur behaviour).
	let notesValue = $state('')
	let loadedId = $state('')
	$effect(() => {
		if (release.id !== loadedId) {
			notesValue = release.notes ?? ''
			loadedId = release.id
		}
	})

	async function handleNotesBlur() {
		if (notesValue === (release.notes ?? '')) return
		await discoveryStore.updateRelease(release.id, { notes: notesValue })
	}

	let tagPickerOpen = $state(false)

	const isCurrentRelease = $derived($previewInfo?.releaseId === release.id)
	const platformName = $derived(getReleasePlatformName(release.source_type))

	// Open on mount (this is only rendered while a release is selected). Dismissal flips `open` false; the
	// Drawer slides out, then `onClosed` clears the store so +page's {#if} unmounts only after the anim.
	let open = $state(true)
</script>

<Drawer
	{open}
	direction="right"
	onClose={() => (open = false)}
	onClosed={mobileUIStore.closeDetail}
	z={30}
	scrimZ={20}
	scrimDismiss={false}
	closeEdgeFrom="left"
	closeEdgeSize={24}
	ariaLabel={release.title ?? $translate('common.untitled')}
	class="pt-safe flex w-full flex-col bg-surface-0"
>
	<!-- Header -->
	<div class="flex items-center gap-1 border-b border-stroke px-2 py-2">
		<button
			type="button"
			class="flex h-10 w-10 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
			aria-label={$translate('common.close')}
			onclick={() => (open = false)}
		>
			<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
		</button>
	</div>

	<!-- Scrollable content; bottom padding clears the mini-player bar. overflow-x is pinned hidden because
	     overflow-y-auto alone computes overflow-x to `auto`, which would let overflowing content scroll
	     sideways (and pinch-zoom) on iOS. -->
	<div class="flex-1 overflow-x-hidden overflow-y-auto px-4 pt-4 pb-28">
		<!-- Artwork -->
		<div class="mb-4 flex justify-center">
			{#if release.artwork_url}
				<img src={release.artwork_url} alt="" class="aspect-square w-full max-w-xs rounded-xl object-cover shadow-lg" />
			{:else}
				<div
					class="flex aspect-square w-full max-w-xs items-center justify-center rounded-xl bg-surface-2 text-text-tertiary"
				>
					<svg viewBox="0 0 24 24" class="h-16 w-16" fill="currentColor">
						<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
					</svg>
				</div>
			{/if}
		</div>

		<!-- Metadata + open-in-source action (icon sits to the right of the info block, vertically centered,
		     to save the vertical whitespace a standalone button row would add). -->
		<div class="flex items-center gap-3">
			<div class="min-w-0 flex-1">
				<h1 class="text-xl font-semibold text-text-primary">
					{release.title ?? $translate('common.untitled')}
				</h1>
				<p class="text-base text-text-secondary">{release.artist ?? $translate('common.unknownArtist')}</p>
				<p class="mt-0.5 text-sm text-text-tertiary">
					{#if release.label}{release.label}{/if}
					{#if release.label && release.release_date}
						·
					{/if}
					{#if release.release_date}{formatDate(release.release_date)}{/if}
				</p>
			</div>
			<!-- Open in the source app (falls back to the browser via Universal/App Links). Icon-only with a
			     source-matched glyph; no hover on touch, so the resting border signals it's tappable. The
			     platform name lives in the aria-label. 44px hit target with a tactile active press. -->
			<button
				type="button"
				class="flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-md border border-stroke text-text-primary transition-transform active:scale-95 active:bg-surface-2"
				aria-label={platformName
					? $translate('discovery.openInApp', { values: { app: platformName } })
					: $translate('discovery.openInBrowser')}
				onclick={() => openUrl(release.url).catch(() => {})}
			>
				<SourceIcon source={release.source_type} />
			</button>
		</div>

		<!-- Track list -->
		<div class="mt-6">
			<h2 class="mb-1.5 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
				{$translate('discovery.tracks')}
			</h2>
			<div class="flex flex-col">
				{#each release.tracks as track, index (track.id)}
					{@const isActive = isCurrentRelease && $previewInfo?.trackIndex === index}
					<div class="flex min-w-0 items-center {isActive ? 'bg-brand-muted' : ''} rounded">
						<button
							type="button"
							class="flex min-h-[44px] min-w-0 flex-1 items-center gap-3 px-2 py-2 text-left active:bg-surface-2"
							aria-label={$translate('discovery.playPreview')}
							onclick={() => playerStore.playPreview(release, index)}
						>
							<span class="w-5 flex-shrink-0 text-center text-xs text-text-tertiary tabular-nums">
								{#if isActive && $previewLoadingReleaseId === release.id}
									<svg class="mx-auto h-3.5 w-3.5 animate-spin" viewBox="0 0 24 24" fill="none">
										<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
										<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 0 1 8-8V0C5.4 0 0 5.4 0 12h4z" />
									</svg>
								{:else}
									{index + 1}
								{/if}
							</span>
							<MarqueeText text={track.name} class="min-w-0 flex-1 text-sm text-text-primary" />
							{#if track.duration_ms != null}
								<span class="flex-shrink-0 text-xs text-text-tertiary tabular-nums">
									{formatDurationCompact(track.duration_ms)}
								</span>
							{/if}
						</button>
						<button
							type="button"
							class="flex h-10 w-10 flex-shrink-0 items-center justify-center text-text-tertiary active:bg-surface-2"
							aria-label={track.is_liked ? $translate('discovery.unlike') : $translate('discovery.like')}
							onclick={() => discoveryStore.toggleTrackLiked(release.id, track.id)}
						>
							<svg
								class="h-4 w-4 {track.is_liked ? 'text-brand-primary' : ''}"
								viewBox="0 0 24 24"
								fill={track.is_liked ? 'currentColor' : 'none'}
								stroke="currentColor"
								stroke-width="2"
							>
								<path
									d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78L12 21.23l8.84-8.84a5.5 5.5 0 0 0 0-7.78z"
								/>
							</svg>
						</button>
					</div>
				{/each}
			</div>
		</div>

		<!-- Tags -->
		<div class="mt-6">
			<h2 class="mb-1.5 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
				{$translate('nav.tags')}
			</h2>
			<div class="flex flex-wrap items-center gap-1.5">
				{#each release.tags as tag (tag.id)}
					{@const color = tag.color ?? '#888888'}
					<span
						class="inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium"
						style="background-color: {color}20; color: {color}; border: 1px solid {color}40;"
					>
						{tag.name}
					</span>
				{/each}
				<button
					type="button"
					class="inline-flex items-center gap-1 rounded border border-dashed border-stroke px-2 py-0.5 text-xs text-text-secondary active:bg-surface-2"
					onclick={() => (tagPickerOpen = true)}
				>
					<svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M12 5v14M5 12h14" stroke-linecap="round" />
					</svg>
					{$translate('discovery.editor.addTags')}
				</button>
			</div>
		</div>

		<!-- Notes -->
		<div class="mt-6">
			<h2 class="mb-1.5 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
				{$translate('discovery.editor.notes')}
			</h2>
			<textarea
				bind:value={notesValue}
				onblur={handleNotesBlur}
				rows="3"
				placeholder={$translate('discovery.editor.notesPlaceholder')}
				class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
			></textarea>
		</div>
	</div>
</Drawer>

<MobileTagPicker open={tagPickerOpen} {release} onClose={() => (tagPickerOpen = false)} />
