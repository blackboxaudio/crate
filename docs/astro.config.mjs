import { defineConfig } from 'astro/config'
import starlight from '@astrojs/starlight'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
	site: 'https://blackboxaudio.github.io',
	base: '/crate',
	vite: {
		plugins: [tailwindcss()],
	},
	integrations: [
		starlight({
			title: 'Crate',
			description: 'Documentation for Crate - DJ library management',
			logo: {
				src: './src/assets/logo.svg',
				replacesTitle: false,
			},
			social: {
				github: 'https://github.com/blackboxaudio/crate',
			},
			customCss: ['./src/styles/custom.css'],
			defaultLocale: 'root',
			locales: {
				root: { label: 'English', lang: 'en' },
			},
			sidebar: [
				{
					label: 'Getting Started',
					items: [{ label: 'Quick Start', slug: 'getting-started' }],
				},
				{
					label: 'User Guide',
					items: [
						{ label: 'Overview', slug: 'user-guide' },
						{ label: 'Library Management', slug: 'user-guide/library-management' },
						{ label: 'Playlists & Folders', slug: 'user-guide/playlists-and-folders' },
						{ label: 'Tagging', slug: 'user-guide/tagging' },
						{ label: 'Playback', slug: 'user-guide/playback' },
						{ label: 'Search & Filtering', slug: 'user-guide/search-and-filtering' },
						{ label: 'Settings', slug: 'user-guide/settings' },
						{ label: 'Device Management', slug: 'user-guide/devices' },
						{ label: 'Bulk Operations', slug: 'user-guide/bulk-operations' },
					],
				},
				{
					label: 'Reference',
					items: [{ label: 'Keyboard Shortcuts', slug: 'reference/keyboard-shortcuts' }],
				},
				{
					label: 'Help',
					items: [
						{ label: 'FAQ', slug: 'faq' },
						{ label: 'Changelog', slug: 'changelog' },
					],
				},
			],
			editLink: {
				baseUrl: 'https://github.com/blackboxaudio/crate/edit/develop/docs/',
			},
			pagination: true,
			tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 3 },
		}),
	],
})
