<script lang="ts">
	import type { Snippet } from 'svelte'
	import { tick } from 'svelte'
	import { translate } from '$shared/i18n'

	// A text field as an inset-grouped row (rendered inside FormSection): small label over a borderless
	// input, so the group card is the only chrome — the free-floating bordered inputs are what made the
	// old forms read as web pages. 16px input text matches the native form size AND stays above iOS
	// Safari's auto-zoom threshold for focused fields. While focused and non-empty, an iOS-style
	// clear (×) affordance appears; `trailing` renders a row action next to the label (e.g. "Paste link").
	type Props = {
		/** Small label above the input; also its accessible name (via id). */
		label?: string
		value: string
		id?: string
		placeholder?: string
		type?: 'text' | 'url' | 'number' | 'date'
		multiline?: boolean
		rows?: number
		inputmode?: 'text' | 'numeric' | 'decimal' | 'url' | 'search'
		autocapitalize?: 'off' | 'sentences' | 'words'
		autocorrect?: 'on' | 'off'
		enterkeyhint?: 'enter' | 'done' | 'go' | 'next' | 'search'
		/** Focus the field as it mounts. `preventScroll` stops iOS from scrolling the document to "reveal"
		 *  the field while the sheet is still sliding in; `tick` (not a timeout) keeps the opening tap's
		 *  activation so the keyboard actually rises (see AddReleaseModal's original focusOnOpen). */
		focusOnOpen?: boolean
		oninput?: () => void
		/** Enter pressed (single-line only) — e.g. submit the sheet from the keyboard's Go key. */
		onenter?: () => void
		/** Row action rendered to the right of the label (e.g. a "Paste link" button). */
		trailing?: Snippet
	}
	let {
		label,
		value = $bindable(),
		id,
		placeholder,
		type = 'text',
		multiline = false,
		rows = 3,
		inputmode,
		autocapitalize,
		autocorrect,
		enterkeyhint,
		focusOnOpen = false,
		oninput,
		onenter,
		trailing,
	}: Props = $props()

	let focused = $state(false)

	function mountFocus(node: HTMLInputElement | HTMLTextAreaElement) {
		if (focusOnOpen) void tick().then(() => node.focus({ preventScroll: true }))
	}

	// pointerdown (not click) so the press never blurs the input — the keyboard must stay up after clearing.
	function clear(e: PointerEvent) {
		e.preventDefault()
		value = ''
		oninput?.()
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key !== 'Enter' || !onenter) return
		e.preventDefault()
		onenter()
	}

	const inputClass =
		'w-full min-w-0 flex-1 border-none bg-transparent p-0 text-base text-text-primary placeholder:text-text-tertiary focus:ring-0 focus:outline-none'
</script>

<div class="flex flex-col gap-0.5 px-4 py-2.5">
	{#if label || trailing}
		<div class="flex items-center justify-between gap-2">
			{#if label}
				<label for={id} class="text-xs font-medium text-text-secondary">{label}</label>
			{:else}
				<span></span>
			{/if}
			{#if trailing}{@render trailing()}{/if}
		</div>
	{/if}

	{#if multiline}
		<textarea use:mountFocus {id} bind:value {placeholder} {rows} {autocapitalize} class={inputClass}></textarea>
	{:else}
		<div class="flex items-center gap-2">
			<!-- Manual value sync: Svelte disallows `bind:value` alongside a dynamic `type` attribute. -->
			<input
				use:mountFocus
				{id}
				{type}
				{value}
				{placeholder}
				{inputmode}
				{autocapitalize}
				{autocorrect}
				{enterkeyhint}
				oninput={(e) => {
					value = e.currentTarget.value
					oninput?.()
				}}
				onkeydown={onKeydown}
				onfocus={() => (focused = true)}
				onblur={() => (focused = false)}
				class={inputClass}
			/>
			{#if focused && value}
				<button
					type="button"
					tabindex="-1"
					class="flex h-5 w-5 flex-shrink-0 items-center justify-center rounded-full bg-text-tertiary/25 text-surface-0"
					aria-label={$translate('common.clear')}
					onpointerdown={clear}
				>
					<svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
						<path d="M18 6L6 18M6 6l12 12" stroke-linecap="round" />
					</svg>
				</button>
			{/if}
		</div>
	{/if}
</div>
