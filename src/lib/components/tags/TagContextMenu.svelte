<script lang="ts">
	import type { Tag, TagCategory, ContextMenuItem } from '$lib/types'
	import { TAG_CATEGORY_COLORS } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	type ContextTarget =
		| { type: 'tag'; tag: Tag; category: TagCategory }
		| { type: 'category'; category: TagCategory }
		| null

	type Props = {
		open: boolean
		x: number
		y: number
		target: ContextTarget
		categories?: TagCategory[]
		categoryCount: number
		onClose: () => void
		onClosed?: () => void
		onAddCategory: () => void
		onAddTag: (categoryId: string) => void
		onRenameTag: (tag: Tag) => void
		onDeleteTag: (tag: Tag) => void
		onRenameCategory: (category: TagCategory) => void
		onDeleteCategory: (category: TagCategory) => void
		onChangeColor?: (category: TagCategory, color: string | null) => void
		onMoveTag?: (tag: Tag, targetCategoryId: string) => void
	}

	let {
		open,
		x,
		y,
		target,
		categories = [],
		categoryCount,
		onClose,
		onClosed,
		onAddCategory,
		onAddTag,
		onRenameTag,
		onDeleteTag,
		onRenameCategory,
		onDeleteCategory,
		onChangeColor,
		onMoveTag,
	}: Props = $props()

	const menuItems = $derived<ContextMenuItem[]>(() => {
		if (!target) return []

		const items: ContextMenuItem[] = []

		if (target.type === 'tag') {
			items.push({
				id: 'rename-tag',
				label: get(translate)('tags.renameTag'),
				icon: 'pencil',
				action: () => onRenameTag(target.tag),
			})
			if (onMoveTag && categoryCount >= 2) {
				const moveTargets = categories.filter((c) => c.id !== target.category.id)
				const submenuItems: ContextMenuItem[] = moveTargets.map((c) => ({
					id: `move-to-${c.id}`,
					label: c.name,
					colorDot: c.color ?? undefined,
					action: () => onMoveTag(target.tag, c.id),
				}))
				items.push({
					id: 'move-to-category',
					label: get(translate)('tags.moveToCategory'),
					icon: 'folder-arrow',
					submenu: submenuItems,
				})
			}
			items.push({ id: 'divider-1', label: '', divider: true })
			items.push({
				id: 'delete-tag',
				label: get(translate)('tags.deleteTag'),
				icon: 'trash',
				variant: 'danger',
				action: () => onDeleteTag(target.tag),
			})
		} else if (target.type === 'category') {
			items.push({
				id: 'add-category',
				label: get(translate)('tags.addCategory'),
				icon: 'plus',
				disabled: categoryCount >= 4,
				action: onAddCategory,
			})
			items.push({
				id: 'add-tag',
				label: get(translate)('tags.addTag'),
				icon: 'tag',
				action: () => onAddTag(target.category.id),
			})
			items.push({ id: 'divider-0', label: '', divider: true })
			items.push({
				id: 'rename-category',
				label: get(translate)('tags.renameCategory'),
				icon: 'pencil',
				action: () => onRenameCategory(target.category),
			})
			if (onChangeColor) {
				const colorItems: ContextMenuItem[] = TAG_CATEGORY_COLORS.map((color) => ({
					id: `color-${color.id}`,
					label: get(translate)(`colors.${color.id}`),
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
					label: get(translate)('contextMenu.removeColor'),
					icon: 'trash',
					action: () => onChangeColor(target.category, null),
				})
				items.push({
					id: 'set-color',
					label: get(translate)('contextMenu.setColor'),
					icon: 'palette',
					submenu: colorItems,
				})
			}
			items.push({ id: 'divider-1', label: '', divider: true })
			items.push({
				id: 'delete-category',
				label: get(translate)('tags.deleteCategory'),
				icon: 'trash',
				variant: 'danger',
				action: () => onDeleteCategory(target.category),
			})
		}

		return items
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems()} {onClose} {onClosed} />
