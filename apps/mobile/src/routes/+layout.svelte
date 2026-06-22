<script lang="ts">
	import '../style.css'
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { initializeI18n } from '$shared/i18n'
	import { settingsStore } from '$shared/stores/settings'
	import { startWebMediaSession } from '$shared/services/webMediaSession'
	import { playerStore, previewInfo } from '$shared/stores/player'
	import { isIOS } from '$shared/utils/platform'
	import { mobileUIStore } from '$lib/stores/mobileUI'
	// @ts-expect-error — PUBLIC_APP_VERSION is set dynamically by vite.config.ts
	import { PUBLIC_APP_VERSION } from '$env/static/public'
	import { splashVisible, dismissSplash } from '$lib/stores/splash'
	import SplashScreen from '$lib/components/common/SplashScreen.svelte'

	let { children } = $props()
	let i18nReady = $state(false)

	const splashVersion = PUBLIC_APP_VERSION

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

	// Returning to the app while a preview is playing → surface the full-screen player. When a track is
	// playing in the background and the user comes back to Crate — by tapping the iOS lock-screen Now
	// Playing widget, the status-bar audio indicator, or just switching back from another app — we assume
	// they're here for the track, so open the full player over whatever tab they left rather than leaving
	// them to hunt for the mini bar. The WebView fires `visibilitychange` on every foreground (iOS
	// suspends its JS while backgrounded). We only act on a real background→foreground round-trip (gate on
	// having seen `hidden` first, so a cold launch with a restored-but-paused preview doesn't auto-expand)
	// and only when a preview is actually loaded.
	onMount(() => {
		let backgrounded = false
		function onVisibility() {
			if (document.visibilityState === 'hidden') {
				backgrounded = true
				return
			}
			if (!backgrounded) return
			backgrounded = false
			if (get(previewInfo) != null) mobileUIStore.expandPlayer()
		}
		document.addEventListener('visibilitychange', onVisibility)
		return () => document.removeEventListener('visibilitychange', onVisibility)
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
		// Hold the splash for at least a beat so a fast boot doesn't flash it (mirrors desktop's 1s floor).
		const splashStart = Date.now()

		await initializeI18n()
		i18nReady = true

		// Reconcile the store with persisted settings (theme/accent/font/language). The inline script
		// in app.html already applied the correct theme pre-paint; this keeps the Svelte store in sync
		// so settings controls reflect the real values. load() is mobile-safe (the desktop-only
		// audio-devices call is guarded), so it resolves rather than rejecting on mobile.
		await settingsStore.load()

		// Boot done — dismiss the splash (honoring the minimum on-screen time); it scale-fades out.
		const elapsed = Date.now() - splashStart
		const MIN_SPLASH_MS = 1000
		if (elapsed < MIN_SPLASH_MS) await new Promise((r) => setTimeout(r, MIN_SPLASH_MS - elapsed))
		dismissSplash()
	})
</script>

<SplashScreen show={$splashVisible} version={splashVersion} />

{#if i18nReady}
	{@render children()}
{/if}
