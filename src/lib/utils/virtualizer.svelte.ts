import { tick, untrack } from 'svelte'
import { Virtualizer, elementScroll, observeElementOffset } from '@tanstack/virtual-core'
import type { VirtualizerOptions } from '@tanstack/virtual-core'

/**
 * Custom observeElementRect that reports the initial size synchronously (so the
 * virtualizer can calculate items in the same frame it mounts) and defers
 * subsequent ResizeObserver notifications via requestAnimationFrame to avoid
 * "ResizeObserver loop completed with undelivered notifications" errors.
 */
const observeElementRect: VirtualizerOptions<HTMLElement, Element>['observeElementRect'] = (instance, cb) => {
	const element = instance.scrollElement
	if (!element) return

	// Report current size synchronously so _willUpdate() has correct dimensions
	// on the very first call, before the ResizeObserver's first (async) callback.
	const rect = element.getBoundingClientRect()
	cb({ width: rect.width, height: rect.height })

	let rafId: number | null = null
	const observer = new ResizeObserver((entries) => {
		if (rafId !== null) cancelAnimationFrame(rafId)
		rafId = requestAnimationFrame(() => {
			rafId = null
			const entry = entries[0]
			if (entry?.borderBoxSize) {
				const box = entry.borderBoxSize[0]
				if (box) {
					cb({ width: box.inlineSize, height: box.blockSize })
				}
			} else {
				cb({
					width: element.getBoundingClientRect().width,
					height: element.getBoundingClientRect().height,
				})
			}
		})
	})

	observer.observe(element, { box: 'border-box' })
	return () => {
		if (rafId !== null) cancelAnimationFrame(rafId)
		observer.disconnect()
	}
}

export type VirtualItem = {
	index: number
	start: number
	end: number
	size: number
	key: string | number
}

type VirtualListOptions = {
	count: () => number
	getScrollElement: () => HTMLElement | null
	estimateSize: () => (index: number) => number
	overscan?: number
	getItemKey?: (index: number) => string | number
}

export function createVirtualList(options: VirtualListOptions) {
	let virtualItems: VirtualItem[] = $state([])
	let totalSize: number = $state(0)

	let instance: Virtualizer<HTMLElement, Element> | null = null

	// Flag to suppress intermediate onChange → syncState calls during the
	// $effect.pre update cycle. measure() and _willUpdate() both trigger
	// onChange internally, but we only want one final syncState after all
	// calculations are complete — otherwise the intermediate (stale) state
	// causes a visible flash.
	let isSyncing = false

	// When true, syncState skips full array replacements to prevent the
	// scroll-restoration scroll event from triggering a secondary syncState
	// that would replace the array and destroy CSS transition "from" values.
	let isRestoringScroll = false

	function syncState() {
		if (isSyncing || !instance) return
		const newItems = instance.getVirtualItems() as VirtualItem[]

		// Wrap in untrack() so reads of virtualItems (for the inPlace check
		// and element access) don't create reactive dependencies in the
		// calling $effect.pre. Without this, element-level writes
		// (virtualItems[i] = ...) bump the array version, which would
		// re-trigger the $effect.pre → syncState() → infinite loop.
		// Writes still fire notifications, so {#each} templates update.
		untrack(() => {
			totalSize = instance.getTotalSize()

			// Check if keys match at each shared position (common prefix).
			// This is true for expand/collapse where items stay in the same
			// order but the overscan boundary may shift, adding/removing items
			// at the edges. Updating the common prefix in-place preserves DOM
			// elements and their computed styles so CSS transitions animate.
			const minLen = Math.min(newItems.length, virtualItems.length)
			let commonPrefix = minLen > 0
			for (let i = 0; i < minLen; i++) {
				if (newItems[i].key !== virtualItems[i].key) {
					commonPrefix = false
					break
				}
			}

			if (commonPrefix) {
				// Update shared items in-place: keeps array reference stable so
				// {#each} reuses DOM elements, preserving the old computed styles
				// that CSS transitions animate from.
				for (let i = 0; i < minLen; i++) {
					virtualItems[i] = newItems[i]
				}
				// Handle items that left or entered the overscan boundary.
				if (newItems.length < virtualItems.length) {
					virtualItems.splice(newItems.length)
				} else {
					for (let i = minLen; i < newItems.length; i++) {
						virtualItems.push(newItems[i])
					}
				}
			} else if (!isRestoringScroll) {
				// Keys reordered (e.g. scrolling brought entirely new items into
				// view): full array replacement required.
				virtualItems = newItems
			}
			// If isRestoringScroll and !commonPrefix: skip the replacement to
			// preserve DOM elements and active CSS transitions. The rAF cleanup
			// will call syncState() to catch up after scroll restoration completes.
		})
	}

	// Create/destroy the virtualizer when the scroll element changes
	$effect(() => {
		const scrollElement = options.getScrollElement()
		if (!scrollElement) return

		const count = untrack(() => options.count())
		const estimateSize = untrack(() => options.estimateSize())
		const getItemKey = options.getItemKey

		instance = new Virtualizer({
			count,
			getScrollElement: () => scrollElement,
			estimateSize,
			overscan: options.overscan ?? 10,
			getItemKey,
			observeElementRect,
			observeElementOffset,
			scrollToFn: elementScroll,
			onChange: syncState,
		})

		instance._didMount()
		instance._willUpdate()
		syncState()

		return () => {
			instance = null
		}
	})

	// Sync options and recalculate BEFORE DOM updates.
	// Using $effect.pre ensures virtualItems are current when Svelte renders,
	// preventing flash from stale positions on expand/collapse/filter changes.
	$effect.pre(() => {
		const count = options.count()
		const estimateSize = options.estimateSize()
		if (!instance) return

		// Save scroll position before recalculation. The DOM update cycle can
		// reset scrollTop to 0 when item sizes change (e.g., expand/collapse).
		const scrollEl = options.getScrollElement()
		const savedScrollTop = scrollEl?.scrollTop ?? 0

		// Suppress intermediate onChange calls during measure/update so only
		// the final syncState (after all recalculations) updates reactive state.
		isSyncing = true
		instance.measure()
		instance.setOptions({
			...instance.options,
			count,
			estimateSize,
		})
		instance._willUpdate()
		isSyncing = false
		syncState()

		// Restore scroll position after DOM update if it was unexpectedly reset.
		// Uses tick() so the restore happens after Svelte's DOM update but before
		// browser event processing, preventing a one-frame flash of wrong scroll.
		// Setting isRestoringScroll suppresses the scroll-event-driven syncState
		// that would otherwise replace the array and destroy CSS transitions.
		if (scrollEl && savedScrollTop > 0) {
			tick().then(() => {
				if (scrollEl.scrollTop !== savedScrollTop) {
					isRestoringScroll = true
					scrollEl.scrollTop = savedScrollTop
					requestAnimationFrame(() => {
						isRestoringScroll = false
						syncState()
					})
				}
			})
		}
	})

	return {
		get virtualItems() {
			return virtualItems
		},
		get totalSize() {
			return totalSize
		},
		scrollToOffset(offset: number) {
			instance?.scrollToOffset(offset)
		},
	}
}
