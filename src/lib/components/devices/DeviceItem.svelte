<script lang="ts">
	import type { UsbDevice } from '$lib/types'
	import { formatFileSize } from '$lib/utils'

	type Props = {
		device: UsbDevice
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
	}

	let { device, onContextMenu }: Props = $props()
</script>

<div
	class="flex items-center gap-2 rounded px-3 py-2 text-text-secondary transition-colors hover:bg-surface-2/50 hover:text-text-primary"
	oncontextmenu={(e) => {
		e.preventDefault()
		onContextMenu?.(e, device)
	}}
	role="button"
	tabindex="0"
>
	<!-- USB Icon -->
	<svg class="h-4 w-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
		<path
			stroke-linecap="round"
			stroke-linejoin="round"
			stroke-width="2"
			d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z"
		/>
	</svg>

	<div class="min-w-0 flex-1">
		<div class="truncate text-sm font-medium">{device.name}</div>
		<div class="text-xs text-text-tertiary">
			{formatFileSize(device.available_space_bytes)} free
		</div>
	</div>

	<!-- Connected indicator -->
	<div class="h-2 w-2 shrink-0 rounded-full bg-emerald-500" title="Connected"></div>
</div>
