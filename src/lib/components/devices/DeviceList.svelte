<script lang="ts">
	import type { UsbDevice } from '$lib/types'
	import DeviceItem from './DeviceItem.svelte'

	type Props = {
		devices: UsbDevice[]
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
	}

	let { devices, onContextMenu }: Props = $props()
</script>

{#if devices.length > 0}
	<div class="border-b border-stroke pb-2">
		<div class="px-3 py-1.5 text-xs font-medium tracking-wide text-text-tertiary uppercase">Devices</div>
		<div class="space-y-0.5">
			{#each devices as device (device.id)}
				<DeviceItem {device} {onContextMenu} />
			{/each}
		</div>
	</div>
{/if}
