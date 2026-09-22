<script lang="ts">
	import type { Snippet } from 'svelte'
	import { translate } from '$shared/i18n'

	// The one header every pushed (drill-in) screen wears — folder, playlist, tag, and followed-source
	// levels — so a push always reads the same: back chevron, an optional glyph (color dot / avatar), the
	// title, and optional trailing actions. Owns the top safe-area inset and mirrors the fixed top bar's
	// surface-1 + hairline, so drill-in headers register as the same app chrome the tab root shows.
	type Props = {
		title: string
		onBack: () => void
		leading?: Snippet
		trailing?: Snippet
	}
	let { title, onBack, leading, trailing }: Props = $props()
</script>

<div class="pt-safe border-b border-stroke-subtle bg-surface-1">
	<div class="flex items-center gap-1 px-2 py-2">
		<button
			type="button"
			class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
			aria-label={$translate('common.back')}
			onclick={onBack}
		>
			<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
		</button>
		{@render leading?.()}
		<h1 class="min-w-0 flex-1 truncate text-lg font-semibold text-text-primary">{title}</h1>
		{@render trailing?.()}
	</div>
</div>
