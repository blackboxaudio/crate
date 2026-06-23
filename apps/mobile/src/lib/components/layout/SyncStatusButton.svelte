<script lang="ts">
	import { translate } from '$shared/i18n'
	import { syncStatus, syncPhase, isSyncAvailable, isSignedIn } from '$shared/stores/cloudSync'
	import SyncSheet from '$lib/components/cloud-sync/SyncSheet.svelte'

	// Trailing header affordance: an avatar/cloud chip that reflects the live cloud-sync phase and opens the
	// account + sync sheet. Hidden entirely when sync isn't configured (`disabled`) — mirrors desktop's
	// SyncStatusIndicator gate — so a build without cloud config keeps a clean brand-only bar. This is the
	// one piece of always-true, app-wide status the bar earns its keep with (the rest of nav is the TabBar).
	let sheetOpen = $state(false)

	// Phase → status-dot color on the mobile theme tokens: syncing pulses in brand, offline = warning,
	// error = danger, idle = success. Signed-out shows no dot — the chip is a plain cloud inviting sign-in.
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
				return ''
		}
	})

	// Phase → accessible label so the control announces sync state without opening the sheet (mirrors the
	// desktop indicator's tooltip). Reuses the existing cloudSync.status.* strings.
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
</script>

{#if $isSyncAvailable}
	<!-- 44pt touch target wrapping a 32px avatar/cloud glyph. -->
	<button
		type="button"
		class="-mr-2 flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-full active:opacity-70"
		aria-label={statusLabel}
		onclick={() => (sheetOpen = true)}
	>
		{#if $isSignedIn}
			<span class="relative">
				<span
					class="flex h-8 w-8 items-center justify-center overflow-hidden rounded-full bg-brand-muted text-brand-primary"
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
						<span class="text-xs font-semibold">{initials || '?'}</span>
					{/if}
				</span>
				{#if dotClass}
					<span class="absolute -right-0.5 -bottom-0.5 h-3 w-3 rounded-full border-2 border-surface-1 {dotClass}"
					></span>
				{/if}
			</span>
		{:else}
			<!-- Signed-out: a neutral cloud glyph inviting sign-in (the sheet hosts the sign-in button). -->
			<svg
				class="h-6 w-6 text-text-secondary"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<path d="M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z" />
			</svg>
		{/if}
	</button>

	<SyncSheet open={sheetOpen} onClose={() => (sheetOpen = false)} />
{/if}
