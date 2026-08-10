<script lang="ts">
	import type { Snippet } from 'svelte'
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import { confirmDialog } from '$lib/utils/dialog'
	import { lightTap } from '$lib/utils/haptics'
	import Drawer from './Drawer.svelte'

	// Form sheet — the one surface for multi-field forms (add release, smart playlist editor, edit
	// release, add follow source). Sibling of `MobileModal` (the picker/option sheet): same Drawer
	// baseline and grab handle, but an iOS-style nav bar (Cancel · title · primary action) instead of a
	// plain title row. Two detents: 'full' pins a near-full height so text fields sit high on screen,
	// well clear of the keyboard rising from the bottom (right for keyboard-first forms that can grow —
	// add release); 'auto' hugs the content like a native medium-detent sheet, with a spacer that tracks
	// `visualViewport` so the content lifts above the iOS keyboard instead of being covered by it (the
	// panel itself stays bottom-anchored — only its content rides up).
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
		/** 'full' (default) pins ~92vh; 'auto' hugs the content (capped at the same height). */
		height?: 'full' | 'auto'
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
		height = 'full',
		children: body,
	}: Props = $props()

	// Keyboard clearance for the 'auto' detent: `visualViewport` reports the region the keyboard doesn't
	// cover, so `innerHeight - vv.height - vv.offsetTop` is the overlap. Rendered as a spacer at the
	// panel's bottom, it pushes the content (not the panel) above the keyboard — same measurement
	// MobilePromptDialog uses to keep its card centered in the visible viewport. The 'full' detent
	// doesn't need it (its fields sit high by construction), so the listeners only attach for 'auto'.
	let keyboardInset = $state(0)
	onMount(() => {
		if (height !== 'auto') return
		const vv = window.visualViewport
		if (!vv) return
		const measure = () => {
			keyboardInset = Math.max(0, window.innerHeight - vv.height - vv.offsetTop)
		}
		measure()
		vv.addEventListener('resize', measure)
		vv.addEventListener('scroll', measure)
		return () => {
			vv.removeEventListener('resize', measure)
			vv.removeEventListener('scroll', measure)
		}
	})

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
	class="pb-safe flex {height === 'auto'
		? 'max-h-[92vh]'
		: 'h-[92vh]'} flex-col overflow-hidden rounded-t-2xl border-t border-stroke bg-surface-0"
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
						onclick={() => {
							void lightTap()
							onSubmit?.()
						}}
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

		{#if keyboardInset > 0}
			<div style="height: {keyboardInset}px" class="flex-shrink-0"></div>
		{/if}
	{/snippet}
</Drawer>
