<script lang="ts">
	import { slide, fade } from 'svelte/transition'
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

	// Track success state for linger effect
	let showSuccess = $state(false)
	let showProgress = $state(false)
	let wasExporting = $state(false)
	let exportStartTime = $state(0)
	let displayPercent = $state(0)
	let lastProgressSnapshot = $state<{ files_copied: number; files_total: number } | null>(null)

	const MIN_DISPLAY_DURATION = 500

	// Detect when export starts/completes to manage UI timing
	$effect(() => {
		if (!wasExporting && isExporting) {
			// Export just started - record start time and show progress
			exportStartTime = Date.now()
			showProgress = true
			displayPercent = 0
			lastProgressSnapshot = null
		} else if (wasExporting && !isExporting) {
			// Export just finished - snapshot the progress data before it clears
			if ($exportProgress) {
				lastProgressSnapshot = {
					files_copied: $exportProgress.files_total,
					files_total: $exportProgress.files_total,
				}
			}

			// Ensure minimum display duration before showing success
			const elapsed = Date.now() - exportStartTime
			const remainingTime = Math.max(0, MIN_DISPLAY_DURATION - elapsed)

			// Animate progress to 100% over the remaining time
			displayPercent = 100

			setTimeout(() => {
				showSuccess = true
				setTimeout(() => {
					showSuccess = false
					showProgress = false
					lastProgressSnapshot = null
				}, 1000)
			}, remainingTime)
		}
		wasExporting = isExporting
	})

	// Use real progress when exporting, animated progress otherwise
	const effectivePercent = $derived(isExporting ? $exportProgressPercent : displayPercent)
	const effectiveProgress = $derived($exportProgress ?? lastProgressSnapshot)

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

	// Extract filename without extension from current file path
	const currentTrackName = $derived(() => {
		if (!$exportProgress?.current_file) return null
		const path = $exportProgress.current_file
		const filename = path.split('/').pop() || path.split('\\').pop() || path
		const lastDot = filename.lastIndexOf('.')
		return lastDot > 0 ? filename.slice(0, lastDot) : filename
	})
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

		<!-- Status indicator: checkmark (success), spinner (exporting/reformatting), or status dot -->
		<div class="relative h-4 w-4 shrink-0">
			{#if showSuccess}
				<div class="absolute inset-0 flex items-center justify-center" transition:fade={{ duration: 150 }}>
					<Icon name="check" class="h-4 w-4 text-success" />
				</div>
			{:else if isExporting || isReformatting}
				<div class="absolute inset-0 flex items-center justify-center" transition:fade={{ duration: 150 }}>
					<Icon name="refresh" class="h-4 w-4 animate-spin text-text-tertiary" />
				</div>
			{:else}
				<div class="absolute inset-0 flex items-center justify-center" transition:fade={{ duration: 150 }}>
					<DeviceStatusIndicator fileSystem={device.file_system} />
				</div>
			{/if}
		</div>
	</div>

	<!-- Inline export progress section (visible during export and success linger) -->
	{#if showProgress && effectiveProgress}
		<div class="mt-2 flex flex-col gap-1" transition:slide={{ duration: 200 }}>
			<!-- Track name and progress counter row -->
			<div class="flex items-center justify-between gap-2">
				{#if isExporting && currentTrackName()}
					<span class="min-w-0 flex-1 truncate text-xs text-text-tertiary">
						{currentTrackName()}
					</span>
				{:else}
					<span></span>
				{/if}
				<Tooltip text={tooltipText} position="bottom">
					<span class="text-xs whitespace-nowrap text-text-secondary">
						{effectiveProgress.files_copied} / {effectiveProgress.files_total}
					</span>
				</Tooltip>
			</div>

			<!-- Progress bar row -->
			<div class="flex items-center gap-2">
				<div class="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-2">
					<div
						class="h-full rounded-full bg-brand-primary transition-[width] duration-300 ease-out"
						style="width: {effectivePercent}%"
					></div>
				</div>

				<!-- Cancel button (hidden during success state) -->
				{#if !showSuccess}
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
				{/if}
			</div>
		</div>
	{/if}
</div>
