<script lang="ts">
	import { translate } from '$shared/i18n'
	import { toasts, toastStore, type ToastType } from '$shared/stores/toast'
	import { previewInfo } from '$shared/stores/player'
	import { fly } from 'svelte/transition'
	import { cubicOut } from 'svelte/easing'

	// Docked at the bottom, above the tab bar (3.5rem + bottom safe area) and lifted further when the
	// mini-player card is showing (~5rem, matching MobileShell's `--mini-player-inset`) so a toast never
	// hides the transport. High z-index (above the expanded player at z-50 and the context menus at
	// z-[70]) so an error is always visible, but below the boot splash (z-9999).
	const bottomOffset = $derived(
		$previewInfo
			? 'calc(3.5rem + env(safe-area-inset-bottom) + 5.75rem)'
			: 'calc(3.5rem + env(safe-area-inset-bottom) + 0.75rem)'
	)

	const typeStyles: Record<ToastType, string> = {
		success: 'bg-green-600/90 border-green-400/40 text-white',
		error: 'bg-red-600/90 border-red-400/40 text-white',
		warning: 'bg-amber-500/90 border-amber-300/40 text-white',
		info: 'bg-surface-2 border-border-subtle text-text-primary',
	}
</script>

{#if $toasts.length > 0}
	<div
		class="pointer-events-none fixed inset-x-0 z-[100] flex flex-col items-center gap-2 px-4"
		style="bottom: {bottomOffset}"
	>
		{#each $toasts as toast (toast.id)}
			<div
				role="alert"
				transition:fly|global={{ y: 24, duration: 220, easing: cubicOut }}
				class="pointer-events-auto flex w-full max-w-sm items-start gap-2.5 rounded-xl border px-4 py-3 shadow-lg backdrop-blur-md {typeStyles[
					toast.type
				]}"
			>
				<!-- Type icon (inline so this stays free of any desktop-only component import) -->
				<svg
					class="mt-0.5 h-5 w-5 flex-shrink-0"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				>
					{#if toast.type === 'success'}
						<path d="M20 6 9 17l-5-5" />
					{:else if toast.type === 'error'}
						<circle cx="12" cy="12" r="10" /><path d="M12 8v4M12 16h.01" />
					{:else if toast.type === 'warning'}
						<path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0Z" /><path
							d="M12 9v4M12 17h.01"
						/>
					{:else}
						<circle cx="12" cy="12" r="10" /><path d="M12 16v-4M12 8h.01" />
					{/if}
				</svg>

				<span class="min-w-0 flex-1 text-sm font-medium break-words">{toast.message}</span>

				{#if toast.action}
					<button
						type="button"
						class="flex-shrink-0 text-sm font-semibold underline underline-offset-2"
						onclick={() => {
							toast.action?.onClick()
							toastStore.dismiss(toast.id)
						}}
					>
						{toast.action.label}
					</button>
				{/if}

				<!-- Dismiss -->
				<button
					type="button"
					class="-mr-1 flex-shrink-0 opacity-70 active:opacity-100"
					aria-label={$translate('common.dismiss')}
					onclick={() => toastStore.dismiss(toast.id)}
				>
					<svg
						class="h-4 w-4"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					>
						<path d="M18 6 6 18M6 6l12 12" />
					</svg>
				</button>
			</div>
		{/each}
	</div>
{/if}
