import { readable } from 'svelte/store'

/**
 * Browser-reported connectivity. `false` is authoritative ("definitely offline" — drives the
 * offline banner under the header); `true` only means "not known to be offline". Playback-level
 * failures are classified separately in the player store — no reachability polling here.
 */
export const isOnline = readable(typeof navigator === 'undefined' ? true : navigator.onLine, (set) => {
	if (typeof window === 'undefined') return
	const on = () => set(true)
	const off = () => set(false)
	window.addEventListener('online', on)
	window.addEventListener('offline', off)
	return () => {
		window.removeEventListener('online', on)
		window.removeEventListener('offline', off)
	}
})
