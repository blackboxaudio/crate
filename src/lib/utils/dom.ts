import { setDialogConflictingItemsEnabled } from '$lib/api/app'

/**
 * Check if the currently focused element is an input or textarea.
 * Used to prevent keyboard shortcuts from triggering while typing.
 */
export function isInputFocused(): boolean {
	const active = document.activeElement
	return active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement
}

let nativeDialogOpen = false

/**
 * Check if a native OS file dialog is currently open.
 * Used to prevent keyboard shortcuts from firing while navigating the dialog.
 */
export function isNativeDialogOpen(): boolean {
	return nativeDialogOpen
}

/**
 * Wrap a native dialog call (open/save) so keyboard shortcuts are suppressed
 * for the duration of the dialog. Disables native menu accelerators that would
 * steal key events (arrows, Space, etc.) from the dialog.
 */
export async function withNativeDialog<T>(fn: () => Promise<T>): Promise<T> {
	nativeDialogOpen = true
	await setDialogConflictingItemsEnabled(false)
	try {
		return await fn()
	} finally {
		nativeDialogOpen = false
		await setDialogConflictingItemsEnabled(true)
	}
}
