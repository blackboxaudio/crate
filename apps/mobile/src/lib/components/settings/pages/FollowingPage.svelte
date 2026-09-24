<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { FollowCheckCadence } from '$shared/types'
	import { settingsStore, followCheckCadence, releaseDayReminders, newReleasesSummary } from '$shared/stores/settings'
	import { followStore } from '$shared/stores/follow'

	// Following: the mobile-relevant slice of desktop's Discovery-tab Following group. The watcher
	// already runs on mobile (start_watching is unconditional); hourly/daily cadences only tick while
	// the app is alive, so in practice they mean "on launch + while foregrounded". Auto-follow-on-
	// import is intentionally omitted — its only consumer is desktop's bulk-import checkbox, so it
	// would be an inert control here. NOTE: the two notification toggles work on Android today; iOS
	// never requests notification permission yet, so they're silent there until that follow-up lands.
	const cadences: { value: FollowCheckCadence; key: string }[] = [
		{ value: 'on-launch', key: 'settings.following.cadenceLaunch' },
		{ value: 'hourly', key: 'settings.following.cadenceHourly' },
		{ value: 'daily', key: 'settings.following.cadenceDaily' },
		{ value: 'manual', key: 'settings.following.cadenceManual' },
	]
</script>

<div class="px-4 py-2">
	<p class="mb-2 text-xs text-text-tertiary">{$translate('settings.following.checkCadenceDescription')}</p>
	<div class="grid grid-cols-2 gap-2">
		{#each cadences as cadence (cadence.value)}
			<button
				type="button"
				class="rounded-md px-3 py-2.5 text-sm font-medium transition-colors {$followCheckCadence === cadence.value
					? 'bg-brand-primary text-white'
					: 'bg-surface-2 text-text-secondary active:opacity-70'}"
				onclick={() => void settingsStore.setFollowCheckCadence(cadence.value)}
			>
				{$translate(cadence.key)}
			</button>
		{/each}
	</div>

	<div class="mt-5 flex flex-col gap-3">
		<button
			type="button"
			class="flex w-full items-center justify-between rounded-md py-1 active:bg-surface-2"
			aria-pressed={$releaseDayReminders}
			onclick={() => void settingsStore.setReleaseDayReminders(!$releaseDayReminders)}
		>
			<span class="text-sm font-medium text-text-primary">{$translate('settings.following.releaseDayReminders')}</span>
			<span
				class="flex h-5 w-9 items-center rounded-full p-0.5 transition-colors {$releaseDayReminders
					? 'bg-brand-primary'
					: 'bg-stroke'}"
			>
				<span class="h-4 w-4 rounded-full bg-white transition-transform {$releaseDayReminders ? 'translate-x-4' : ''}"
				></span>
			</span>
		</button>
		<p class="-mt-2 text-xs text-text-tertiary">{$translate('settings.following.releaseDayRemindersHelper')}</p>

		<button
			type="button"
			class="flex w-full items-center justify-between rounded-md py-1 active:bg-surface-2"
			aria-pressed={$newReleasesSummary}
			onclick={() => void settingsStore.setNewReleasesSummary(!$newReleasesSummary)}
		>
			<span class="text-sm font-medium text-text-primary">{$translate('settings.following.newReleasesSummary')}</span>
			<span
				class="flex h-5 w-9 items-center rounded-full p-0.5 transition-colors {$newReleasesSummary
					? 'bg-brand-primary'
					: 'bg-stroke'}"
			>
				<span class="h-4 w-4 rounded-full bg-white transition-transform {$newReleasesSummary ? 'translate-x-4' : ''}"
				></span>
			</span>
		</button>
	</div>

	<button
		type="button"
		class="mt-5 w-full rounded-md bg-surface-2 px-3 py-2.5 text-sm font-medium text-text-primary active:opacity-70 disabled:opacity-50"
		disabled={$followStore.checkingAll}
		onclick={() => void followStore.checkAll()}
	>
		{$followStore.checkingAll ? $translate('common.loading') : $translate('settings.following.checkAllNow')}
	</button>

	<p class="mt-4 text-xs text-text-tertiary">{$translate('settings.following.discogsRateLimitNote')}</p>
</div>
