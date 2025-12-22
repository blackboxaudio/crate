import { invoke } from '@tauri-apps/api/core'
import type { AnalysisResult, Track } from '$lib/types'

/**
 * Analyze tracks for BPM and key detection
 */
export async function analyzeTracks(trackIds: string[]): Promise<AnalysisResult[]> {
	return invoke<AnalysisResult[]>('analyze_tracks', { trackIds })
}

/**
 * Get updated tracks after analysis
 */
export async function getAnalyzedTracks(trackIds: string[]): Promise<Track[]> {
	return invoke<Track[]>('get_analyzed_tracks', { trackIds })
}
