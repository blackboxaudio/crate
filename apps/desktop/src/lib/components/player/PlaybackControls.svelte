<script lang="ts">
	import { IconButton, Tooltip } from '$lib/components/common'
	import { translate } from '$shared/i18n'
	import type { RepeatMode } from '$shared/stores/player'

	type Props = {
		isPlaying: boolean
		hasTrack: boolean
		/** Whether the queue has anything to advance to (repeat off at the end of the list greys Next). */
		canAdvance?: boolean
		shuffleEnabled?: boolean
		repeatMode?: RepeatMode
		onPlayPause?: () => void
		onPrevious?: () => void
		onNext?: () => void
		onStop?: () => void
		onToggleShuffle?: () => void
		onCycleRepeat?: () => void
	}

	let {
		isPlaying,
		hasTrack,
		canAdvance = true,
		shuffleEnabled = false,
		repeatMode = 'off',
		onPlayPause,
		onPrevious,
		onNext,
		onStop,
		onToggleShuffle,
		onCycleRepeat,
	}: Props = $props()

	const repeatIcon = $derived(
		repeatMode === 'track' ? 'repeat-once' : repeatMode === 'release' ? 'repeat-dot' : 'repeat'
	)
	const repeatLabelKey = $derived(
		repeatMode === 'track'
			? 'player.repeatTrack'
			: repeatMode === 'release'
				? 'player.repeatRelease'
				: repeatMode === 'context'
					? 'player.repeatAll'
					: 'player.repeat'
	)
</script>

<div class="flex items-center gap-1">
	<!-- Shuffle -->
	<Tooltip text={$translate('player.shuffle')} position="top" delay={250}>
		<IconButton size="lg" iconClass="h-4 w-4" active={shuffleEnabled} icon="shuffle" onclick={onToggleShuffle} />
	</Tooltip>

	<!-- Previous -->
	<Tooltip text={$translate('player.previous')} position="top" delay={250}>
		<IconButton size="lg" iconClass="h-5 w-5" disabled={!hasTrack} icon="skip-back" fill onclick={onPrevious} />
	</Tooltip>

	<!-- Play/Pause -->
	<Tooltip text={isPlaying ? $translate('player.pause') : $translate('player.play')} position="top" delay={250}>
		<IconButton
			size="lg"
			iconClass="h-6 w-6"
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
		<IconButton
			size="lg"
			iconClass="h-5 w-5"
			disabled={!hasTrack || !canAdvance}
			icon="skip-forward"
			fill
			onclick={onNext}
		/>
	</Tooltip>

	<!-- Repeat (cycles off → track → release → all) -->
	<Tooltip text={$translate(repeatLabelKey)} position="top" delay={250}>
		<IconButton size="lg" iconClass="h-4 w-4" active={repeatMode !== 'off'} icon={repeatIcon} onclick={onCycleRepeat} />
	</Tooltip>
</div>
