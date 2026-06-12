<script lang="ts">
	import Icon from '$lib/components/common/Icon.svelte'
	import { translate } from '$shared/i18n'

	type Props = {
		onSearchChange: (query: string) => void
		placeholder?: string
		initialValue?: string
	}

	let { onSearchChange, placeholder, initialValue = '' }: Props = $props()

	let inputValue = $derived(initialValue)

	// Debounced search
	let debounceTimer: ReturnType<typeof setTimeout>

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement
		inputValue = target.value

		clearTimeout(debounceTimer)
		debounceTimer = setTimeout(() => {
			onSearchChange(inputValue)
		}, 300)
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			inputValue = ''
			onSearchChange('')
			;(e.target as HTMLInputElement).blur()
		}
	}

	function handleClear() {
		inputValue = ''
		onSearchChange('')
	}

	const displayPlaceholder = $derived(placeholder ?? $translate('library.searchPlaceholder'))
</script>

<div class="relative">
	<div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
		<Icon name="search" class="h-4 w-4 text-text-tertiary" />
	</div>

	<input
		type="search"
		placeholder={displayPlaceholder}
		value={inputValue}
		class="w-full rounded-md border border-stroke bg-surface-2 py-1.5 pr-8 pl-10 text-sm text-text-primary placeholder-text-tertiary transition-colors duration-150 focus:border-transparent focus:ring-2 focus:ring-brand-primary focus:outline-none"
		oninput={handleInput}
		onkeydown={handleKeydown}
	/>

	<!-- Clear button -->
	{#if inputValue}
		<button
			type="button"
			aria-label="Clear search"
			class="absolute inset-y-0 right-0 flex cursor-pointer items-center pr-3 text-text-tertiary hover:text-text-secondary"
			onclick={handleClear}
		>
			<Icon name="x" />
		</button>
	{/if}
</div>
