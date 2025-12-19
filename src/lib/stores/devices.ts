import { writable, derived } from 'svelte/store'
import type { UsbDevice } from '$lib/types'
import * as devicesApi from '$lib/api/devices'

// =============================================================================
// Types
// =============================================================================

interface DevicesState {
	devices: UsbDevice[]
	loading: boolean
}

// =============================================================================
// State
// =============================================================================

const initialState: DevicesState = {
	devices: [],
	loading: false,
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
