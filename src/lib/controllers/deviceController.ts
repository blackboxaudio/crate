import { openPath } from '@tauri-apps/plugin-opener'
import type { UsbDevice } from '$lib/types'
import type { devicesStore as DevicesStoreType } from '$lib/stores/devices'
import type { settingsStore as SettingsStoreType } from '$lib/stores/settings'
import type { toastStore as ToastStoreType } from '$lib/stores/toast'
import * as devicesApi from '$lib/api/devices'

// =============================================================================
// Types
// =============================================================================

export interface DeviceControllerDeps {
	devicesStore: typeof DevicesStoreType
	settingsStore: typeof SettingsStoreType
	toastStore: typeof ToastStoreType
}

export interface DeviceControllerModalActions {
	openDeviceInfoModal: (device: UsbDevice) => void
	openReformatDeviceModal: (device: UsbDevice) => void
}

export interface DeviceController {
	handleEjectDevice: (device: UsbDevice) => Promise<void>
	handleViewDeviceInfo: (device: UsbDevice) => void
	handleDeviceRevealInFinder: (device: UsbDevice) => Promise<void>
	handleDeviceReformat: (device: UsbDevice) => void
	handleDeviceIgnore: (device: UsbDevice) => void
	handleReformatDevice: (device: UsbDevice, volumeName: string) => Promise<void>
}

// =============================================================================
// Controller Factory
// =============================================================================

export function createDeviceController(
	deps: DeviceControllerDeps,
	modalActions: DeviceControllerModalActions
): DeviceController {
	const { devicesStore, settingsStore, toastStore } = deps

	/**
	 * Eject a device
	 */
	async function handleEjectDevice(device: UsbDevice): Promise<void> {
		try {
			await devicesApi.ejectDevice(device.mount_point)
			toastStore.success(`${device.name} ejected`)
		} catch (error) {
			toastStore.error(`Failed to eject ${device.name}`)
			console.error('Eject error:', error)
		}
	}

	/**
	 * Open device info modal
	 */
	function handleViewDeviceInfo(device: UsbDevice): void {
		modalActions.openDeviceInfoModal(device)
	}

	/**
	 * Reveal device in system file explorer
	 */
	async function handleDeviceRevealInFinder(device: UsbDevice): Promise<void> {
		await openPath(device.mount_point)
	}

	/**
	 * Open reformat device modal
	 */
	function handleDeviceReformat(device: UsbDevice): void {
		modalActions.openReformatDeviceModal(device)
	}

	/**
	 * Ignore a device (hide from sidebar)
	 */
	function handleDeviceIgnore(device: UsbDevice): void {
		settingsStore.ignoreDevice(device.id)
	}

	/**
	 * Reformat a device
	 */
	async function handleReformatDevice(device: UsbDevice, volumeName: string): Promise<void> {
		devicesStore.setReformattingDevice(device.id)
		try {
			await devicesApi.reformatDevice(device.mount_point, volumeName)
			toastStore.success(`Device reformatted as "${volumeName}"`)
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error)
			// Handle user cancellation gracefully - don't show error toast or log
			if (message.includes('cancelled') || message.includes('canceled')) {
				return
			}
			toastStore.error(`Failed to reformat: ${message}`)
			console.error('Reformat error:', error)
		} finally {
			devicesStore.clearReformattingDevice()
		}
	}

	return {
		handleEjectDevice,
		handleViewDeviceInfo,
		handleDeviceRevealInFinder,
		handleDeviceReformat,
		handleDeviceIgnore,
		handleReformatDevice,
	}
}
