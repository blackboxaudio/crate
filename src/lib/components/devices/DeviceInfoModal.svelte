<script lang="ts">
	import type { UsbDevice } from '$lib/types'
	import { formatFileSize } from '$lib/utils'
	import Modal from '$lib/components/common/Modal.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		open: boolean
		device: UsbDevice | null
		onClose: () => void
	}

	let { open, device, onClose }: Props = $props()

	const usedSpace = $derived(device ? device.total_space_bytes - device.available_space_bytes : 0)

	const usagePercentage = $derived(
		device && device.total_space_bytes > 0 ? Math.round((usedSpace / device.total_space_bytes) * 100) : 0
	)

	const progressColor = $derived.by(() => {
		if (usagePercentage >= 90) return 'bg-red-500'
		if (usagePercentage >= 75) return 'bg-amber-500'
		return 'bg-brand-primary'
	})
</script>

<Modal {open} title="Device Info" {onClose}>
	{#if device}
		<div class="space-y-4">
			<!-- Device Header -->
			<div class="flex items-center gap-3">
				<Icon name="usb" class="h-8 w-8 text-text-secondary" />
				<div>
					<h3 class="text-lg font-semibold text-text-primary">{device.name}</h3>
					<p class="text-sm text-text-tertiary">{device.mount_point}</p>
				</div>
			</div>

			<!-- Info Grid -->
			<div class="grid grid-cols-2 gap-4 rounded-lg bg-surface-2 p-4">
				<div>
					<p class="text-xs font-medium text-text-tertiary uppercase">File System</p>
					<p class="text-sm font-medium text-text-primary">{device.file_system || 'Unknown'}</p>
				</div>
				<div>
					<p class="text-xs font-medium text-text-tertiary uppercase">Disk Type</p>
					<p class="text-sm font-medium text-text-primary">{device.disk_kind}</p>
				</div>
			</div>

			<!-- Storage Section -->
			<div class="space-y-3">
				<p class="text-xs font-medium text-text-tertiary uppercase">Storage</p>

				<!-- Progress Bar -->
				<div class="h-3 w-full overflow-hidden rounded-full border border-stroke-subtle bg-surface-2">
					<div class="h-full transition-all duration-300 {progressColor}" style="width: {usagePercentage}%"></div>
				</div>

				<!-- Storage Breakdown -->
				<div class="flex justify-between text-sm">
					<div>
						<span class="text-text-secondary">Used:</span>
						<span class="ml-1 font-medium text-text-primary">
							{formatFileSize(usedSpace)}
						</span>
					</div>
					<div>
						<span class="text-text-secondary">Available:</span>
						<span class="ml-1 font-medium text-text-primary">
							{formatFileSize(device.available_space_bytes)}
						</span>
					</div>
				</div>

				<!-- Total capacity -->
				<div class="text-center text-sm">
					<span class="text-text-secondary">Total:</span>
					<span class="ml-1 font-medium text-text-primary">
						{formatFileSize(device.total_space_bytes)}
					</span>
					<span class="ml-1 text-text-tertiary">({usagePercentage}% used)</span>
				</div>
			</div>
		</div>
	{/if}

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>Close</Button>
	{/snippet}
</Modal>
