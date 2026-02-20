<script lang="ts">
	type Props = {
		value?: string
		placeholder?: string
		type?: 'text' | 'search' | 'number'
		disabled?: boolean
		autofocus?: boolean
		class?: string
		oninput?: (e: Event) => void
		onfocus?: (e: FocusEvent) => void
		onblur?: (e: FocusEvent) => void
		onkeydown?: (e: KeyboardEvent) => void
	}

	let {
		value = $bindable(''),
		placeholder = '',
		type = 'text',
		disabled = false,
		autofocus = false,
		class: className = '',
		oninput,
		onfocus,
		onblur,
		onkeydown,
	}: Props = $props()

	let inputEl: HTMLInputElement | undefined = $state()

	$effect(() => {
		if (autofocus && inputEl) {
			setTimeout(() => inputEl?.focus(), 50)
		}
	})
</script>

<input
	bind:this={inputEl}
	{type}
	{placeholder}
	{disabled}
	bind:value
	class="w-full rounded-md border border-stroke bg-surface-2 px-3 py-2 text-sm text-text-primary placeholder-text-tertiary focus:border-transparent focus:ring-2 focus:ring-brand-primary focus:outline-none disabled:cursor-not-allowed disabled:opacity-50 {className}"
	{oninput}
	{onfocus}
	{onblur}
	{onkeydown}
/>
