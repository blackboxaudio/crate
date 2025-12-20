<script lang="ts">
	import type { UsbDevice } from '$lib/types'
	import { formatFileSize } from '$lib/utils'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		device: UsbDevice
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
	}

	let { device, onContextMenu }: Props = $props()
</script>

<div
	class="flex cursor-pointer items-center gap-2 rounded px-3 py-2 text-text-secondary transition-colors hover:bg-surface-2 hover:text-text-primary"
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
			{formatFileSize(device.available_space_bytes)} free
		</div>
	</div>

	<!-- Connected indicator -->
	<span class="relative flex size-2" title="Connected">
		<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75"></span>
		<span class="relative inline-flex size-2 rounded-full bg-emerald-500"></span>
	</span>
</div>
