import { listen, type UnlistenFn } from '@tauri-apps/api/event'

// =============================================================================
// Types
// =============================================================================

export interface MediaKeyHandlers {
	onPlayPause: () => void
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
 * @returns Promise of cleanup function to remove all listeners
 */
export async function useMediaKeys(handlers: MediaKeyHandlers): Promise<() => void> {
	const { onPlayPause, onNextTrack, onPreviousTrack } = handlers

	const unlisteners: UnlistenFn[] = await Promise.all([
		listen('media-toggle', () => onPlayPause()),
		listen('media-play', () => onPlayPause()),
		listen('media-pause', () => onPlayPause()),
		listen('media-next', () => onNextTrack()),
		listen('media-previous', () => onPreviousTrack()),
	])

	return () => {
		for (const unlisten of unlisteners) {
			unlisten()
		}
	}
}
