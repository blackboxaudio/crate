<script lang="ts">
	import Modal from './Modal.svelte'
	import Input from './Input.svelte'
	import Button from './Button.svelte'
	import { translate } from '$lib/i18n'

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

	// Reset value when modal opens
	$effect(() => {
		if (open) {
			inputValue = initialValue
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
	<Input bind:value={inputValue} {placeholder} autofocus onkeydown={handleKeydown} />

	{#snippet footer()}
		<Button variant="ghost" onclick={handleCancel}>{$translate('common.cancel')}</Button>
		<Button variant="primary" onclick={handleSubmit} disabled={!inputValue.trim()}>
			{submitLabel || $translate('common.submit')}
		</Button>
	{/snippet}
</Modal>
