<script lang="ts">
	import { Tooltip } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import { syncPhase, isSyncAvailable, isSignedIn } from '$lib/stores/cloudSync'
	import { translate } from '$lib/i18n'

	type Props = {
		onclick?: () => void
	}

	let { onclick }: Props = $props()

	const tooltipText = $derived.by(() => {
		switch ($syncPhase) {
			case 'idle':
				return $translate('cloudSync.status.idle')
			case 'syncing':
				return $translate('cloudSync.status.syncing')
			case 'error':
				return $translate('cloudSync.status.error')
			case 'signedout':
				return $translate('cloudSync.status.signedOut')
			default:
				return ''
		}
	})

	const iconColor = $derived.by(() => {
		switch ($syncPhase) {
			case 'idle':
				return 'text-text-secondary'
			case 'syncing':
				return 'text-brand-primary'
			case 'error':
				return 'text-red-500'
			case 'signedout':
				return 'text-text-tertiary'
			default:
				return 'text-text-secondary'
		}
	})
</script>

{#if $isSyncAvailable}
	<Tooltip text={tooltipText} position="bottom" delay={250}>
		<button
			type="button"
			class="flex h-8 w-8 items-center justify-center rounded-md transition-colors hover:bg-surface-2 {iconColor}"
			{onclick}
		>
			{#if $syncPhase === 'syncing'}
				<Icon name="loader" class="h-[18px] w-[18px] animate-spin" />
			{:else if $syncPhase === 'error'}
				<Icon name="cloud-off" class="h-[18px] w-[18px]" />
			{:else if $isSignedIn}
				<Icon name="cloud" class="h-[18px] w-[18px]" />
			{:else}
				<Icon name="cloud-off" class="h-[18px] w-[18px]" />
			{/if}
		</button>
	</Tooltip>
{/if}
