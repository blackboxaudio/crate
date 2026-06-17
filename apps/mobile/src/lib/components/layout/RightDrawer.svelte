<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { Theme } from '$shared/types'
	import { settingsStore, theme } from '$shared/stores/settings'
	import { syncStatus, signingIn, isSignedIn, cloudSyncError } from '$shared/stores/cloudSync'
	import { signInMobile } from '$lib/signInMobile'
	import { swipe } from '$lib/actions/swipe'

	// Right drawer: a minimal Settings + Sync panel. Full settings (accent/font/language pickers) is a
	// later issue; here it exposes a theme toggle and surfaces the cloud sign-in flow (#133). Slides in
	// from the right; supports finger-follow swipe-to-close.
	type Props = {
		open: boolean
		widthPx: number
		onClose: () => void
	}

	let { open, widthPx, onClose }: Props = $props()

	let closeDrag = $state<number | null>(null)

	const openness = $derived(closeDrag ?? (open ? 1 : 0))
	const transitionOn = $derived(closeDrag === null)
	const visible = $derived(openness > 0)
	const offset = $derived((1 - openness) * 100)

	const themeOptions: { value: Theme; key: string }[] = [
		{ value: 'light', key: 'settings.appearance.themeLight' },
		{ value: 'dark', key: 'settings.appearance.themeDark' },
		{ value: 'system', key: 'settings.appearance.themeSystem' },
	]
</script>

<aside
	class="fixed inset-y-0 right-0 z-50 flex w-[85%] max-w-[320px] flex-col overflow-hidden border-l border-stroke bg-surface-1 {transitionOn
		? 'transition-transform duration-300 ease-out motion-reduce:transition-none'
		: ''}"
	style="transform: translateX({offset}%)"
	aria-hidden={!open}
	use:swipe={{
		side: 'right',
		mode: 'close',
		enabled: open,
		onProgress: (o) => (closeDrag = o),
		onOpen: () => (closeDrag = null),
		onClose: () => {
			closeDrag = null
			onClose()
		},
	}}
>
	{#if visible}
		<div class="pt-safe pr-safe flex-1 overflow-y-auto pb-6">
			<h2 class="px-4 pt-4 pb-1 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
				{$translate('settings.title')}
			</h2>

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
	{/if}
</aside>

{#if visible}
	<!-- Edge-grab strip on the backdrop side of the drawer's edge: grab to drag the drawer closed
	     (pairs with the panel's own swipe so the edge is grabbable from either side), or tap to
	     dismiss like the rest of the backdrop. -->
	<button
		type="button"
		aria-label={$translate('common.close')}
		class="fixed inset-y-0 z-[45] w-8 bg-transparent {transitionOn
			? 'transition-[right] duration-300 ease-out motion-reduce:transition-none'
			: ''}"
		style="right: {openness * widthPx}px"
		onclick={onClose}
		use:swipe={{
			side: 'right',
			mode: 'close',
			enabled: open,
			width: widthPx,
			onProgress: (o) => (closeDrag = o),
			onOpen: () => (closeDrag = null),
			onClose: () => {
				closeDrag = null
				onClose()
			},
		}}
	></button>
{/if}
