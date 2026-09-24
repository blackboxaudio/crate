<script lang="ts">
	import { scale } from 'svelte/transition'
	import { cubicOut } from 'svelte/easing'
	import { onMount } from 'svelte'
	import { invoke } from '@tauri-apps/api/core'
	import { isIOS } from '$shared/utils/platform'

	// The crate logo over the app background, the wordmark, and the version, fading out once boot
	// completes. Must stay a pixel-identical mirror of the static pre-paint splash in app.html (which
	// itself mirrors the native launch screen — see the comment there), so the static→Svelte swap and
	// the earlier OS-launch-image crossfade are both seamless.
	type Props = {
		show: boolean
		version: string
		onOutroEnd?: () => void
	}

	let { show, version, onOutroEnd }: Props = $props()

	// iOS: a native copy of the launch screen sits above the webview so the OS launch crossfade
	// never reveals an unpainted (white) page. By mount time the splash markup has definitely
	// painted, so after the next frame commits, tell the backend to fade the overlay out — that
	// fade is also what eases the wordmark + version in (they're the only pixels that differ).
	// Double-rAF: the second callback runs after a frame containing this component has rendered.
	onMount(() => {
		if (!isIOS()) return
		requestAnimationFrame(() => {
			requestAnimationFrame(() => void invoke('dismiss_native_splash').catch(() => {}))
		})
	})

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
				// identical values via its [data-theme]/[data-font] rules — but only once these inline
				// overrides are gone, so later theme/font switches actually re-theme. (settingsStore
				// clears the surface/text ones too; --font-family is cleared only here.)
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
	     and centered even then. Fixed px (not rem) because mobile's `html { font: -apple-system-body }`
	     rebinds the rem base to the iOS Dynamic Type body size once style.css loads — px keeps both
	     splashes the same size across the handoff and immune to Dynamic Type scaling. The logo box,
	     position, and #14b8a6 tint match the native launch screen (see app.html). -->
	<div
		style="position:fixed;inset:0;z-index:9999;background:var(--surface-0)"
		out:scale={{ start: 1, duration: 400, easing: cubicOut, opacity: 0 }}
		onoutroend={onOutroEnd}
	>
		<div
			style="position:absolute;left:50%;top:50%;width:99px;height:104px;transform:translate(-50%,-50%);background:#14b8a6;-webkit-mask-image:url('/crate-mark.png');-webkit-mask-size:contain;-webkit-mask-repeat:no-repeat;-webkit-mask-position:center;mask-image:url('/crate-mark.png');mask-size:contain;mask-repeat:no-repeat;mask-position:center"
		></div>
		<div
			style="position:absolute;left:0;right:0;top:calc(50% + 64px);display:flex;flex-direction:column;align-items:center;gap:12px"
		>
			<span
				style="font-size:18px;line-height:28px;font-weight:700;color:var(--text-primary);font-family:var(--font-family)"
				>Crate</span
			>
			<span style="font-size:12px;line-height:16px;color:var(--text-tertiary);font-family:var(--font-family)"
				>v{version}</span
			>
		</div>
	</div>
{/if}
