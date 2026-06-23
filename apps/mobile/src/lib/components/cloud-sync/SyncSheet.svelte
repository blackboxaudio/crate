<script lang="ts">
	import { translate } from '$shared/i18n'
	import { syncStatus, syncPhase, isSignedIn, signingIn, cloudSyncStore } from '$shared/stores/cloudSync'
	import { signInMobile } from '$lib/signInMobile'
	import { formatRelativeDate } from '$shared/utils/format'
	import MobileModal from '$lib/components/common/MobileModal.svelte'

	// Account + cloud-sync bottom sheet, opened from the header's SyncStatusButton. It surfaces the
	// signed-in identity, the live sync phase, when it last synced, and the two actions that belong on a
	// phone — sync-now and sign-out. When signed out it falls back to the same Google sign-in button the
	// Settings panel uses, so the chip is a complete account entry point in its own right.
	type Props = { open: boolean; onClose: () => void }
	let { open, onClose }: Props = $props()

	// Phase → human label (mirrors desktop's SyncStatusIndicator copy; reuses the existing cloudSync.status.*
	// strings so there are no new keys to translate).
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

	// Phase → status-dot color, onto the mobile theme tokens (syncing pulses in brand, offline = warning,
	// error = danger, idle = success).
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

	// "Last synced 5 minutes ago" — `formatRelativeDate` now resolves sub-day spans, so this stays honest as
	// the status polls. Passing the `$translate` function keeps it reactive to a live locale change.
	const lastSynced = $derived(
		$syncStatus.last_synced_at
			? $translate('cloudSync.account.lastSynced', {
					values: { time: formatRelativeDate($syncStatus.last_synced_at, $translate) },
				})
			: null
	)

	// Avatar fallback: up to two initials from the display name or email (mirrors desktop's Avatar).
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
		// Reset the error (retry the image) only when the URL itself changes — not on every status poll,
		// which would otherwise re-attempt a genuinely broken avatar every few seconds.
		const url = $syncStatus.photo_url
		if (url !== lastPhotoUrl) {
			lastPhotoUrl = url
			photoError = false
		}
	})
	const showPhoto = $derived(!!$syncStatus.photo_url && !photoError)

	const syncing = $derived($syncPhase === 'syncing')

	function handleSignOut() {
		void cloudSyncStore.signOut()
		onClose()
	}
</script>

<MobileModal {open} {onClose} title={$translate('cloudSync.account.title')}>
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
			<button
				type="button"
				class="w-full rounded-md bg-brand-primary px-3 py-2.5 text-sm font-medium text-white active:opacity-80 disabled:opacity-50"
				onclick={() => void signInMobile('google')}
				disabled={$signingIn}
			>
				{$signingIn
					? $translate('common.loading')
					: $translate('cloudSync.signIn.button', { values: { provider: 'Google' } })}
			</button>
		</div>
	{/if}
</MobileModal>
