<script lang="ts">
	import { translate } from '$shared/i18n'
	import { activeTab } from '$lib/stores/mobileUI'
	import SyncStatusButton from './SyncStatusButton.svelte'
	import SettingsButton from './SettingsButton.svelte'

	// Top bar, laid out as three zones so every tab reads the same: the LEADING slot always shows the
	// accent-masked Crate logo mark (brand identity, constant across tabs); the CENTER shows the active tab's
	// destination title ("Discovery", "Playlists", …), absolutely centered so it stays put regardless of how
	// wide the leading/trailing zones are; the TRAILING slot holds the cloud-sync / account chip (self-hides
	// when sync isn't configured) and the settings gear. Navigation lives in the bottom TabBar.

	// The centered page title for the active tab.
	const title = $derived.by(() => {
		switch ($activeTab) {
			case 'following':
				return $translate('discovery.following.title')
			case 'playlists':
				return $translate('nav.playlists')
			case 'tags':
				return $translate('nav.tags')
			default:
				return $translate('nav.discovery')
		}
	})
</script>

<header class="pt-safe fixed inset-x-0 top-0 z-30 border-b border-stroke-subtle bg-surface-1">
	<div class="relative flex h-14 items-center px-4">
		<!-- Leading: the accent-masked Crate logo mark — constant on every tab so the bar keeps a brand
		     anchor. The SVG wraps an alpha-only raster, so mask it (tracking the active accent, mirroring the
		     desktop Sidebar / splash) rather than rendering it as an <img>. -->
		<div
			class="h-7 w-7 flex-shrink-0 bg-brand-primary"
			style="-webkit-mask-image: url('/crate-logo.svg'); -webkit-mask-size: contain; -webkit-mask-repeat: no-repeat; -webkit-mask-position: center; mask-image: url('/crate-logo.svg'); mask-size: contain; mask-repeat: no-repeat; mask-position: center;"
		></div>

		<!-- Center: the active tab's title, absolutely centered so it's independent of the leading/trailing
		     widths. `pointer-events-none` keeps it from swallowing taps meant for the controls it overlaps at
		     its edges. -->
		<div class="pointer-events-none absolute inset-x-0 flex justify-center">
			<h1 class="max-w-[55%] truncate text-base font-semibold tracking-tight text-text-primary">
				{title}
			</h1>
		</div>

		<!-- Trailing: cloud-sync / account chip (self-hides when sync is unavailable) + settings gear. -->
		<div class="ml-auto flex flex-shrink-0 items-center gap-1">
			<SyncStatusButton />
			<SettingsButton />
		</div>
	</div>
</header>
