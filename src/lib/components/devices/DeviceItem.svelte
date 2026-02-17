<script lang="ts">
	import { slide, fade } from 'svelte/transition'
	import type { UsbDevice } from '$lib/types'
	import { translate } from '$lib/i18n'
	import { formatFileSize } from '$lib/utils'
	import Icon from '$lib/components/common/Icon.svelte'
	import IconButton from '$lib/components/common/IconButton.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import Checkbox from '$lib/components/common/Checkbox.svelte'
	import DeviceStatusIndicator from './DeviceStatusIndicator.svelte'
	import { deviceItem } from '$lib/transitions'
	import { exportProgress, exportProgressPercent, playlistCount } from '$lib/stores/export'

	type Props = {
		device: UsbDevice
		isReformatting?: boolean
		isExporting?: boolean
		isSyncing?: boolean
		selectable?: boolean
		selected?: boolean
		disabled?: boolean
		isDragHovered?: boolean
		ignored?: boolean
		isConnected?: boolean
		onContextMenu?: (e: MouseEvent, device: UsbDevice) => void
		onCancelExport?: () => void
		onSelect?: (deviceId: string) => void
		onUnignore?: () => void
	}

	let {
		device,
		isReformatting = false,
		isExporting = false,
		isSyncing = false,
		selectable = false,
		selected = false,
		disabled = false,
		isDragHovered = false,
		ignored = false,
		isConnected = true,
		onContextMenu,
		onCancelExport,
		onSelect,
		onUnignore,
	}: Props = $props()

	// Track success state for linger effect (export)
	let showSuccess = $state(false)
	let showProgress = $state(false)
	let wasExporting = $state(false)
	let visuallyExporting = $state(false) // Stays true during linger period for fast exports
	let exportStartTime = $state(0)
	let displayPercent = $state(0)
	let lastProgressSnapshot = $state<{ files_copied: number; files_total: number } | null>(null)
	let lastTrackName = $state<string | null>(null)

	// Track success state for sync (no progress bar, just spinner → checkmark)
	let showSyncSuccess = $state(false)
	let wasSyncing = $state(false)
	let syncStartTime = $state(0)

	const MIN_ANIMATION_DURATION = 800 // Minimum time for progress bar to animate 0%→100%
	const PROGRESS_ANIMATION_DURATION = 300 // CSS transition duration for progress bar
	const SUCCESS_DISPLAY_DURATION = 1200 // How long checkmark stays visible

	// Continuously track progress data while exporting so we have a valid snapshot when export completes
	// This is necessary because the store clears progress immediately when export completes
	// Also sync displayPercent to prevent stutter when switching from real progress to displayPercent
	$effect(() => {
		if ($exportProgress) {
			lastProgressSnapshot = {
				files_copied: $exportProgress.files_copied,
				files_total: $exportProgress.files_total,
			}
			displayPercent = $exportProgressPercent

			// Capture track name for linger period
			if ($exportProgress.current_file) {
				const path = $exportProgress.current_file
				const filename = path.split('/').pop() || path.split('\\').pop() || path
				const lastDot = filename.lastIndexOf('.')
				lastTrackName = lastDot > 0 ? filename.slice(0, lastDot) : filename
			}
		}
	})

	// Detect when export starts/completes to manage UI timing
	$effect(() => {
		if (!wasExporting && isExporting) {
			// Export just started - show progress immediately
			exportStartTime = Date.now()
			showProgress = true
			visuallyExporting = true
			displayPercent = 0
			lastProgressSnapshot = null
			lastTrackName = null
		} else if (wasExporting && !isExporting) {
			// Export just finished - calculate if we need to linger
			const elapsed = Date.now() - exportStartTime
			// Ensure minimum animation time (subtract animation duration since it's handled in completeExport)
			const remainingTime = Math.max(0, MIN_ANIMATION_DURATION - elapsed - PROGRESS_ANIMATION_DURATION)

			const completeExport = () => {
				// Update snapshot to show 100% completion
				if (lastProgressSnapshot) {
					lastProgressSnapshot = {
						files_copied: lastProgressSnapshot.files_total,
						files_total: lastProgressSnapshot.files_total,
					}
				}

				// Animate progress to 100%
				displayPercent = 100

				// Wait for progress bar animation to complete, THEN swap spinner → checkmark
				setTimeout(() => {
					visuallyExporting = false
					showSuccess = true

					setTimeout(() => {
						showSuccess = false
						showProgress = false
						lastProgressSnapshot = null
						lastTrackName = null
					}, SUCCESS_DISPLAY_DURATION)
				}, PROGRESS_ANIMATION_DURATION)
			}

			if (remainingTime > 0) {
				// Fast export: keep UI visible until minimum duration is met
				setTimeout(completeExport, remainingTime)
			} else {
				// Normal export: complete immediately
				completeExport()
			}
		}
		wasExporting = isExporting
	})

	// Detect when sync starts/completes to manage UI timing (simpler than export - no progress bar)
	$effect(() => {
		if (!wasSyncing && isSyncing) {
			// Sync just started
			syncStartTime = Date.now()
		} else if (wasSyncing && !isSyncing) {
			// Sync just finished - show success with minimum duration
			const elapsed = Date.now() - syncStartTime
			const remainingTime = Math.max(0, MIN_ANIMATION_DURATION - elapsed)

			setTimeout(() => {
				showSyncSuccess = true
				setTimeout(() => {
					showSyncSuccess = false
				}, SUCCESS_DISPLAY_DURATION)
			}, remainingTime)
		}
		wasSyncing = isSyncing
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

	// Display track name from progress (already formatted as "Artist - Title" by backend)
	const currentTrackName = $derived(() => {
		if ($exportProgress?.current_file) {
			return $exportProgress.current_file
		}
		// Use snapshot during linger period when store is cleared
		return lastTrackName
	})
</script>

<div
	data-drop-target="device-{device.id}"
	class="rounded px-3 py-2 text-text-secondary transition-colors {selectable
		? disabled
			? 'cursor-not-allowed border border-transparent opacity-50'
			: 'cursor-pointer border hover:bg-surface-2 hover:text-text-primary ' +
				(selected ? 'border-brand-primary bg-brand-primary-10' : 'border-transparent')
		: 'cursor-default hover:bg-surface-2 hover:text-text-primary'} {isDragHovered
		? 'bg-brand-primary-5 ring-2 ring-brand-primary'
		: ''}"
	transition:deviceItem={{ duration: 200 }}
	onclick={() => {
		if (selectable && !disabled) {
			onSelect?.(device.id)
		}
	}}
	onkeydown={(e) => {
		if (selectable && !disabled && (e.key === 'Enter' || e.key === ' ')) {
			e.preventDefault()
			onSelect?.(device.id)
		}
	}}
	oncontextmenu={(e) => {
		e.preventDefault()
		onContextMenu?.(e, device)
	}}
	role="button"
	tabindex="0"
>
	<!-- Main row: Checkbox (if selectable), USB Icon, device info, status indicator -->
	<div class="flex items-center gap-2">
		{#if selectable}
			<Checkbox
				checked={selected}
				{disabled}
				onchange={() => onSelect?.(device.id)}
				ariaLabel={$translate('devices.selectDevice')}
			/>
		{/if}
		<Icon name="usb" class="h-4 w-4 shrink-0" />

		<div class="min-w-0 flex-1">
			<div class="truncate text-sm font-medium">{device.name}</div>
			{#if isConnected}
				<div class="text-xs text-text-tertiary">
					{formatFileSize(device.available_space_bytes)}
					{$translate('devices.free')}
				</div>
			{:else}
				<div class="text-xs text-text-tertiary">
					{$translate('settings.library.deviceNotConnected')}
				</div>
			{/if}
		</div>

		<!-- Status indicator (always shown) -->
		<div class="relative h-4 w-4 shrink-0">
			{#if showSuccess || showSyncSuccess}
				<div class="absolute inset-0 flex items-center justify-center" transition:fade={{ duration: 150 }}>
					<Icon name="check" class="h-4 w-4 text-success" />
				</div>
			{:else if visuallyExporting || isReformatting || isSyncing}
				<div class="absolute inset-0 flex items-center justify-center" transition:fade={{ duration: 150 }}>
					<Spinner />
				</div>
			{:else}
				<div class="absolute inset-0 flex items-center justify-center" transition:fade={{ duration: 150 }}>
					<DeviceStatusIndicator fileSystem={device.file_system} />
				</div>
			{/if}
		</div>

		<!-- Unignore button (only for ignored devices) -->
		{#if ignored}
			<!--			<div class="z-10 shrink-0">-->
			<Tooltip text={$translate('devices.unignore')} delay={250} position="left">
				<IconButton
					icon="x"
					size="sm"
					onclick={(e) => {
						e.stopPropagation()
						onUnignore?.()
					}}
				/>
			</Tooltip>
			<!--			</div>-->
		{/if}
	</div>

	<!-- Inline export progress section (visible during export and success linger, hidden in selectable mode) -->
	{#if !selectable && showProgress && effectiveProgress}
		<div class="mt-2 flex flex-col gap-1" transition:slide={{ duration: 200 }}>
			<!-- Track name and progress counter row -->
			<div class="flex items-center justify-between gap-2">
				<span class="min-w-0 flex-1 truncate text-xs text-text-tertiary" transition:fade={{ duration: 150 }}>
					{currentTrackName() || $translate('export.exporting')}
				</span>
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
						class="h-full rounded-full bg-brand-primary transition-[width] ease-out"
						style="width: {effectivePercent}%; transition-duration: {PROGRESS_ANIMATION_DURATION}ms"
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
