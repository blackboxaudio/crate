<script lang="ts">
	import type { Toast, ToastType } from '$lib/stores/toast'
	import { fly } from 'svelte/transition'
	import { cubicOut } from 'svelte/easing'
	import Icon from './Icon.svelte'
	import IconButton from './IconButton.svelte'
	import Text from './Text.svelte'

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

<div class="bg-surface-0" transition:fly|global={{ x: 40, duration: 250, easing: cubicOut }}>
	<div class="{baseStyles} {typeStyles[toast.type]}" role="alert">
		<!-- Icon -->
		<Icon name={iconNames[toast.type]} class="h-5 w-5 flex-shrink-0" />

		<!-- Message -->
		<Text as="span" class="flex-1">{toast.message}</Text>

		<!-- Close button -->
		<IconButton icon="x" size="sm" class="flex-shrink-0 opacity-70 hover:opacity-100" onclick={onDismiss} />
	</div>
</div>
