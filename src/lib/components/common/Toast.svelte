<script lang="ts">
	import type { Toast, ToastType } from '$lib/stores/toast'
	import { fly } from 'svelte/transition'

	type Props = {
		toast: Toast
		onDismiss: () => void
	}

	let { toast, onDismiss }: Props = $props()

	const baseStyles = 'flex items-center gap-3 rounded-lg border px-4 py-3 shadow-lg min-w-[300px] max-w-[400px]'

	const typeStyles: Record<ToastType, string> = {
		success: 'bg-green-600/20 border-green-500/50 text-green-100',
		error: 'bg-red-600/20 border-red-500/50 text-red-100',
		warning: 'bg-amber-600/20 border-amber-500/50 text-amber-100',
		info: 'bg-blue-600/20 border-blue-500/50 text-blue-100',
	}

	const iconPaths: Record<ToastType, string> = {
		success: 'M5 13l4 4L19 7',
		error: 'M6 18L18 6M6 6l12 12',
		warning:
			'M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z',
		info: 'M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z',
	}
</script>

<div class="{baseStyles} {typeStyles[toast.type]}" role="alert" transition:fly={{ x: 100, duration: 200 }}>
	<!-- Icon -->
	<svg
		class="h-5 w-5 flex-shrink-0"
		fill="none"
		stroke="currentColor"
		viewBox="0 0 24 24"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
	>
		<path d={iconPaths[toast.type]} />
	</svg>

	<!-- Message -->
	<span class="flex-1 text-sm">{toast.message}</span>

	<!-- Close button -->
	<button
		type="button"
		class="flex-shrink-0 rounded p-1 opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-white/20 focus:outline-none"
		onclick={onDismiss}
		aria-label="Dismiss"
	>
		<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
			<path d="M6 18L18 6M6 6l12 12" stroke-linecap="round" stroke-linejoin="round" />
		</svg>
	</button>
</div>
