/**
 * HTML5 Audio singleton wrapper for preview playback.
 * Not a Svelte store — orchestrated by playerStore.
 */

let audio: HTMLAudioElement | null = null

// True between a programmatic seek() and the element settling at the new position. WebKit-based
// webviews (Tauri's WKWebView on macOS/iOS, WebKitGTK on Linux) keep reporting the PRE-seek
// currentTime in `timeupdate` events until 'seeked' fires — so without this guard the playhead
// briefly snaps back to the old position before landing on the seeked target. While it's set we drop
// timeupdate emissions and let playerStore's optimistic position stand. Cleared by 'seeked', with a
// timeout fallback for the rare case that event never fires.
let seeking = false
let seekSettleTimeout: ReturnType<typeof setTimeout> | null = null

function clearSeekGuard() {
	seeking = false
	if (seekSettleTimeout) {
		clearTimeout(seekSettleTimeout)
		seekSettleTimeout = null
	}
}

let _onEnded: (() => void) | null = null
let _onTimeUpdate: ((positionMs: number) => void) | null = null
let _onDurationChange: ((durationMs: number) => void) | null = null
let _onError: ((msg: string) => void) | null = null
let _onWaiting: (() => void) | null = null
let _onPlaying: (() => void) | null = null

function getAudio(): HTMLAudioElement {
	if (!audio) {
		audio = new Audio()
		audio.preservesPitch = false
		audio.addEventListener('ended', () => _onEnded?.())
		audio.addEventListener('timeupdate', () => {
			// Drop stale ticks while a programmatic seek settles (see `seeking`).
			if (seeking) return
			_onTimeUpdate?.(Math.round(audio!.currentTime * 1000))
		})
		audio.addEventListener('durationchange', () => {
			if (audio!.duration && isFinite(audio!.duration)) {
				_onDurationChange?.(Math.round(audio!.duration * 1000))
			}
		})
		audio.addEventListener('seeked', clearSeekGuard)
		audio.addEventListener('error', () => {
			const msg = audio?.error?.message || 'Preview playback error'
			_onError?.(msg)
		})
		audio.addEventListener('waiting', () => _onWaiting?.())
		audio.addEventListener('playing', () => _onPlaying?.())
	}
	return audio
}

export function play(url: string) {
	const el = getAudio()
	clearSeekGuard()
	el.src = url
	el.play().catch((e) => {
		_onError?.(e.message || 'Failed to play preview')
	})
}

export function pause() {
	audio?.pause()
}

export function resume() {
	audio?.play().catch((e) => {
		_onError?.(e.message || 'Failed to resume preview')
	})
}

export function stop() {
	if (audio) {
		clearSeekGuard()
		audio.pause()
		audio.src = ''
		audio.currentTime = 0
	}
}

export function seek(ms: number) {
	if (audio) {
		// Guard against WebKit's stale post-seek timeupdate (see `seeking`). Normally cleared by the
		// 'seeked' event; the timeout is a fallback in case it never fires (e.g. seeking to the current
		// position) so position tracking can't get stuck suppressed.
		seeking = true
		if (seekSettleTimeout) clearTimeout(seekSettleTimeout)
		seekSettleTimeout = setTimeout(clearSeekGuard, 1000)
		audio.currentTime = ms / 1000
	}
}

export function setVolume(vol: number) {
	if (audio) {
		audio.volume = Math.max(0, Math.min(1, vol))
	}
}

export function setPlaybackRate(rate: number) {
	if (audio) {
		audio.playbackRate = Math.max(0.9, Math.min(1.1, rate))
	}
}

export function setOnEnded(cb: (() => void) | null) {
	_onEnded = cb
}

export function setOnTimeUpdate(cb: ((positionMs: number) => void) | null) {
	_onTimeUpdate = cb
}

export function setOnDurationChange(cb: ((durationMs: number) => void) | null) {
	_onDurationChange = cb
}

export function setOnError(cb: ((msg: string) => void) | null) {
	_onError = cb
}

export function setOnWaiting(cb: (() => void) | null) {
	_onWaiting = cb
}

export function setOnPlaying(cb: (() => void) | null) {
	_onPlaying = cb
}
