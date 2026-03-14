import { readFileSync } from 'node:fs'
import { defineConfig } from 'vite'
import { sveltekit } from '@sveltejs/kit/vite'
import tailwindcss from '@tailwindcss/vite'

const host = process.env.TAURI_DEV_HOST as string | undefined

// Compute version string for use in app.html via %sveltekit.env.PUBLIC_APP_VERSION%
const pkg = JSON.parse(readFileSync('package.json', 'utf-8'))
const crateEnv = process.env.CRATE_ENV
if (crateEnv === 'staging') {
	const build = process.env.CRATE_BUILD_NUMBER
	process.env.PUBLIC_APP_VERSION = build ? `${pkg.version}-staging.${build}` : `${pkg.version}-staging`
} else if (!crateEnv || crateEnv === 'development') {
	process.env.PUBLIC_APP_VERSION = `${pkg.version}-dev`
} else {
	process.env.PUBLIC_APP_VERSION = pkg.version
}

// https://vite.dev/config/
export default defineConfig(async () => ({
	plugins: [sveltekit(), tailwindcss()],

	// Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
	//
	// 1. prevent Vite from obscuring rust errors
	clearScreen: false,
	// 2. tauri expects a fixed port, fail if that port is not available
	server: {
		port: 1420,
		strictPort: true,
		host: host || false,
		hmr: host
			? {
					protocol: 'ws',
					host,
					port: 1421,
				}
			: undefined,
		watch: {
			// 3. tell Vite to ignore watching `src-tauri`
			ignored: ['**/src-tauri/**'],
		},
	},
}))
