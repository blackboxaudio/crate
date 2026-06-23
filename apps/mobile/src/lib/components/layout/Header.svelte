<script lang="ts">
	import { onMount } from 'svelte'
	import { fade } from 'svelte/transition'
	import { translate } from '$shared/i18n'
	import { activeTab } from '$lib/stores/mobileUI'
	import SyncStatusButton from './SyncStatusButton.svelte'

	// Top bar. The LEADING slot is contextual: Discovery (home) shows the accent-masked Crate logo +
	// wordmark (brand identity), while the other tabs show their destination title — so the bar reads as a
	// place ("Playlists", "Tags", "Settings") instead of repeating the brand four times. The views no longer
	// render their own title; this bar owns it. The TRAILING slot holds the cloud-sync / account chip, which
	// self-hides when sync isn't configured. Navigation still lives in the bottom TabBar; this bar is brand
	// identity + global account status only.

	// Per-tab leading title for the non-home tabs (Discovery keeps the brand lockup below instead).
	const tabTitle = $derived.by(() => {
		switch ($activeTab) {
			case 'playlists':
				return $translate('nav.playlists')
			case 'tags':
				return $translate('nav.tags')
			case 'settings':
				return $translate('settings.title')
			default:
				return ''
		}
	})

	// The leading slot cross-fades as tabs change; collapse to an instant swap under reduced motion
	// (mirrors MobileShell's approach).
	let reduceMotion = $state(false)
	onMount(() => {
		const mq = window.matchMedia('(prefers-reduced-motion: reduce)')
		reduceMotion = mq.matches
		const update = () => (reduceMotion = mq.matches)
		mq.addEventListener('change', update)
		return () => mq.removeEventListener('change', update)
	})
	const fadeMs = $derived(reduceMotion ? 0 : 160)
</script>

<header class="pt-safe fixed inset-x-0 top-0 z-30 border-b border-stroke-subtle bg-surface-1">
	<div class="flex h-14 items-center justify-between gap-2 px-4">
		<!-- Leading: brand lockup on Discovery, destination title elsewhere. Keyed by tab so the swap
		     cross-fades rather than hard-cutting. -->
		<div class="min-w-0 flex-1">
			{#key $activeTab}
				<div in:fade={{ duration: fadeMs }}>
					{#if $activeTab === 'discovery'}
						<div class="flex items-center gap-2.5">
							<!-- The Crate logo, accent-masked so it tracks the active accent (mirrors the desktop
							     Sidebar and the splash brand treatment). The SVG wraps an alpha-only raster, so mask
							     it rather than rendering it as an <img>. -->
							<div
								class="h-7 w-7 flex-shrink-0 bg-brand-primary"
								style="-webkit-mask-image: url('/crate-logo.svg'); -webkit-mask-size: contain; -webkit-mask-repeat: no-repeat; -webkit-mask-position: center; mask-image: url('/crate-logo.svg'); mask-size: contain; mask-repeat: no-repeat; mask-position: center;"
							></div>
							<span class="text-lg font-semibold tracking-tight text-text-primary">Crate</span>
						</div>
					{:else}
						<h1 class="truncate text-lg font-semibold tracking-tight text-text-primary">{tabTitle}</h1>
					{/if}
				</div>
			{/key}
		</div>

		<!-- Trailing: cloud-sync / account chip (self-hides when sync is unavailable). -->
		<SyncStatusButton />
	</div>
</header>
