<script lang="ts">
	import type { Snippet } from 'svelte'

	type Props = {
		variant?: 'primary' | 'secondary' | 'ghost' | 'danger' | 'ghost-danger' | 'outline'
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
		'inline-flex items-center justify-center font-medium rounded-md transition-[colors,filter,opacity] hover:cursor-pointer focus:outline-none focus:ring-0 focus-visible:outline-none focus-visible:ring-0 disabled:opacity-50 disabled:cursor-not-allowed'

	const variantStyles = {
		primary: 'bg-brand-primary text-white hover:bg-brand-hover',
		secondary: 'bg-surface-2 text-text-primary hover:brightness-95',
		ghost: 'bg-transparent text-text-secondary hover:bg-surface-2 hover:text-text-primary',
		danger: 'bg-danger text-white hover:bg-danger/90',
		'ghost-danger': 'bg-transparent text-red-500 hover:bg-red-500/10',
		outline: 'bg-surface-2 border border-stroke text-text-primary hover:brightness-95',
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
