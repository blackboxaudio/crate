<script lang="ts">
	import type { Snippet } from 'svelte'
	import { scale } from 'svelte/transition'
	import Text from './Text.svelte'

	type Props = {
		open: boolean
		title?: string
		size?: 'sm' | 'md' | 'lg'
		onClose: () => void
		onSubmit?: () => void
		children: Snippet
		footer?: Snippet
	}

	let { open, title, size = 'sm', onClose, onSubmit, children, footer }: Props = $props()

	let dialogEl: HTMLDialogElement | undefined = $state()
	let visible = $state(false)

	// Open dialog when open becomes true
	$effect(() => {
		if (!dialogEl) return
		if (open) {
			visible = true
			dialogEl.showModal()
		}
	})

	// Handle transition end to close dialog
	function handleOutroEnd() {
		dialogEl?.close()
		visible = false
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.stopPropagation()
			e.preventDefault()
			onClose()
		} else if (e.key === 'Enter' && onSubmit) {
			const target = e.target as HTMLElement
			if (target.tagName !== 'TEXTAREA' && target.tagName !== 'BUTTON') {
				e.stopPropagation()
				e.preventDefault()
				onSubmit()
			}
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
	class="fixed inset-0 m-0 h-full max-h-none w-full max-w-none bg-transparent p-0 backdrop:bg-black/60"
	onkeydown={handleKeydown}
	onclick={handleBackdropClick}
>
	{#if visible}
		<div
			class="fixed top-1/2 left-1/2 flex max-h-[85vh] w-full max-w-md -translate-x-1/2
				-translate-y-1/2 flex-col rounded-lg border border-stroke bg-surface-1 text-text-primary shadow-xl"
			transition:scale={{ start: 0.95, duration: 200 }}
			onoutroend={handleOutroEnd}
		>
			{#if title}
				<div class="border-b border-stroke-subtle px-4 py-3">
					<Text variant="header-1" weight="medium">{title}</Text>
				</div>
			{/if}

			<div class="min-h-0 overflow-y-auto px-4 py-4">
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
