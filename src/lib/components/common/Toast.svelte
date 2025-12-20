<script lang="ts">
	import type { Toast, ToastType } from '$lib/stores/toast'
	import { fly } from 'svelte/transition'
	import Icon from './Icon.svelte'

	type Props = {
		toast: Toast
		onDismiss: () => void
	}

	let { toast, onDismiss }: Props = $props()

	const baseStyles = 'flex items-center gap-3 rounded-md border px-4 py-3 shadow-lg min-w-[300px] max-w-[400px]'

	const typeStyles: Record<ToastType, string> = {
		success: 'bg-green-600/40 border-green-500/50 text-text-primary',
		error: 'bg-red-600/40 border-red-500/50 text-text-primary',
		warning: 'bg-amber-600/40 border-amber-500/50 text-text-primary',
		info: 'bg-blue-600/40 border-blue-500/50 text-text-primary',
	}

	const iconNames: Record<ToastType, string> = {
		success: 'check',
		error: 'alert-circle',
		warning: 'warning',
		info: 'info',
	}
</script>

<div class="bg-surface-0">
	<div class="{baseStyles} {typeStyles[toast.type]}" role="alert" transition:fly={{ x: 100, duration: 200 }}>
		<!-- Icon -->
		<Icon name={iconNames[toast.type]} class="h-5 w-5 flex-shrink-0" />

		<!-- Message -->
		<span class="flex-1 text-sm">{toast.message}</span>

		<!-- Close button -->
		<button
			type="button"
			class="flex-shrink-0 rounded p-1 opacity-70 transition-opacity hover:cursor-pointer hover:opacity-100 focus:ring-2 focus:ring-white/20 focus:outline-none"
			onclick={onDismiss}
			aria-label="Dismiss"
		>
			<Icon name="x" />
		</button>
	</div>
</div>
