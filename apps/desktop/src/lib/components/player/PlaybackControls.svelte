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
		/** Whether the queue panel is open (renders the queue button active). */
		queueOpen?: boolean
		/** Explicit user-queue length, shown as a badge on the queue button. */
		queueCount?: number
		onToggleQueue?: () => void
		/** The queue button's element, exposed so the panel can anchor to it. */
		queueButtonEl?: HTMLDivElement
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
		queueOpen = false,
		queueCount = 0,
		onToggleQueue,
		queueButtonEl = $bindable(),
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

<!-- The row is symmetric around Play so it stays centred over the seek bar: the queue button on the
     right is balanced by an equal-width spacer on the left. -->
<div class="flex items-center gap-1">
	<div class="h-10 w-10" aria-hidden="true"></div>

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

	<!-- Queue (opens the Up Next panel; also View → Toggle Queue) -->
	<Tooltip text={$translate('queue.openQueue')} position="top" delay={250}>
		<div class="relative" bind:this={queueButtonEl}>
			<IconButton size="lg" iconClass="h-4 w-4" active={queueOpen} icon="queue" onclick={onToggleQueue} />
			{#if queueCount > 0}
				<span
					class="pointer-events-none absolute top-0.5 right-0.5 min-w-4 rounded-full bg-brand-primary px-1 text-center text-[10px] leading-4 font-semibold text-white tabular-nums"
				>
					{queueCount}
				</span>
			{/if}
		</div>
	</Tooltip>
</div>
