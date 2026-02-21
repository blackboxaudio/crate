<script lang="ts">
	import '../style.css'
	import type { Snippet } from 'svelte'
	import type { Language } from '$lib/types'
	import ToastContainer from '$lib/components/common/ToastContainer.svelte'
	import CrashScreen from '$lib/components/common/CrashScreen.svelte'
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { isDev } from '$lib/stores/app'
	import { settingsStore } from '$lib/stores/settings'
	import { useGlobalErrorHandler, hasAudioDrag } from '$lib/hooks'
	import { initializeI18n } from '$lib/i18n'

	interface Props {
		children: Snippet
	}

	let { children }: Props = $props()
	let i18nReady = $state(false)

	onMount(() => {
		// Start async initialization
		async function init() {
			// Initialize i18n with cached language from localStorage (or system language)
			const cachedLanguage = localStorage.getItem('crate-language') as Language | null
			await initializeI18n(cachedLanguage)
			i18nReady = true

			// Load settings early so theme is applied before most errors can occur
			// This will also update i18n to the correct language if different from cached
			await settingsStore.load()
		}
		init()

		// Set up global error handlers
		const cleanupErrorHandler = useGlobalErrorHandler()

		// Only accept drag when audio files are being dragged (controls the native OS drop cursor).
		// The drop handler always prevents default as a safety net against browser navigation.
		const dragoverHandler = (e: DragEvent) => {
			if (hasAudioDrag) {
				e.preventDefault()
			}
			e.stopPropagation()
		}

		const dropHandler = (e: DragEvent) => {
			e.preventDefault()
			e.stopPropagation()
		}

		window.addEventListener('dragover', dragoverHandler)
		window.addEventListener('drop', dropHandler)

		// Disable default context menu in production
		const contextMenuHandler = (e: MouseEvent) => {
			if (!get(isDev)) {
				e.preventDefault()
			}
		}
		document.addEventListener('contextmenu', contextMenuHandler)

		return () => {
			cleanupErrorHandler()
			window.removeEventListener('dragover', dragoverHandler)
			window.removeEventListener('drop', dropHandler)
			document.removeEventListener('contextmenu', contextMenuHandler)
		}
	})
</script>

<div class="flex h-screen w-screen flex-col overflow-hidden bg-surface-0 text-text-primary">
	{#if i18nReady}
		{@render children()}
	{/if}
</div>

<ToastContainer />
<CrashScreen />
