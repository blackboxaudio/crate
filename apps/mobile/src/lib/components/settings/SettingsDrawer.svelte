<script lang="ts">
	import { translate } from '$shared/i18n'
	import { mobileUIStore, settingsPage, type SettingsPage } from '$lib/stores/mobileUI'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import SettingsView from './SettingsView.svelte'

	// Full-width, right-side settings drawer. Settings was pulled out of the bottom TabBar (long localized
	// labels overflowed the 5-tab indicator), so it now pushes in from the right — mirroring the release /
	// playlist / tag detail screens (same Drawer, same left-edge back-swipe). Only mounted while open (by
	// +page's `{#if $settingsOpen}`), so it opens on mount; dismissal flips `open` false, the Drawer slides
	// out, then `onClosed` clears the store so +page unmounts it only after the animation. The panel is
	// opaque and sits above the mini-player (z-45 > z-40), so it simply covers it — the mini-player stays
	// mounted (no hide flag, no slide-down) and is revealed intact when the drawer slides away.
	//
	// Inside, settings is a two-level iOS hierarchy (root grouped list → flat sub-pages, SettingsView owns
	// the in-place slide). This ONE pinned header serves every level: the title tracks the open page, the
	// chevron pops a sub-page back to root and only closes the drawer from the root, and the Drawer's own
	// edge-swipe-to-close is root-only (`panelDrag`) — sub-pages run their own edge swipe that pops instead.
	let open = $state(true)

	// Start the dismissal — both close paths route here: the root chevron and the Drawer's swipe/Esc `onClose`.
	function startClose() {
		open = false
	}

	// Root has no pinned title — it renders an iOS large title in its scrolling content instead
	// (like Settings.app at rest); sub-pages show theirs in the bar.
	const pageTitleKey: Record<Exclude<SettingsPage, 'root'>, string> = {
		general: 'settings.tabs.general',
		appearance: 'settings.tabs.appearance',
		following: 'settings.following.title',
		collection: 'settings.collection.title',
		cloudSync: 'settings.tabs.cloudSync',
		storage: 'settings.tabs.storage',
		about: 'settings.tabs.about',
	}

	function onChevron() {
		if ($settingsPage === 'root') startClose()
		else mobileUIStore.setSettingsPage('root')
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
	panelDrag={$settingsPage === 'root'}
	closeEdgeFrom="left"
	closeEdgeSize={24}
	ariaLabel={$translate('settings.title')}
	class="flex w-full flex-col bg-surface-0"
>
	<!-- Header. Owns the top safe-area inset and mirrors the fixed top bar's surface-1 + hairline so
	     drill-in headers read as the same app chrome. -->
	<div class="pt-safe border-b border-stroke-subtle bg-surface-1">
		<div class="flex items-center gap-1 px-2 py-2">
			<button
				type="button"
				class="flex h-10 w-10 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
				aria-label={$settingsPage === 'root' ? $translate('common.close') : $translate('common.back')}
				onclick={onChevron}
			>
				<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M15 18l-6-6 6-6" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</button>
			{#if $settingsPage !== 'root'}
				<h1 class="truncate text-lg font-semibold tracking-tight text-text-primary">
					{$translate(pageTitleKey[$settingsPage])}
				</h1>
			{/if}
		</div>
	</div>

	<!-- Page router (owns the in-place level slide + per-page scroll containers). -->
	<div class="min-h-0 flex-1">
		<SettingsView />
	</div>
</Drawer>
