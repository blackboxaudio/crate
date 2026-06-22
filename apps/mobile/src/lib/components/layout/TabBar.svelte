<script lang="ts">
	import { translate } from '$shared/i18n'
	import { mobileUIStore, activeTab, type MobileTab } from '$lib/stores/mobileUI'

	// Bottom tab bar: the app's primary navigation (iOS-conventional). Three destinations — Discovery,
	// Playlists, Settings — each an icon over a label. Pinned to the bottom edge and owns the bottom
	// safe-area inset; the mini-player docks directly above it. Full-screen surfaces (the release detail
	// push, the expanded player) cover it, so it only shows on the main shell. Each button is a 44pt+ hit
	// area.
	type Tab = { id: MobileTab; label: string }
	const tabs: Tab[] = $derived([
		{ id: 'discovery', label: $translate('nav.discovery') },
		{ id: 'playlists', label: $translate('nav.playlists') },
		{ id: 'settings', label: $translate('settings.title') },
	])

	// Switch tabs on pointer-DOWN for touch — not on click. iOS WebKit defers `click` dispatch to a
	// fixed element like this bar until an in-progress momentum ("flick") scroll of the current tab's
	// content settles, so tapping a tab mid-scroll felt dead until the list coasted to a stop.
	// `pointerdown` fires on finger-down — the same touch that cancels the momentum — so the tab switches
	// immediately. Mouse, pen, and keyboard/VoiceOver keep activating via `onclick` below (natural press
	// semantics + synthesized-click a11y); `setTab` no-ops when already on the tab, so the trailing click
	// after a touch tap is harmless. A future tab could veto/confirm the switch here before navigating.
	function navigateOnTouch(e: PointerEvent, tab: MobileTab) {
		if (e.pointerType === 'touch') mobileUIStore.setTab(tab)
	}
</script>

<nav class="pb-safe fixed inset-x-0 bottom-0 z-30 border-t border-stroke-subtle bg-surface-1">
	<div class="flex h-14 items-stretch">
		{#each tabs as tab (tab.id)}
			{@const active = $activeTab === tab.id}
			<button
				type="button"
				class="flex flex-1 flex-col items-center justify-center gap-1 {active
					? 'text-brand-primary'
					: 'text-text-tertiary'} active:bg-surface-2"
				aria-current={active ? 'page' : undefined}
				aria-label={tab.label}
				onpointerdown={(e) => navigateOnTouch(e, tab.id)}
				onclick={() => mobileUIStore.setTab(tab.id)}
			>
				{#if tab.id === 'discovery'}
					<!-- `globe` — matches the desktop Discovery icon (settings Discovery tab / Icon.svelte). -->
					<svg
						class="h-6 w-6"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					>
						<path
							d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z M3.6 9h16.8M3.6 15h16.8 M12 3a15.3 15.3 0 014 9 15.3 15.3 0 01-4 9 15.3 15.3 0 01-4-9 15.3 15.3 0 014-9z"
						/>
					</svg>
				{:else if tab.id === 'playlists'}
					<svg
						class="h-6 w-6"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					>
						<line x1="4" y1="6" x2="20" y2="6" />
						<line x1="4" y1="12" x2="20" y2="12" />
						<line x1="4" y1="18" x2="13" y2="18" />
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
						<circle cx="12" cy="12" r="3" />
						<path
							d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
						/>
					</svg>
				{/if}
				<span class="text-[11px] leading-none font-medium">{tab.label}</span>
			</button>
		{/each}
	</div>
</nav>
