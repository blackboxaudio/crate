/**
 * Drag and drop utilities for pointer-event based drag operations.
 * Uses data attributes on elements to identify drop targets.
 */

// =============================================================================
// Types
// =============================================================================

export interface DropTarget {
	element: HTMLElement
	id: string
	type: 'playlist' | 'folder' | 'device' | 'category' | 'tracklist' | 'releaselist' | 'root'
	rect: DOMRect
}

// =============================================================================
// Constants
// =============================================================================

// Data attribute used to mark drop targets
export const DROP_TARGET_ATTR = 'data-drop-target'

// Minimum distance (px) to move before considering it a drag
export const DRAG_THRESHOLD = 5

// =============================================================================
// Hit Testing
// =============================================================================

/**
 * Find all drop target elements in the DOM
 */
export function findDropTargets(): DropTarget[] {
	const elements = document.querySelectorAll<HTMLElement>(`[${DROP_TARGET_ATTR}]`)
	const targets: DropTarget[] = []

	elements.forEach((element) => {
		const value = element.getAttribute(DROP_TARGET_ATTR)
		if (!value) return

		// Parse the value: "playlist-{id}", "folder-{id}", or "device-{id}"
		// Split only on the first dash to preserve UUIDs which contain dashes
		const dashIndex = value.indexOf('-')
		if (dashIndex === -1) return

		const type = value.substring(0, dashIndex)
		const id = value.substring(dashIndex + 1)
		if (!type || !id) return
		if (
			type !== 'playlist' &&
			type !== 'folder' &&
			type !== 'device' &&
			type !== 'category' &&
			type !== 'tracklist' &&
			type !== 'releaselist' &&
			type !== 'root'
		)
			return

		targets.push({
			element,
			id,
			type: type as DropTarget['type'],
			rect: element.getBoundingClientRect(),
		})
	})

	return targets
}

/**
 * Find which drop target contains the given point
 */
export function findDropTargetAtPoint(x: number, y: number, targets: DropTarget[]): DropTarget | null {
	// Iterate in reverse to get topmost element first (later in DOM = on top)
	for (let i = targets.length - 1; i >= 0; i--) {
		const target = targets[i]
		const { left, right, top, bottom } = target.rect

		if (x >= left && x <= right && y >= top && y <= bottom) {
			return target
		}
	}

	return null
}

/**
 * Check if a point is within an element's bounds
 */
export function isPointInElement(x: number, y: number, element: HTMLElement): boolean {
	const rect = element.getBoundingClientRect()
	return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

/**
 * Calculate distance between two points
 */
export function getDistance(x1: number, y1: number, x2: number, y2: number): number {
	return Math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2)
}
