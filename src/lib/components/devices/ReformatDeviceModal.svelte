<script lang="ts">
	import type { UsbDevice } from '$lib/types'
	import { translate } from '$lib/i18n'
	import Modal from '$lib/components/common/Modal.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		open: boolean
		device: UsbDevice | null
		onSubmit: (volumeName: string) => void
		onClose: () => void
	}

	let { open, device, onSubmit, onClose }: Props = $props()

	let volumeName = $state('')
	let inputEl: HTMLInputElement | undefined = $state()

	const maxLength = 11
	const isValid = $derived(volumeName.trim().length > 0 && volumeName.trim().length <= maxLength)
	const charCount = $derived(volumeName.length)

	// Reset and focus when modal opens
	$effect(() => {
		if (open && device) {
			volumeName = device.name.substring(0, maxLength)
			setTimeout(() => inputEl?.focus(), 50)
		}
	})

	function handleSubmit() {
		if (isValid) {
			onSubmit(volumeName.trim())
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault()
			handleSubmit()
		}
	}

	function handleClose() {
		volumeName = ''
		onClose()
	}
</script>

<Modal {open} title={$translate('devices.reformat.title')} onClose={handleClose}>
	{#if device}
		<div class="space-y-4">
			<!-- Data Loss Warning -->
			<div class="rounded-md border border-red-500/20 bg-red-500/10 p-3">
				<div class="flex gap-2">
					<Icon name="warning" class="h-5 w-5 flex-shrink-0 text-red-500" />
					<div class="space-y-1">
						<Text variant="body-2" class="font-medium text-red-500">
							{$translate('devices.reformat.warning')}
						</Text>
						<Text variant="body-2" class="text-red-400">
							{$translate('devices.reformat.warningDetails', { values: { deviceName: device.name } })}
						</Text>
					</div>
				</div>
			</div>

			<!-- Volume Name Input -->
			<div class="space-y-2">
				<label for="volume-name" class="block text-sm font-medium text-text-primary">
					{$translate('devices.reformat.volumeName')}
				</label>
				<input
					id="volume-name"
					type="text"
					bind:this={inputEl}
					bind:value={volumeName}
					placeholder={$translate('devices.reformat.volumeNamePlaceholder')}
					maxlength={maxLength}
					onkeydown={handleKeydown}
					class="w-full rounded-md border border-stroke bg-surface-2 px-3 py-2 text-text-primary placeholder-text-tertiary focus:border-transparent focus:ring-2 focus:ring-brand-primary focus:outline-none"
				/>
				<div class="flex justify-between text-xs text-text-tertiary">
					<span>{$translate('devices.reformat.fat32Hint')}</span>
					<span class:text-red-500={charCount > maxLength}>{charCount}/{maxLength}</span>
				</div>
			</div>
		</div>
	{/if}

	{#snippet footer()}
		<Button variant="secondary" onclick={handleClose}>{$translate('common.cancel')}</Button>
		<Button variant="primary" class="bg-red-600 hover:bg-red-700" onclick={handleSubmit} disabled={!isValid}>
			{$translate('devices.reformat.confirm')}
		</Button>
	{/snippet}
</Modal>
