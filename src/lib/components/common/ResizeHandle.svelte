<script lang="ts">
	type Props = {
		onResize?: (delta: number) => void
	}

	let { onResize }: Props = $props()

	let isDragging = $state(false)
	let startX = $state(0)

	function handleMouseDown(e: MouseEvent) {
		e.preventDefault()
		isDragging = true
		startX = e.clientX
		document.body.style.cursor = 'col-resize'
		document.body.style.userSelect = 'none'
		window.addEventListener('mousemove', handleMouseMove)
		window.addEventListener('mouseup', handleMouseUp)
	}

	function handleMouseMove(e: MouseEvent) {
		if (!isDragging) return
		const delta = e.clientX - startX
		startX = e.clientX
		onResize?.(delta)
	}

	function handleMouseUp() {
		isDragging = false
		document.body.style.cursor = ''
		document.body.style.userSelect = ''
		window.removeEventListener('mousemove', handleMouseMove)
		window.removeEventListener('mouseup', handleMouseUp)
	}
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
	role="separator"
	aria-orientation="vertical"
	tabindex="0"
	class="group relative h-full w-[1px] cursor-col-resize"
	onmousedown={handleMouseDown}
>
	<div
		class="absolute inset-y-0 left-0 w-px bg-stroke transition-colors {isDragging
			? 'bg-accent'
			: 'group-hover:bg-accent'}"
	></div>
	<div class="absolute inset-y-0 -left-1 w-3"></div>
</div>
