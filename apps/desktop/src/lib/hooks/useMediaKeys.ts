import { listen, type UnlistenFn } from '@tauri-apps/api/event'

// =============================================================================
// Types
// =============================================================================

export interface MediaKeyHandlers {
	/** `media-toggle` only — a genuine toggle request from the OS. */
	onPlayPause: () => void
	/** `media-play` — resume. Must NOT toggle. */
	onPlay: () => void
	/** `media-pause` — pause. Must NOT toggle. */
	onPause: () => void
	onNextTrack: () => void
	onPreviousTrack: () => void
}

// =============================================================================
// Hook
// =============================================================================

/**
 * Listen for OS-level media key events emitted by the souvlaki-backed
 * MediaControlsService on the Rust side. Works even when Crate is in the
 * background because media keys are routed through the OS Now Playing
 * infrastructure (MPRemoteCommandCenter on macOS).
 *
 * Each transport event maps to its own verb — play resumes, pause pauses, only
 * toggle toggles. Collapsing them into one toggle is NOT safe: the OS sends a
 * real `pause` command to the Now Playing app when a Bluetooth route drops (and
 * many AVRCP devices emit a stray `play` on connect), so a toggle turns
 * "headphones disconnected" into "start playing". This matches the Android and
 * web media-session backends, which have always mapped them separately.
 *
 * @returns Promise of cleanup function to remove all listeners
 */
export async function useMediaKeys(handlers: MediaKeyHandlers): Promise<() => void> {
	const { onPlayPause, onPlay, onPause, onNextTrack, onPreviousTrack } = handlers

	const unlisteners: UnlistenFn[] = await Promise.all([
		listen('media-toggle', () => onPlayPause()),
		listen('media-play', () => onPlay()),
		listen('media-pause', () => onPause()),
		listen('media-next', () => onNextTrack()),
		listen('media-previous', () => onPreviousTrack()),
	])

	return () => {
		for (const unlisten of unlisteners) {
			unlisten()
		}
	}
}
