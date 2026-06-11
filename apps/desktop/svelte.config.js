// Tauri doesn't have a Node.js server to do proper SSR, so we use adapter-static with a
// fallback to index.html to put the site in SPA mode.
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/
import adapter from '@sveltejs/adapter-static'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const dirname = path.dirname(fileURLToPath(import.meta.url))

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			fallback: 'index.html',
		}),
		// Shared cross-platform code lives in the repo-root `shared/` directory. SvelteKit wires
		// this alias into both the Vite resolver and the generated tsconfig paths.
		alias: {
			$shared: path.resolve(dirname, '../../shared'),
		},
	},
}

export default config
