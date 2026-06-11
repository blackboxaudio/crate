import { invoke } from '@tauri-apps/api/core'
import type { DiagnosticEntry, DiagnosticsReport, SystemInfo } from '../types'

/**
 * Get all diagnostic entries
 */
export async function getDiagnosticEntries(): Promise<DiagnosticEntry[]> {
	return invoke<DiagnosticEntry[]>('get_diagnostic_entries')
}

/**
 * Get system information
 */
export async function getSystemInfo(): Promise<SystemInfo> {
	return invoke<SystemInfo>('get_system_info')
}

/**
 * Get a full diagnostics report for export
 */
export async function getDiagnosticsReport(): Promise<DiagnosticsReport> {
	return invoke<DiagnosticsReport>('get_diagnostics_report')
}

/**
 * Clear all diagnostic entries
 */
export async function clearDiagnosticEntries(): Promise<void> {
	return invoke<void>('clear_diagnostic_entries')
}

/**
 * Log an error to the diagnostics service
 */
export async function logError(category: string, message: string, details?: string): Promise<void> {
	return invoke<void>('log_error', { category, message, details })
}
