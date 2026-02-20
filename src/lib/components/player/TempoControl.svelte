<script lang="ts">
	import { Slider, Text, Tooltip } from '$lib/components/common'
	import { translate } from '$lib/i18n'

	type Props = {
		speed: number
		onSpeedChange?: (speed: number) => void
		onSpeedCommit?: (speed: number) => void
		disabled?: boolean
	}

	let { speed, onSpeedChange, onSpeedCommit, disabled = false }: Props = $props()

	let editing = $state(false)
	let editValue = $state('')
	let inputEl = $state<HTMLInputElement | null>(null)

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

	let clickTimer: ReturnType<typeof setTimeout> | null = null

	function handleClick() {
		if (editing) return
		if (clickTimer) clearTimeout(clickTimer)
		clickTimer = setTimeout(() => {
			clickTimer = null
			onSpeedChange?.(1.0)
			onSpeedCommit?.(1.0)
		}, 200)
	}

	function handleDblClick() {
		if (clickTimer) {
			clearTimeout(clickTimer)
			clickTimer = null
		}
		editing = true
		editValue = percentage.toFixed(1)
		requestAnimationFrame(() => {
			inputEl?.select()
		})
	}

	function commitEdit() {
		editing = false
		const parsed = parseFloat(editValue)
		if (!isNaN(parsed)) {
			const clamped = Math.max(-10, Math.min(10, parsed))
			const rounded = Math.round(clamped * 10) / 10
			const newSpeed = 1.0 + rounded / 100
			onSpeedChange?.(newSpeed)
			onSpeedCommit?.(newSpeed)
		}
	}

	function cancelEdit() {
		editing = false
	}

	const ALLOWED_KEYS = new Set(['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '.', '-', '+'])

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault()
			commitEdit()
			return
		}
		if (e.key === 'Escape') {
			e.preventDefault()
			cancelEdit()
			return
		}

		if (
			['Backspace', 'Delete', 'Tab', 'ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key) ||
			e.metaKey ||
			e.ctrlKey
		) {
			return
		}

		if (!ALLOWED_KEYS.has(e.key)) {
			e.preventDefault()
			return
		}

		if (inputEl) {
			const val = inputEl.value
			const start = inputEl.selectionStart ?? val.length
			const end = inputEl.selectionEnd ?? val.length
			const hasSelection = start !== end

			if (!hasSelection) {
				const dotIndex = val.indexOf('.')
				if (e.key === '.' && dotIndex !== -1) {
					e.preventDefault()
					return
				}
				if (/[0-9]/.test(e.key) && dotIndex !== -1 && val.length - dotIndex - 1 >= 1 && start > dotIndex) {
					e.preventDefault()
					return
				}
			}
		}
	}

	function handlePaste(e: ClipboardEvent) {
		e.preventDefault()
		const text = e.clipboardData?.getData('text') ?? ''
		const sanitized = text.replace(/[^0-9.\-+]/g, '')
		const match = sanitized.match(/^([+-]?\d*\.?\d?)/)
		if (match?.[1]) document.execCommand('insertText', false, match[1])
	}
</script>

<div class="flex flex-col gap-1">
	<Text variant="header-4">{$translate('player.tempo')}</Text>
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
			{#if editing}
				<span class="flex h-6 w-8 items-center">
					<input
						bind:this={inputEl}
						type="text"
						inputmode="numeric"
						bind:value={editValue}
						onblur={commitEdit}
						onkeydown={handleKeydown}
						onpaste={handlePaste}
						class="w-8 border-none bg-transparent text-center text-xs text-text-secondary tabular-nums outline-none"
					/>
				</span>
			{:else}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<span class="flex h-6 cursor-pointer items-center" onclick={handleClick} ondblclick={handleDblClick}>
					<Text variant="caption" color="secondary" tabular class="w-8 text-right">
						{formattedPercent}
					</Text>
				</span>
			{/if}
		</Tooltip>
	</div>
</div>
