<script lang="ts">
	import { translate } from '$shared/i18n'
	import { syncStatus, syncPhase, isSyncAvailable, isSignedIn } from '$shared/stores/cloudSync'
	import { mobileUIStore } from '$lib/stores/mobileUI'

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
</script>

{#if $isSyncAvailable}
	<button
		type="button"
		class="-mr-2 flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-full active:opacity-70"
		aria-label={statusLabel}
		onclick={() => mobileUIStore.navigateToSettings('sync')}
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
{/if}
