import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'
import { sveltekit } from '@sveltejs/kit/vite'
import tailwindcss from '@tailwindcss/vite'

const host = process.env.TAURI_DEV_HOST as string | undefined

// Read the repo-root package.json regardless of the cwd Vite is launched from. This config lives
// at apps/desktop/vite.config.ts, so the root is two levels up. The version is the source of truth
// surfaced in app.html via %sveltekit.env.PUBLIC_APP_VERSION%.
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

export default defineConfig(async () => ({
	plugins: [sveltekit(), tailwindcss()],

	clearScreen: false,
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
			ignored: ['**/src-tauri/**'],
		},
	},
}))
