<script lang="ts">
	import { translate } from '$shared/i18n'
	import Tooltip from '$lib/components/common/Tooltip.svelte'

	type Props = {
		fileSystem: string
	}

	let { fileSystem }: Props = $props()

	// FAT32 is compatible with Pioneer gear
	// Note: FAT32 sometimes reports as "msdos" on some systems
	const isCompatible = $derived(fileSystem.toLowerCase() === 'fat32' || fileSystem.toLowerCase() === 'msdos')

	const tooltipMessage = $derived.by(() => {
		let message = isCompatible
			? $translate('devices.statusCompatible')
			: $translate('devices.statusIncompatible', { values: { fileSystem } })
		// UX-safeguard in case fileSystem is an empty string
		message = message.replace(' ()', '')

		return message
	})
</script>

<Tooltip text={tooltipMessage} delay={250} position="left">
	<span class="relative flex size-2" role="status" aria-label={tooltipMessage}>
		<!-- Pulsing indicator -->
		{#if isCompatible}
			<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75"></span>
			<span class="relative inline-flex size-2 rounded-full bg-emerald-500"></span>
		{:else}
			<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-amber-400 opacity-75"></span>
			<span class="relative inline-flex size-2 rounded-full bg-amber-500"></span>
		{/if}
	</span>
</Tooltip>
