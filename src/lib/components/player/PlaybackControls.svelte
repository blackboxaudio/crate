<script lang="ts">
	import { IconButton, Tooltip } from '$lib/components/common'
	import { translate } from '$lib/i18n'

	type Props = {
		isPlaying: boolean
		hasTrack: boolean
		shuffleEnabled?: boolean
		onPlayPause?: () => void
		onPrevious?: () => void
		onNext?: () => void
		onStop?: () => void
		onToggleShuffle?: () => void
	}

	let {
		isPlaying,
		hasTrack,
		shuffleEnabled = false,
		onPlayPause,
		onPrevious,
		onNext,
		onStop,
		onToggleShuffle,
	}: Props = $props()
</script>

<div class="flex items-center gap-1">
	<!-- Previous -->
	<Tooltip text={$translate('player.previous')} position="top" delay={250}>
		<IconButton size="lg" iconClass="h-5 w-5" disabled={!hasTrack} icon="skip-back" fill onclick={onPrevious} />
	</Tooltip>

	<!-- Play/Pause -->
	<Tooltip text={isPlaying ? $translate('player.pause') : $translate('player.play')} position="top" delay={250}>
		<IconButton
			size="lg"
			iconClass="h-5 w-5"
			disabled={!hasTrack}
			icon={isPlaying ? 'pause' : 'play'}
			fill
			onclick={onPlayPause}
		/>
	</Tooltip>

	<!-- Stop -->
	<!--	<IconButton title={$translate('player.stop')} disabled={!hasTrack} icon="stop" fill onclick={onStop} />-->

	<!-- Next -->
	<Tooltip text={$translate('player.next')} position="top" delay={250}>
		<IconButton size="lg" iconClass="h-5 w-5" disabled={!hasTrack} icon="skip-forward" fill onclick={onNext} />
	</Tooltip>

	<!-- Shuffle -->
	<Tooltip text={$translate('player.shuffle')} position="top" delay={250}>
		<IconButton size="lg" iconClass="h-4 w-4" active={shuffleEnabled} icon="shuffle" onclick={onToggleShuffle} />
	</Tooltip>
</div>
