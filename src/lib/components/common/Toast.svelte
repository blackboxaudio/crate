<script lang="ts">
	import type { Toast, ToastType } from '$lib/stores/toast'
	import { fly } from 'svelte/transition'
	import Icon from './Icon.svelte'

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

	const iconNames: Record<ToastType, string> = {
		success: 'check',
		error: 'x',
		warning: 'warning',
		info: 'info',
	}
</script>

<div class="{baseStyles} {typeStyles[toast.type]}" role="alert" transition:fly={{ x: 100, duration: 200 }}>
	<!-- Icon -->
	<Icon name={iconNames[toast.type]} class="h-5 w-5 flex-shrink-0" />

	<!-- Message -->
	<span class="flex-1 text-sm">{toast.message}</span>

	<!-- Close button -->
	<button
		type="button"
		class="flex-shrink-0 rounded p-1 opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-white/20 focus:outline-none"
		onclick={onDismiss}
		aria-label="Dismiss"
	>
		<Icon name="x" />
	</button>
</div>
