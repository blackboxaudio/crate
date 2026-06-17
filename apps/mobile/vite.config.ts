import { defineConfig } from 'vite'
import { sveltekit } from '@sveltejs/kit/vite'
import tailwindcss from '@tailwindcss/vite'

// `tauri ios/android dev` sets TAURI_DEV_HOST to the machine's LAN IP so a physical device can
// reach the dev server; on a simulator/emulator it's unset and Tauri forwards localhost. The port
// is fixed (1421, distinct from desktop's 1420) because Tauri's `devUrl` points at it.
const host = process.env.TAURI_DEV_HOST

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
