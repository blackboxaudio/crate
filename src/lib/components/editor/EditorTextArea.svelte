<script lang="ts">
	import { MIXED_PLACEHOLDER } from '$lib/utils'
	import Text from '$lib/components/common/Text.svelte'

	type Props = {
		label: string
		value: string | null
		mixed?: boolean
		placeholder?: string
		disabled?: boolean
		rows?: number
		resize?: boolean
		onchange?: (value: string | null) => void
		onblur?: () => void
	}

	let {
		label,
		value = $bindable(),
		mixed = false,
		placeholder = '',
		disabled = false,
		rows = 3,
		resize = false,
		onchange,
		onblur,
	}: Props = $props()

	// Local state for the textarea - shows placeholder when mixed
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
		const target = e.target as HTMLTextAreaElement
		inputValue = target.value

		const strValue = target.value === '' ? null : target.value
		value = strValue
		onchange?.(strValue)
	}
</script>

<label class="block space-y-1">
	<Text as="span" size="xs" weight="medium" color="secondary" class="block">{label}</Text>
	<textarea
		{rows}
		value={inputValue}
		placeholder={mixed ? MIXED_PLACEHOLDER : placeholder}
		{disabled}
		oninput={handleInput}
		onblur={() => onblur?.()}
		class="w-full rounded-md border border-stroke bg-surface-2 px-3 py-1.5 text-sm text-text-primary placeholder-text-tertiary focus:border-transparent focus:ring-2 focus:ring-brand-primary focus:outline-none disabled:cursor-not-allowed disabled:opacity-50 {resize
			? 'resize-y'
			: 'resize-none'}"
	></textarea>
</label>
