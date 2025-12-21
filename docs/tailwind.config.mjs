import starlightPlugin from '@astrojs/starlight-tailwind'

/** @type {import('tailwindcss').Config} */
export default {
	content: ['./src/**/*.{astro,html,js,jsx,md,mdx,svelte,ts,tsx,vue}'],
	plugins: [starlightPlugin()],
	theme: {
		extend: {
			colors: {
				accent: {
					DEFAULT: '#3b82f6',
					hover: '#2563eb',
					muted: 'rgba(59, 130, 246, 0.2)',
				},
				surface: {
					0: '#09090b',
					1: '#18181b',
					2: '#27272a',
				},
			},
			fontFamily: {
				sans: ['IBM Plex Mono', 'ui-monospace', 'monospace'],
				mono: ['IBM Plex Mono', 'ui-monospace', 'monospace'],
			},
		},
	},
}
