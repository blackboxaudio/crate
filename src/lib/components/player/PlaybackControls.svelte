<script lang="ts">
	import { IconButton } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		isPlaying: boolean
		hasTrack: boolean
		onPlayPause?: () => void
		onPrevious?: () => void
		onNext?: () => void
		onStop?: () => void
	}

	let { isPlaying, hasTrack, onPlayPause, onPrevious, onNext, onStop }: Props = $props()
</script>

<div class="flex items-center gap-1">
	<!-- Previous -->
	<IconButton title="Previous" disabled={!hasTrack} onclick={onPrevious}>
		<Icon name="skip-back" fill />
	</IconButton>

	<!-- Play/Pause -->
	<IconButton title={isPlaying ? 'Pause' : 'Play'} disabled={!hasTrack} size="lg" onclick={onPlayPause}>
		{#if isPlaying}
			<Icon name="pause" class="h-6 w-6" fill />
		{:else}
			<Icon name="play" class="h-6 w-6" fill />
		{/if}
	</IconButton>

	<!-- Stop -->
	<IconButton title="Stop" disabled={!hasTrack} onclick={onStop}>
		<Icon name="stop" fill />
	</IconButton>

	<!-- Next -->
	<IconButton title="Next" disabled={!hasTrack} onclick={onNext}>
		<Icon name="skip-forward" fill />
	</IconButton>
</div>
