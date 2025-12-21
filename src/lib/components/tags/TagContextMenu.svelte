<script lang="ts">
	import type { Tag, TagCategory, ContextMenuItem } from '$lib/types'
	import { TAG_CATEGORY_COLORS } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'

	type ContextTarget =
		| { type: 'tag'; tag: Tag; category: TagCategory }
		| { type: 'category'; category: TagCategory }
		| null

	type Props = {
		open: boolean
		x: number
		y: number
		target: ContextTarget
		onClose: () => void
		onRenameTag: (tag: Tag) => void
		onDeleteTag: (tag: Tag) => void
		onRenameCategory: (category: TagCategory) => void
		onDeleteCategory: (category: TagCategory) => void
		onChangeColor?: (category: TagCategory, color: string | null) => void
	}

	let {
		open,
		x,
		y,
		target,
		onClose,
		onRenameTag,
		onDeleteTag,
		onRenameCategory,
		onDeleteCategory,
		onChangeColor,
	}: Props = $props()

	const menuItems = $derived<ContextMenuItem[]>(() => {
		if (!target) return []

		const items: ContextMenuItem[] = []

		if (target.type === 'tag') {
			items.push({
				id: 'rename-tag',
				label: 'Rename Tag',
				icon: 'pencil',
				action: () => onRenameTag(target.tag),
			})
			items.push({ id: 'divider-1', label: '', divider: true })
			items.push({
				id: 'delete-tag',
				label: 'Delete Tag',
				icon: 'trash',
				variant: 'danger',
				action: () => onDeleteTag(target.tag),
			})
		} else if (target.type === 'category') {
			items.push({
				id: 'rename-category',
				label: 'Rename Category',
				icon: 'pencil',
				action: () => onRenameCategory(target.category),
			})
			if (onChangeColor) {
				const colorItems: ContextMenuItem[] = TAG_CATEGORY_COLORS.map((color) => ({
					id: `color-${color.id}`,
					label: color.label,
					colorDot: color.hex,
					selected: target.category.color === color.hex,
					action: () => onChangeColor(target.category, color.hex),
				}))
				colorItems.push({
					id: 'color-divider',
					label: '',
					divider: true,
				})
				colorItems.push({
					id: 'remove-color',
					label: 'Remove Color',
					icon: 'trash',
					action: () => onChangeColor(target.category, null),
				})
				items.push({
					id: 'set-color',
					label: 'Set Color',
					icon: 'palette',
					submenu: colorItems,
				})
			}
			items.push({ id: 'divider-1', label: '', divider: true })
			items.push({
				id: 'delete-category',
				label: 'Delete Category',
				icon: 'trash',
				variant: 'danger',
				action: () => onDeleteCategory(target.category),
			})
		}

		return items
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems()} {onClose} />
