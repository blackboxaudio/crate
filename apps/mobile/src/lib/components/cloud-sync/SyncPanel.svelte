<script lang="ts">
	import { translate } from '$shared/i18n'
	import {
		syncStatus,
		syncPhase,
		isSignedIn,
		signingIn,
		cloudSyncError,
		cloudSyncStore,
		syncErrorMessageKey,
		deletingAccount,
	} from '$shared/stores/cloudSync'
	import { getSyncDiagnostics } from '$shared/api/cloudSync'
	import { writeText } from '@tauri-apps/plugin-clipboard-manager'
	import { signInMobile } from '$lib/signInMobile'
	import { formatRelativeDate } from '$shared/utils/format'
	import { isIOS } from '$shared/utils/platform'
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
				return $translate(syncErrorMessageKey($syncStatus.last_error_kind))
			default:
				return $translate('cloudSync.status.signedOut')
		}
	})

	let diagnosticsCopied = $state(false)

	async function copyDiagnostics() {
		try {
			await writeText(await getSyncDiagnostics())
			diagnosticsCopied = true
			setTimeout(() => (diagnosticsCopied = false), 2000)
		} catch {
			// Best-effort — the log is also on disk.
		}
	}

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

	async function handleDeleteAccount() {
		const t = get(translate)
		const confirmed = await confirmDialog(t('cloudSync.danger.deleteAccountConfirmMessage'), {
			title: t('cloudSync.danger.deleteAccountConfirmTitle'),
			confirmLabel: t('cloudSync.danger.deleteAccount'),
			kind: 'error',
		})
		if (!confirmed) return
		void cloudSyncStore.deleteAccount()
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
							decoding="async"
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
				{#if $syncPhase === 'error' && $syncStatus.last_error}
					<!-- Sanitized at the error-construction layer (no URLs / API keys). -->
					<p class="mt-0.5 text-xs break-words text-danger">{$syncStatus.last_error}</p>
				{/if}
				{#if lastSynced}
					<p class="text-xs text-text-tertiary">{lastSynced}</p>
				{/if}
			</div>
			{#if $syncPhase === 'error'}
				<button
					type="button"
					class="flex-shrink-0 rounded-md bg-surface-1 px-2 py-1 text-xs text-text-secondary active:opacity-70"
					onclick={copyDiagnostics}
				>
					{diagnosticsCopied ? $translate('settings.diagnostics.copied') : $translate('cloudSync.diagnostics.copy')}
				</button>
			{/if}
		</div>

		<!-- Auto-sync hint -->
		<p class="text-xs text-text-tertiary">{$translate('cloudSync.account.autoSyncHint')}</p>

		<!-- Actions -->
		<div class="flex flex-col gap-2">
			<button
				type="button"
				class="w-full rounded-md bg-brand-primary px-3 py-2.5 text-sm font-medium text-white active:opacity-80 disabled:opacity-50"
				onclick={() => void cloudSyncStore.syncNow()}
				disabled={syncing || $deletingAccount}
			>
				{syncing ? $translate('cloudSync.account.syncing') : $translate('cloudSync.account.syncNow')}
			</button>
			<button
				type="button"
				class="w-full rounded-md bg-surface-2 px-3 py-2.5 text-sm font-medium text-text-secondary active:opacity-70 disabled:opacity-50"
				onclick={handleSignOut}
				disabled={$deletingAccount}
			>
				{$translate('cloudSync.account.signOut')}
			</button>
			<button
				type="button"
				class="flex w-full items-center justify-center gap-2 rounded-md px-3 py-2.5 text-sm font-medium text-danger active:opacity-70 disabled:opacity-50"
				onclick={handleDeleteAccount}
				disabled={$deletingAccount}
			>
				{#if $deletingAccount}
					<Spinner class="h-4 w-4 text-danger" />
					{$translate('cloudSync.danger.deletingAccount')}
				{:else}
					{$translate('cloudSync.danger.deleteAccount')}
				{/if}
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
		{#if isIOS()}
			<!--
				Sign in with Apple per Apple's HIG (App Store Guideline 4.8): black fill, the Apple logo, and
				the "Sign in with Apple" wording — as prominent as the Google button above (same h-11 w-full).
				Recreated in HTML (the native ASAuthorizationAppleIDButton isn't available inside the WebView).
				iOS-only; the native flow runs through cloudSyncStore.signInApple().
			-->
			<button
				type="button"
				class="flex h-11 w-full items-center justify-center gap-2 rounded-md bg-black px-3 text-sm font-medium text-white active:opacity-80 disabled:opacity-60"
				onclick={() => void cloudSyncStore.signInApple()}
				disabled={$signingIn}
			>
				{#if $signingIn}
					<Spinner class="h-5 w-5 text-white" />
				{:else}
					<svg class="h-4 w-4" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
						<path
							d="M11.182.008C11.148-.03 9.923.023 8.857 1.18c-1.066 1.156-.902 2.482-.878 2.516.024.034 1.52.087 2.475-1.258.955-1.345.762-2.391.728-2.43zm3.314 11.733c-.048-.096-2.325-1.234-2.113-3.422.212-2.189 1.675-2.789 1.698-2.854.023-.065-.597-.79-1.254-1.157a3.692 3.692 0 0 0-1.563-.434c-.108-.003-.483-.095-1.254.116-.508.139-1.653.589-1.968.607-.316.018-1.256-.522-2.267-.665-.647-.125-1.333.131-1.824.328-.49.196-1.422.754-2.074 2.237-.652 1.482-.311 3.83-.067 4.56.244.729.625 1.924 1.273 2.796.576.984 1.34 1.667 1.659 1.899.319.232 1.219.386 1.843.067.502-.308 1.408-.485 1.766-.472.357.013 1.061.154 1.782.539.571.197 1.111.115 1.652-.105.541-.221 1.324-1.059 2.238-2.758.347-.79.505-1.217.473-1.282z"
						/>
					</svg>
				{/if}
				<span>{$translate('cloudSync.signIn.button', { values: { provider: 'Apple' } })}</span>
			</button>
		{/if}
		{#if $cloudSyncError}
			<p class="text-xs text-danger">{$cloudSyncError}</p>
		{/if}
	</div>
{/if}
