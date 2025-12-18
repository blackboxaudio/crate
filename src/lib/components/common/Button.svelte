<script lang="ts">
	import type { Snippet } from 'svelte'

	type Props = {
		variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
		size?: 'sm' | 'md' | 'lg'
		disabled?: boolean
		type?: 'button' | 'submit' | 'reset'
		class?: string
		onclick?: (e: MouseEvent) => void
		children: Snippet
	}

	let {
		variant = 'secondary',
		size = 'md',
		disabled = false,
		type = 'button',
		class: className = '',
		onclick,
		children,
	}: Props = $props()

	const baseStyles =
		'inline-flex items-center justify-center font-medium rounded-md transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-offset-zinc-900 disabled:opacity-50 disabled:cursor-not-allowed'

	const variantStyles = {
		primary: 'bg-blue-600 text-white hover:bg-blue-700 focus:ring-blue-500',
		secondary: 'bg-zinc-700 text-zinc-100 hover:bg-zinc-600 focus:ring-zinc-500',
		ghost: 'bg-transparent text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100 focus:ring-zinc-500',
		danger: 'bg-red-600 text-white hover:bg-red-700 focus:ring-red-500',
	}

	const sizeStyles = {
		sm: 'px-2.5 py-1.5 text-xs',
		md: 'px-3 py-2 text-sm',
		lg: 'px-4 py-2 text-base',
	}
</script>

<button {type} {disabled} class="{baseStyles} {variantStyles[variant]} {sizeStyles[size]} {className}" {onclick}>
	{@render children()}
</button>
