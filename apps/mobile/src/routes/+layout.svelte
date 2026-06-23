<script lang="ts">
	import '../style.css'
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { listen, type UnlistenFn } from '@tauri-apps/api/event'
	import { initializeI18n } from '$shared/i18n'
	import { settingsStore } from '$shared/stores/settings'
	import { cloudSyncStore } from '$shared/stores/cloudSync'
	import { startWebMediaSession } from '$shared/services/webMediaSession'
	import { playerStore, previewInfo } from '$shared/stores/player'
	import { isIOS } from '$shared/utils/platform'
	import { mobileUIStore, isPlayerExpanded } from '$lib/stores/mobileUI'
	// @ts-expect-error — PUBLIC_APP_VERSION is set dynamically by vite.config.ts
	import { PUBLIC_APP_VERSION } from '$env/static/public'
	import { splashVisible, dismissSplash } from '$lib/stores/splash'
	import SplashScreen from '$lib/components/common/SplashScreen.svelte'

	let { children } = $props()
	let i18nReady = $state(false)

	const splashVersion = PUBLIC_APP_VERSION

	// Drive the OS lock screen / Control Center, and auto-advance at track end. On iOS, discovery preview
	// plays through the native AVPlayer engine, which OWNS the Now Playing surface (real prev/next/scrubber
	// that work while locked) and advances WITHIN a release itself — so we start its event bridge and must
	// NOT also start the Web Media Session (running both would double-drive Now Playing). Android/web use
	// the Web Media Session API (WKWebView owns the surface for the HTML5 <audio> element). Either way we
	// register the same track-end handler: it fires after every track on Android/web, and on iOS only when
	// the engine's loaded playlist ends (a shuffle track, or a release's last track) — both cases where the
	// queue/shuffle logic must decide what plays next across the whole list.
	onMount(() => {
		playerStore.onTrackEnd(() => void playerStore.nextTrack())

		if (isIOS()) {
			let cleanup: (() => void) | undefined
			void playerStore.startNativeBridge().then((c) => (cleanup = c))
			return () => {
				cleanup?.()
				playerStore.onTrackEnd(null)
			}
		}
		const stopWebMediaSession = startWebMediaSession()
		return () => {
			stopWebMediaSession()
			playerStore.onTrackEnd(null)
		}
	})

	// Auto-open the full-screen player when the user returns to Crate from the lock screen while a
	// preview is loaded — tapping the Now Playing controls to get back to the track is a clear "take me
	// to it" intent, so we surface the player instead of leaving them on whatever tab they left. The
	// native engine fires `native-preview-entered-from-lock` only on a real device-unlock→active
	// transition (it can tell that apart from a plain app-switch, which the WebView can't), so we don't
	// have to second-guess intent here. We skip it when the player is already open — "remember where I
	// left it," a state the in-memory store already preserves across backgrounding. iOS-only: the event
	// has no Android counterpart, where the player just stays where the user left it.
	onMount(() => {
		if (!isIOS()) return
		let unlisten: UnlistenFn | undefined
		void listen('native-preview-entered-from-lock', () => {
			if (get(previewInfo) != null && !get(isPlayerExpanded)) mobileUIStore.expandPlayer()
		}).then((u) => (unlisten = u))
		return () => unlisten?.()
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

	// Boot the cloud-sync store so the header's account/sync chip and the Settings panel reflect the real
	// signed-in status on launch — not just after a fresh sign-in (mirrors desktop's useAppSetup). load()
	// fetches the persisted status, polling keeps it live, and the override listener toasts when another
	// device supersedes a local edit. On a build without cloud config the status stays `disabled` and the
	// chip stays hidden.
	onMount(() => {
		void cloudSyncStore.load()
		cloudSyncStore.startPolling()
		void cloudSyncStore.startOverrideListener()
		return () => {
			cloudSyncStore.stopPolling()
			cloudSyncStore.stopOverrideListener()
		}
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
