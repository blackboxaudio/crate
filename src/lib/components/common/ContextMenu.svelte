<script lang="ts">
	import { onMount } from 'svelte'
	import type { ContextMenuItem } from '$lib/types'

	type Props = {
		open: boolean
		x: number
		y: number
		items: ContextMenuItem[]
		onClose: () => void
	}

	let { open, x, y, items, onClose }: Props = $props()

	let menuEl: HTMLDivElement | undefined = $state()
	// Track active submenu path as array of item IDs (supports unlimited nesting)
	let activeSubmenuPath = $state<string[]>([])
	let adjustedPosition = $derived({ x, y })

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

	onMount(() => {
		if (open) {
			document.addEventListener('click', handleClickOutside)
			document.addEventListener('keydown', handleKeydown)
		}

		return () => {
			document.removeEventListener('click', handleClickOutside)
			document.removeEventListener('keydown', handleKeydown)
		}
	})

	$effect(() => {
		if (open) {
			document.addEventListener('click', handleClickOutside)
			document.addEventListener('keydown', handleKeydown)
		} else {
			document.removeEventListener('click', handleClickOutside)
			document.removeEventListener('keydown', handleKeydown)
			activeSubmenuPath = []
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
			<div class="my-1 border-t border-zinc-700"></div>
		{:else}
			<div class="group relative" role="none" onmouseenter={() => handleItemMouseEnter(item, depth)}>
				<button
					type="button"
					class="flex w-full items-center gap-3 px-3 py-1.5 text-left text-sm transition-colors
						{item.disabled ? 'cursor-not-allowed text-zinc-500' : 'text-zinc-200 hover:bg-zinc-700'}"
					onclick={() => handleItemClick(item)}
					disabled={item.disabled}
					role="menuitem"
				>
					<span class="flex-1">{item.label}</span>
					{#if item.shortcut}
						<span class="text-xs text-zinc-500">{item.shortcut}</span>
					{/if}
					{#if item.submenu}
						<svg class="h-4 w-4 text-zinc-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
						</svg>
					{/if}
				</button>

				{#if item.submenu && isSubmenuActive(item.id, depth)}
					<div
						class="absolute z-50 min-w-48 rounded-md border border-zinc-700 bg-zinc-800 py-1 shadow-lg"
						style={getSubmenuStyle(null)}
						role="menu"
					>
						{@render menuItems(item.submenu, depth + 1)}
					</div>
				{/if}
			</div>
		{/if}
	{/each}
{/snippet}

{#if open}
	<div
		bind:this={menuEl}
		class="fixed z-50 min-w-48 rounded-md border border-zinc-700 bg-zinc-800 py-1 shadow-lg"
		style="left: {adjustedPosition.x}px; top: {adjustedPosition.y}px;"
		role="menu"
	>
		{@render menuItems(items, 0)}
	</div>
{/if}
