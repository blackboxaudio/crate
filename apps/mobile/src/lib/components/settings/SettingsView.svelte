<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { Theme } from '$shared/types'
	import { settingsStore, theme } from '$shared/stores/settings'
	import { syncStatus, signingIn, isSignedIn, cloudSyncError } from '$shared/stores/cloudSync'
	import { signInMobile } from '$lib/signInMobile'

	// Settings tab: a minimal appearance (theme) + cloud account panel. Owns its own scroll container (the
	// shell frame is overflow-hidden). Full settings (accent / font / language pickers) is a later issue;
	// here it exposes a theme toggle and surfaces cloud sign-in (#133).
	const themeOptions: { value: Theme; key: string }[] = [
		{ value: 'light', key: 'settings.appearance.themeLight' },
		{ value: 'dark', key: 'settings.appearance.themeDark' },
		{ value: 'system', key: 'settings.appearance.themeSystem' },
	]
</script>

<div class="h-full overflow-y-auto pt-2" style="padding-bottom: var(--mini-player-inset, 0px)">
	<!-- Appearance: theme toggle -->
	<div class="px-4 py-2">
		<h3 class="mb-1.5 text-sm font-medium text-text-secondary">
			{$translate('settings.appearance.theme')}
		</h3>
		<div class="flex gap-2">
			{#each themeOptions as option (option.value)}
				<button
					type="button"
					class="flex-1 rounded-md px-3 py-2 text-sm font-medium transition-colors {$theme === option.value
						? 'bg-brand-primary text-white'
						: 'bg-surface-2 text-text-secondary active:bg-surface-2'}"
					onclick={() => settingsStore.setTheme(option.value)}
				>
					{$translate(option.key)}
				</button>
			{/each}
		</div>
	</div>

	<!-- Sync / account -->
	<div class="mt-2 border-t border-stroke-subtle px-4 py-3">
		{#if $isSignedIn}
			<p class="text-sm text-text-primary">{$translate('cloudSync.status.idle')}</p>
			{#if $syncStatus.email}
				<p class="mt-0.5 text-xs text-text-tertiary">{$syncStatus.email}</p>
			{/if}
		{:else}
			<p class="mb-2 text-sm text-text-secondary">{$translate('cloudSync.signIn.title')}</p>
			<button
				type="button"
				class="w-full rounded-md bg-brand-primary px-3 py-2 text-sm font-medium text-white active:opacity-80 disabled:opacity-50"
				onclick={() => signInMobile('google')}
				disabled={$signingIn}
			>
				{$signingIn
					? $translate('common.loading')
					: $translate('cloudSync.signIn.button', { values: { provider: 'Google' } })}
			</button>
		{/if}
		{#if $cloudSyncError}
			<p class="mt-2 text-xs text-danger">{$cloudSyncError}</p>
		{/if}
	</div>
</div>
