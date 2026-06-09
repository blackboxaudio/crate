<script lang="ts">
	import Icon from '$lib/components/common/Icon.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { translate } from '$lib/i18n'
	import { followNewCount } from '$lib/stores'
	import FollowingModal from './FollowingModal.svelte'

	let showFollowingModal = $state(false)
</script>

<Tooltip text={$translate('discovery.following.title')} position="bottom" delay={250}>
	<button
		type="button"
		class="relative inline-flex h-6 w-6 items-center justify-center rounded-md text-text-secondary transition-colors hover:cursor-pointer hover:bg-surface-2 hover:text-text-primary"
		onclick={() => (showFollowingModal = true)}
		aria-label={$translate('discovery.following.title')}
	>
		<Icon name="rss" />
		{#if $followNewCount > 0}
			<span
				class="absolute -top-1.5 -right-1.5 flex h-3.5 min-w-3.5 items-center justify-center rounded-full bg-brand-primary px-0.5 text-[9px] leading-none font-bold text-white"
			>
				{$followNewCount > 99 ? '99+' : $followNewCount}
			</span>
		{/if}
	</button>
</Tooltip>

{#if showFollowingModal}
	<FollowingModal open={showFollowingModal} onClose={() => (showFollowingModal = false)} />
{/if}
