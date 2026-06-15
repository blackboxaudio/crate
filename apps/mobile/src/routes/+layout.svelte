<script lang="ts">
	import { onMount } from 'svelte'
	import { initializeI18n } from '$shared/i18n'

	let { children } = $props()
	let i18nReady = $state(false)

	// Mirror the desktop layout: svelte-i18n loads the active locale's dictionary asynchronously, so
	// gate rendering until it's ready — otherwise the first `$translate()` throws "Cannot format a
	// message without first setting the initial locale" and the page bails to a blank screen.
	onMount(async () => {
		await initializeI18n()
		i18nReady = true
	})
</script>

{#if i18nReady}
	{@render children()}
{:else}
	<main style="font-family: sans-serif; padding: 2rem;">Loading…</main>
{/if}
