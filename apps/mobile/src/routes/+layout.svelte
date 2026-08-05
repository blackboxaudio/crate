<script lang="ts">
	import '../style.css'
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { invoke } from '@tauri-apps/api/core'
	import { listen, type UnlistenFn } from '@tauri-apps/api/event'
	import { initializeI18n, type Language } from '$shared/i18n'
	import { settingsStore } from '$shared/stores/settings'
	import { cloudSyncStore } from '$shared/stores/cloudSync'
	import { startWebMediaSession } from '$shared/services/webMediaSession'
	import { startAndroidMediaSession } from '$shared/services/androidMediaSession'
	import { playerStore, previewInfo } from '$shared/stores/player'
	import { isAndroid, isIOS } from '$shared/utils/platform'
	import { mobileUIStore, isPlayerExpanded, flushNavPersistence } from '$lib/stores/mobileUI'
	import { collectionStore } from '$shared/stores/collection'
	import { offlineCacheStore } from '$lib/stores/offlineCache'
	import { pendingReleasesStore } from '$lib/stores/pendingReleases'
	import { initAndroidShareIntake } from '$lib/androidShare'
	import { initAndroidBack } from '$lib/androidBack'
	import { setupCloudSyncMergeListener } from '$lib/cloudSyncMerge'
	// @ts-expect-error — PUBLIC_APP_VERSION is set dynamically by vite.config.ts
	import { PUBLIC_APP_VERSION } from '$env/static/public'
	import { splashVisible, dismissSplash } from '$lib/stores/splash'
	import SplashScreen from '$lib/components/common/SplashScreen.svelte'
	import ToastContainer from '$lib/components/common/ToastContainer.svelte'

	let { children } = $props()
	let i18nReady = $state(false)

	const splashVersion = PUBLIC_APP_VERSION

	// Drive the OS lock screen / Control Center, and auto-advance at track end. On iOS, discovery preview
	// plays through the native AVPlayer engine, which OWNS the Now Playing surface (real prev/next/scrubber
	// that work while locked) and advances WITHIN a release itself — so we start its event bridge and must
	// NOT also start the Web Media Session (running both would double-drive Now Playing). On Android the
	// System WebView never surfaces the Web Media Session to the OS, so a native MediaSessionCompat +
	// foreground service owns the surface instead (androidMediaSession.ts ↔ CrateMediaService.kt, #62).
	// Other webviews use the Web Media Session API (WKWebView owns the surface for the HTML5 <audio>
	// element). Either way we register the same track-end handler: it fires after every track on
	// Android/web, and on iOS only when the engine's loaded playlist ends (a shuffle track, or a release's
	// last track) — both cases where the queue/shuffle logic must decide what plays next across the list.
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
		const stopMediaSession = isAndroid() ? startAndroidMediaSession() : startWebMediaSession()
		return () => {
			stopMediaSession()
			playerStore.onTrackEnd(null)
		}
	})

	// Repopulate the mini-player on launch with whatever preview was playing when the app last closed —
	// the track plus its saved progress, shuffle, and tempo (those non-preview values are restored from
	// storage when the store initializes). Mirrors desktop's useAppSetup; mobile is preview-only, so we
	// restore the preview, not a library track. restorePreview() only sets previewInfo — it never expands
	// the player — so the state lands in the mini-player, paused and ready to resume, by design. Needs only
	// Tauri IPC (getRelease), which is ready at mount, so it doesn't wait on i18n/settings; the mini-player
	// renders reactively once previewInfo resolves.
	onMount(() => {
		void playerStore.restorePreview()
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
	// fetches the persisted status. Unlike desktop, mobile does NOT run an always-on poll (that would drain
	// the battery and iOS/Android freeze the process when backgrounded anyway) — startForegroundSync() runs
	// one pull-then-push pass on launch and on every foreground (visibilitychange / focus) instead. The
	// override listener toasts when another device supersedes a local edit. On a build without cloud config
	// the status stays `disabled` and the chip stays hidden.
	onMount(() => {
		void cloudSyncStore.load()
		cloudSyncStore.startForegroundSync()
		void cloudSyncStore.startOverrideListener()
		return () => {
			cloudSyncStore.stopForegroundSync()
			cloudSyncStore.stopOverrideListener()
		}
	})

	// Live-refresh: when the backend merges remote changes pulled from another device, reload the
	// affected shared stores so the UI reflects them without a tab-switch. Mirrors desktop's
	// reloadStoresForBuckets, scoped to the stores mobile actually uses.
	onMount(() => {
		let unlisten: (() => void) | undefined
		void setupCloudSyncMergeListener().then((u) => (unlisten = u))
		return () => unlisten?.()
	})

	// Hydrate the offline add queue from localStorage; if we're online, drain any pending items now.
	// Network listeners auto-process the queue when connectivity returns.
	onMount(() => {
		pendingReleasesStore.hydrate()
		pendingReleasesStore.attachNetworkListeners()
		if (navigator.onLine) void pendingReleasesStore.processQueue()
	})

	// Offline-cache state (row badges / the Downloaded filter): seed on boot, then refetch whenever
	// the backend reports a cache change (play-through download, precache pin, purge, clear,
	// eviction). Debounced — a whole-release download fires one event per track.
	onMount(() => {
		void offlineCacheStore.refresh()
		let unlisten: UnlistenFn | undefined
		let timer: ReturnType<typeof setTimeout> | null = null
		void listen('discovery-cache-changed', () => {
			if (timer) clearTimeout(timer)
			timer = setTimeout(() => {
				timer = null
				void offlineCacheStore.refresh()
			}, 500)
		}).then((u) => (unlisten = u))
		return () => {
			if (timer) clearTimeout(timer)
			unlisten?.()
		}
	})

	// Purchased-collection state (owned badges / the Purchased view): seed on boot, then refetch
	// whenever the backend reports a change (link, unlink, background refresh finding new items).
	// Debounced — an initial multi-batch scrape fires an event per completed sync, not per batch,
	// but a refresh-all of several accounts still lands a burst.
	onMount(() => {
		void collectionStore.load()
		let unlisten: UnlistenFn | undefined
		let timer: ReturnType<typeof setTimeout> | null = null
		void listen('collection-changed', () => {
			if (timer) clearTimeout(timer)
			timer = setTimeout(() => {
				timer = null
				void collectionStore.refresh()
			}, 500)
		}).then((u) => (unlisten = u))
		return () => {
			if (timer) clearTimeout(timer)
			unlisten?.()
		}
	})

	// Android share-intent intake (#62): drain URLs shared from other apps (queued by MainActivity)
	// and open the add-release sheet prefilled. Gated on i18n so its toasts can translate — a
	// cold-start share sits safely in the Kotlin queue until this first drain.
	$effect(() => {
		if (!i18nReady || !isAndroid()) return
		return initAndroidShareIntake()
	})

	// Android hardware/gesture Back (#62): MainActivity's back callback asks `window.__CRATE_BACK__`
	// whether the frontend consumed the press (close topmost surface / pop folder / return to
	// Discovery); an unhandled press backgrounds the app.
	onMount(() => {
		if (!isAndroid()) return
		return initAndroidBack()
	})

	// Navigation persistence (mobileUI store): the discovery scroll position writes behind a debounce, so
	// flush it the moment the app is backgrounded — mobile apps die backgrounded, not mid-fling, so this is
	// what makes the very latest scroll position survive an iOS/Android process kill. The same signal also
	// tells the backend whether the app is foregrounded (set_app_foreground), so Rust-side loops like the
	// follow watch sweep don't burn background CPU while background audio keeps the process alive.
	onMount(() => {
		const onVisibility = () => {
			if (document.visibilityState === 'hidden') flushNavPersistence()
			void invoke('set_app_foreground', { foreground: document.visibilityState === 'visible' }).catch(() => {})
		}
		document.addEventListener('visibilitychange', onVisibility)
		return () => document.removeEventListener('visibilitychange', onVisibility)
	})

	// Mirror the desktop layout: svelte-i18n loads the active locale's dictionary asynchronously, so
	// gate rendering until it's ready — otherwise the first `$translate()` throws "Cannot format a
	// message without first setting the initial locale" and the page bails to a blank screen.
	onMount(async () => {
		// Hold the splash for at least a beat so a fast boot doesn't flash it (mirrors desktop's 1s floor).
		const splashStart = Date.now()

		// Boot with the cached language (the same key setLanguage writes) so a non-English UI paints
		// correctly from the first frame — mirrors desktop's +layout. Without this, a stored language
		// never applied on mobile.
		await initializeI18n(localStorage.getItem('crate-language') as Language | null)
		i18nReady = true

		// Reconcile the store with persisted settings (theme/accent/font/language). The inline script
		// in app.html already applied the correct theme pre-paint; this keeps the Svelte store in sync
		// so settings controls reflect the real values. load() is mobile-safe (the desktop-only
		// audio-devices call is guarded), so it resolves rather than rejecting on mobile. Language is
		// NOT skipped anymore: the DB value is authoritative (it may have synced from another device),
		// and the desktop-only menu rebuild inside setLanguage is caught on mobile.
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

<!-- Global toast host: mobile had none, so every `toastStore.error(...)` (preview failures, sync
     errors, …) was silently swallowed. Rendered outside the i18n gate so errors still show if i18n
     is mid-load. -->
<ToastContainer />
