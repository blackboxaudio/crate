import { writable } from 'svelte/store'

export const splashVisible = writable(true)

export function dismissSplash() {
	splashVisible.set(false)
}
