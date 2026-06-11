<script lang="ts">
	import type { UsbDevice, ContextMenuItem } from '$shared/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$shared/i18n'
	import { get } from 'svelte/store'

	type Props = {
		open: boolean
		x: number
		y: number
		device: UsbDevice | null
		isReformatting?: boolean
		isExporting?: boolean
		onClose: () => void
		onClosed?: () => void
		onExport: (device: UsbDevice) => void
		onViewInfo: (device: UsbDevice) => void
		onRevealInFinder: (device: UsbDevice) => void
		onReformat: (device: UsbDevice) => void
		onEject: (device: UsbDevice) => void
		onIgnore: (device: UsbDevice) => void
	}

	let {
		open,
		x,
		y,
		device,
		isReformatting = false,
		isExporting = false,
		onClose,
		onClosed,
		onExport,
		onViewInfo,
		onRevealInFinder,
		onReformat,
		onEject,
		onIgnore,
	}: Props = $props()

	const isDeviceBusy = $derived(isReformatting || isExporting)

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
						id: 'export',
						label: get(translate)('devices.exportTo'),
						icon: 'arrow-up-from-bracket',
						disabled: isDeviceBusy,
						action: () => onExport(device),
					},
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
						id: 'reformat',
						label: get(translate)('devices.reformat.menuItem'),
						icon: 'hard-drive',
						disabled: isDeviceBusy,
						action: () => onReformat(device),
					},
					{
						id: 'ignore',
						label: get(translate)('devices.ignore'),
						icon: 'eye-slash',
						disabled: isDeviceBusy,
						action: () => onIgnore(device),
					},
					{
						id: 'eject',
						label: get(translate)('devices.eject'),
						icon: 'eject',
						disabled: isDeviceBusy,
						action: () => onEject(device),
					},
				]
			: []
	)
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} {onClosed} />
