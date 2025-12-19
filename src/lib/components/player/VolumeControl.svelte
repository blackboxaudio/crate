<script lang="ts">
	import { IconButton, Slider } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		volume: number
		onVolumeChange?: (volume: number) => void
	}

	let { volume, onVolumeChange }: Props = $props()

	let previousVolume = $state(0.5)
	let isMuted = $derived(volume === 0)

	function toggleMute() {
		if (isMuted) {
			onVolumeChange?.(previousVolume || 0.5)
		} else {
			previousVolume = volume
			onVolumeChange?.(0)
		}
	}

	function handleVolumeChange(e: Event) {
		const target = e.target as HTMLInputElement
		onVolumeChange?.(parseFloat(target.value))
	}
</script>

<div class="flex items-center gap-2">
	<IconButton title={isMuted ? 'Unmute' : 'Mute'} size="sm" onclick={toggleMute}>
		{#if isMuted}
			<Icon name="volume-muted" fill />
		{:else if volume < 0.5}
			<Icon name="volume-low" fill />
		{:else}
			<Icon name="volume-full" fill />
		{/if}
	</IconButton>

	<div class="w-24">
		<Slider value={volume} min={0} max={1} step={0.01} onchange={handleVolumeChange} />
	</div>
</div>
