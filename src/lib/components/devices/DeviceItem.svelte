<script lang="ts">
	import { scale } from 'svelte/transition'
	import type { UsbDevice } from '$lib/types'
	import { translate } from '$lib/i18n'
	import { formatFileSize } from '$lib/utils'
	import Icon from '$lib/components/common/Icon.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import DeviceStatusIndicator from './DeviceStatusIndicator.svelte'
	import { deviceItem } from '$lib/transitions'
	import { exportProgress, exportProgressPercent, exportStatusLabel } from '$lib/stores/export'

	type Props = {
		device: UsbDevice
		isReformatting?: boolean
		isExporting?: boolean
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
		onCancelExport?: () => void
	}

	let { device, isReformatting = false, isExporting = false, onContextMenu, onCancelExport }: Props = $props()

	const MINIMUM_POPUP_DURATION = 3000 // 3 seconds

	let showExportPopup = $state(false)
	let forceVisible = $state(false) // Force visibility during minimum duration

	// Auto-show popup when export starts with guaranteed minimum duration
	$effect(() => {
		if (isExporting) {
			// Export started - force visibility for minimum duration
			showExportPopup = true
			forceVisible = true

			// After minimum duration, allow normal hide behavior
			setTimeout(() => {
				forceVisible = false
			}, MINIMUM_POPUP_DURATION)
		}
	})

	// Handle mouse leave - only hide if not in forced visibility period
	function handleMouseLeave() {
		if (!forceVisible) {
			showExportPopup = false
		}
	}

	// Handle mouse enter
	function handleMouseEnter() {
		if (isExporting || forceVisible) {
			showExportPopup = true
		}
	}

	// Truncate filename to max length
	function truncateFilename(filename: string | null, maxLength = 30): string {
		if (!filename) return ''
		if (filename.length <= maxLength) return filename
		return '...' + filename.slice(-(maxLength - 3))
	}
</script>

<div
	class="flex items-center gap-2 rounded px-3 py-2 text-text-secondary transition-colors hover:bg-surface-2 hover:text-text-primary"
	transition:deviceItem={{ duration: 200 }}
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

	<!-- Status indicator or spinner during reformat/export -->
	{#if isExporting || isReformatting || forceVisible}
		<div class="relative" onmouseenter={handleMouseEnter} onmouseleave={handleMouseLeave}>
			<Icon name="refresh" class="h-4 w-4 shrink-0 animate-spin text-text-tertiary" />

			<!-- Export progress popup -->
			{#if (isExporting || forceVisible) && showExportPopup && $exportProgress}
				<div class="absolute top-full left-0 z-[200] pt-1" transition:scale={{ start: 0.95, duration: 200 }}>
					<div class="min-w-[280px] rounded-md border border-stroke bg-surface-1 p-3 shadow-lg">
						<!-- Header -->
						<div class="mb-2 flex items-center justify-between">
							<span class="text-sm font-semibold text-text-primary">
								{$translate('export.exportingTo', { values: { deviceName: device.name } })}
							</span>
							<Button variant="ghost" size="sm" onclick={() => onCancelExport?.()}>
								{$translate('export.cancel')}
							</Button>
						</div>

						<!-- Status and file count -->
						<div class="mb-2 flex items-center justify-between text-xs">
							<span class="text-text-secondary">{$exportStatusLabel}</span>
							<span class="font-medium text-text-primary">
								{$exportProgress.files_copied} / {$exportProgress.files_total}
							</span>
						</div>

						<!-- Progress bar -->
						<div class="mb-2 h-1.5 overflow-hidden rounded-full bg-surface-2">
							<div
								class="h-full rounded-full bg-brand-primary transition-[width] duration-200"
								style="width: {$exportProgressPercent}%"
							></div>
						</div>

						<!-- Current file -->
						{#if $exportProgress.current_file}
							<div class="truncate text-xs text-text-tertiary">
								{truncateFilename($exportProgress.current_file)}
							</div>
						{/if}
					</div>
				</div>
			{/if}
		</div>
	{:else}
		<DeviceStatusIndicator fileSystem={device.file_system} />
	{/if}
</div>
