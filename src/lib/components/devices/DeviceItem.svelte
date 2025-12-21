<script lang="ts">
	import { slide } from 'svelte/transition'
	import type { UsbDevice } from '$lib/types'
	import { translate } from '$lib/i18n'
	import { formatFileSize } from '$lib/utils'
	import Icon from '$lib/components/common/Icon.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import DeviceStatusIndicator from './DeviceStatusIndicator.svelte'
	import { deviceItem } from '$lib/transitions'
	import { exportProgress, exportProgressPercent, playlistCount } from '$lib/stores/export'

	type Props = {
		device: UsbDevice
		isReformatting?: boolean
		isExporting?: boolean
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
		onCancelExport?: () => void
	}

	let { device, isReformatting = false, isExporting = false, onContextMenu, onCancelExport }: Props = $props()

	// Generate tooltip text for progress display
	const tooltipText = $derived(
		$exportProgress && $playlistCount
			? $translate('export.transferringProgress', {
					values: {
						trackCount: $exportProgress.files_total,
						playlistCount: $playlistCount,
					},
				})
			: ''
	)
</script>

<div
	class="rounded px-3 py-2 text-text-secondary transition-colors hover:bg-surface-2 hover:text-text-primary"
	transition:deviceItem={{ duration: 200 }}
	oncontextmenu={(e) => {
		e.preventDefault()
		onContextMenu?.(e, device)
	}}
	role="button"
	tabindex="0"
>
	<!-- Main row: USB Icon, device info, status indicator -->
	<div class="flex items-center gap-2">
		<Icon name="usb" class="h-4 w-4 shrink-0" />

		<div class="min-w-0 flex-1">
			<div class="truncate text-sm font-medium">{device.name}</div>
			<div class="text-xs text-text-tertiary">
				{formatFileSize(device.available_space_bytes)}
				{$translate('devices.free')}
			</div>
		</div>

		<!-- Status indicator or spinner during reformat/export -->
		{#if isExporting || isReformatting}
			<Icon name="refresh" class="h-4 w-4 shrink-0 animate-spin text-text-tertiary" />
		{:else}
			<DeviceStatusIndicator fileSystem={device.file_system} />
		{/if}
	</div>

	<!-- Inline export progress section -->
	{#if isExporting && $exportProgress}
		<div class="mt-2 flex items-center gap-2" transition:slide={{ duration: 200 }}>
			<!-- Progress bar -->
			<div class="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-2">
				<div
					class="h-full rounded-full bg-brand-primary transition-[width] duration-200 ease-out"
					style="width: {$exportProgressPercent}%"
				></div>
			</div>

			<!-- Track count with tooltip -->
			<Tooltip text={tooltipText} position="bottom">
				<span class="text-xs whitespace-nowrap text-text-secondary">
					{$exportProgress.files_copied} / {$exportProgress.files_total}
				</span>
			</Tooltip>

			<!-- Cancel button -->
			<button
				class="rounded p-0.5 text-text-tertiary transition-colors hover:bg-surface-2 hover:text-text-secondary"
				onclick={(e) => {
					e.stopPropagation()
					onCancelExport?.()
				}}
				aria-label={$translate('export.cancel')}
			>
				<Icon name="close" class="h-3 w-3" />
			</button>
		</div>
	{/if}
</div>
