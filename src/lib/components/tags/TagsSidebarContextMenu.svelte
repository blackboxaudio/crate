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
		onClosed?: () => void
		onAddCategory: () => void
	}

	let { open, x, y, categoryCount, onClose, onClosed, onAddCategory }: Props = $props()

	const menuItems = $derived<ContextMenuItem[]>([
		{
			id: 'add-category',
			label: get(translate)('tags.addCategory'),
			icon: 'plus',
			disabled: categoryCount >= 4,
			action: onAddCategory,
		},
	])
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} {onClosed} />
