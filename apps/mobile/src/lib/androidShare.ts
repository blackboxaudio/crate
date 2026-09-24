import { get } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'
import { translate } from '$shared/i18n'
import { toastStore } from '$shared/stores/toast'
import { extractFirstUrl } from '$shared/utils/discoveryLinks'
import { mobileUIStore } from '$lib/stores/mobileUI'
import { pendingReleasesStore } from '$lib/stores/pendingReleases'

// Android share-intent intake (#62): URLs shared from other apps (ACTION_SEND text/plain) land in
// MainActivity, which queues the raw text in the Kotlin ShareIntentQueue. This module drains that
// queue via the `take_shared_texts` command and routes each entry:
//   - no URL in the text  -> error toast
//   - offline             -> pendingReleasesStore queue (auto-created when connectivity returns)
//   - online              -> open the add-release sheet prefilled (auto-fetch kicks in), per the
//                            "process immediately" decision — Android runs in the main process, so
//                            unlike an iOS share extension there's no memory ceiling to respect.
//
// Drains happen on init (cold-start share: the intent arrives in onCreate long before the webview
// exists), on the `crate-android-share` DOM event (warm share: MainActivity nudges the running
// webview — also covers split-screen shares where no visibilitychange fires), and on
// foregrounding. Platform-specific by design, so it lives in apps/mobile (the `signInMobile.ts`
// precedent), keeping shared/ platform-agnostic.

/**
 * Start draining shared URLs. Returns a cleanup function. Call only on Android, after i18n is
 * ready (toasts translate).
 */
export function initAndroidShareIntake(): () => void {
	let draining = false

	const drain = async () => {
		if (draining) return
		draining = true
		try {
			const texts = await invoke<string[]>('take_shared_texts')
			if (texts.length === 0) return

			const urls = texts.map(extractFirstUrl).filter((url): url is string => url !== null)
			if (urls.length === 0) {
				toastStore.error(get(translate)('discovery.shareNoUrl'))
				return
			}

			if (!navigator.onLine) {
				for (const url of urls) pendingReleasesStore.enqueue(url)
				toastStore.info(get(translate)('discovery.offlineQueueNotice'))
				return
			}

			// A backlog (several shares queued while the app was dead) can only prefill one sheet —
			// queue the rest; processQueue auto-creates them since we're online.
			const newest = urls[urls.length - 1]
			for (const url of urls.slice(0, -1)) pendingReleasesStore.enqueue(url)
			if (urls.length > 1) void pendingReleasesStore.processQueue()
			mobileUIStore.openAddReleaseWithUrl(newest)
		} catch (error) {
			console.error('share intake drain failed:', error)
		} finally {
			draining = false
		}
	}

	const onShareEvent = () => void drain()
	const onVisibility = () => {
		if (document.visibilityState === 'visible') void drain()
	}

	window.addEventListener('crate-android-share', onShareEvent)
	document.addEventListener('visibilitychange', onVisibility)
	void drain()

	return () => {
		window.removeEventListener('crate-android-share', onShareEvent)
		document.removeEventListener('visibilitychange', onVisibility)
	}
}
