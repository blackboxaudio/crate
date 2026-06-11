import { defineConfig } from 'vite'
import { sveltekit } from '@sveltejs/kit/vite'

// Minimal scaffold config. The Tauri/version wiring and Tailwind are intentionally omitted until
// the mobile app grows real UI; it shares cross-platform code from the repo-root `shared/` via the
// $shared alias declared in svelte.config.js.
export default defineConfig({
	plugins: [sveltekit()],
})
