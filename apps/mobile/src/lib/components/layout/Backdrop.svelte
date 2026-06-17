<script lang="ts">
	import { fade } from 'svelte/transition'
	import { translate } from '$shared/i18n'

	// Full-screen dimming overlay shown behind an open drawer; tap (or Enter/Space when focused) to
	// dismiss. A <button> rather than a div so it's keyboard-accessible with correct semantics.
	type Props = {
		show: boolean
		onclick: () => void
	}

	let { show, onclick }: Props = $props()
</script>

{#if show}
	<button
		type="button"
		class="fixed inset-0 z-40 bg-black/50 motion-reduce:transition-none"
		transition:fade={{ duration: 200 }}
		aria-label={$translate('common.close')}
		{onclick}
	></button>
{/if}
