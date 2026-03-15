import { defineConfig } from 'vite'
import { sveltekit } from '@sveltejs/kit/vite'

// https://vite.dev/config/
export default defineConfig(async () => ({
	plugins: [sveltekit()],
}))
