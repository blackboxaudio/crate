<script lang="ts">
	import { scale } from 'svelte/transition'
	import type { ContextMenuItem } from '$lib/types'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		open: boolean
		x: number
		y: number
		items: ContextMenuItem[]
		onClose: () => void
		onClosed?: () => void
	}

	let { open, x, y, items, onClose, onClosed }: Props = $props()

	let menuEl: HTMLDivElement | undefined = $state()
	/* eslint-disable svelte/prefer-writable-derived */
	let visible = $state(false)
	// Track active submenu path as array of item IDs (supports unlimited nesting)
	let activeSubmenuPath = $state<string[]>([])
	let adjustedPosition = $derived({ x, y })

	// Track open state changes
	$effect(() => {
		visible = open
	})

	// Handle transition end
	function handleOutroEnd() {
		visible = false
		onClosed?.()
	}

	// Adjust position when menu opens to prevent overflow
	$effect(() => {
		if (open && menuEl) {
			const rect = menuEl.getBoundingClientRect()
			const viewportWidth = window.innerWidth
			const viewportHeight = window.innerHeight

			let newX = x
			let newY = y

			// Adjust horizontal position
			if (x + rect.width > viewportWidth - 8) {
				newX = viewportWidth - rect.width - 8
			}

			// Adjust vertical position
			if (y + rect.height > viewportHeight - 8) {
				newY = viewportHeight - rect.height - 8
			}

			adjustedPosition = { x: Math.max(8, newX), y: Math.max(8, newY) }
		}
	})

	// Reset position when x/y change
	$effect(() => {
		adjustedPosition = { x, y }
	})

	// Close on click outside
	function handleClickOutside(e: MouseEvent) {
		if (menuEl && !menuEl.contains(e.target as Node)) {
			onClose()
		}
	}

	// Close on Escape
	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose()
		}
	}

	$effect(() => {
		if (open) {
			document.addEventListener('click', handleClickOutside)
			document.addEventListener('keydown', handleKeydown)

			return () => {
				document.removeEventListener('click', handleClickOutside)
				document.removeEventListener('keydown', handleKeydown)
				activeSubmenuPath = []
			}
		}
	})

	function handleItemClick(item: ContextMenuItem) {
		if (item.disabled) return
		if (item.submenu) return // Submenus are handled by hover
		if (item.action) {
			item.action()
		}
		onClose()
	}

	function handleItemMouseEnter(item: ContextMenuItem, depth: number) {
		if (item.submenu) {
			// Keep path up to current depth, then add this item
			activeSubmenuPath = [...activeSubmenuPath.slice(0, depth), item.id]
		} else {
			// No submenu, truncate path to current depth
			activeSubmenuPath = activeSubmenuPath.slice(0, depth)
		}
	}

	function isSubmenuActive(itemId: string, depth: number): boolean {
		return activeSubmenuPath[depth] === itemId
	}

	function getSubmenuStyle(parentEl: HTMLElement | null): string {
		if (!parentEl) return 'left: 100%; top: 0;'

		const parentRect = parentEl.getBoundingClientRect()
		const submenuWidth = 192 // min-w-48 = 12rem = 192px

		// Check if submenu fits on the right
		const fitsRight = parentRect.right + submenuWidth < window.innerWidth - 8

		if (fitsRight) {
			return 'left: 100%; top: 0;'
		} else {
			return 'right: 100%; top: 0;'
		}
	}
</script>

{#snippet menuItems(itemList: ContextMenuItem[], depth: number)}
	{#each itemList as item (item.id)}
		{#if item.divider}
			<div class="my-1 border-t border-stroke"></div>
		{:else}
			<div class="group relative" role="none" onmouseenter={() => handleItemMouseEnter(item, depth)}>
				<button
					type="button"
					class="flex w-full items-center gap-3 px-3 py-1.5 text-left text-sm transition-colors
						{item.disabled
						? 'cursor-not-allowed text-text-tertiary'
						: item.variant === 'danger'
							? `text-red-500 hover:cursor-pointer hover:bg-red-500/10 ${item.submenu && isSubmenuActive(item.id, depth) ? 'bg-red-500/10' : ''}`
							: `text-text-primary hover:cursor-pointer hover:bg-surface-2 ${item.submenu && isSubmenuActive(item.id, depth) ? 'bg-surface-2' : ''}`}"
					onclick={() => handleItemClick(item)}
					disabled={item.disabled}
					role="menuitem"
				>
					{#if item.colorDot}
						<span class="h-3 w-3 rounded-full border border-black/10" style="background-color: {item.colorDot};"></span>
					{/if}
					{#if item.icon}
						<Icon name={item.icon} />
					{/if}
					<span class="flex-1 whitespace-nowrap">{item.label}</span>
					{#if item.selected}
						<Icon name="check" class="h-4 w-4 text-text-primary" />
					{/if}
					{#if item.shortcut}
						<span class="text-xs text-text-tertiary">{item.shortcut}</span>
					{/if}
					{#if item.submenu}
						<Icon name="chevron-right" class="h-4 w-4 text-text-tertiary" />
					{/if}
				</button>

				{#if item.submenu && isSubmenuActive(item.id, depth)}
					<div
						class="absolute z-50 min-w-48 origin-top-left rounded-md border border-stroke bg-surface-1 py-1 shadow-lg"
						style={getSubmenuStyle(null)}
						role="menu"
						transition:scale={{ start: 0.95, duration: 200 }}
					>
						{@render menuItems(item.submenu, depth + 1)}
					</div>
				{/if}
			</div>
		{/if}
	{/each}
{/snippet}

{#if visible}
	<div
		bind:this={menuEl}
		class="fixed z-50 min-w-48 origin-top-left rounded-md border border-stroke bg-surface-1 py-1 shadow-lg"
		style="left: {adjustedPosition.x}px; top: {adjustedPosition.y}px;"
		role="menu"
		transition:scale={{ start: 0.95, duration: 200 }}
		onoutroend={handleOutroEnd}
	>
		{@render menuItems(items, 0)}
	</div>
{/if}
