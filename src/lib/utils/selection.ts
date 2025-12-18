/**
 * Handle multi-select logic for track selection
 * Supports Shift+Click for range selection and Cmd/Ctrl+Click for toggle
 */
export function handleSelection<T extends { id: string }>(
	items: T[],
	selectedIds: Set<string>,
	clickedId: string,
	lastClickedId: string | null,
	event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean }
): { selectedIds: Set<string>; lastClickedId: string } {
	const newSelection = new Set(selectedIds)
	const isMultiSelectKey = event.metaKey || event.ctrlKey

	if (event.shiftKey && lastClickedId) {
		// Range selection
		const clickedIndex = items.findIndex((item) => item.id === clickedId)
		const lastIndex = items.findIndex((item) => item.id === lastClickedId)

		if (clickedIndex !== -1 && lastIndex !== -1) {
			const startIndex = Math.min(clickedIndex, lastIndex)
			const endIndex = Math.max(clickedIndex, lastIndex)

			// Clear current selection unless holding Cmd/Ctrl
			if (!isMultiSelectKey) {
				newSelection.clear()
			}

			// Add range to selection
			for (let i = startIndex; i <= endIndex; i++) {
				newSelection.add(items[i].id)
			}
		}
	} else if (isMultiSelectKey) {
		// Toggle selection
		if (newSelection.has(clickedId)) {
			newSelection.delete(clickedId)
		} else {
			newSelection.add(clickedId)
		}
	} else {
		// Single selection - clear and select only clicked
		newSelection.clear()
		newSelection.add(clickedId)
	}

	return {
		selectedIds: newSelection,
		lastClickedId: clickedId,
	}
}

/**
 * Select all items
 */
export function selectAll<T extends { id: string }>(items: T[]): Set<string> {
	return new Set(items.map((item) => item.id))
}

/**
 * Clear selection
 */
export function clearSelection(): Set<string> {
	return new Set()
}

/**
 * Check if an item is selected
 */
export function isSelected(selectedIds: Set<string>, id: string): boolean {
	return selectedIds.has(id)
}

/**
 * Get selected items from a list
 */
export function getSelectedItems<T extends { id: string }>(items: T[], selectedIds: Set<string>): T[] {
	return items.filter((item) => selectedIds.has(item.id))
}
