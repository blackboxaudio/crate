<script lang="ts">
	import '../style.css'
	import type { Snippet } from 'svelte'
	import ToastContainer from '$lib/components/common/ToastContainer.svelte'
	import { onMount } from 'svelte'

	interface Props {
		children: Snippet
	}

	let { children }: Props = $props()

	onMount(() => {
		const handler = (e: DragEvent) => {
			console.log('[Window DragOver]', e.target, e.dataTransfer?.types ? Array.from(e.dataTransfer.types) : null)
		}
		window.addEventListener('dragover', handler)
		return () => window.removeEventListener('dragover', handler)
	})
</script>

<div class="flex h-screen w-screen flex-col overflow-hidden bg-zinc-950 text-zinc-100">
	{@render children()}
</div>

<ToastContainer />
