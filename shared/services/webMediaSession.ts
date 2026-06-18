import type { PreviewInfo } from '../types'
import { playerStore, previewInfo, isPlaying } from '../stores/player'

// Drives the W3C Media Session API (`navigator.mediaSession`) from the shared player store so the OS
// lock screen / Control Center shows discovery-preview metadata, artwork, playback state, and
// transport controls.
//
// Why the Web API and not native MPNowPlayingInfoCenter/MPRemoteCommandCenter (#79): preview audio
// plays through a WebView HTML5 `<audio>` element, and iOS WKWebView already owns the Now Playing
// surface for it. Setting it natively via objc2 would be overwritten; the Media Session API is the
// WebKit-blessed way for a web player to control that surface. (Background playback still relies on
// the native AVAudioSession `.playback` activation — see services/media_controls/ios.rs.)
//
// iOS button caveat: for an `<audio>` element iOS leans toward the skip ±10s buttons and the choice
// of prev/next vs skip is only partly controllable from web code. We register prev/next + clear the
// seek handlers (the documented lever), re-assert them once playback is active (WKWebView installs
// its own default skip commands when the element becomes the active session), and deliberately do
// NOT call setPositionState (providing a seekable timeline biases iOS toward skip). Guaranteed
// prev/next would require native playback (AVPlayer) + MPRemoteCommandCenter.
//
// Used by the mobile app. Desktop uses souvlaki (native, via the `update_now_playing` IPC), so it
// does not start this — running both would double-drive the OS Now Playing center.

type Cleanup = () => void

const SEEK_ACTIONS: MediaSessionAction[] = ['seekforward', 'seekbackward', 'seekto']
const ALL_ACTIONS: MediaSessionAction[] = ['play', 'pause', 'previoustrack', 'nexttrack', ...SEEK_ACTIONS]

function updateMetadata(ms: MediaSession, info: PreviewInfo | null) {
	if (!info) {
		ms.metadata = null
		return
	}
	const track = info.release.tracks[info.trackIndex]
	ms.metadata = new MediaMetadata({
		title: track?.name ?? info.release.title ?? '',
		artist: info.release.artist ?? '',
		album: info.release.title ?? '',
		artwork: info.release.artwork_url ? [{ src: info.release.artwork_url, sizes: '512x512' }] : [],
	})
}

/**
 * Register play/pause + prev/next handlers and clear the seek handlers (so iOS prefers prev/next
 * over the default skip ±10s buttons). Re-applied when playback (re)starts because WKWebView installs
 * its own default remote commands once the `<audio>` element becomes the active media session.
 */
function applyActionHandlers(ms: MediaSession) {
	const set = (action: MediaSessionAction, handler: MediaSessionActionHandler | null) => {
		try {
			ms.setActionHandler(action, handler)
		} catch {
			// A given action may be unsupported on this platform; ignore.
		}
	}
	set('play', () => void playerStore.resume())
	set('pause', () => void playerStore.pause())
	set('previoustrack', () => void playerStore.previousTrack())
	set('nexttrack', () => void playerStore.nextTrack())
	for (const action of SEEK_ACTIONS) set(action, null)
}

/**
 * Start syncing the player store to `navigator.mediaSession`. Returns a cleanup function that
 * unsubscribes and clears the OS Now Playing surface.
 */
export function startWebMediaSession(): Cleanup {
	if (typeof navigator === 'undefined' || !('mediaSession' in navigator)) {
		return () => {}
	}
	const ms = navigator.mediaSession

	applyActionHandlers(ms)

	const unsubPreview = previewInfo.subscribe((info) => updateMetadata(ms, info))
	const unsubPlaying = isPlaying.subscribe((playing) => {
		ms.playbackState = playing ? 'playing' : 'paused'
		// Re-assert our handlers once the <audio> element is the active session so WKWebView's
		// default skip ±10s commands don't take over the transport buttons.
		if (playing) applyActionHandlers(ms)
	})

	return () => {
		unsubPreview()
		unsubPlaying()
		ms.metadata = null
		ms.playbackState = 'none'
		for (const action of ALL_ACTIONS) {
			try {
				ms.setActionHandler(action, null)
			} catch {
				// ignore
			}
		}
	}
}
