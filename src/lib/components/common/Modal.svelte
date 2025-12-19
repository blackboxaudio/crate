<script lang="ts">
	import type { Snippet } from 'svelte'

	type Props = {
		open: boolean
		title?: string
		onClose: () => void
		children: Snippet
		footer?: Snippet
	}

	let { open, title, onClose, children, footer }: Props = $props()

	let dialogEl: HTMLDialogElement | undefined = $state()

	// Sync dialog open state
	$effect(() => {
		if (!dialogEl) return

		if (open && !dialogEl.open) {
			dialogEl.showModal()
		} else if (!open && dialogEl.open) {
			dialogEl.close()
		}
	})

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault()
			onClose()
		}
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === dialogEl) {
			onClose()
		}
	}
</script>

<dialog
	bind:this={dialogEl}
	class="fixed top-1/2 left-1/2 max-h-[85vh] w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg border border-stroke bg-surface-1 p-0 text-text-primary shadow-xl backdrop:bg-black/60"
	onkeydown={handleKeydown}
	onclick={handleBackdropClick}
>
	{#if open}
		<div class="flex flex-col">
			{#if title}
				<div class="border-b border-stroke-subtle px-4 py-3">
					<h2 class="text-lg font-medium">{title}</h2>
				</div>
			{/if}

			<div class="px-4 py-4">
				{@render children()}
			</div>

			{#if footer}
				<div class="flex justify-end gap-2 border-t border-stroke-subtle px-4 py-3">
					{@render footer()}
				</div>
			{/if}
		</div>
	{/if}
</dialog>
