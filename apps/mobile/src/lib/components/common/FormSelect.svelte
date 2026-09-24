<script lang="ts">
	import type { Snippet } from 'svelte'

	// A native <select> dressed as a tappable value row: borderless, with the iOS up/down disclosure
	// glyph instead of the browser's default widget chrome. Stays a REAL select — iOS renders its wheel
	// picker and Android its option dialog, which is the whole point (no custom dropdown to build or
	// maintain). Options come in as children so callers keep their own {#each} logic.
	type Props = {
		value: string
		onchange: (value: string) => void
		ariaLabel?: string
		children: Snippet
		class?: string
	}
	let { value, onchange, ariaLabel, children, class: className = '' }: Props = $props()
</script>

<div class="relative min-w-0 {className}">
	<select
		{value}
		aria-label={ariaLabel}
		onchange={(e) => onchange(e.currentTarget.value)}
		class="w-full appearance-none rounded-lg bg-surface-2 py-2 pr-8 pl-3 text-sm text-text-primary focus:outline-none"
	>
		{@render children()}
	</select>
	<svg
		class="pointer-events-none absolute top-1/2 right-2.5 h-3.5 w-3.5 -translate-y-1/2 text-text-tertiary"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
	>
		<path d="M7 9l5-5 5 5M7 15l5 5 5-5" stroke-linecap="round" stroke-linejoin="round" />
	</svg>
</div>
