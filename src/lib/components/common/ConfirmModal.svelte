<script lang="ts">
	import Modal from './Modal.svelte'
	import Button from './Button.svelte'
	import Text from './Text.svelte'
	import Icon from './Icon.svelte'
	import { translate } from '$lib/i18n'

	type Props = {
		open: boolean
		title: string
		message: string
		warnings?: string[]
		checkboxLabel?: string
		checkboxChecked?: boolean
		confirmLabel?: string
		cancelLabel?: string
		destructive?: boolean
		onConfirm: (checkboxChecked: boolean) => void
		onCancel: () => void
	}

	let {
		open,
		title,
		message,
		warnings = [],
		checkboxLabel,
		checkboxChecked = $bindable(false),
		confirmLabel,
		cancelLabel,
		destructive = false,
		onConfirm,
		onCancel,
	}: Props = $props()

	function handleConfirm() {
		onConfirm(checkboxChecked)
	}
</script>

<Modal {open} {title} onClose={onCancel} onSubmit={handleConfirm}>
	<div class="space-y-4">
		<Text color="secondary">{message}</Text>

		{#if warnings.length > 0}
			<div class="rounded-md border border-warning/20 bg-warning/10 p-3">
				<div class="flex gap-2">
					<Icon name="warning" class="h-5 w-5 flex-shrink-0 text-warning" />
					<div class="space-y-1">
						{#each warnings as warning (warning)}
							<Text color="warning">{warning}</Text>
						{/each}
					</div>
				</div>
			</div>
		{/if}

		{#if checkboxLabel}
			<label class="flex cursor-pointer items-center gap-3">
				<input
					type="checkbox"
					bind:checked={checkboxChecked}
					class="h-4 w-4 rounded border-stroke bg-surface-2 text-brand-primary"
				/>
				<Text color="secondary" as="span">{checkboxLabel}</Text>
			</label>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onCancel}>{cancelLabel || $translate('common.cancel')}</Button>
		<Button
			variant={destructive ? 'primary' : 'primary'}
			class={destructive ? 'bg-red-600 hover:bg-red-700' : ''}
			onclick={handleConfirm}
		>
			{confirmLabel || $translate('common.confirm')}
		</Button>
	{/snippet}
</Modal>
