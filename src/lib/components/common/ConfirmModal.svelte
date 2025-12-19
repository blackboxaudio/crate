<script lang="ts">
	import Modal from './Modal.svelte'
	import Button from './Button.svelte'

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
		confirmLabel = 'Confirm',
		cancelLabel = 'Cancel',
		destructive = false,
		onConfirm,
		onCancel,
	}: Props = $props()

	function handleConfirm() {
		onConfirm(checkboxChecked)
	}
</script>

<Modal {open} {title} onClose={onCancel}>
	<div class="space-y-4">
		<p class="text-sm text-text-secondary">{message}</p>

		{#if warnings.length > 0}
			<div class="rounded-md border border-warning/20 bg-warning/10 p-3">
				<div class="flex gap-2">
					<svg class="h-5 w-5 flex-shrink-0 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width="2"
							d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
						/>
					</svg>
					<div class="space-y-1">
						{#each warnings as warning (warning)}
							<p class="text-sm text-warning">{warning}</p>
						{/each}
					</div>
				</div>
			</div>
		{/if}

		{#if checkboxLabel}
			<label class="flex cursor-pointer items-center gap-2">
				<input
					type="checkbox"
					bind:checked={checkboxChecked}
					class="h-4 w-4 rounded border-stroke bg-surface-2 text-brand-primary focus:ring-brand-primary focus:ring-offset-0"
				/>
				<span class="text-sm text-text-secondary">{checkboxLabel}</span>
			</label>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onCancel}>{cancelLabel}</Button>
		<Button
			variant={destructive ? 'primary' : 'primary'}
			class={destructive ? 'bg-red-600 hover:bg-red-700' : ''}
			onclick={handleConfirm}
		>
			{confirmLabel}
		</Button>
	{/snippet}
</Modal>
