<script lang="ts">
	import '../style.css'
	import type { Snippet } from 'svelte'

	interface Props {
		children: Snippet
	}

	let { children }: Props = $props()
</script>

<div class="flex h-screen w-screen flex-col overflow-hidden bg-zinc-950 text-zinc-100">
	{@render children()}
</div>
