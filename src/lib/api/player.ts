import { invoke } from '@tauri-apps/api/core';
import type { PlaybackState } from '$lib/types';

/**
 * Start playing a track by ID
 */
export async function playTrack(id: string): Promise<PlaybackState> {
  return invoke<PlaybackState>('play_track', { id });
}

/**
 * Pause playback
 */
export async function pause(): Promise<PlaybackState> {
  return invoke<PlaybackState>('pause');
}

/**
 * Resume playback
 */
export async function resume(): Promise<PlaybackState> {
  return invoke<PlaybackState>('resume');
}

/**
 * Stop playback
 */
export async function stop(): Promise<PlaybackState> {
  return invoke<PlaybackState>('stop');
}

/**
 * Seek to position in milliseconds
 */
export async function seek(positionMs: number): Promise<PlaybackState> {
  return invoke<PlaybackState>('seek', { positionMs });
}

/**
 * Set volume (0.0 to 1.0)
 */
export async function setVolume(volume: number): Promise<PlaybackState> {
  return invoke<PlaybackState>('set_volume', { volume });
}

/**
 * Get current playback state
 */
export async function getPlaybackState(): Promise<PlaybackState> {
  return invoke<PlaybackState>('get_playback_state');
}
