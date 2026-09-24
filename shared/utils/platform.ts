/**
 * Lightweight, synchronous platform detection shared by both apps.
 *
 * The primary desktop-vs-mobile split is the separate SvelteKit builds (`apps/desktop` never imports
 * mobile code and vice-versa), so this util is for fine-grained branches inside shared code and as a
 * convenience for the mobile shell. It relies only on `navigator` (a web global) so it stays free of
 * any desktop-only import and degrades gracefully to `'web'` outside a browser context.
 *
 * If pixel-accurate OS detection is ever needed (e.g. to gate a platform-specific Tauri call), prefer
 * upgrading to `@tauri-apps/plugin-os`.
 */

export type Platform = 'ios' | 'android' | 'macos' | 'windows' | 'linux' | 'web'

/**
 * Resolve the current platform from the user-agent. Recomputed per call (cheap) so it never caches a
 * stale `'web'` from a non-browser evaluation.
 */
export function getPlatform(): Platform {
	if (typeof navigator === 'undefined') return 'web'

	const ua = navigator.userAgent || ''

	// iPadOS 13+ reports a desktop Safari user-agent ("Macintosh"), so treat a touch-capable Mac as iOS.
	const isIPadOS = /Macintosh/.test(ua) && typeof navigator.maxTouchPoints === 'number' && navigator.maxTouchPoints > 1

	if (/iPad|iPhone|iPod/.test(ua) || isIPadOS) return 'ios'
	if (/Android/.test(ua)) return 'android'
	if (/Macintosh|Mac OS X/.test(ua)) return 'macos'
	if (/Windows/.test(ua)) return 'windows'
	if (/Linux/.test(ua)) return 'linux'
	return 'web'
}

/** True on iOS (iPhone/iPad/iPod). */
export function isIOS(): boolean {
	return getPlatform() === 'ios'
}

/** True on Android. */
export function isAndroid(): boolean {
	return getPlatform() === 'android'
}

/** True on any mobile platform (iOS or Android). */
export function isMobile(): boolean {
	const p = getPlatform()
	return p === 'ios' || p === 'android'
}

/** True on a desktop OS (macOS, Windows, or Linux). */
export function isDesktop(): boolean {
	const p = getPlatform()
	return p === 'macos' || p === 'windows' || p === 'linux'
}
