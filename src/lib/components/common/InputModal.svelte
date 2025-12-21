<script lang="ts">
	import Modal from './Modal.svelte'
	import Input from './Input.svelte'
	import Button from './Button.svelte'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	type Props = {
		open: boolean
		title: string
		placeholder?: string
		initialValue?: string
		submitLabel?: string
		onSubmit: (value: string) => void
		onCancel: () => void
	}

	let { open, title, placeholder = '', initialValue = '', submitLabel, onSubmit, onCancel }: Props = $props()

	let inputValue = $state('')
	let inputEl: HTMLInputElement | undefined = $state()

	// Reset value when modal opens
	$effect(() => {
		if (open) {
			inputValue = initialValue
			// Focus the input after a short delay to allow the modal to render
			setTimeout(() => inputEl?.focus(), 50)
		}
	})

	function handleSubmit() {
		if (inputValue.trim()) {
			onSubmit(inputValue.trim())
			inputValue = ''
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault()
			handleSubmit()
		}
	}

	function handleCancel() {
		inputValue = ''
		onCancel()
	}
</script>

<Modal {open} {title} onClose={handleCancel}>
	<Input bind:value={inputValue} {placeholder} onkeydown={handleKeydown} />

	{#snippet footer()}
		<Button variant="ghost" onclick={handleCancel}>{$translate('common.cancel')}</Button>
		<Button variant="primary" onclick={handleSubmit} disabled={!inputValue.trim()}>
			{submitLabel || $translate('common.submit')}
		</Button>
	{/snippet}
</Modal>
