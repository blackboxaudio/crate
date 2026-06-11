/**
 * Utility functions for calculating bounded positions for floating elements
 * like context menus and dropdowns to keep them within viewport bounds.
 */

export const VIEWPORT_PADDING = 8

export type Position = { x: number; y: number }
export type Dimensions = { width: number; height: number }
export type AnchorOrigin = 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right'

/**
 * Calculate adjusted position for a floating element to stay within viewport bounds.
 * Returns both the adjusted position and the transform origin for animations.
 */
export function calculateBoundedPosition(
	anchor: Position,
	elementDimensions: Dimensions,
	padding: number = VIEWPORT_PADDING
): { position: Position; origin: AnchorOrigin } {
	const viewportWidth = window.innerWidth
	const viewportHeight = window.innerHeight

	let x = anchor.x
	let y = anchor.y
	let originX: 'left' | 'right' = 'left'
	let originY: 'top' | 'bottom' = 'top'

	// Check horizontal overflow
	if (x + elementDimensions.width > viewportWidth - padding) {
		x = Math.max(padding, viewportWidth - elementDimensions.width - padding)
		originX = 'right'
	}

	// Check vertical overflow
	if (y + elementDimensions.height > viewportHeight - padding) {
		y = Math.max(padding, viewportHeight - elementDimensions.height - padding)
		originY = 'bottom'
	}

	// Ensure minimum bounds
	x = Math.max(padding, x)
	y = Math.max(padding, y)

	return {
		position: { x, y },
		origin: `${originY}-${originX}` as AnchorOrigin,
	}
}

/**
 * Calculate submenu position relative to parent menu item.
 * Handles both horizontal and vertical overflow.
 */
export function calculateSubmenuPosition(
	parentRect: DOMRect,
	submenuDimensions: Dimensions,
	padding: number = VIEWPORT_PADDING
): { style: string; origin: AnchorOrigin } {
	const viewportWidth = window.innerWidth
	const viewportHeight = window.innerHeight

	// Check horizontal: prefer right, fallback to left
	const fitsRight = parentRect.right + submenuDimensions.width < viewportWidth - padding
	const fitsLeft = parentRect.left - submenuDimensions.width > padding

	let horizontal: 'left' | 'right'
	let horizontalStyle: string

	if (fitsRight) {
		horizontal = 'left'
		horizontalStyle = 'left: 100%;'
	} else if (fitsLeft) {
		horizontal = 'right'
		horizontalStyle = 'right: 100%;'
	} else {
		// Neither fits perfectly, choose the side with more space
		horizontal = parentRect.left > viewportWidth - parentRect.right ? 'right' : 'left'
		horizontalStyle = horizontal === 'right' ? 'right: 100%;' : 'left: 100%;'
	}

	// Check vertical: prefer aligned with parent top, adjust if needed
	let verticalStyle = 'top: 0;'
	const submenuBottom = parentRect.top + submenuDimensions.height

	if (submenuBottom > viewportHeight - padding) {
		// Need to shift up - use bottom alignment instead
		verticalStyle = 'bottom: 0;'
	}

	const originX = horizontal === 'left' ? 'left' : 'right'
	const originY = verticalStyle.includes('bottom') ? 'bottom' : 'top'

	return {
		style: `${horizontalStyle} ${verticalStyle}`,
		origin: `${originY}-${originX}` as AnchorOrigin,
	}
}

/**
 * Get transform-origin CSS class based on anchor origin
 */
export function getOriginClass(origin: AnchorOrigin): string {
	const classes: Record<AnchorOrigin, string> = {
		'top-left': 'origin-top-left',
		'top-right': 'origin-top-right',
		'bottom-left': 'origin-bottom-left',
		'bottom-right': 'origin-bottom-right',
	}
	return classes[origin]
}
