<script lang="ts">
	import type { UsbDevice } from '$lib/types'
	import DeviceItem from './DeviceItem.svelte'
	import { Text } from '$lib/components/common'
	import { translate } from '$lib/i18n'

	type Props = {
		devices: UsbDevice[]
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
	}

	let { devices, onContextMenu }: Props = $props()
</script>

<div class="p-2 pt-0">
	<div class="flex items-center px-3 pb-1.5">
		<Text variant="header-4">{$translate('devices.title')}</Text>
		<Text variant="caption" class="ml-auto">{devices.length}</Text>
	</div>
	{#if devices.length > 0}
		<div class="space-y-0.5">
			{#each devices as device (device.id)}
				<DeviceItem {device} {onContextMenu} />
			{/each}
		</div>
	{:else}
		<Text variant="caption" as="div" italic class="px-3 py-2">{$translate('devices.noDevices')}</Text>
	{/if}
</div>
