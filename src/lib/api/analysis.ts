import { invoke } from '@tauri-apps/api/core'
import type { AnalysisResult, Track } from '$lib/types'

/**
 * Analyze tracks for BPM and key detection
 * Progress events are emitted via 'analysis-progress' Tauri event
 */
export async function analyzeTracks(trackIds: string[]): Promise<AnalysisResult[]> {
	return invoke<AnalysisResult[]>('analyze_tracks', { trackIds })
}

/**
 * Cancel the current analysis operation
 */
export async function cancelAnalysis(): Promise<void> {
	return invoke('cancel_analysis')
}

/**
 * Get updated tracks after analysis
 */
export async function getAnalyzedTracks(trackIds: string[]): Promise<Track[]> {
	return invoke<Track[]>('get_analyzed_tracks', { trackIds })
}
