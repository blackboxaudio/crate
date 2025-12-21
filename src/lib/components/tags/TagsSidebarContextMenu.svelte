<script lang="ts">
	import type { ContextMenuItem } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

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
			label: get(translate)('tags.addCategory'),
			icon: 'plus',
			disabled: categoryCount >= 4,
			action: onAddCategory,
		},
		{
			id: 'add-tag',
			label: get(translate)('tags.addTag'),
			icon: 'tag',
			disabled: categoryCount === 0,
			action: onAddTag,
		},
	])
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} />
