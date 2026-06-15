<script lang="ts">
	// Minimal mobile sign-in trigger to exercise the native OAuth flow end-to-end (#133). The full
	// localized settings UI (account card, devices, sync status) is implemented in #55.
	import { translate } from '$shared/i18n'
	import { syncStatus, signingIn, isSignedIn, cloudSyncError } from '$shared/stores/cloudSync'
	import { signInMobile } from '$lib/signInMobile'
</script>

<main style="font-family: sans-serif; padding: 2rem;">
	<h1>Crate Mobile</h1>

	{#if $isSignedIn}
		<p>{$translate('cloudSync.status.idle')}</p>
		{#if $syncStatus.email}
			<p>{$syncStatus.email}</p>
		{/if}
	{:else}
		<h2>{$translate('cloudSync.signIn.title')}</h2>
		<button onclick={() => signInMobile('google')} disabled={$signingIn}>
			{$signingIn
				? $translate('common.loading')
				: $translate('cloudSync.signIn.button', { values: { provider: 'Google' } })}
		</button>
	{/if}

	{#if $cloudSyncError}
		<p style="color: #c0392b;">{$cloudSyncError}</p>
	{/if}
</main>
