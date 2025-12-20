<script lang="ts">
	import type { ContextMenuItem } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'

	type Props = {
		open: boolean
		x: number
		y: number
		categoryCount: number
		onClose: () => void
		onAddCategory: () => void
		onAddTag: () => void
	}

	let { open, x, y, categoryCount, onClose, onAddCategory, onAddTag }: Props = $props()

	const menuItems = $derived<ContextMenuItem[]>([
		{
			id: 'add-category',
			label: 'Add Category',
			icon: 'plus',
			disabled: categoryCount >= 4,
			action: onAddCategory,
		},
		{
			id: 'add-tag',
			label: 'Add Tag',
			icon: 'tag',
			disabled: categoryCount === 0,
			action: onAddTag,
		},
	])
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} />
