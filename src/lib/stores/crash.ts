import { writable, derived } from 'svelte/store'

// =============================================================================
// Types
// =============================================================================

export interface CrashInfo {
	message: string
	stack?: string
	source?: string
	timestamp: string
}

// =============================================================================
// State
// =============================================================================

interface CrashState {
	hasCrashed: boolean
	error: CrashInfo | null
}

const initialState: CrashState = {
	hasCrashed: false,
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createCrashStore() {
	const { subscribe, set, update } = writable<CrashState>(initialState)

	return {
		subscribe,

		/**
		 * Record a fatal crash
		 */
		setCrash(error: CrashInfo): void {
			set({
				hasCrashed: true,
				error,
			})
		},

		/**
		 * Clear crash state
		 */
		clear(): void {
			set(initialState)
		},

		/**
		 * Format error details for copying to clipboard
		 */
		formatErrorDetails(state: CrashState): string {
			if (!state.error) return 'No error details available'

			const lines = [
				`Timestamp: ${state.error.timestamp}`,
				`Source: ${state.error.source || 'Unknown'}`,
				`Message: ${state.error.message}`,
			]

			if (state.error.stack) {
				lines.push('', 'Stack Trace:', state.error.stack)
			}

			return lines.join('\n')
		},

		/**
		 * Reset to initial state
		 */
		reset(): void {
			set(initialState)
		},
	}
}

export const crashStore = createCrashStore()

// =============================================================================
// Derived Stores
// =============================================================================

/** Whether the app has crashed */
export const hasCrashed = derived(crashStore, ($store) => $store.hasCrashed)

/** The crash error info */
export const crashError = derived(crashStore, ($store) => $store.error)
