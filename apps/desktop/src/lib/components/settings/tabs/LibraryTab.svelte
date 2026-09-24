<script lang="ts">
	import type { KeyNotationFormat, ExportFormat, UsbDevice, LibraryFolderScanResult } from '$shared/types'
	import { Text, Checkbox, Button } from '$lib/components/common'
	import DeviceItem from '$lib/components/devices/DeviceItem.svelte'
	import {
		settingsStore,
		keyNotationFormat,
		exportFormat,
		autoAnalyzeOnImport,
		autoSyncOnConnect,
		autoSyncOnChange,
		ignoredDeviceIds,
	} from '$shared/stores/settings'
	import { devices } from '$lib/stores/devices'
	import { libraryStore } from '$lib/stores/library'
	import * as libraryApi from '$shared/api/library'
	import { translate } from '$shared/i18n'
	import { withNativeDialog } from '$shared/utils'
	import { open } from '@tauri-apps/plugin-dialog'

	let musicFolderPath = $state<string | null>(null)
	let busy = $state(false)
	let scanning = $state(false)
	let scanResult = $state<LibraryFolderScanResult | null>(null)
	let folderError = $state<string | null>(null)
	let scanError = $state<string | null>(null)

	function toErrorMessage(error: unknown): string {
		return error instanceof Error ? error.message : String(error)
	}

	$effect(() => {
		// Load the stored folder once so Settings shows the current path when reopened.
		libraryApi
			.getMusicLibraryFolder()
			.then((folder) => {
				musicFolderPath = folder
			})
			.catch((error) => {
				console.error('Failed to load music library folder:', error)
			})
	})

	async function runScan() {
		scanResult = null
		scanError = null
		scanning = true
		try {
			scanResult = await libraryStore.scanMusicLibraryFolder()
		} catch (error) {
			scanError = toErrorMessage(error)
		} finally {
			scanning = false
		}
	}

	async function handleChooseFolder() {
		if (busy) return
		busy = true
		folderError = null
		try {
			const selected = await withNativeDialog(() => open({ directory: true, multiple: false }))
			const path = typeof selected === 'string' ? selected : null
			if (!path) return
			// Persist the choice, then import what it contains.
			musicFolderPath = await libraryApi.setMusicLibraryFolder(path)
			await runScan()
		} catch (error) {
			folderError = toErrorMessage(error)
		} finally {
			busy = false
		}
	}

	async function handleRescan() {
		if (busy || !musicFolderPath) return
		busy = true
		try {
			await runScan()
		} finally {
			busy = false
		}
	}

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
	<!-- Music Folder Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.library.musicFolder')}</Text>
		<Text variant="caption" as="p" class="mb-4">{$translate('settings.library.musicFolderDescription')}</Text>

		<div class="flex items-center justify-between gap-4 rounded-lg border border-stroke bg-surface-1 px-4 py-3">
			<div class="min-w-0 flex-1">
				{#if musicFolderPath}
					<Text variant="caption" as="p" truncate title={musicFolderPath}>{musicFolderPath}</Text>
				{:else}
					<Text variant="caption" as="p" class="text-text-tertiary">
						{$translate('settings.library.musicFolderNotSet')}
					</Text>
				{/if}
			</div>
			<div class="flex flex-shrink-0 items-center gap-2">
				<Button variant="secondary" size="sm" onclick={handleChooseFolder} disabled={busy}>
					{$translate(musicFolderPath ? 'settings.library.musicFolderChange' : 'settings.library.musicFolderChoose')}
				</Button>
				<Button variant="secondary" size="sm" onclick={handleRescan} disabled={busy || !musicFolderPath}>
					{scanning
						? $translate('settings.library.musicFolderScanning')
						: $translate('settings.library.musicFolderRescan')}
				</Button>
			</div>
		</div>

		{#if folderError}
			<Text variant="caption" as="p" color="danger" class="mt-2">
				{$translate('settings.library.musicFolderSetError', { values: { error: folderError } })}
			</Text>
		{/if}

		{#if scanError}
			<Text variant="caption" as="p" color="danger" class="mt-2">
				{$translate('settings.library.musicFolderScanError', { values: { error: scanError } })}
			</Text>
		{/if}

		{#if scanResult}
			<div class="mt-3 grid grid-cols-2 gap-x-6 gap-y-2 sm:grid-cols-4">
				<div>
					<Text variant="caption" as="p">{$translate('settings.library.musicFolderFound')}</Text>
					<Text variant="body-2" as="p" tabular>{scanResult.scanned_count}</Text>
				</div>
				<div>
					<Text variant="caption" as="p">{$translate('settings.library.musicFolderImported')}</Text>
					<Text variant="body-2" as="p" tabular>{scanResult.imported_count}</Text>
				</div>
				<div>
					<Text variant="caption" as="p">{$translate('settings.library.musicFolderSkipped')}</Text>
					<Text variant="body-2" as="p" tabular>{scanResult.skipped_existing_count}</Text>
				</div>
				<div>
					<Text variant="caption" as="p">{$translate('settings.library.musicFolderFailed')}</Text>
					<Text variant="body-2" as="p" tabular>{scanResult.failed_count}</Text>
				</div>
			</div>
		{/if}
	</section>

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
