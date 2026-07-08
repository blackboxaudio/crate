<script lang="ts">
	import { translate } from '$shared/i18n'
	import { mobileUIStore } from '$lib/stores/mobileUI'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import SettingsView from './SettingsView.svelte'

	// Full-width, right-side settings drawer. Settings was pulled out of the bottom TabBar (long localized
	// labels overflowed the 5-tab indicator), so it now pushes in from the right — mirroring the release /
	// playlist / tag detail screens (same Drawer, same left-edge back-swipe). Only mounted while open (by
	// +page's `{#if $settingsOpen}`), so it opens on mount; dismissal flips `open` false, the Drawer slides
	// out, then `onClosed` clears the store so +page unmounts it only after the animation. The panel is
	// opaque and sits above the mini-player (z-45 > z-40), so it simply covers it — the mini-player stays
	// mounted (no hide flag, no slide-down) and is revealed intact when the drawer slides away.
	let open = $state(true)

	// Start the dismissal — both close paths route here: the back chevron and the Drawer's swipe/Esc `onClose`.
	function startClose() {
		open = false
	}
</script>

<Drawer
	{open}
	direction="right"
	onClose={startClose}
	onClosed={mobileUIStore.closeSettings}
	z={45}
	scrimZ={44}
	scrimDismiss={false}
	closeEdgeFrom="left"
	closeEdgeSize={24}
	ariaLabel={$translate('settings.title')}
	class="pt-safe flex w-full flex-col bg-surface-0"
>
	<!-- Header -->
	<div class="flex items-center gap-1 px-2 py-2">
		<button
			type="button"
			class="flex h-10 w-10 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
			aria-label={$translate('common.close')}
			onclick={startClose}
		>
			<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
		</button>
		<h1 class="truncate text-lg font-semibold tracking-tight text-text-primary">
			{$translate('settings.title')}
		</h1>
	</div>

	<!-- Scrollable settings content (owns its own scroll + `settingsScrollTarget` handling). -->
	<div class="min-h-0 flex-1">
		<SettingsView />
	</div>
</Drawer>
