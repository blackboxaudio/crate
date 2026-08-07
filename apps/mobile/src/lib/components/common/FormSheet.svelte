<script lang="ts">
	import type { Snippet } from 'svelte'
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import { confirmDialog } from '$lib/utils/dialog'
	import Drawer from './Drawer.svelte'

	// Full-height form sheet — the one surface for multi-field forms (add release, smart playlist editor,
	// edit release, add follow source). Sibling of `MobileModal` (the picker/option sheet): same Drawer
	// baseline and grab handle, but an iOS-style nav bar (Cancel · title · primary action) instead of a
	// plain title row, at a uniform near-full height so text fields sit high on screen, well clear of the
	// keyboard rising from the bottom.
	//
	// Dismissal is CANCEL semantics, uniformly: scrim tap, swipe-down, Escape, Android Back, and the
	// Cancel button all discard — vetoed by a native "Discard changes?" confirm while `dirty`. Submitting
	// is the only committing path; the parent closes programmatically after it (never guarded).
	type Props = {
		open: boolean
		title: string
		/** Cancel-path close (user dismissed / discard confirmed). The parent flips `open` false. */
		onClose: () => void
		/** Fired after the slide-out lands — the place to reset form state (see Drawer). */
		onClosed?: () => void
		/** Primary action, rendered as the nav bar's trailing button (unless `action` replaces it). */
		onSubmit?: () => void
		submitLabel?: string
		submitDisabled?: boolean
		/** Unsaved edits exist: a user dismissal must first confirm discarding via the native dialog. */
		dirty?: boolean
		/** Replace the default trailing button (e.g. add-release swaps in "Add to Queue" when offline). */
		action?: Snippet
		/** See Drawer: keyboard-first sheets that focus a field on open slide by `bottom`, not transform. */
		positionSlide?: boolean
		children: Snippet
	}
	let {
		open,
		title,
		onClose,
		onClosed,
		onSubmit,
		submitLabel,
		submitDisabled = false,
		dirty = false,
		action,
		positionSlide = false,
		children: body,
	}: Props = $props()

	async function confirmDiscard(): Promise<boolean> {
		if (!dirty) return true
		const t = get(translate)
		return confirmDialog(t('modals.confirm.discardChangesMessage'), {
			title: t('modals.confirm.discardChangesTitle'),
			confirmLabel: t('common.discard'),
		})
	}

	// The Cancel button goes through the same discard guard as the Drawer's own dismiss paths.
	async function cancel() {
		if (await confirmDiscard()) onClose()
	}
</script>

<Drawer
	{open}
	{onClose}
	{onClosed}
	guardClose={confirmDiscard}
	direction="bottom"
	z={50}
	{positionSlide}
	panelDrag={false}
	portal
	ariaLabel={title}
	class="pb-safe flex h-[92vh] flex-col overflow-hidden rounded-t-2xl border-t border-stroke bg-surface-0"
>
	{#snippet children({ drag, animating })}
		<!-- Grab handle + nav bar (drag either to dismiss); the body below scrolls freely. -->
		<div use:drag>
			<div class="flex justify-center pt-2 pb-1">
				<span class="h-1 w-10 rounded-full bg-text-tertiary/50"></span>
			</div>
			<div class="grid grid-cols-[1fr_auto_1fr] items-center gap-2 border-b border-stroke-subtle px-4 py-3">
				<button
					type="button"
					class="justify-self-start text-sm font-medium text-text-secondary active:opacity-60"
					onclick={cancel}
				>
					{$translate('common.cancel')}
				</button>
				<h2 class="truncate text-base font-medium text-text-primary">{title}</h2>
				{#if action}
					<div class="justify-self-end">{@render action()}</div>
				{:else if onSubmit}
					<button
						type="button"
						class="justify-self-end text-sm font-semibold text-brand-primary active:opacity-60 disabled:opacity-40"
						disabled={submitDisabled}
						onclick={onSubmit}
					>
						{submitLabel ?? $translate('common.save')}
					</button>
				{:else}
					<span></span>
				{/if}
			</div>
		</div>

		<div class="min-h-0 flex-1 {animating ? 'overflow-hidden' : 'overflow-y-auto'}">
			{@render body()}
		</div>
	{/snippet}
</Drawer>
