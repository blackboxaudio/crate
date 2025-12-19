import { writable, derived } from 'svelte/store'

// =============================================================================
// Types
// =============================================================================

export type ToastType = 'success' | 'error' | 'warning' | 'info'

export interface Toast {
	id: string
	type: ToastType
	message: string
	duration: number
}

interface ToastState {
	toasts: Toast[]
}

// =============================================================================
// State
// =============================================================================

const initialState: ToastState = {
	toasts: [],
}

const DEFAULT_DURATION = 5000

// =============================================================================
// Store
// =============================================================================

function createToastStore() {
	const { subscribe, update } = writable<ToastState>(initialState)

	const timeouts = new Map<string, ReturnType<typeof setTimeout>>()

	function generateId(): string {
		return `toast-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
	}

	function scheduleRemoval(id: string, duration: number) {
		const timeout = setTimeout(() => {
			dismiss(id)
		}, duration)
		timeouts.set(id, timeout)
	}

	function dismiss(id: string) {
		const timeout = timeouts.get(id)
		if (timeout) {
			clearTimeout(timeout)
			timeouts.delete(id)
		}
		update((state) => ({
			...state,
			toasts: state.toasts.filter((t) => t.id !== id),
		}))
	}

	return {
		subscribe,

		/**
		 * Show a toast notification
		 */
		show(type: ToastType, message: string, duration: number = DEFAULT_DURATION) {
			const id = generateId()
			const toast: Toast = { id, type, message, duration }

			update((state) => ({
				...state,
				toasts: [...state.toasts, toast],
			}))

			if (duration > 0) {
				scheduleRemoval(id, duration)
			}

			return id
		},

		/**
		 * Show a success toast
		 */
		success(message: string, duration?: number) {
			return this.show('success', message, duration)
		},

		/**
		 * Show an error toast
		 */
		error(message: string, duration?: number) {
			return this.show('error', message, duration)
		},

		/**
		 * Show a warning toast
		 */
		warning(message: string, duration?: number) {
			return this.show('warning', message, duration)
		},

		/**
		 * Show an info toast
		 */
		info(message: string, duration?: number) {
			return this.show('info', message, duration)
		},

		/**
		 * Dismiss a specific toast
		 */
		dismiss,

		/**
		 * Clear all toasts
		 */
		clear() {
			timeouts.forEach((timeout) => clearTimeout(timeout))
			timeouts.clear()
			update(() => ({ toasts: [] }))
		},
	}
}

export const toastStore = createToastStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const toasts = derived(toastStore, ($store) => $store.toasts)

export const hasToasts = derived(toastStore, ($store) => $store.toasts.length > 0)
