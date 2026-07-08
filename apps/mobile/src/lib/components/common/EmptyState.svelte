<script lang="ts">
	import type { Snippet } from 'svelte'

	// Shared list-tab empty state: optional icon, a title, an optional hint line, and an optional muted
	// CTA. Generalizes the Following tab's empty anatomy so every tab's "nothing here" moment reads the
	// same. Discovery's full-screen onboarding hero (filled CTA) is intentionally NOT this component.
	type Props = {
		icon?: Snippet
		title: string
		hint?: string
		ctaLabel?: string
		onCta?: () => void
	}

	let { icon, title, hint, ctaLabel, onCta }: Props = $props()
</script>

<div class="flex flex-col items-center gap-2 py-10 text-center">
	{#if icon}
		<span class="text-text-tertiary">{@render icon()}</span>
	{/if}
	<div class="text-sm font-medium text-text-primary">{title}</div>
	{#if hint}
		<div class="max-w-xs text-xs text-text-tertiary">{hint}</div>
	{/if}
	{#if ctaLabel && onCta}
		<button
			type="button"
			class="mt-2 rounded-lg bg-brand-muted px-4 py-2 text-sm font-medium text-brand-primary active:opacity-80"
			onclick={onCta}
		>
			{ctaLabel}
		</button>
	{/if}
</div>
