import { writable, derived } from 'svelte/store'
import type { UsbDevice } from '$shared/types'
import * as devicesApi from '$shared/api/devices'
import { ignoredDeviceIds } from '$shared/stores/settings'

// =============================================================================
// Types
// =============================================================================

interface DevicesState {
	devices: UsbDevice[]
	loading: boolean
	reformattingDeviceId: string | null
}

// =============================================================================
// State
// =============================================================================

const initialState: DevicesState = {
	devices: [],
	loading: false,
	reformattingDeviceId: null,
}

// =============================================================================
// Store
// =============================================================================

function createDevicesStore() {
	const { subscribe, set, update } = writable<DevicesState>(initialState)

	return {
		subscribe,

		/**
		 * Load devices from the backend
		 */
		async loadDevices() {
			update((state) => ({ ...state, loading: true }))
			try {
				const devices = await devicesApi.getDevices()
				update((state) => ({ ...state, devices, loading: false }))
			} catch (error) {
				console.error('Failed to load devices:', error)
				update((state) => ({ ...state, loading: false }))
			}
		},

		/**
		 * Set devices (called from event listener)
		 */
		setDevices(devices: UsbDevice[]) {
			update((state) => ({ ...state, devices }))
		},

		/**
		 * Get current devices synchronously
		 */
		getDevices(): UsbDevice[] {
			let devices: UsbDevice[] = []
			subscribe((state) => {
				devices = state.devices
			})()
			return devices
		},

		/**
		 * Get current reformatting device ID synchronously
		 */
		getReformattingDeviceId(): string | null {
			let reformattingDeviceId: string | null = null
			subscribe((state) => {
				reformattingDeviceId = state.reformattingDeviceId
			})()
			return reformattingDeviceId
		},

		/**
		 * Set reformatting device ID (called when reformat starts)
		 */
		setReformattingDevice(deviceId: string) {
			update((state) => ({ ...state, reformattingDeviceId: deviceId }))
		},

		/**
		 * Clear reformatting device ID (called when reformat completes)
		 */
		clearReformattingDevice() {
			update((state) => ({ ...state, reformattingDeviceId: null }))
		},

		/**
		 * Reset to initial state
		 */
		reset() {
			set(initialState)
		},
	}
}

export const devicesStore = createDevicesStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const devices = derived(devicesStore, ($store) => $store.devices)

export const deviceCount = derived(devicesStore, ($store) => $store.devices.length)

export const hasDevices = derived(devicesStore, ($store) => $store.devices.length > 0)

export const devicesLoading = derived(devicesStore, ($store) => $store.loading)

export const reformattingDeviceId = derived(devicesStore, ($store) => $store.reformattingDeviceId)

// Filter out ignored devices for display in the UI
export const visibleDevices = derived([devices, ignoredDeviceIds], ([$devices, $ignoredDeviceIds]) =>
	$devices.filter((device) => !$ignoredDeviceIds.includes(device.id))
)

export const visibleDeviceCount = derived(visibleDevices, ($devices) => $devices.length)

export const hasVisibleDevices = derived(visibleDevices, ($devices) => $devices.length > 0)
