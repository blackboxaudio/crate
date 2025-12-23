<script lang="ts">
	import type { Snippet } from 'svelte'
	import { fade } from 'svelte/transition'

	type TooltipPosition = 'top' | 'bottom' | 'left' | 'right'

	type Props = {
		text?: string
		position?: TooltipPosition
		delay?: number
		class?: string
		children: Snippet
	}

	let { text, position = 'top', delay = 0, class: className = '', children }: Props = $props()

	let visible = $state(false)
	let message = $state('')
	let timeoutId: ReturnType<typeof setTimeout> | undefined = $state()
	let hoverTimeoutId: ReturnType<typeof setTimeout> | undefined = $state()

	const positionStyles: Record<TooltipPosition, string> = {
		top: 'bottom-full left-1/2 -translate-x-1/2 mb-2',
		bottom: 'top-full left-1/2 -translate-x-1/2 mt-2',
		left: 'right-full top-1/2 -translate-y-1/2 mr-2',
		right: 'left-full top-1/2 -translate-y-1/2 ml-2',
	}

	// Programmatic API
	export function show(text: string, duration: number = 2000): void {
		if (timeoutId) {
			clearTimeout(timeoutId)
		}

		message = text
		visible = true

		if (duration > 0) {
			timeoutId = setTimeout(() => {
				visible = false
			}, duration)
		}
	}

	export function hide(): void {
		if (timeoutId) {
			clearTimeout(timeoutId)
			timeoutId = undefined
		}
		visible = false
	}

	// Hover handlers for declarative usage with `text` prop
	function handleMouseEnter() {
		if (!text) return

		if (delay > 0) {
			hoverTimeoutId = setTimeout(() => {
				message = text
				visible = true
			}, delay)
		} else {
			message = text
			visible = true
		}
	}

	function handleMouseLeave() {
		if (!text) return

		if (hoverTimeoutId) {
			clearTimeout(hoverTimeoutId)
			hoverTimeoutId = undefined
		}
		visible = false
	}
</script>

<div class="relative inline-flex" role="group" onmouseenter={handleMouseEnter} onmouseleave={handleMouseLeave}>
	{@render children()}

	{#if visible}
		<div
			class="pointer-events-none absolute z-50 rounded border border-stroke bg-surface-1 px-2 py-1 text-xs font-medium whitespace-nowrap text-text-primary shadow-lg {positionStyles[
				position
			]} {className}"
			role="tooltip"
			transition:fade={{ duration: 200 }}
		>
			{message}
		</div>
	{/if}
</div>
