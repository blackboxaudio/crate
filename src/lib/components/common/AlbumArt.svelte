<script lang="ts">
	import { getArtworkUrl } from '$lib/utils'
	import Icon from './Icon.svelte'

	type Size = 'sm' | 'md' | 'lg'

	type Props = {
		artworkPath: string | null
		size?: Size
		class?: string
		onclick?: () => void
	}

	let { artworkPath, size = 'md', class: className = '', onclick }: Props = $props()

	const sizeClasses: Record<Size, string> = {
		sm: 'h-8 w-8',
		md: 'h-12 w-12',
		lg: 'aspect-square w-full max-w-[400px]',
	}

	const iconSizes: Record<Size, string> = {
		sm: 'h-4 w-4',
		md: 'h-6 w-6',
		lg: 'h-16 w-16',
	}

	let artworkUrl = $derived(getArtworkUrl(artworkPath))
	let hasError = $state(false)

	function handleError() {
		hasError = true
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault()
			onclick?.()
		}
	}
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
	class="flex flex-shrink-0 items-center justify-center overflow-hidden rounded bg-surface-2 {sizeClasses[
		size
	]} {className}"
	role={onclick ? 'button' : undefined}
	tabindex={onclick ? 0 : undefined}
	{onclick}
	onkeydown={onclick ? handleKeydown : undefined}
>
	{#if artworkUrl && !hasError}
		<img src={artworkUrl} alt="Album artwork" class="h-full w-full object-cover" onerror={handleError} />
	{:else}
		<Icon name="music-note" class="{iconSizes[size]} text-text-tertiary" />
	{/if}
</div>
