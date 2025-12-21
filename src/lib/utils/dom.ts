/**
 * Check if the currently focused element is an input or textarea.
 * Used to prevent keyboard shortcuts from triggering while typing.
 */
export function isInputFocused(): boolean {
	const active = document.activeElement
	return active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement
}
