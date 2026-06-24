<script lang="ts">
	import type { DiscoveryRelease } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { followStore, followedSources } from '$shared/stores/follow'
	import { deriveArtistUrl, deriveLabelUrl, isCompilation, looseUrlEq } from '$shared/utils'
	import { getReleasePlatformName } from '$shared/utils/discoveryLinks'
	import { lightTap } from '$lib/utils/haptics'
	import MobileModal from '$lib/components/common/MobileModal.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'

	// Inline follow sheet for a discovery release — the mobile counterpart of the desktop FollowPopover. A
	// release yields up to two follow targets: the artist (its own Bandcamp subdomain / SoundCloud profile)
	// and the label (the page it was discovered from). Each is followed independently. Mounted once (in
	// `+page`) and driven by `release` / `open` so it slides out cleanly on dismiss; the resolved release is
	// latched into `displayed` so content keeps rendering through that animation.
	type Props = {
		/** The release to follow, or null when the sheet is closed. */
		release: DiscoveryRelease | null
		onClose: () => void
	}
	let { release, onClose }: Props = $props()

	const open = $derived(release != null)

	// Latch the release so the sheet's content survives the close animation (after `release` goes null).
	let displayed = $state<DiscoveryRelease | null>(null)
	let lastId = ''
	let selectedOverride = $state<'artist' | 'label' | null>(null)
	$effect(() => {
		if (!release) return
		displayed = release
		// Reset the tab pick and refresh follow state only when a *new* release opens (not on every tick),
		// so the user's tab choice isn't clobbered and we don't reload mid-interaction.
		if (release.id !== lastId) {
			lastId = release.id
			selectedOverride = null
			followStore.load()
		}
	})

	// A Various-Artists compilation is a label release, not an artist's: drop the artist target and treat the
	// release's own page as the label when no separate label page is known (mirrors FollowPopover).
	const isComp = $derived(displayed ? isCompilation(displayed.artist) : false)
	const artistUrl = $derived(displayed && !isComp ? deriveArtistUrl(displayed) : null)
	const labelUrl = $derived(
		displayed ? (isComp ? (deriveLabelUrl(displayed) ?? deriveArtistUrl(displayed)) : deriveLabelUrl(displayed)) : null
	)
	const artistAvailable = $derived(!!artistUrl)
	const labelAvailable = $derived(!!labelUrl)
	const hasFollowable = $derived(artistAvailable || labelAvailable)

	// Selected target — honor the user's pick when that side exists, else default to Label (the common
	// discovery case) when present, otherwise Artist.
	const selected = $derived(
		selectedOverride === 'artist' && artistAvailable
			? 'artist'
			: selectedOverride === 'label' && labelAvailable
				? 'label'
				: labelAvailable
					? 'label'
					: 'artist'
	)

	const artistFollow = $derived(artistUrl ? $followedSources.find((s) => looseUrlEq(s.url, artistUrl)) : undefined)
	const labelFollow = $derived(labelUrl ? $followedSources.find((s) => looseUrlEq(s.url, labelUrl)) : undefined)

	const currentUrl = $derived(selected === 'label' ? labelUrl : artistUrl)
	const currentFollow = $derived(selected === 'label' ? labelFollow : artistFollow)
	const currentName = $derived(
		displayed
			? selected === 'label'
				? (displayed.label ?? displayed.artist)
				: (displayed.artist ?? displayed.label)
			: null
	)
	const displayName = $derived(currentFollow?.name ?? currentName ?? $translate('common.unknownArtist'))
	const typeLabel = $derived(
		selected === 'label' ? $translate('discovery.following.label') : $translate('discovery.following.artist')
	)
	const platformName = $derived(displayed ? getReleasePlatformName(displayed.source_type) : '')

	let busy = $state(false)
	async function toggle() {
		if (busy || !currentUrl || !displayed) return
		void lightTap()
		busy = true
		if (currentFollow) {
			await followStore.unfollow(currentFollow.id)
		} else {
			await followStore.followEntity({
				url: currentUrl,
				name: currentName ?? null,
				sourceType: displayed.source_type,
				followType: selected,
			})
		}
		busy = false
	}
</script>

<MobileModal {open} title={$translate('discovery.following.popoverTitle')} {onClose}>
	{#if displayed}
		{#if hasFollowable}
			<div class="flex flex-col gap-4">
				<!-- Target row: avatar + name + type/platform + follow/unfollow toggle. -->
				<div class="flex items-center gap-3">
					{#if currentFollow?.artworkUrl}
						<img src={currentFollow.artworkUrl} alt="" class="h-12 w-12 flex-shrink-0 rounded object-cover" />
					{:else}
						<div
							class="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded bg-surface-2 text-text-tertiary"
						>
							{#if selected === 'label'}
								<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<circle cx="12" cy="12" r="9" />
									<circle cx="12" cy="12" r="2.5" />
								</svg>
							{:else}
								<svg
									class="h-6 w-6"
									viewBox="0 0 24 24"
									fill="none"
									stroke="currentColor"
									stroke-width="2"
									stroke-linecap="round"
									stroke-linejoin="round"
								>
									<circle cx="12" cy="8" r="4" />
									<path d="M4 20c0-4 4-6 8-6s8 2 8 6" />
								</svg>
							{/if}
						</div>
					{/if}
					<div class="min-w-0 flex-1">
						<div class="truncate text-sm font-medium text-text-primary">{displayName}</div>
						<div class="truncate text-xs text-text-tertiary">{typeLabel} · {platformName}</div>
					</div>
					<button
						type="button"
						class="flex shrink-0 items-center gap-1.5 rounded-lg px-3 py-2 text-sm font-medium transition-colors disabled:opacity-60 {currentFollow
							? 'bg-brand-muted text-brand-primary'
							: 'bg-surface-2 text-text-secondary active:opacity-80'}"
						disabled={busy}
						onclick={toggle}
					>
						{#if busy}
							<Spinner class="h-3.5 w-3.5" />
						{/if}
						{currentFollow ? $translate('discovery.following.following') : $translate('discovery.following.follow')}
					</button>
				</div>

				<!-- Label | Artist segmented control — shown only when both targets exist (otherwise the single
				     available target is implied by the row above). -->
				{#if artistAvailable && labelAvailable}
					<div class="relative grid grid-cols-2 rounded-lg border border-stroke bg-surface-2 p-0.5 text-xs font-medium">
						<div
							class="absolute top-0.5 bottom-0.5 left-0.5 w-[calc(50%-2px)] rounded-md bg-surface-0 shadow-sm transition-transform duration-200 ease-out motion-reduce:transition-none"
							style="transform: translateX({selected === 'artist' ? '100%' : '0%'})"
						></div>
						<button
							type="button"
							class="relative z-10 rounded-md py-2 text-center transition-colors {selected === 'label'
								? 'text-text-primary'
								: 'text-text-tertiary'}"
							onclick={() => (selectedOverride = 'label')}
						>
							{$translate('discovery.following.label')}
						</button>
						<button
							type="button"
							class="relative z-10 rounded-md py-2 text-center transition-colors {selected === 'artist'
								? 'text-text-primary'
								: 'text-text-tertiary'}"
							onclick={() => (selectedOverride = 'artist')}
						>
							{$translate('discovery.following.artist')}
						</button>
					</div>
				{/if}
			</div>
		{:else}
			<p class="py-2 text-sm text-text-tertiary">{$translate('discovery.following.followViaPaste')}</p>
		{/if}
	{/if}
</MobileModal>
