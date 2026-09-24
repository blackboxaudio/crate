<script lang="ts">
	import type { Snippet } from 'svelte'

	// Inset-grouped form section (the iOS Settings look, matching the grouped lists the settings pages
	// already use): an optional uppercase header above a rounded card of hairline-separated rows, with an
	// optional footnote below. Rows are supplied by the caller — FormTextField/FormRow render as rows, but
	// any content works. `footer` is for the persistent explainer line; transient state lines (errors,
	// progress) fit there too via the `footerExtra` snippet so they stack under the card, not inside it.
	type Props = {
		title?: string
		/** Footnote line under the card (the iOS grouped-table footer). */
		footer?: string
		/** Extra footnote content below `footer` (validation errors, busy spinners, …). */
		footerExtra?: Snippet
		children: Snippet
	}

	let { title, footer, footerExtra, children }: Props = $props()
</script>

<section class="flex flex-col">
	{#if title}
		<h3 class="px-4 pb-1.5 text-xs font-semibold tracking-wide text-text-tertiary uppercase">{title}</h3>
	{/if}

	<div class="divide-y divide-stroke-subtle overflow-hidden rounded-xl border border-stroke-subtle bg-surface-1">
		{@render children()}
	</div>

	{#if footer}
		<p class="px-4 pt-1.5 text-xs text-text-tertiary">{footer}</p>
	{/if}
	{#if footerExtra}
		<div class="px-4 pt-1.5">{@render footerExtra()}</div>
	{/if}
</section>
