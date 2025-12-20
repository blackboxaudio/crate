<script lang="ts">
	import '../style.css'
	import type { Snippet } from 'svelte'
	import ToastContainer from '$lib/components/common/ToastContainer.svelte'
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { isDev } from '$lib/stores/app'

	interface Props {
		children: Snippet
	}

	let { children }: Props = $props()

	onMount(() => {
		// Prevent browser default drag/drop behavior (which would navigate to dropped files)
		const dragoverHandler = (e: DragEvent) => {
			e.preventDefault()
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
			window.removeEventListener('dragover', dragoverHandler)
			window.removeEventListener('drop', dropHandler)
			document.removeEventListener('contextmenu', contextMenuHandler)
		}
	})
</script>

<div class="flex h-screen w-screen flex-col overflow-hidden bg-surface-0 text-text-primary">
	{@render children()}
</div>

<ToastContainer />
