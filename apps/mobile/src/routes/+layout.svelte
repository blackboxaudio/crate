<script lang="ts">
	import '../style.css'
	import { onMount } from 'svelte'
	import { initializeI18n } from '$shared/i18n'
	import { settingsStore } from '$shared/stores/settings'

	let { children } = $props()
	let i18nReady = $state(false)

	// Mirror the desktop layout: svelte-i18n loads the active locale's dictionary asynchronously, so
	// gate rendering until it's ready — otherwise the first `$translate()` throws "Cannot format a
	// message without first setting the initial locale" and the page bails to a blank screen.
	onMount(async () => {
		await initializeI18n()
		i18nReady = true

		// Reconcile the store with persisted settings (theme/accent/font/language). The inline script
		// in app.html already applied the correct theme pre-paint; this keeps the Svelte store in sync
		// so settings controls reflect the real values. load() is mobile-safe (the desktop-only
		// audio-devices call is guarded), so it resolves rather than rejecting on mobile.
		await settingsStore.load()
	})
</script>

{#if i18nReady}
	{@render children()}
{:else}
	<main class="p-8 text-text-secondary">Loading…</main>
{/if}
