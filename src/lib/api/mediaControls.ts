import { invoke } from '@tauri-apps/api/core'

export async function updateNowPlaying(
	title: string | null,
	artist: string | null,
	album: string | null,
	artworkPath: string | null,
	durationMs: number | null
): Promise<void> {
	return invoke('update_now_playing', {
		title,
		artist,
		album,
		artworkPath,
		durationMs,
	})
}

export async function updatePlaybackState(isPlaying: boolean): Promise<void> {
	return invoke('update_playback_state', { isPlaying })
}

export async function clearNowPlaying(): Promise<void> {
	return invoke('clear_now_playing')
}
