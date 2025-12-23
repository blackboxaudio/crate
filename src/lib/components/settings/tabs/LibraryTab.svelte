<script lang="ts">
	import type { KeyNotationFormat, ExportFormat, UsbDevice } from '$lib/types'
	import { Text, Checkbox } from '$lib/components/common'
	import DeviceItem from '$lib/components/devices/DeviceItem.svelte'
	import {
		settingsStore,
		keyNotationFormat,
		exportFormat,
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

	function handleExportFormatChange(format: ExportFormat) {
		settingsStore.setExportFormat(format)
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

	// Create a minimal UsbDevice object for disconnected ignored devices
	function createMinimalDevice(id: string): UsbDevice {
		return {
			id,
			name: id,
			mount_point: '',
			volume_uuid: null,
			total_space_bytes: 0,
			available_space_bytes: 0,
			is_removable: true,
			file_system: '',
			disk_kind: '',
		}
	}

	// Map ignored device IDs to device info with full device data when connected
	const ignoredDevicesWithInfo = $derived(
		$ignoredDeviceIds.map((id) => {
			const connectedDevice = $devices.find((d) => d.id === id)
			return {
				id,
				device: connectedDevice ?? createMinimalDevice(id),
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

	<!-- Export Format Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.library.exportFormat')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.library.exportFormatDescription')}</Text>
		<div class="flex gap-3">
			<button
				type="button"
				class="flex flex-1 flex-col items-center gap-2 rounded-lg border-2 p-4
				transition-colors {$exportFormat === 'pdb'
					? 'border-brand-primary bg-brand-muted'
					: 'border-stroke hover:cursor-pointer hover:border-text-tertiary'}"
				onclick={() => handleExportFormatChange('pdb')}
			>
				<Text variant="body-2" as="span">{$translate('settings.library.exportFormatPdb')}</Text>
				<Text variant="caption" color="secondary">{$translate('settings.library.exportFormatPdbDescription')}</Text>
			</button>
			<button
				type="button"
				class="flex flex-1 flex-col items-center gap-2 rounded-lg border-2 p-4
				transition-colors {$exportFormat === 'device_library_plus'
					? 'border-brand-primary bg-brand-muted'
					: 'border-stroke hover:cursor-pointer hover:border-text-tertiary'}"
				onclick={() => handleExportFormatChange('device_library_plus')}
			>
				<Text variant="body-2" as="span">{$translate('settings.library.exportFormatDeviceLibraryPlus')}</Text>
				<Text variant="caption" color="secondary"
					>{$translate('settings.library.exportFormatDeviceLibraryPlusDescription')}</Text
				>
			</button>
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
				{#each ignoredDevicesWithInfo as deviceInfo (deviceInfo.id)}
					<DeviceItem
						device={deviceInfo.device}
						ignored={true}
						isConnected={deviceInfo.isConnected}
						onUnignore={() => handleUnignoreDevice(deviceInfo.id)}
					/>
				{/each}
			</div>
		{/if}
	</section>
</div>
