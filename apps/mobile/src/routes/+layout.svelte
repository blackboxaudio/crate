<script lang="ts">
	import '../style.css'
	import { onMount } from 'svelte'
	import { initializeI18n } from '$shared/i18n'
	import { settingsStore } from '$shared/stores/settings'
	import { startWebMediaSession } from '$shared/services/webMediaSession'
	import { playerStore } from '$shared/stores/player'
	import { isIOS } from '$shared/utils/platform'

	let { children } = $props()
	let i18nReady = $state(false)

	// Drive the OS lock screen / Control Center. On iOS, discovery preview plays through the native
	// AVPlayer engine, which OWNS the Now Playing surface (real prev/next/scrubber that work while
	// locked) AND auto-advance — so we start its event bridge and must NOT also start the Web Media
	// Session (running both would double-drive Now Playing). Android/web use the Web Media Session API
	// (WKWebView owns the surface for the HTML5 <audio> element) plus a JS auto-advance handler.
	onMount(() => {
		if (isIOS()) {
			let cleanup: (() => void) | undefined
			void playerStore.startNativeBridge().then((c) => (cleanup = c))
			return () => cleanup?.()
		}
		playerStore.onTrackEnd(() => void playerStore.nextTrack())
		return startWebMediaSession()
	})

	// Never allow zooming the UI itself. The viewport meta (user-scalable=no) covers most cases, but
	// iOS WKWebView still fires non-standard `gesture*` events for pinch — preventing them blocks the
	// zoom. These are pinch-specific and don't interfere with the pointer-based swipe gestures.
	onMount(() => {
		const events = ['gesturestart', 'gesturechange', 'gestureend']
		const prevent = (e: Event) => e.preventDefault()
		events.forEach((name) => document.addEventListener(name, prevent))
		return () => events.forEach((name) => document.removeEventListener(name, prevent))
	})

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
