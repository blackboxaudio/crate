<script lang="ts">
	import Icon from './Icon.svelte'

	type Props = {
		title?: string
		disabled?: boolean
		active?: boolean
		size?: 'sm' | 'md' | 'lg'
		class?: string
		icon: string
		iconClass?: string
		fill?: boolean
		onclick?: (e: MouseEvent) => void
	}

	let {
		title = '',
		disabled = false,
		active = false,
		size = 'md',
		class: className = '',
		icon,
		iconClass = '',
		fill = false,
		onclick,
	}: Props = $props()

	const sizeStyles = {
		sm: 'w-6 h-6 text-sm',
		md: 'w-8 h-8 text-base',
		lg: 'w-10 h-10 text-lg',
	}
</script>

<button
	type="button"
	{title}
	{disabled}
	class="inline-flex items-center justify-center rounded-md transition-colors focus:ring-2 focus:ring-brand-primary focus:outline-none disabled:cursor-not-allowed disabled:opacity-50 {active
		? 'bg-surface-2 text-text-primary'
		: 'text-text-secondary hover:cursor-pointer hover:bg-surface-2 hover:text-text-primary'} {sizeStyles[
		size
	]} {className}"
	{onclick}
>
	<Icon name={icon} class={iconClass || undefined} {fill} />
</button>
