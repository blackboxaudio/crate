import { writable, derived } from 'svelte/store'
import type { DiagnosticEntry, DiagnosticsReport, SystemInfo } from '$lib/types'
import * as diagnosticsApi from '$lib/api/diagnostics'

// =============================================================================
// State
// =============================================================================

interface DiagnosticsState {
	entries: DiagnosticEntry[]
	systemInfo: SystemInfo | null
	loading: boolean
	error: string | null
}

const initialState: DiagnosticsState = {
	entries: [],
	systemInfo: null,
	loading: false,
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createDiagnosticsStore() {
	const { subscribe, set, update } = writable<DiagnosticsState>(initialState)

	return {
		subscribe,

		/**
		 * Load diagnostic entries and system info
		 */
		async load() {
			update((s) => ({ ...s, loading: true, error: null }))

			try {
				const [entries, systemInfo] = await Promise.all([
					diagnosticsApi.getDiagnosticEntries(),
					diagnosticsApi.getSystemInfo(),
				])

				update((s) => ({
					...s,
					entries,
					systemInfo,
					loading: false,
				}))
			} catch (error) {
				update((s) => ({
					...s,
					loading: false,
					error: error instanceof Error ? error.message : 'Failed to load diagnostics',
				}))
			}
		},

		/**
		 * Refresh only the diagnostic entries
		 */
		async refresh() {
			try {
				const entries = await diagnosticsApi.getDiagnosticEntries()
				update((s) => ({ ...s, entries }))
			} catch (error) {
				console.error('Failed to refresh diagnostics:', error)
			}
		},

		/**
		 * Clear all diagnostic entries
		 */
		async clear() {
			try {
				await diagnosticsApi.clearDiagnosticEntries()
				update((s) => ({ ...s, entries: [] }))
			} catch (error) {
				console.error('Failed to clear diagnostics:', error)
			}
		},

		/**
		 * Log an error to the diagnostics service
		 */
		async logError(category: string, message: string, details?: string) {
			try {
				await diagnosticsApi.logError(category, message, details)
			} catch (error) {
				console.error('Failed to log error:', error)
			}
		},

		/**
		 * Get a full diagnostics report for export
		 */
		async getReport(): Promise<DiagnosticsReport> {
			return diagnosticsApi.getDiagnosticsReport()
		},

		/**
		 * Reset store to initial state
		 */
		reset() {
			set(initialState)
		},
	}
}

export const diagnosticsStore = createDiagnosticsStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const diagnosticEntries = derived(diagnosticsStore, ($s) => $s.entries)

export const systemInfo = derived(diagnosticsStore, ($s) => $s.systemInfo)

export const diagnosticsLoading = derived(diagnosticsStore, ($s) => $s.loading)

export const errorCount = derived(diagnosticsStore, ($s) => $s.entries.filter((e) => e.level === 'error').length)
