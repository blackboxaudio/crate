<script lang="ts">
	import type { Snippet } from 'svelte'
	import { ResizeHandle } from '$lib/components/common'

	type Props = {
		hasContent: boolean
		isVisible: boolean
		width: number
		onResize: (delta: number) => void
		children: Snippet
	}

	let { hasContent, isVisible, width, onResize, children }: Props = $props()

	// Resize state for disabling transitions during resize
	let isResizing = $state(false)

	// Whether sidebar should be open (visible and has content)
	let sidebarOpen = $derived(isVisible && hasContent)

	// Track whether content should be rendered (persists during close transition)
	let showContent = $state(false)

	$effect(() => {
		if (sidebarOpen) {
			showContent = true
		}
	})
</script>

<div
	class="flex h-full flex-shrink-0 overflow-hidden ease-out"
	class:transition-[width]={!isResizing}
	class:duration-250={!isResizing}
	class:animate-[fade-in_250ms_ease-out]={sidebarOpen}
	style="width: {sidebarOpen ? width : 0}px"
	ontransitionend={(e) => {
		if (e.propertyName === 'width' && !sidebarOpen) {
			showContent = false
		}
	}}
>
	<ResizeHandle
		onResize={(delta) => onResize(-delta)}
		onResizeStart={() => (isResizing = true)}
		onResizeEnd={() => (isResizing = false)}
	/>
	<div style="width: {width}px">
		{#if showContent}
			{@render children()}
		{/if}
	</div>
</div>
