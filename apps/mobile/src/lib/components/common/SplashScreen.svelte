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
		// Hand off from the static app.html splash only once style.css has actually been applied. The
		// SPA fallback page has no render-blocking stylesheet — the compiled CSS is injected at runtime
		// by the JS entry — so removing #splash (and the boot script's inline CSS variables) any earlier
		// races the stylesheet and can paint an unstyled frame. --text-secondary is defined only by
		// theme.css (the boot script never sets it inline), so it doubles as a "CSS applied" signal.
		let cancelled = false
		const handoff = () => {
			if (cancelled) return
			if (getComputedStyle(document.documentElement).getPropertyValue('--text-secondary')) {
				// Remove the static splash app.html painted now that the Svelte one has taken over.
				document.getElementById('splash')?.remove()
				// Drop the inline CSS the boot script set for the pre-paint window. theme.css supplies
				// identical values via its [data-theme]/[data-accent]/[data-font] rules — but only once these
				// inline overrides are gone, so later theme/accent/font switches actually re-theme.
				// (settingsStore clears the surface/text ones too; --brand-primary and --font-family are
				// cleared only here.)
				const s = document.documentElement.style
				s.removeProperty('--font-family')
				s.removeProperty('--surface-0')
				s.removeProperty('--text-primary')
				s.removeProperty('--text-tertiary')
				s.removeProperty('--brand-primary')
			} else {
				requestAnimationFrame(handoff)
			}
		}
		handoff()
		return () => {
			cancelled = true
		}
	})
</script>

{#if show}
	<!-- Inline styles (mirroring the static #splash in app.html) instead of Tailwind classes: this
	     component can mount before the runtime-injected style.css applies, and it must render branded
	     and centered even then. Fixed px (not rem) because mobile's `html { font: -apple-system-body }`
	     rebinds the rem base to the iOS Dynamic Type body size once style.css loads — px keeps both
	     splashes the same size across the handoff and immune to Dynamic Type scaling. -->
	<div
		style="position:fixed;inset:0;z-index:9999;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;background:var(--surface-0)"
		out:scale={{ start: 1, duration: 400, easing: cubicOut, opacity: 0 }}
		onoutroend={onOutroEnd}
	>
		<div
			style="width:64px;height:64px;background:var(--brand-primary);-webkit-mask-image:url('/crate-logo.svg');-webkit-mask-size:contain;-webkit-mask-repeat:no-repeat;-webkit-mask-position:center;mask-image:url('/crate-logo.svg');mask-size:contain;mask-repeat:no-repeat;mask-position:center"
		></div>
		<span
			style="font-size:18px;line-height:28px;font-weight:700;color:var(--text-primary);font-family:var(--font-family)"
			>Crate</span
		>
		<span style="font-size:12px;line-height:16px;color:var(--text-tertiary);font-family:var(--font-family)"
			>v{version}</span
		>
	</div>
{/if}
