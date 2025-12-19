<script lang="ts">
	import type { UsbDevice, ContextMenuItem } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'

	type Props = {
		open: boolean
		x: number
		y: number
		device: UsbDevice | null
		onClose: () => void
		onViewInfo: (device: UsbDevice) => void
		onEject: (device: UsbDevice) => void
	}

	let { open, x, y, device, onClose, onViewInfo, onEject }: Props = $props()

	const menuItems = $derived<ContextMenuItem[]>(
		device
			? [
					{
						id: 'view-info',
						label: 'View info',
						icon: 'info',
						action: () => onViewInfo(device),
					},
					{ id: 'divider-1', label: '', divider: true },
					{
						id: 'eject',
						label: 'Eject',
						icon: 'eject',
						action: () => onEject(device),
					},
				]
			: []
	)
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} />
