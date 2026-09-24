<script lang="ts">
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { CollectionAccount, FollowCheckCadence } from '$shared/types'
	import { collectionStore, collectionAccounts } from '$shared/stores/collection'
	import { settingsStore, collectionRefreshCadence } from '$shared/stores/settings'
	import { confirmDialog } from '$lib/utils/dialog'
	import MobilePromptDialog from '$lib/components/common/MobilePromptDialog.svelte'

	// Collection: manage the linked Bandcamp fan accounts whose public purchase collections mark
	// releases as owned. Roster + link-by-URL (the FollowingView add-source pattern: centered prompt,
	// no client-side validation — the backend fetches the page and errors via toast, keeping the
	// dialog open) + per-account pause/unlink + refresh cadence + a manual refresh-all. As with
	// Following, hourly/daily cadences only tick while the app is alive, so in practice they mean
	// "on launch + while foregrounded".
	const cadences: { value: FollowCheckCadence; key: string }[] = [
		{ value: 'on-launch', key: 'settings.following.cadenceLaunch' },
		{ value: 'hourly', key: 'settings.following.cadenceHourly' },
		{ value: 'daily', key: 'settings.following.cadenceDaily' },
		{ value: 'manual', key: 'settings.following.cadenceManual' },
	]

	let linkOpen = $state(false)
	let linkUrl = $state('')
	let linkBusy = $state(false)

	async function submitLink() {
		const trimmed = linkUrl.trim()
		if (!trimmed || linkBusy) return
		linkBusy = true
		try {
			const account = await collectionStore.linkFromUrl(trimmed)
			if (account) {
				linkOpen = false
				linkUrl = ''
			}
		} finally {
			linkBusy = false
		}
	}

	async function remove(account: CollectionAccount) {
		const t = get(translate)
		const ok = await confirmDialog(t('settings.collection.removeConfirmMessage'), {
			title: t('settings.collection.removeConfirmTitle'),
			confirmLabel: t('common.remove'),
		})
		if (ok) await collectionStore.unlink(account.id)
	}

	function displayName(account: CollectionAccount): string {
		return account.name ?? account.username ?? account.url
	}
</script>

<div class="px-4 py-2">
	<p class="mb-3 text-xs text-text-tertiary">{$translate('settings.collection.description')}</p>

	{#if $collectionAccounts.length > 0}
		<div class="overflow-hidden rounded-xl bg-surface-1 [[data-theme=light]_&]:bg-surface-2">
			{#each $collectionAccounts as account, i (account.id)}
				{#if i > 0}
					<div class="ml-14 h-px bg-stroke-subtle"></div>
				{/if}
				<div class="flex items-center gap-3 px-4 py-2.5">
					{#if account.avatarUrl}
						<img src={account.avatarUrl} alt="" class="h-8 w-8 flex-shrink-0 rounded-full object-cover" />
					{:else}
						<div
							class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-surface-2 text-text-tertiary"
						>
							<svg
								class="h-4 w-4"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
								stroke-linecap="round"
								stroke-linejoin="round"
							>
								<path d="M6 8h12l-1.2 12H7.2L6 8z" />
								<path d="M9 8V6a3 3 0 0 1 6 0v2" />
							</svg>
						</div>
					{/if}
					<div class="flex min-w-0 flex-1 flex-col leading-tight">
						<span class="truncate text-[15px] text-text-primary">{displayName(account)}</span>
						<span class="truncate text-xs text-text-tertiary">
							{$translate('settings.collection.itemCount', { values: { count: account.itemCount } })}
							{#if account.health === 'error' || account.health === 'rate_limited'}
								· <span class="text-danger">{account.lastError ?? $translate('errors.generic')}</span>
							{/if}
						</span>
					</div>
					<!-- Pause: items stop counting as owned (syncs, like a follow's enabled flag). -->
					<button
						type="button"
						aria-pressed={account.enabled}
						aria-label={$translate('settings.collection.enabledToggle')}
						class="flex h-5 w-9 flex-shrink-0 items-center rounded-full p-0.5 transition-colors {account.enabled
							? 'bg-brand-primary'
							: 'bg-stroke'}"
						onclick={() => void collectionStore.setEnabled(account.id, !account.enabled)}
					>
						<span class="h-4 w-4 rounded-full bg-white transition-transform {account.enabled ? 'translate-x-4' : ''}"
						></span>
					</button>
					<button
						type="button"
						aria-label={$translate('common.remove')}
						class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md text-text-tertiary active:bg-surface-2"
						onclick={() => void remove(account)}
					>
						<svg
							class="h-4 w-4"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							stroke-linejoin="round"
						>
							<path d="M18 6L6 18M6 6l12 12" />
						</svg>
					</button>
				</div>
			{/each}
		</div>
	{:else}
		<p class="py-3 text-center text-sm text-text-secondary">{$translate('settings.collection.noAccounts')}</p>
	{/if}

	<button
		type="button"
		class="mt-4 w-full rounded-md bg-brand-primary px-3 py-2.5 text-sm font-semibold text-white active:opacity-90 disabled:opacity-50"
		disabled={$collectionStore.linking}
		onclick={() => (linkOpen = true)}
	>
		{$collectionStore.linking ? $translate('common.loading') : $translate('settings.collection.linkAccount')}
	</button>

	{#if $collectionAccounts.length > 0}
		<p class="mt-5 mb-2 text-xs text-text-tertiary">{$translate('settings.collection.refreshCadenceDescription')}</p>
		<div class="grid grid-cols-2 gap-2">
			{#each cadences as cadence (cadence.value)}
				<button
					type="button"
					class="rounded-md px-3 py-2.5 text-sm font-medium transition-colors {$collectionRefreshCadence ===
					cadence.value
						? 'bg-brand-primary text-white'
						: 'bg-surface-2 text-text-secondary active:opacity-70'}"
					onclick={() => void settingsStore.setCollectionRefreshCadence(cadence.value)}
				>
					{$translate(cadence.key)}
				</button>
			{/each}
		</div>

		<button
			type="button"
			class="mt-4 w-full rounded-md bg-surface-2 px-3 py-2.5 text-sm font-medium text-text-primary active:opacity-70 disabled:opacity-50"
			disabled={$collectionStore.refreshingAll}
			onclick={() => void collectionStore.refreshAllAccounts()}
		>
			{$collectionStore.refreshingAll ? $translate('common.loading') : $translate('settings.collection.refreshNow')}
		</button>
	{/if}

	<p class="mt-4 text-xs text-text-tertiary">{$translate('settings.collection.privacyNote')}</p>
</div>

<MobilePromptDialog
	open={linkOpen}
	bind:value={linkUrl}
	title={$translate('settings.collection.linkPrompt')}
	message={$translate('settings.collection.linkPromptInfo')}
	placeholder={$translate('settings.collection.linkPlaceholder')}
	confirmLabel={$translate('settings.collection.link')}
	confirmDisabled={!linkUrl.trim() || linkBusy}
	onConfirm={() => void submitLink()}
	onCancel={() => (linkOpen = false)}
/>
