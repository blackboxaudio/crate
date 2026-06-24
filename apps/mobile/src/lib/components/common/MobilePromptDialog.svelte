<script lang="ts">
	import { onMount, tick } from 'svelte'
	import { fade, scale } from 'svelte/transition'
	import { translate } from '$shared/i18n'
	import { easeFluid } from '$lib/easing'

	// A centered iOS-alert-style prompt with a single text field (the UIAlertController-with-text-field
	// pattern used by Files' "New Folder", Music's "New Playlist", etc.). Replaces the bottom-sheet modal
	// for naming actions: a sheet docked at the bottom is immediately covered by the iOS keyboard, whereas a
	// compact card centered in the *visible* viewport (we track `visualViewport`, which shrinks to the area
	// above the keyboard) always stays in view as the keyboard rises. Used for create-folder / create-playlist
	// / rename. Controlled by `open`; the parent flips it and Svelte's in/out transitions animate the card.
	type Props = {
		open: boolean
		title: string
		/** Optional helper line under the title. */
		message?: string
		/** The text field's value (bindable). */
		value: string
		placeholder?: string
		confirmLabel?: string
		cancelLabel?: string
		/** Disable the confirm action (e.g. while the field is blank). */
		confirmDisabled?: boolean
		onConfirm: () => void
		onCancel: () => void
	}
	let {
		open,
		title,
		message,
		value = $bindable(),
		placeholder,
		confirmLabel,
		cancelLabel,
		confirmDisabled = false,
		onConfirm,
		onCancel,
	}: Props = $props()

	// Center the card within the visible viewport (above the keyboard). `visualViewport` reports the region
	// not covered by the keyboard; we size+offset the flex frame to it so the card re-centers as the keyboard
	// animates in. Falls back to the layout viewport where `visualViewport` is unavailable.
	let frameTop = $state(0)
	let frameH = $state(0)
	onMount(() => {
		const vv = window.visualViewport
		const measure = () => {
			frameTop = vv?.offsetTop ?? 0
			frameH = vv?.height ?? window.innerHeight
		}
		measure()
		vv?.addEventListener('resize', measure)
		vv?.addEventListener('scroll', measure)
		window.addEventListener('resize', measure)
		return () => {
			vv?.removeEventListener('resize', measure)
			vv?.removeEventListener('scroll', measure)
			window.removeEventListener('resize', measure)
		}
	})

	// Autofocus the field when the dialog mounts and select any existing text (so a rename can be typed over).
	function autofocus(node: HTMLInputElement) {
		tick().then(() => {
			node.focus()
			node.select()
		})
	}

	function confirm() {
		if (confirmDisabled) return
		onConfirm()
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault()
			confirm()
		} else if (e.key === 'Escape') {
			e.preventDefault()
			onCancel()
		}
	}
</script>

{#if open}
	<!-- Backdrop: dim + light blur; tap to cancel. -->
	<button
		type="button"
		aria-label={$translate('common.cancel')}
		class="fixed inset-0 z-[70]"
		style="background-color: rgba(0,0,0,0.4); -webkit-backdrop-filter: blur(2px); backdrop-filter: blur(2px);"
		transition:fade={{ duration: 160 }}
		onclick={onCancel}
	></button>

	<!-- Frame sized to the visible viewport so the centered card clears the keyboard. -->
	<div
		class="pointer-events-none fixed left-0 z-[70] flex w-full items-center justify-center px-10"
		style="top: {frameTop}px; height: {frameH}px;"
	>
		<div
			role="dialog"
			aria-modal="true"
			aria-label={title}
			class="pointer-events-auto w-full max-w-[20rem] overflow-hidden rounded-2xl border border-stroke-subtle bg-surface-1 shadow-2xl"
			transition:scale={{ duration: 220, start: 1.08, opacity: 0, easing: easeFluid }}
		>
			<div class="px-5 pt-5 pb-4 text-center">
				<h2 class="text-base font-semibold text-text-primary">{title}</h2>
				{#if message}
					<p class="mt-1 text-sm text-text-secondary">{message}</p>
				{/if}
				<input
					type="text"
					bind:value
					{placeholder}
					use:autofocus
					onkeydown={onKeydown}
					autocapitalize="words"
					autocorrect="off"
					spellcheck="false"
					class="mt-4 w-full rounded-lg border border-stroke bg-surface-0 px-3 py-2 text-center text-sm text-text-primary placeholder:text-text-tertiary focus:border-brand-primary focus:outline-none"
				/>
			</div>

			<!-- iOS-style split button row: Cancel | Confirm, divided by hairlines. -->
			<div class="grid grid-cols-2 border-t border-stroke-subtle">
				<button
					type="button"
					class="border-r border-stroke-subtle py-3 text-base text-text-secondary active:bg-surface-2"
					onclick={onCancel}
				>
					{cancelLabel ?? $translate('common.cancel')}
				</button>
				<button
					type="button"
					class="py-3 text-base font-semibold text-brand-primary active:bg-surface-2 disabled:opacity-40"
					disabled={confirmDisabled}
					onclick={confirm}
				>
					{confirmLabel ?? $translate('common.create')}
				</button>
			</div>
		</div>
	</div>
{/if}
