<script lang="ts">
	import { scale } from 'svelte/transition'
	import { cubicOut } from 'svelte/easing'
	import Icon from './Icon.svelte'
	import Text from './Text.svelte'

	type Props = {
		show: boolean
		version: string
		onOutroEnd?: () => void
	}

	let { show, version, onOutroEnd }: Props = $props()
</script>

{#if show}
	<div
		class="fixed inset-0 z-[9999] flex flex-col items-center justify-center gap-3 bg-surface-0"
		out:scale={{ start: 1, duration: 400, easing: cubicOut, opacity: 0 }}
		onoutroend={onOutroEnd}
	>
		<Icon name="logo" class="h-16 w-16 text-brand-primary" fill />
		<Text variant="header-1" as="span" weight="bold">Crate</Text>
		<Text variant="caption" color="tertiary">v{version}</Text>
	</div>
{/if}
