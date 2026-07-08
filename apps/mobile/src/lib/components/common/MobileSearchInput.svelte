<script lang="ts">
	// Shared mobile search field: a leading magnifier glyph over a rounded input, used by the Discovery,
	// Following, and Playlists toolbars so every searchable context reads the same. Controlled (not
	// `bind:value`) so callers can drive it from either local state or a store — Discovery binds it to the
	// shared discovery filter, the others to a local query — via the `value` prop + `oninput` callback.
	interface Props {
		value: string
		placeholder?: string
		oninput?: (value: string) => void
		ariaLabel?: string
	}
	let { value, placeholder = '', oninput, ariaLabel }: Props = $props()
</script>

<div class="relative min-w-0 flex-1">
	<svg
		class="pointer-events-none absolute top-1/2 left-2.5 h-4 w-4 -translate-y-1/2 text-text-tertiary"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
	>
		<circle cx="11" cy="11" r="7" />
		<path d="M21 21l-4.3-4.3" stroke-linecap="round" />
	</svg>
	<input
		type="text"
		{value}
		oninput={(e) => oninput?.(e.currentTarget.value)}
		{placeholder}
		aria-label={ariaLabel ?? placeholder}
		autocapitalize="off"
		autocomplete="off"
		autocorrect="off"
		spellcheck="false"
		class="h-9 w-full rounded-md border border-stroke bg-surface-2 pr-3 pl-8 text-sm text-text-primary transition-colors placeholder:text-text-tertiary focus:border-brand-primary focus:shadow-[0_0_0_3px_var(--brand-muted)]"
	/>
</div>
