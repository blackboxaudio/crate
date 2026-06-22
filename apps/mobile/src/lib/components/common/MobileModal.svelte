<script lang="ts">
	import type { Snippet } from 'svelte'
	import { translate } from '$shared/i18n'
	import Drawer from './Drawer.svelte'

	// Bottom-sheet modal. Mirrors the desktop Modal's prop API (open / title / onClose / onSubmit /
	// children / footer) so callers feel identical, but presents as a sheet that slides up from the bottom,
	// clears the home indicator, and can be dragged down to dismiss. Slide / scrim / drag come from the
	// shared `Drawer` baseline (direction="bottom"); the drag is confined to the handle+header (Drawer's
	// `drag` action) so the content area stays independently scrollable. Foundation component for sheets.
	type Props = {
		open: boolean
		title?: string
		onClose: () => void
		onSubmit?: () => void
		children: Snippet
		footer?: Snippet
	}

	let { open, title, onClose, onSubmit, children: body, footer }: Props = $props()

	// Enter submits (unless focus is in a textarea/button), mirroring the desktop modal. Scoped to while the
	// sheet is open. Lives on `window` now that there's no <dialog> to host the handler.
	$effect(() => {
		if (!open || !onSubmit) return
		function onKey(e: KeyboardEvent) {
			if (e.key !== 'Enter') return
			const target = e.target as HTMLElement
			if (target.tagName !== 'TEXTAREA' && target.tagName !== 'BUTTON') {
				e.preventDefault()
				onSubmit?.()
			}
		}
		window.addEventListener('keydown', onKey)
		return () => window.removeEventListener('keydown', onKey)
	})
</script>

<Drawer
	{open}
	{onClose}
	direction="bottom"
	z={50}
	panelDrag={false}
	ariaLabel={title ?? $translate('common.close')}
	class="pb-safe glass-strong flex max-h-[85vh] flex-col overflow-hidden rounded-t-2xl border-t border-stroke"
>
	{#snippet children({ drag, animating })}
		<!-- Drag zone: the handle + header follow the finger to dismiss; the content below scrolls freely. -->
		<div use:drag>
			<div class="flex justify-center pt-2 pb-1">
				<span class="h-1 w-10 rounded-full bg-text-tertiary/50"></span>
			</div>

			{#if title}
				<div class="border-b border-stroke-subtle px-4 py-3">
					<h2 class="text-base font-medium text-text-primary">{title}</h2>
				</div>
			{/if}
		</div>

		<div class="min-h-0 flex-1 px-4 py-4 {animating ? 'overflow-y-hidden' : 'overflow-y-auto'}">
			{@render body()}
		</div>

		{#if footer}
			<div class="flex justify-end gap-2 border-t border-stroke-subtle px-4 py-3">
				{@render footer()}
			</div>
		{/if}
	{/snippet}
</Drawer>
