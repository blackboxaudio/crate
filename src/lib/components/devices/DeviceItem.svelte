<script lang="ts">
	import type { UsbDevice } from '$lib/types'
	import { translate } from '$lib/i18n'
	import { formatFileSize } from '$lib/utils'
	import Icon from '$lib/components/common/Icon.svelte'
	import DeviceStatusIndicator from './DeviceStatusIndicator.svelte'

	type Props = {
		device: UsbDevice
		isReformatting?: boolean
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
	}

	let { device, isReformatting = false, onContextMenu }: Props = $props()
</script>

<div
	class="flex items-center gap-2 rounded px-3 py-2 text-text-secondary transition-colors hover:bg-surface-2 hover:text-text-primary"
	oncontextmenu={(e) => {
		e.preventDefault()
		onContextMenu?.(e, device)
	}}
	role="button"
	tabindex="0"
>
	<!-- USB Icon -->
	<Icon name="usb" class="h-4 w-4 shrink-0" />

	<div class="min-w-0 flex-1">
		<div class="truncate text-sm font-medium">{device.name}</div>
		<div class="text-xs text-text-tertiary">
			{formatFileSize(device.available_space_bytes)}
			{$translate('devices.free')}
		</div>
	</div>

	<!-- Status indicator or spinner during reformat -->
	{#if isReformatting}
		<Icon name="refresh" class="h-4 w-4 shrink-0 animate-spin text-text-tertiary" />
	{:else}
		<DeviceStatusIndicator fileSystem={device.file_system} />
	{/if}
</div>
