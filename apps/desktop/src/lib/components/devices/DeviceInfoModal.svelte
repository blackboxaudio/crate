<script lang="ts">
	import type { UsbDevice } from '$shared/types'
	import { formatFileSize } from '$shared/utils'
	import { translate } from '$shared/i18n'
	import Modal from '$lib/components/common/Modal.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Text from '$lib/components/common/Text.svelte'
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

<Modal {open} title={$translate('modals.deviceInfo.title')} {onClose}>
	{#if device}
		<div class="space-y-4">
			<!-- Device Header -->
			<div class="flex items-center gap-3">
				<Icon name="usb" class="h-8 w-8 text-text-secondary" />
				<div>
					<Text variant="header-1" as="h3">{device.name}</Text>
					<Text color="tertiary" variant="code">{device.mount_point}</Text>
				</div>
			</div>

			<!-- Info Grid -->
			<div class="grid grid-cols-2 gap-4 rounded-lg bg-surface-2 p-4">
				<div>
					<Text variant="header-4" as="p">{$translate('modals.deviceInfo.diskType')}</Text>
					<Text variant="body-2">{device.disk_kind}</Text>
				</div>
				<div>
					<Text variant="header-4" as="p">{$translate('modals.deviceInfo.fileSystem')}</Text>
					<Text variant="body-2">{device.file_system || $translate('common.unknown')}</Text>
				</div>
			</div>

			<!-- Storage Section -->
			<div class="space-y-3">
				<Text variant="header-4" as="p">{$translate('modals.deviceInfo.storage')}</Text>

				<!-- Progress Bar -->
				<div class="h-3 w-full overflow-hidden rounded-full border border-stroke-subtle bg-surface-2">
					<div class="h-full transition-all duration-300 {progressColor}" style="width: {usagePercentage}%"></div>
				</div>

				<!-- Storage Breakdown -->
				<div class="flex justify-between">
					<div>
						<Text size="sm" color="secondary" as="span">{$translate('modals.deviceInfo.used')}</Text>
						<Text size="sm" weight="medium" as="span" class="ml-1">
							{formatFileSize(usedSpace)}
						</Text>
					</div>
					<div>
						<Text size="sm" color="secondary" as="span">{$translate('modals.deviceInfo.available')}</Text>
						<Text size="sm" weight="medium" as="span" class="ml-1">
							{formatFileSize(device.available_space_bytes)}
						</Text>
					</div>
				</div>

				<!-- Total capacity -->
				<div class="text-center">
					<Text size="sm" color="secondary" as="span">{$translate('modals.deviceInfo.total')}</Text>
					<Text size="sm" weight="medium" as="span" class="ml-1">
						{formatFileSize(device.total_space_bytes)}
					</Text>
					<Text size="sm" color="tertiary" as="span" class="ml-1"
						>({usagePercentage}{$translate('modals.deviceInfo.percentUsed')})</Text
					>
				</div>
			</div>
		</div>
	{/if}

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>{$translate('common.close')}</Button>
	{/snippet}
</Modal>
