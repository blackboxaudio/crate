<script lang="ts">
	import { translate } from '$shared/i18n'
	import { mobileUIStore, type SettingsPage } from '$lib/stores/mobileUI'
	import { lightTap } from '$lib/utils/haptics'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'

	// The settings root: iOS-style inset-grouped sections of chevron rows, one per sub-page. Pure
	// navigation — every actual control lives in its page. Rows carry iOS-style colored icon tiles
	// (fixed hues on purpose, like Settings.app — they don't follow the accent), and each group is a
	// borderless filled card with separators inset past the tile. The fill is surface-1 in dark
	// (cells lighter than the black page, like iOS dark grouped) but surface-2 in light, where
	// surface-1 is near-invisible against the white drawer.
	type Row = { page: SettingsPage; labelKey: string; icon: SettingsPage; tint: string }
	const groups: Row[][] = [
		[
			{ page: 'general', labelKey: 'settings.tabs.general', icon: 'general', tint: 'bg-zinc-500' },
			{ page: 'appearance', labelKey: 'settings.tabs.appearance', icon: 'appearance', tint: 'bg-blue-500' },
		],
		[
			{ page: 'following', labelKey: 'settings.following.title', icon: 'following', tint: 'bg-orange-500' },
			{ page: 'cloudSync', labelKey: 'settings.tabs.cloudSync', icon: 'cloudSync', tint: 'bg-sky-500' },
			{ page: 'storage', labelKey: 'settings.tabs.storage', icon: 'storage', tint: 'bg-emerald-500' },
		],
		[{ page: 'about', labelKey: 'settings.tabs.about', icon: 'about', tint: 'bg-indigo-500' }],
	]

	function goTo(page: SettingsPage) {
		void lightTap()
		mobileUIStore.setSettingsPage(page)
	}
</script>

<div class="px-4 pt-1 pb-6">
	<!-- iOS large title: the drawer's pinned header stays empty on the root (chevron only), so the
	     title lives in the scrolling content like a pushed Settings screen. -->
	<h1 class="pb-3 text-[28px] font-bold tracking-tight text-text-primary">{$translate('settings.title')}</h1>
	<div class="space-y-6">
		{#each groups as group, groupIndex (groupIndex)}
			<div class="overflow-hidden rounded-xl bg-surface-1 [[data-theme=light]_&]:bg-surface-2">
				{#each group as row, i (row.page)}
					{#if i > 0}
						<!-- Inset separator: starts where the label does (px-4 + 28px tile + gap-3 = 56px). -->
						<div class="ml-14 h-px bg-stroke-subtle"></div>
					{/if}
					<MobileListItem onclick={() => goTo(row.page)}>
						{#snippet leading()}
							<span class="flex h-7 w-7 items-center justify-center rounded-md {row.tint} text-white">
								<svg
									class="h-4 w-4"
									viewBox="0 0 24 24"
									fill="none"
									stroke="currentColor"
									stroke-width="2"
									stroke-linecap="round"
									stroke-linejoin="round"
								>
									{#if row.icon === 'general'}
										<circle cx="12" cy="12" r="3" />
										<path
											d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 11-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 11-4 0v-.09a1.65 1.65 0 00-1-1.51 1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 11-2.83-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 110-4h.09a1.65 1.65 0 001.51-1 1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 112.83-2.83l.06.06a1.65 1.65 0 001.82.33h0a1.65 1.65 0 001-1.51V3a2 2 0 114 0v.09a1.65 1.65 0 001 1.51h0a1.65 1.65 0 001.82-.33l.06-.06a2 2 0 112.83 2.83l-.06.06a1.65 1.65 0 00-.33 1.82v0a1.65 1.65 0 001.51 1H21a2 2 0 110 4h-.09a1.65 1.65 0 00-1.51 1z"
										/>
									{:else if row.icon === 'appearance'}
										<circle cx="12" cy="12" r="5" />
										<path
											d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"
										/>
									{:else if row.icon === 'following'}
										<path d="M5 12a7 7 0 0 1 7 7" />
										<path d="M5 5a14 14 0 0 1 14 14" />
										<circle cx="5.5" cy="18.5" r="1.5" fill="currentColor" stroke="none" />
									{:else if row.icon === 'cloudSync'}
										<path d="M17.5 19a4.5 4.5 0 100-9 6 6 0 00-11.5 1.5A4 4 0 007 19h10.5z" />
									{:else if row.icon === 'storage'}
										<ellipse cx="12" cy="5" rx="8" ry="3" />
										<path d="M4 5v6c0 1.66 3.58 3 8 3s8-1.34 8-3V5" />
										<path d="M4 11v6c0 1.66 3.58 3 8 3s8-1.34 8-3v-6" />
									{:else}
										<circle cx="12" cy="12" r="9" />
										<path d="M12 8h.01M11 12h1v4h1" />
									{/if}
								</svg>
							</span>
						{/snippet}
						{#snippet trailing()}
							<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
								<path d="M9 18l6-6-6-6" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						{/snippet}
						<span class="block truncate text-[15px] text-text-primary">{$translate(row.labelKey)}</span>
					</MobileListItem>
				{/each}
			</div>
		{/each}
	</div>
</div>
