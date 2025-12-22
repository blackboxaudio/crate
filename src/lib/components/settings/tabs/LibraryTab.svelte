<script lang="ts">
	import type { KeyNotationFormat } from '$lib/types'
	import { Text, Checkbox, Icon } from '$lib/components/common'
	import {
		settingsStore,
		keyNotationFormat,
		autoAnalyzeOnImport,
		autoSyncOnConnect,
		autoSyncOnChange,
		ignoredDeviceIds,
	} from '$lib/stores/settings'
	import { devices } from '$lib/stores/devices'
	import { translate } from '$lib/i18n'

	function handleKeyNotationFormatChange(format: KeyNotationFormat) {
		settingsStore.setKeyNotationFormat(format)
	}

	function handleAutoAnalyzeOnImportChange(checked: boolean) {
		settingsStore.setAutoAnalyzeOnImport(checked)
	}

	function handleAutoSyncOnConnectChange(checked: boolean) {
		settingsStore.setAutoSyncOnConnect(checked)
	}

	function handleAutoSyncOnChangeChange(checked: boolean) {
		settingsStore.setAutoSyncOnChange(checked)
	}

	function handleUnignoreDevice(deviceId: string) {
		settingsStore.unignoreDevice(deviceId)
	}

	// Map ignored device IDs to device info (name if connected, otherwise just ID)
	const ignoredDevicesWithInfo = $derived(
		$ignoredDeviceIds.map((id) => {
			const connectedDevice = $devices.find((d) => d.id === id)
			return {
				id,
				name: connectedDevice?.name || null,
				isConnected: !!connectedDevice,
			}
		})
	)
</script>

<div class="space-y-8">
	<!-- Key Notation Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.library.keyNotation')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.library.keyNotationDescription')}</Text>
		<div class="flex gap-3">
			<button
				type="button"
				class="flex flex-1 flex-col items-center gap-2 rounded-lg border-2 p-4
				transition-colors {$keyNotationFormat === 'camelot'
					? 'border-brand-primary bg-brand-muted'
					: 'border-stroke hover:cursor-pointer hover:border-text-tertiary'}"
				onclick={() => handleKeyNotationFormatChange('camelot')}
			>
				<Text variant="body-2" as="span">{$translate('settings.library.keyNotationCamelot')}</Text>
				<Text variant="caption" color="secondary">8A, 8B, 11A</Text>
			</button>
			<button
				type="button"
				class="flex flex-1 flex-col items-center gap-2 rounded-lg border-2 p-4
				transition-colors {$keyNotationFormat === 'standard'
					? 'border-brand-primary bg-brand-muted'
					: 'border-stroke hover:cursor-pointer hover:border-text-tertiary'}"
				onclick={() => handleKeyNotationFormatChange('standard')}
			>
				<Text variant="body-2" as="span">{$translate('settings.library.keyNotationStandard')}</Text>
				<Text variant="caption" color="secondary">Am, C, F#m</Text>
			</button>
		</div>
	</section>

	<!-- Analysis Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.library.analysis')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.library.autoAnalyzeOnImportDescription')}</Text>

		<!-- Auto-analyze on Import -->
		<Checkbox
			checked={$autoAnalyzeOnImport}
			onchange={handleAutoAnalyzeOnImportChange}
			label={$translate('settings.library.autoAnalyzeOnImport')}
		/>
	</section>

	<!-- Sync Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.library.sync')}</Text>
		<Text variant="caption" as="p" class="mb-4">{$translate('settings.library.syncDescription')}</Text>

		<div class="space-y-3">
			<div>
				<Checkbox
					checked={$autoSyncOnConnect}
					onchange={handleAutoSyncOnConnectChange}
					label={$translate('settings.library.autoSyncOnConnect')}
				/>
				<Text variant="caption" as="p" class="mt-1 ml-6 text-text-tertiary">
					{$translate('settings.library.autoSyncOnConnectDescription')}
				</Text>
			</div>

			<div>
				<Checkbox
					checked={$autoSyncOnChange}
					onchange={handleAutoSyncOnChangeChange}
					label={$translate('settings.library.autoSyncOnChange')}
				/>
				<Text variant="caption" as="p" class="mt-1 ml-6 text-text-tertiary">
					{$translate('settings.library.autoSyncOnChangeDescription')}
				</Text>
			</div>
		</div>
	</section>

	<!-- Ignored Devices Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.library.ignoredDevices')}</Text>
		<Text variant="caption" as="p" class="mb-4">{$translate('settings.library.ignoredDevicesDescription')}</Text>

		{#if ignoredDevicesWithInfo.length === 0}
			<Text variant="caption" class="text-text-tertiary italic">
				{$translate('settings.library.noIgnoredDevices')}
			</Text>
		{:else}
			<div class="space-y-2">
				{#each ignoredDevicesWithInfo as device (device.id)}
					<div class="flex items-center justify-between rounded border border-stroke px-3 py-2">
						<div class="flex items-center gap-2">
							<Icon name="usb" class="h-4 w-4 text-text-tertiary" />
							<div class="flex flex-col">
								{#if device.name}
									<Text variant="body-2">{device.name}</Text>
								{:else}
									<Text variant="body-2" class="font-mono text-sm">{device.id}</Text>
								{/if}
								{#if !device.isConnected}
									<Text variant="caption" class="text-text-tertiary">
										{$translate('settings.library.deviceNotConnected')}
									</Text>
								{/if}
							</div>
						</div>
						<button
							type="button"
							class="rounded p-1 text-text-tertiary hover:bg-surface-2 hover:text-text-primary"
							onclick={() => handleUnignoreDevice(device.id)}
							aria-label={$translate('common.remove')}
						>
							<Icon name="close" class="h-4 w-4" />
						</button>
					</div>
				{/each}
			</div>
		{/if}
	</section>
</div>
