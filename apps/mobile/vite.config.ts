import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'
import { sveltekit } from '@sveltejs/kit/vite'
import tailwindcss from '@tailwindcss/vite'

// `tauri ios/android dev` sets TAURI_DEV_HOST to the machine's LAN IP so a physical device can
// reach the dev server; on a simulator/emulator it's unset and Tauri forwards localhost. The port
// is fixed (1421, distinct from desktop's 1420) because Tauri's `devUrl` points at it.
const host = process.env.TAURI_DEV_HOST

// Surface the app version (repo-root package.json is the source of truth, two levels up) as
// PUBLIC_APP_VERSION for the splash: app.html reads %sveltekit.env.PUBLIC_APP_VERSION% and the Svelte
// SplashScreen reads it via $env/static/public. Mirrors apps/desktop/vite.config.ts.
const pkg = JSON.parse(readFileSync(fileURLToPath(new URL('../../package.json', import.meta.url)), 'utf-8'))
const crateEnv = process.env.CRATE_ENV
if (crateEnv === 'staging') {
	const build = process.env.CRATE_BUILD_NUMBER
	process.env.PUBLIC_APP_VERSION = build ? `${pkg.version}-staging.${build}` : `${pkg.version}-staging`
} else if (!crateEnv || crateEnv === 'development') {
	process.env.PUBLIC_APP_VERSION = `${pkg.version}-dev`
} else {
	process.env.PUBLIC_APP_VERSION = pkg.version
}

export default defineConfig({
	plugins: [sveltekit(), tailwindcss()],
	clearScreen: false,
	server: {
		host: host || false,
		port: 1421,
		strictPort: true,
		hmr: host ? { protocol: 'ws', host, port: 1430 } : undefined,
		watch: { ignored: ['**/src-tauri/**'] },
	},
})
