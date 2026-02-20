<script lang="ts">
	import { IconButton } from '$lib/components/common'
	import { translate } from '$lib/i18n'

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
	<IconButton title={$translate('player.previous')} disabled={!hasTrack} icon="skip-back" fill onclick={onPrevious} />

	<!-- Play/Pause -->
	<IconButton
		title={isPlaying ? $translate('player.pause') : $translate('player.play')}
		disabled={!hasTrack}
		size="lg"
		icon={isPlaying ? 'pause' : 'play'}
		iconClass="h-6 w-6"
		fill
		onclick={onPlayPause}
	/>

	<!-- Stop -->
	<!--	<IconButton title={$translate('player.stop')} disabled={!hasTrack} icon="stop" fill onclick={onStop} />-->

	<!-- Next -->
	<IconButton title={$translate('player.next')} disabled={!hasTrack} icon="skip-forward" fill onclick={onNext} />
</div>
