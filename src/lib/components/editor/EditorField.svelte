<script lang="ts">
	import { MIXED_PLACEHOLDER } from '$lib/utils'

	type Props = {
		label: string
		value: string | number | null
		mixed?: boolean
		type?: 'text' | 'number'
		placeholder?: string
		disabled?: boolean
		onchange?: (value: string | number | null) => void
		onsubmit?: () => void
	}

	let {
		label,
		value = $bindable(),
		mixed = false,
		type = 'text',
		placeholder = '',
		disabled = false,
		onchange,
		onsubmit,
	}: Props = $props()

	// Local state for the input - shows placeholder when mixed
	let inputValue = $state('')

	// Sync inputValue with value prop
	$effect(() => {
		if (mixed) {
			inputValue = ''
		} else if (value !== null && value !== undefined) {
			inputValue = String(value)
		} else {
			inputValue = ''
		}
	})

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement
		inputValue = target.value

		if (type === 'number') {
			const numValue = target.value === '' ? null : Number(target.value)
			value = numValue
			onchange?.(numValue)
		} else {
			const strValue = target.value === '' ? null : target.value
			value = strValue
			onchange?.(strValue)
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault()
			onsubmit?.()
		}
	}
</script>

<label class="block space-y-1">
	<span class="block text-xs font-medium text-text-secondary">{label}</span>
	<input
		{type}
		value={inputValue}
		placeholder={mixed ? MIXED_PLACEHOLDER : placeholder}
		{disabled}
		oninput={handleInput}
		onkeydown={handleKeydown}
		class="w-full rounded-md border border-stroke bg-surface-2 px-3 py-1.5 text-sm text-text-primary placeholder-text-tertiary focus:border-transparent focus:ring-2 focus:ring-brand-primary focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
	/>
</label>
