<script lang="ts">
	import { IconButton, Slider } from '$lib/components/common'
	import { translate } from '$lib/i18n'

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
	<IconButton
		title={isMuted ? $translate('player.unmute') : $translate('player.mute')}
		size="sm"
		icon={isMuted ? 'volume-muted' : volume < 0.5 ? 'volume-low' : 'volume-full'}
		fill
		onclick={toggleMute}
	/>

	<div class="flex h-6 w-24 items-center">
		<Slider value={volume} min={0} max={1} step={0.01} oninput={handleVolumeChange} />
	</div>
</div>
