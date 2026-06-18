<script lang="ts">
	import type { Snippet } from 'svelte'
	import { fly, fade } from 'svelte/transition'
	import { cubicOut } from 'svelte/easing'
	import { translate } from '$shared/i18n'

	// Bottom-sheet modal. Mirrors the desktop Modal's prop API (open / title / onClose / onSubmit /
	// children / footer) so callers feel identical, but presents as a sheet that slides up from the
	// bottom and clears the home indicator. Foundation component for later mobile features.
	type Props = {
		open: boolean
		title?: string
		onClose: () => void
		onSubmit?: () => void
		children: Snippet
		footer?: Snippet
	}

	let { open, title, onClose, onSubmit, children, footer }: Props = $props()

	let dialogEl: HTMLDialogElement | undefined = $state()
	let visible = $state(false)

	// Drive the native <dialog> from the `open` prop, but keep it mounted through the outro so the
	// slide-down animation can play before close().
	$effect(() => {
		if (!dialogEl) return
		if (open) {
			visible = true
			dialogEl.showModal()
		} else if (visible) {
			visible = false
		}
	})

	function handleOutroEnd() {
		dialogEl?.close()
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && onSubmit) {
			const target = e.target as HTMLElement
			if (target.tagName !== 'TEXTAREA' && target.tagName !== 'BUTTON') {
				e.preventDefault()
				onSubmit()
			}
		}
	}
</script>

<dialog
	bind:this={dialogEl}
	class="m-0 h-full max-h-none w-full max-w-none bg-transparent p-0"
	onkeydown={handleKeydown}
	oncancel={(e) => {
		e.preventDefault()
		onClose()
	}}
>
	{#if visible}
		<button
			type="button"
			class="fixed inset-0 bg-black/50"
			transition:fade={{ duration: 200 }}
			aria-label={$translate('common.close')}
			onclick={onClose}
		></button>
		<div
			class="pb-safe fixed inset-x-0 bottom-0 flex max-h-[85vh] flex-col overflow-hidden rounded-t-2xl border-t border-stroke bg-surface-1"
			transition:fly={{ y: 400, duration: 300, easing: cubicOut, opacity: 1 }}
			onoutroend={handleOutroEnd}
		>
			<div class="flex justify-center pt-2 pb-1">
				<span class="h-1 w-10 rounded-full bg-stroke"></span>
			</div>

			{#if title}
				<div class="border-b border-stroke-subtle px-4 py-3">
					<h2 class="text-base font-medium text-text-primary">{title}</h2>
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
