<script lang="ts">
	import type { UsbDevice } from '$lib/types'
	import DeviceItem from './DeviceItem.svelte'
	import { Text } from '$lib/components/common'

	type Props = {
		devices: UsbDevice[]
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
	}

	let { devices, onContextMenu }: Props = $props()
</script>

<div class="border-b border-stroke p-2">
	<div class="flex items-center px-3 py-1.5">
		<Text variant="header-4">Devices</Text>
		<Text variant="caption" class="ml-auto">{devices.length}</Text>
	</div>
	{#if devices.length > 0}
		<div class="space-y-0.5">
			{#each devices as device (device.id)}
				<DeviceItem {device} {onContextMenu} />
			{/each}
		</div>
	{:else}
		<Text variant="caption" as="div" italic class="px-3 py-2">No devices connected</Text>
	{/if}
</div>
