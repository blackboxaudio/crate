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

	const GAP = 8
	const VIEWPORT_PADDING = 8

	let { text, position = 'top', delay = 0, class: className = '', children }: Props = $props()

	let visible = $state(false)
	let message = $state('')
	let timeoutId: ReturnType<typeof setTimeout> | undefined
	let hoverTimeoutId: ReturnType<typeof setTimeout> | undefined

	const HIDDEN_STYLE = 'position:fixed;visibility:hidden;'

	let wrapperEl: HTMLDivElement | undefined = $state()
	let tooltipEl: HTMLDivElement | undefined = $state()
	let fixedStyle = $state(HIDDEN_STYLE)
	let rafId: number | undefined

	// Portal action: moves the element out of overflow/transform containers.
	// If inside a <dialog>, portal to the dialog (stays in top layer).
	// Otherwise, portal to document.body.
	function portal(node: HTMLElement) {
		const dialog = wrapperEl?.closest('dialog')
		const target = dialog ?? document.body
		target.appendChild(node)
		return {
			destroy() {
				node.remove()
			},
		}
	}

	function computePosition() {
		if (!wrapperEl || !tooltipEl) return

		const wr = wrapperEl.getBoundingClientRect()
		const tw = tooltipEl.offsetWidth
		const th = tooltipEl.offsetHeight

		let top: number
		let left: number

		switch (position) {
			case 'top':
				top = wr.top - th - GAP
				left = wr.left + wr.width / 2 - tw / 2
				break
			case 'bottom':
				top = wr.bottom + GAP
				left = wr.left + wr.width / 2 - tw / 2
				break
			case 'left':
				top = wr.top + wr.height / 2 - th / 2
				left = wr.left - tw - GAP
				break
			case 'right':
				top = wr.top + wr.height / 2 - th / 2
				left = wr.right + GAP
				break
		}

		// Clamp to viewport
		left = Math.max(VIEWPORT_PADDING, Math.min(left, window.innerWidth - tw - VIEWPORT_PADDING))
		top = Math.max(VIEWPORT_PADDING, Math.min(top, window.innerHeight - th - VIEWPORT_PADDING))

		fixedStyle = `position:fixed;top:${top}px;left:${left}px;`
	}

	$effect(() => {
		if (visible && tooltipEl && wrapperEl) {
			if (rafId) cancelAnimationFrame(rafId)
			fixedStyle = HIDDEN_STYLE
			rafId = requestAnimationFrame(() => {
				computePosition()
			})
		} else {
			fixedStyle = HIDDEN_STYLE
		}

		return () => {
			if (rafId) cancelAnimationFrame(rafId)
		}
	})

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

	// Keep message in sync when text prop changes while tooltip is visible
	$effect(() => {
		if (visible && text) {
			message = text
		}
	})

	// Hover handlers for declarative usage with `text` prop
	function handleMouseEnter() {
		if (!text) return

		if (hoverTimeoutId) clearTimeout(hoverTimeoutId)

		if (delay > 0) {
			hoverTimeoutId = setTimeout(() => {
				hoverTimeoutId = undefined
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

	// Cleanup all timers on destroy
	$effect(() => {
		return () => {
			if (hoverTimeoutId) clearTimeout(hoverTimeoutId)
			if (timeoutId) clearTimeout(timeoutId)
		}
	})
</script>

<div
	bind:this={wrapperEl}
	class="inline-flex"
	role="group"
	onmouseenter={handleMouseEnter}
	onmouseleave={handleMouseLeave}
>
	{@render children()}

	{#if visible}
		<div
			bind:this={tooltipEl}
			use:portal
			class="pointer-events-none z-50 rounded border border-stroke bg-surface-1 px-2 py-1 text-xs font-medium whitespace-nowrap text-text-primary shadow-lg {className}"
			style={fixedStyle}
			role="tooltip"
			transition:fade={{ duration: 200 }}
		>
			{message}
		</div>
	{/if}
</div>
