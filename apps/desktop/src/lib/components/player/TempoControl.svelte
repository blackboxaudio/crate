<script lang="ts">
	import { IconButton, Slider, Text, Tooltip } from '$lib/components/common'
	import { translate } from '$shared/i18n'

	type Props = {
		speed: number
		onSpeedChange?: (speed: number) => void
		onSpeedCommit?: (speed: number) => void
		disabled?: boolean
	}

	let { speed, onSpeedChange, onSpeedCommit, disabled = false }: Props = $props()

	let percentage = $derived(Math.round((speed - 1.0) * 1000) / 10)

	let formattedPercent = $derived(percentage >= 0 ? `+${percentage.toFixed(1)}%` : `${percentage.toFixed(1)}%`)

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement
		const pct = parseFloat(target.value)
		onSpeedChange?.(1.0 + pct / 100)
	}

	function handleChange(e: Event) {
		const target = e.target as HTMLInputElement
		const pct = parseFloat(target.value)
		onSpeedCommit?.(1.0 + pct / 100)
	}

	function handleReset() {
		onSpeedChange?.(1.0)
		onSpeedCommit?.(1.0)
	}
</script>

<div class="flex flex-col gap-1">
	<Text variant="header-4">{$translate('player.tempo')} ({formattedPercent})</Text>
	<div class="flex items-center gap-2">
		<div class="flex h-6 w-40 items-center">
			<Slider
				value={percentage}
				min={-10}
				max={10}
				step={0.1}
				bipolar
				snapToCenter={0.5}
				{disabled}
				oninput={handleInput}
				onchange={handleChange}
			/>
		</div>
		<Tooltip text={$translate('player.resetTempo')} position="top" delay={250}>
			<IconButton icon="reset" size="sm" disabled={disabled || percentage === 0} onclick={handleReset} />
		</Tooltip>
	</div>
</div>
