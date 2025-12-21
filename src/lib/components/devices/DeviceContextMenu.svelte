<script lang="ts">
	import type { UsbDevice, ContextMenuItem } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	type Props = {
		open: boolean
		x: number
		y: number
		device: UsbDevice | null
		onClose: () => void
		onViewInfo: (device: UsbDevice) => void
		onRevealInFinder: (device: UsbDevice) => void
		onEject: (device: UsbDevice) => void
	}

	let { open, x, y, device, onClose, onViewInfo, onRevealInFinder, onEject }: Props = $props()

	const revealLabel = $derived(() => {
		const ua = navigator.userAgent
		if (ua.includes('Mac')) return get(translate)('contextMenu.viewInFinder')
		if (ua.includes('Windows')) return get(translate)('contextMenu.viewInExplorer')
		return get(translate)('contextMenu.viewInFileManager')
	})

	const menuItems = $derived<ContextMenuItem[]>(
		device
			? [
					{
						id: 'view-info',
						label: get(translate)('devices.viewInfo'),
						icon: 'info',
						action: () => onViewInfo(device),
					},
					{
						id: 'reveal-in-finder',
						label: revealLabel(),
						icon: 'folder-open',
						action: () => onRevealInFinder(device),
					},
					{ id: 'divider-1', label: '', divider: true },
					{
						id: 'eject',
						label: get(translate)('devices.eject'),
						icon: 'eject',
						action: () => onEject(device),
					},
				]
			: []
	)
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} />
