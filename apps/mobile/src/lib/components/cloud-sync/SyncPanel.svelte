<script lang="ts">
	import { translate } from '$shared/i18n'
	import { syncStatus, syncPhase, isSignedIn, signingIn, cloudSyncStore } from '$shared/stores/cloudSync'
	import { signInMobile } from '$lib/signInMobile'
	import { formatRelativeDate } from '$shared/utils/format'
	import { confirmDialog } from '$lib/utils/dialog'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import { get } from 'svelte/store'

	type Props = { onSignedOut?: () => void }
	let { onSignedOut }: Props = $props()

	const statusLabel = $derived.by(() => {
		switch ($syncPhase) {
			case 'idle':
				return $translate('cloudSync.status.idle')
			case 'syncing':
				return $translate('cloudSync.status.syncing')
			case 'offline':
				return $translate('cloudSync.status.offline')
			case 'error':
				return $translate('cloudSync.status.error')
			default:
				return $translate('cloudSync.status.signedOut')
		}
	})

	const dotClass = $derived.by(() => {
		switch ($syncPhase) {
			case 'syncing':
				return 'bg-brand-primary animate-pulse'
			case 'offline':
				return 'bg-warning'
			case 'error':
				return 'bg-danger'
			case 'idle':
				return 'bg-success'
			default:
				return 'bg-text-tertiary'
		}
	})

	const lastSynced = $derived(
		$syncStatus.last_synced_at
			? $translate('cloudSync.account.lastSynced', {
					values: { time: formatRelativeDate($syncStatus.last_synced_at, $translate) },
				})
			: null
	)

	const initials = $derived.by(() => {
		const source = ($syncStatus.display_name ?? $syncStatus.email ?? '').trim()
		if (!source) return ''
		return source
			.split(/[\s@._-]+/)
			.filter(Boolean)
			.slice(0, 2)
			.map((p) => p[0]?.toUpperCase() ?? '')
			.join('')
	})
	let photoError = $state(false)
	let lastPhotoUrl: string | null = null
	$effect(() => {
		const url = $syncStatus.photo_url
		if (url !== lastPhotoUrl) {
			lastPhotoUrl = url
			photoError = false
		}
	})
	const showPhoto = $derived(!!$syncStatus.photo_url && !photoError)

	const syncing = $derived($syncPhase === 'syncing')

	async function handleSignOut() {
		const t = get(translate)
		const confirmed = await confirmDialog(t('cloudSync.account.signOutConfirmMessage'), {
			title: t('cloudSync.account.signOutConfirmTitle'),
			confirmLabel: t('cloudSync.account.signOut'),
			kind: 'warning',
		})
		if (!confirmed) return
		void cloudSyncStore.signOut()
		onSignedOut?.()
	}
</script>

{#if $isSignedIn}
	<div class="flex flex-col gap-4">
		<!-- Identity row -->
		<div class="flex items-center gap-3">
			<span class="relative flex-shrink-0">
				<span
					class="flex h-12 w-12 items-center justify-center overflow-hidden rounded-full bg-brand-muted text-brand-primary"
				>
					{#if showPhoto}
						<img
							src={$syncStatus.photo_url}
							alt=""
							class="h-full w-full object-cover"
							referrerpolicy="no-referrer"
							onerror={() => (photoError = true)}
						/>
					{:else}
						<span class="text-base font-semibold">{initials || '?'}</span>
					{/if}
				</span>
				<span class="absolute -right-0.5 -bottom-0.5 h-3.5 w-3.5 rounded-full border-2 border-surface-1 {dotClass}"
				></span>
			</span>
			<div class="min-w-0 flex-1">
				{#if $syncStatus.display_name}
					<p class="truncate text-sm font-medium text-text-primary">{$syncStatus.display_name}</p>
				{/if}
				{#if $syncStatus.email}
					<p class="truncate text-xs text-text-tertiary">{$syncStatus.email}</p>
				{/if}
			</div>
		</div>

		<!-- Live status -->
		<div class="flex items-center gap-2.5 rounded-lg bg-surface-2 px-3 py-2.5">
			<span class="h-2 w-2 flex-shrink-0 rounded-full {dotClass}"></span>
			<div class="min-w-0 flex-1">
				<p class="text-sm text-text-primary">{statusLabel}</p>
				{#if lastSynced}
					<p class="text-xs text-text-tertiary">{lastSynced}</p>
				{/if}
			</div>
		</div>

		<!-- Auto-sync hint -->
		<p class="text-xs text-text-tertiary">{$translate('cloudSync.account.autoSyncHint')}</p>

		<!-- Actions -->
		<div class="flex flex-col gap-2">
			<button
				type="button"
				class="w-full rounded-md bg-brand-primary px-3 py-2.5 text-sm font-medium text-white active:opacity-80 disabled:opacity-50"
				onclick={() => void cloudSyncStore.syncNow()}
				disabled={syncing}
			>
				{syncing ? $translate('cloudSync.account.syncing') : $translate('cloudSync.account.syncNow')}
			</button>
			<button
				type="button"
				class="w-full rounded-md bg-surface-2 px-3 py-2.5 text-sm font-medium text-text-secondary active:opacity-70"
				onclick={handleSignOut}
			>
				{$translate('cloudSync.account.signOut')}
			</button>
		</div>
	</div>
{:else}
	<div class="flex flex-col gap-3">
		<p class="text-sm text-text-secondary">{$translate('cloudSync.signIn.title')}</p>
		<!--
			Google-branded sign-in button per Google Identity guidelines: white surface, gray border,
			the official 4-color "G", and the required "Sign in with Google" wording. Brand colors are
			hard-coded (not theme tokens) on purpose — this is a third-party brand element.
		-->
		<button
			type="button"
			class="flex h-11 w-full items-center justify-center gap-3 rounded-md border border-[#747775] bg-white px-3 text-sm font-medium text-[#1f1f1f] active:opacity-80 disabled:opacity-60"
			style="font-family: 'Roboto', system-ui, sans-serif;"
			onclick={() => void signInMobile('google')}
			disabled={$signingIn}
		>
			{#if $signingIn}
				<Spinner class="h-5 w-5 text-[#1f1f1f]" />
			{:else}
				<svg class="h-5 w-5" viewBox="0 0 18 18" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
					<path
						fill="#EA4335"
						d="M9 3.48c1.69 0 2.83.73 3.48 1.34l2.54-2.48C13.46.89 11.43 0 9 0 5.48 0 2.44 2.02.96 4.96l2.91 2.26C4.6 5.05 6.62 3.48 9 3.48Z"
					/>
					<path
						fill="#4285F4"
						d="M17.64 9.2c0-.74-.06-1.28-.19-1.84H9v3.34h4.96c-.1.83-.64 2.08-1.84 2.92l2.84 2.2c1.7-1.57 2.68-3.88 2.68-6.62Z"
					/>
					<path
						fill="#FBBC05"
						d="M3.88 10.78A5.54 5.54 0 0 1 3.58 9c0-.62.11-1.22.29-1.78L.96 4.96A9 9 0 0 0 0 9c0 1.45.35 2.82.96 4.04l2.92-2.26Z"
					/>
					<path
						fill="#34A853"
						d="M9 18c2.43 0 4.47-.8 5.96-2.18l-2.84-2.2c-.76.53-1.78.9-3.12.9-2.38 0-4.4-1.57-5.12-3.74L.96 13.04C2.44 15.98 5.48 18 9 18Z"
					/>
				</svg>
			{/if}
			<span>{$translate('cloudSync.signIn.button', { values: { provider: 'Google' } })}</span>
		</button>
	</div>
{/if}
