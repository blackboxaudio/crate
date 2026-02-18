<script lang="ts">
	import type { Snippet } from 'svelte'

	type TextVariant =
		| 'header-1'
		| 'header-2'
		| 'header-3'
		| 'header-4'
		| 'header-table'
		| 'body-1'
		| 'body-2'
		| 'caption'
		| 'code'

	type TextSize = 'xs' | 'sm' | 'base' | 'lg'
	type TextWeight = 'normal' | 'medium' | 'semibold' | 'bold'
	type TextColor = 'primary' | 'secondary' | 'tertiary' | 'brand' | 'danger' | 'warning' | 'success'
	type TextElement = 'p' | 'span' | 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6' | 'div' | 'label'
	type TextTracking = 'normal' | 'wide' | 'wider'

	type VariantConfig = {
		element: TextElement
		size: TextSize
		weight: TextWeight
		color: TextColor
		tracking?: TextTracking
		uppercase?: boolean
	}

	type Props = {
		variant?: TextVariant
		size?: TextSize
		weight?: TextWeight
		color?: TextColor
		as?: TextElement
		truncate?: boolean
		uppercase?: boolean
		tracking?: TextTracking
		tabular?: boolean
		mono?: boolean
		italic?: boolean
		title?: string
		for?: string
		class?: string
		children: Snippet
	}

	let {
		variant = 'body-1',
		size,
		weight,
		color,
		as,
		truncate = false,
		uppercase,
		tracking,
		tabular = false,
		mono = false,
		italic = false,
		title,
		for: htmlFor,
		class: className = '',
		children,
	}: Props = $props()

	const variantConfig: Record<TextVariant, VariantConfig> = {
		'header-1': {
			element: 'h2',
			size: 'lg',
			weight: 'semibold',
			color: 'primary',
		},
		'header-2': {
			element: 'h3',
			size: 'sm',
			weight: 'semibold',
			color: 'primary',
		},
		'header-3': {
			element: 'h3',
			size: 'sm',
			weight: 'semibold',
			color: 'secondary',
			tracking: 'wide',
			uppercase: true,
		},
		'header-4': {
			element: 'span',
			size: 'xs',
			weight: 'medium',
			color: 'tertiary',
			tracking: 'wide',
			uppercase: true,
		},
		'header-table': {
			element: 'div',
			size: 'xs',
			weight: 'medium',
			color: 'tertiary',
			tracking: 'wider',
			uppercase: true,
		},
		'body-1': {
			element: 'p',
			size: 'sm',
			weight: 'normal',
			color: 'primary',
		},
		'body-2': {
			element: 'p',
			size: 'sm',
			weight: 'medium',
			color: 'primary',
		},
		caption: {
			element: 'span',
			size: 'xs',
			weight: 'normal',
			color: 'tertiary',
		},
		code: {
			element: 'span',
			size: 'xs',
			weight: 'normal',
			color: 'primary',
		},
	}

	const sizeStyles: Record<TextSize, string> = {
		xs: 'text-xs',
		sm: 'text-sm',
		base: 'text-base',
		lg: 'text-lg',
	}

	const weightStyles: Record<TextWeight, string> = {
		normal: 'font-normal',
		medium: 'font-medium',
		semibold: 'font-semibold',
		bold: 'font-bold',
	}

	const colorStyles: Record<TextColor, string> = {
		primary: 'text-text-primary',
		secondary: 'text-text-secondary',
		tertiary: 'text-text-tertiary',
		brand: 'text-brand-primary',
		danger: 'text-danger',
		warning: 'text-warning',
		success: 'text-success',
	}

	const trackingStyles: Record<TextTracking, string> = {
		normal: 'tracking-normal',
		wide: 'tracking-wide',
		wider: 'tracking-wider',
	}

	const config = $derived(variantConfig[variant])
	const resolvedElement = $derived(as ?? config.element)
	const resolvedSize = $derived(size ?? config.size)
	const resolvedWeight = $derived(weight ?? config.weight)
	const resolvedColor = $derived(color ?? config.color)
	const resolvedTracking = $derived(tracking ?? config.tracking)
	const resolvedUppercase = $derived(uppercase ?? config.uppercase ?? false)

	const classes = $derived(
		[
			sizeStyles[resolvedSize],
			weightStyles[resolvedWeight],
			colorStyles[resolvedColor],
			resolvedTracking ? trackingStyles[resolvedTracking] : '',
			resolvedUppercase ? 'uppercase' : '',
			truncate ? 'truncate' : '',
			tabular ? 'tabular-nums' : '',
			mono || variant === 'code' ? 'font-mono' : '',
			italic ? 'italic' : '',
			className,
		]
			.filter(Boolean)
			.join(' ')
	)
</script>

{#if resolvedElement === 'h1'}
	<h1 class={classes} {title}>{@render children()}</h1>
{:else if resolvedElement === 'h2'}
	<h2 class={classes} {title}>{@render children()}</h2>
{:else if resolvedElement === 'h3'}
	<h3 class={classes} {title}>{@render children()}</h3>
{:else if resolvedElement === 'h4'}
	<h4 class={classes} {title}>{@render children()}</h4>
{:else if resolvedElement === 'h5'}
	<h5 class={classes} {title}>{@render children()}</h5>
{:else if resolvedElement === 'h6'}
	<h6 class={classes} {title}>{@render children()}</h6>
{:else if resolvedElement === 'span'}
	<span class={classes} {title}>{@render children()}</span>
{:else if resolvedElement === 'div'}
	<div class={classes} {title}>{@render children()}</div>
{:else if resolvedElement === 'label'}
	<label class={classes} {title} for={htmlFor}>{@render children()}</label>
{:else}
	<p class={classes} {title}>{@render children()}</p>
{/if}
