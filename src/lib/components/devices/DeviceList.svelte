<script lang="ts">
	import type { UsbDevice } from '$lib/types'
	import DeviceItem from './DeviceItem.svelte'

	type Props = {
		devices: UsbDevice[]
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
	}

	let { devices, onContextMenu }: Props = $props()
</script>

<div class="border-b border-stroke p-2">
	<div class="flex items-center px-3 py-1.5">
		<span class="text-xs font-medium tracking-wide text-text-tertiary uppercase">Devices</span>
		<span class="ml-auto text-xs text-text-tertiary">{devices.length}</span>
	</div>
	{#if devices.length > 0}
		<div class="space-y-0.5">
			{#each devices as device (device.id)}
				<DeviceItem {device} {onContextMenu} />
			{/each}
		</div>
	{:else}
		<div class="px-3 py-2 text-xs text-text-tertiary italic">No devices connected</div>
	{/if}
</div>
