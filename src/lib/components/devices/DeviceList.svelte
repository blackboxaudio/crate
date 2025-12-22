<script lang="ts">
	import { fade } from 'svelte/transition'
	import type { UsbDevice } from '$lib/types'
	import DeviceItem from './DeviceItem.svelte'
	import { Text } from '$lib/components/common'
	import { translate } from '$lib/i18n'
	import { reformattingDeviceId } from '$lib/stores/devices'
	import { activeDeviceId } from '$lib/stores/export'
	import { syncingDeviceIds } from '$lib/stores/sync'
	import { hoveredDropTarget, isDraggingPlaylist } from '$lib/stores'

	type Props = {
		devices: UsbDevice[]
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
		onCancelExport?: () => void
	}

	let { devices, onContextMenu, onCancelExport }: Props = $props()
</script>

<div class="p-2 pt-0">
	<div class="flex items-center px-3 pb-1.5">
		<Text variant="header-4">{$translate('devices.title')}</Text>
		<Text variant="caption" class="ml-auto">{devices.length}</Text>
	</div>
	<div class="space-y-0.5">
		{#each devices as device (device.id)}
			<DeviceItem
				{device}
				isReformatting={device.id === $reformattingDeviceId}
				isExporting={device.id === $activeDeviceId}
				isSyncing={$syncingDeviceIds.includes(device.id)}
				isDragHovered={$isDraggingPlaylist && $hoveredDropTarget === `device-${device.id}`}
				{onContextMenu}
				{onCancelExport}
			/>
		{/each}
	</div>
	{#if devices.length === 0}
		<div in:fade={{ duration: 150 }}>
			<Text variant="caption" as="div" italic class="px-3 py-2">
				{$translate('devices.noDevices')}
			</Text>
		</div>
	{/if}
</div>
