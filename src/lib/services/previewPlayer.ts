/**
 * HTML5 Audio singleton wrapper for preview playback.
 * Not a Svelte store — orchestrated by playerStore.
 */

let audio: HTMLAudioElement | null = null

let _onEnded: (() => void) | null = null
let _onTimeUpdate: ((positionMs: number) => void) | null = null
let _onDurationChange: ((durationMs: number) => void) | null = null
let _onError: ((msg: string) => void) | null = null

function getAudio(): HTMLAudioElement {
	if (!audio) {
		audio = new Audio()
		audio.addEventListener('ended', () => _onEnded?.())
		audio.addEventListener('timeupdate', () => {
			_onTimeUpdate?.(Math.round(audio!.currentTime * 1000))
		})
		audio.addEventListener('durationchange', () => {
			if (audio!.duration && isFinite(audio!.duration)) {
				_onDurationChange?.(Math.round(audio!.duration * 1000))
			}
		})
		audio.addEventListener('error', () => {
			const msg = audio?.error?.message || 'Preview playback error'
			_onError?.(msg)
		})
	}
	return audio
}

export function play(url: string) {
	const el = getAudio()
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
		audio.pause()
		audio.src = ''
		audio.currentTime = 0
	}
}

export function seek(ms: number) {
	if (audio) {
		audio.currentTime = ms / 1000
	}
}

export function setVolume(vol: number) {
	if (audio) {
		audio.volume = Math.max(0, Math.min(1, vol))
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
