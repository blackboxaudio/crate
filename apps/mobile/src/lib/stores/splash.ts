import { writable } from 'svelte/store'

// Whether the launch splash is showing. Starts true so the Svelte splash is up the moment the app
// mounts (taking over from the pre-paint splash in app.html); the layout calls dismissSplash() once
// i18n + persisted settings have loaded. Mirrors the desktop splash store.
export const splashVisible = writable(true)

export function dismissSplash() {
	splashVisible.set(false)
}
