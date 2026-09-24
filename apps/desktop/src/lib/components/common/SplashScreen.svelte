<script lang="ts">
	import { scale } from 'svelte/transition'
	import { cubicOut } from 'svelte/easing'
	import { onMount } from 'svelte'

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
		// style.css (the boot script never sets it inline), so it doubles as a "CSS applied" signal.
		let cancelled = false
		const handoff = () => {
			if (cancelled) return
			if (getComputedStyle(document.documentElement).getPropertyValue('--text-secondary')) {
				document.getElementById('splash')?.remove()
				// Remove inline CSS properties set by app.html's startup script.
				// Now that style.css is loaded, the [data-theme]/[data-font] CSS rules take over.
				const s = document.documentElement.style
				s.removeProperty('--font-family')
				s.removeProperty('--surface-0')
				s.removeProperty('--text-primary')
				s.removeProperty('--text-tertiary')
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
	     and centered even then. --brand-primary comes from theme.css once loaded; until then it falls
	     back to --color-brand-primary, which the app.html boot script sets inline. -->
	<div
		style="position:fixed;inset:0;z-index:9999;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;background:var(--surface-0)"
		out:scale={{ start: 1, duration: 400, easing: cubicOut, opacity: 0 }}
		onoutroend={onOutroEnd}
	>
		<div
			style="width:64px;height:64px;background:var(--brand-primary, var(--color-brand-primary));-webkit-mask-image:url('/crate-logo.svg');-webkit-mask-size:contain;-webkit-mask-repeat:no-repeat;-webkit-mask-position:center;mask-image:url('/crate-logo.svg');mask-size:contain;mask-repeat:no-repeat;mask-position:center"
		></div>
		<span
			style="font-size:1.125rem;line-height:1.75rem;font-weight:700;color:var(--text-primary);font-family:var(--font-family)"
			>Crate</span
		>
		<span style="font-size:0.75rem;line-height:1rem;color:var(--text-tertiary);font-family:var(--font-family)"
			>v{version}</span
		>
	</div>
{/if}
