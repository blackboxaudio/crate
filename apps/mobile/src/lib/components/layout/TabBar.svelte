<script lang="ts">
	import { translate } from '$shared/i18n'
	import { mobileUIStore, activeTab, type MobileTab } from '$lib/stores/mobileUI'

	// Bottom tab bar: the app's primary navigation (iOS-conventional). Four destinations — Discovery,
	// Following, Playlists, Tags — each an icon over a label. Settings is not a tab: it opens as a right-side
	// drawer from the Header's gear button, which keeps the labels roomy enough that long localizations (e.g.
	// Japanese) don't overflow the sliding indicator. Pinned to the bottom edge and owns the bottom safe-area
	// inset; the mini-player docks directly above it. Full-screen surfaces (the release detail push, the
	// expanded player) cover it, so it only shows on the main shell. Each button is a 44pt+ hit area.
	type Tab = { id: MobileTab; label: string }
	const tabs: Tab[] = $derived([
		{ id: 'discovery', label: $translate('nav.discovery') },
		{ id: 'following', label: $translate('discovery.following.title') },
		{ id: 'playlists', label: $translate('nav.playlists') },
		{ id: 'tags', label: $translate('nav.tags') },
	])

	// Index of the active tab within `tabs` — drives the horizontal offset of the sliding highlight
	// pill in the bar below. Clamped to 0 so the pill never slides off-bar if `activeTab` is ever
	// unmatched.
	const activeIndex = $derived(
		Math.max(
			0,
			tabs.findIndex((t) => t.id === $activeTab)
		)
	)

	// Activate tabs on pointer-DOWN for touch — not on click. iOS WebKit defers `click` dispatch to a
	// fixed element like this bar until an in-progress momentum ("flick") scroll of the current tab's
	// content settles, so tapping a tab mid-scroll felt dead until the list coasted to a stop.
	// `pointerdown` fires on finger-down — the same touch that cancels the momentum — so the tab activates
	// immediately. Mouse, pen, and keyboard/VoiceOver activate via `onclick` (natural press semantics +
	// synthesized-click a11y).
	//
	// `activateTab` re-taps do real work now (pop-to-root / scroll-to-top drive one-shot nonces), so — unlike
	// the old idempotent `setTab` — the trailing synthesized click after a touch tap must NOT fire it a second
	// time (that would pop two levels or double-scroll). Latch on the touch pointerdown and swallow that one
	// click; a mouse pointerdown clears the latch so its own click still activates.
	let suppressClick = false
	function onTabPointerDown(e: PointerEvent, tab: MobileTab) {
		suppressClick = false
		if (e.pointerType === 'touch') {
			suppressClick = true
			mobileUIStore.activateTab(tab)
		}
	}
	function onTabClick(tab: MobileTab) {
		if (suppressClick) {
			suppressClick = false
			return
		}
		mobileUIStore.activateTab(tab)
	}
</script>

<nav class="pb-safe fixed inset-x-0 bottom-0 z-30 border-t border-stroke-subtle bg-surface-1">
	<div class="relative flex h-14 items-stretch">
		<!-- Sliding highlight: one brand-tinted pill that slides horizontally to sit behind the active
		     tab, carrying the highlight from the old tab to the new (mirrors the desktop tabs' sliding
		     indicator). Decorative and behind the buttons (z-0 vs their z-10); each tab's icon + label
		     crossfade their color in sync as it arrives. `ease-fluid` is the app's iOS-sheet easing;
		     reduced-motion users get an instant jump. Width and offset are derived from the tab count
		     so adding or removing a destination needs no hand-tuning. -->
		<div
			class="ease-fluid pointer-events-none absolute inset-y-0 left-0 z-0 transition-transform duration-300 motion-reduce:transition-none"
			style="width: {100 / tabs.length}%; transform: translateX({activeIndex * 100}%)"
			aria-hidden="true"
		>
			<div class="absolute inset-x-1.5 inset-y-1.5 rounded-2xl bg-brand-muted"></div>
		</div>
		{#each tabs as tab (tab.id)}
			{@const active = $activeTab === tab.id}
			<button
				type="button"
				class="relative z-10 flex flex-1 flex-col items-center justify-center gap-1 transition-colors duration-300 {active
					? 'text-brand-primary'
					: 'text-text-tertiary'}"
				aria-current={active ? 'page' : undefined}
				aria-label={tab.label}
				onpointerdown={(e) => onTabPointerDown(e, tab.id)}
				onclick={() => onTabClick(tab.id)}
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
				{:else if tab.id === 'tags'}
					<!-- `tag` — matches the desktop tag icon (Icon.svelte). -->
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
							d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A2 2 0 013 12V7a4 4 0 014-4z"
						/>
					</svg>
				{:else}
					<!-- `rss` — matches the desktop Following icon (Icon.svelte): two broadcast arcs over a dot. -->
					<svg
						class="h-6 w-6"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					>
						<path d="M5 12a7 7 0 0 1 7 7" />
						<path d="M5 5a14 14 0 0 1 14 14" />
						<circle cx="5.5" cy="18.5" r="1.5" fill="currentColor" stroke="none" />
					</svg>
				{/if}
				<span class="text-[11px] leading-none font-medium">{tab.label}</span>
			</button>
		{/each}
	</div>
</nav>
