<script lang="ts">
	import { scale } from 'svelte/transition'
	import { cubicOut } from 'svelte/easing'
	import { onMount } from 'svelte'

	// Mirrors the desktop splash: the accent-masked crate logo over the app background, the wordmark, and
	// the version, scaling + fading out once boot completes. The matching pre-paint splash in app.html
	// renders before this component (and before any JS), so swapping to this one is seamless.
	type Props = {
		show: boolean
		version: string
		onOutroEnd?: () => void
	}

	let { show, version, onOutroEnd }: Props = $props()

	onMount(() => {
		// Remove the static splash app.html painted now that the Svelte one has taken over.
		document.getElementById('splash')?.remove()
		// Drop the inline CSS the boot script set for the pre-paint window. theme.css is loaded now and its
		// [data-theme]/[data-accent]/[data-font] rules supply identical values — but only once these inline
		// overrides are gone, so later theme/accent/font switches actually re-theme. (settingsStore clears
		// the surface/text ones too; --brand-primary and --font-family are cleared only here.)
		const s = document.documentElement.style
		s.removeProperty('--font-family')
		s.removeProperty('--surface-0')
		s.removeProperty('--text-primary')
		s.removeProperty('--text-tertiary')
		s.removeProperty('--brand-primary')
	})
</script>

{#if show}
	<div
		class="fixed inset-0 z-[9999] flex flex-col items-center justify-center gap-3 bg-surface-0"
		out:scale={{ start: 1, duration: 400, easing: cubicOut, opacity: 0 }}
		onoutroend={onOutroEnd}
	>
		<div
			class="h-16 w-16 bg-brand-primary"
			style="-webkit-mask-image: url('/crate-logo.svg'); -webkit-mask-size: contain; -webkit-mask-repeat: no-repeat; -webkit-mask-position: center; mask-image: url('/crate-logo.svg'); mask-size: contain; mask-repeat: no-repeat; mask-position: center;"
		></div>
		<span class="text-lg font-bold text-text-primary">Crate</span>
		<span class="text-xs text-text-tertiary">v{version}</span>
	</div>
{/if}
