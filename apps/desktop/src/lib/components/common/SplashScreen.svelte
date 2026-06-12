<script lang="ts">
	import { scale } from 'svelte/transition'
	import { cubicOut } from 'svelte/easing'
	import { onMount } from 'svelte'
	import Text from './Text.svelte'

	type Props = {
		show: boolean
		version: string
		onOutroEnd?: () => void
	}

	let { show, version, onOutroEnd }: Props = $props()

	onMount(() => {
		document.getElementById('splash')?.remove()
		// Remove inline CSS properties set by app.html's startup script.
		// Now that style.css is loaded, the [data-theme]/[data-font] CSS rules take over.
		const s = document.documentElement.style
		s.removeProperty('--font-family')
		s.removeProperty('--surface-0')
		s.removeProperty('--text-primary')
		s.removeProperty('--text-tertiary')
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
		<Text variant="header-1" as="span" weight="bold">Crate</Text>
		<Text variant="caption" color="tertiary">v{version}</Text>
	</div>
{/if}
