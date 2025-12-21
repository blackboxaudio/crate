import { crashStore } from '$lib/stores/crash'
import * as diagnosticsApi from '$lib/api/diagnostics'
import { markSvelteKitReady } from '../../hooks.client'

// =============================================================================
// Hook
// =============================================================================

/**
 * Set up global error handlers for uncaught exceptions and unhandled rejections.
 *
 * This also signals to the early error handler (hooks.client.ts) that SvelteKit
 * has successfully initialized, so it can stop intercepting errors.
 *
 * Should be called once at app startup (in +layout.svelte onMount).
 * Returns a cleanup function to remove the handlers.
 */
export function useGlobalErrorHandler(): () => void {
	// Signal that SvelteKit is ready (removes early error handlers)
	markSvelteKitReady()

	// Handle synchronous errors
	function handleError(event: ErrorEvent): void {
		event.preventDefault()

		const errorInfo = {
			message: event.message || 'An unexpected error occurred',
			stack: event.error?.stack,
			source: 'window.onerror',
			timestamp: new Date().toISOString(),
		}

		// Attempt to log to diagnostics (fire and forget)
		diagnosticsApi.logError('Crash', errorInfo.message, errorInfo.stack).catch(() => {
			// Silently ignore logging failures
		})

		crashStore.setCrash(errorInfo)
	}

	// Handle unhandled promise rejections
	function handleRejection(event: PromiseRejectionEvent): void {
		event.preventDefault()

		const reason = event.reason
		const message =
			reason instanceof Error ? reason.message : typeof reason === 'string' ? reason : 'Unhandled promise rejection'
		const stack = reason instanceof Error ? reason.stack : undefined

		const errorInfo = {
			message,
			stack,
			source: 'unhandledrejection',
			timestamp: new Date().toISOString(),
		}

		// Attempt to log to diagnostics (fire and forget)
		diagnosticsApi.logError('Crash', errorInfo.message, errorInfo.stack).catch(() => {
			// Silently ignore logging failures
		})

		crashStore.setCrash(errorInfo)
	}

	// Register handlers
	window.addEventListener('error', handleError)
	window.addEventListener('unhandledrejection', handleRejection)

	// Return cleanup function
	return () => {
		window.removeEventListener('error', handleError)
		window.removeEventListener('unhandledrejection', handleRejection)
	}
}
