import { get } from 'svelte/store'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { PreviewInfo } from '../types'
import {
	playerStore,
	previewInfo,
	isPlaying,
	playbackPosition,
	playbackDuration,
	volume,
	isMuted,
} from '../stores/player'
import { updateNowPlaying, updatePlaybackState, clearNowPlaying } from '../api/mediaControls'
import * as previewPlayer from './previewPlayer'

// Drives the NATIVE Android media session from the shared player store. Android System WebView —
// unlike Chrome — never surfaces the W3C Media Session API to the OS, so `webMediaSession.ts` is
// inert there. Instead, metadata/playback flow through the `update_now_playing` /
// `update_playback_state` IPC into `CrateMediaService.kt` (MediaSessionCompat + MediaStyle
// notification + foreground service + audio focus — see services/media_controls/android.rs), and
// remote commands / focus changes come back as `media-*` Tauri events handled here against the
// HTML5 <audio> player.
//
// Used by the mobile app on Android only. iOS uses the native AVPlayer engine's bridge; other
// webviews use webMediaSession.ts.

type Cleanup = () => void

/** Volume multiplier while another app holds transient focus with ducking. */
const DUCK_FACTOR = 0.2

/** Position discontinuity (ms) beyond which we re-push state so the notification seekbar tracks. */
const SEEK_JUMP_THRESHOLD_MS = 1500

function pushMetadata(info: PreviewInfo | null, durationMs: number) {
	if (!info) {
		void clearNowPlaying()
		return
	}
	const track = info.release.tracks[info.trackIndex]
	void updateNowPlaying(
		track?.name ?? info.release.title ?? '',
		info.release.artist ?? '',
		info.release.title ?? '',
		info.release.artwork_url ?? null,
		durationMs > 0 ? durationMs : (track?.duration_ms ?? null)
	)
}

/**
 * Start syncing the player store to the native Android media session. Returns a cleanup function
 * that unsubscribes and clears the OS Now Playing surface.
 */
export function startAndroidMediaSession(): Cleanup {
	let lastReleaseId: string | null = null
	let lastTrackIndex = -1
	let lastDurationMs = 0

	// The Android notification seekbar extrapolates from (position, speed=1, timestamp), so only
	// discontinuities (seeks, track changes) need a re-push — tracked against wall-clock drift.
	let lastPushedPositionMs = 0
	let lastPushedAt = Date.now()
	let resumeOnFocusGain = false

	const pushPlayback = (playing: boolean, positionMs: number) => {
		lastPushedPositionMs = positionMs
		lastPushedAt = Date.now()
		void updatePlaybackState(playing, Math.max(0, Math.round(positionMs)))
	}

	const unsubPreview = previewInfo.subscribe((info) => {
		const releaseId = info?.releaseId ?? null
		const trackIndex = info?.trackIndex ?? -1
		if (releaseId === lastReleaseId && trackIndex === lastTrackIndex) return
		lastReleaseId = releaseId
		lastTrackIndex = trackIndex
		pushMetadata(info, get(playbackDuration))
		// A track change resets the timeline.
		if (info) pushPlayback(get(isPlaying), 0)
	})

	// Duration typically resolves after the stream loads — re-push metadata so the seekbar gets it.
	const unsubDuration = playbackDuration.subscribe((durationMs) => {
		if (durationMs === lastDurationMs) return
		lastDurationMs = durationMs
		const info = get(previewInfo)
		if (info && durationMs > 0) pushMetadata(info, durationMs)
	})

	const unsubPlaying = isPlaying.subscribe((playing) => {
		pushPlayback(playing, get(playbackPosition))
	})

	const unsubPosition = playbackPosition.subscribe((positionMs) => {
		if (!get(isPlaying)) return
		const expected = lastPushedPositionMs + (Date.now() - lastPushedAt)
		if (Math.abs(positionMs - expected) > SEEK_JUMP_THRESHOLD_MS) pushPlayback(true, positionMs)
	})

	const unlisteners: UnlistenFn[] = []
	const on = (event: string, handler: (payload: unknown) => void) => {
		void listen(event, (e) => handler(e.payload)).then((u) => unlisteners.push(u))
	}

	on('media-toggle', () => void playerStore.togglePlayPause())
	on('media-play', () => void playerStore.resume())
	on('media-pause', () => void playerStore.pause())
	on('media-next', () => void playerStore.nextTrack())
	on('media-previous', () => void playerStore.previousTrack())
	on('media-seek', (payload) => {
		const positionMs = (payload as { positionMs?: number })?.positionMs
		if (typeof positionMs === 'number') void playerStore.seek(positionMs)
	})

	// Audio focus: permanent loss pauses outright; transient loss (phone call, navigation prompt)
	// pauses and resumes when focus returns; ducking lowers the element volume directly —
	// deliberately NOT via playerStore.setVolume, which would persist the ducked level.
	on('media-focus-loss', () => {
		resumeOnFocusGain = false
		void playerStore.pause()
	})
	on('media-focus-loss-transient', () => {
		resumeOnFocusGain = get(isPlaying)
		void playerStore.pause()
	})
	on('media-focus-gain', () => {
		if (resumeOnFocusGain) {
			resumeOnFocusGain = false
			void playerStore.resume()
		}
	})
	on('media-duck', (payload) => {
		const active = (payload as { active?: boolean })?.active === true
		const baseVolume = get(isMuted) ? 0 : get(volume)
		previewPlayer.setVolume(active ? baseVolume * DUCK_FACTOR : baseVolume)
	})

	return () => {
		unsubPreview()
		unsubDuration()
		unsubPlaying()
		unsubPosition()
		for (const unlisten of unlisteners) unlisten()
		void clearNowPlaying()
	}
}
