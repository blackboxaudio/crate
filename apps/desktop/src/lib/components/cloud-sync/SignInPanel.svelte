<script lang="ts">
	import { Text } from '$lib/components/common'
	import ProviderButton from './ProviderButton.svelte'
	import { cloudSyncStore, signingIn } from '$shared/stores/cloudSync'
	import { translate } from '$shared/i18n'

	const providers = [{ id: 'google', displayName: 'Google' }]

	function handleSignIn(providerId: string) {
		cloudSyncStore.signIn(providerId)
	}
</script>

<div class="flex flex-col items-center py-8">
	<svg
		class="mb-4 h-12 w-12 text-text-tertiary"
		fill="none"
		stroke="currentColor"
		stroke-width="1.5"
		viewBox="0 0 24 24"
	>
		<path stroke-linecap="round" stroke-linejoin="round" d="M18 10h-1.26A8 8 0 109 20h9a5 5 0 000-10z" />
	</svg>
	<Text variant="header-2" class="mb-2">{$translate('cloudSync.signIn.title')}</Text>
	<Text variant="caption" as="p" class="mb-6 max-w-xs text-center">{$translate('cloudSync.signIn.description')}</Text>
	<div class="w-full max-w-xs space-y-3">
		{#each providers as provider (provider.id)}
			<ProviderButton
				providerId={provider.id}
				displayName={provider.displayName}
				loading={$signingIn}
				disabled={$signingIn}
				onclick={() => handleSignIn(provider.id)}
			/>
		{/each}
	</div>
</div>
