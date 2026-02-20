<script lang="ts">
	import { dateFormat } from '$lib/stores/settings'
	import type { DateFormat } from '$lib/types'
	import Text from '$lib/components/common/Text.svelte'

	type Props = {
		label: string
		value: string | null
		mixed?: boolean
		disabled?: boolean
		onchange?: (value: string | null) => void
		onblur?: () => void
	}

	let { label, value = $bindable(), mixed = false, disabled = false, onchange, onblur }: Props = $props()

	let year = $state('')
	let month = $state('')
	let day = $state('')

	let containerEl: HTMLDivElement | undefined = $state()
	let yearEl: HTMLInputElement | undefined = $state()
	let monthEl: HTMLInputElement | undefined = $state()
	let dayEl: HTMLInputElement | undefined = $state()

	function getSegmentOrder(fmt: DateFormat): ('y' | 'm' | 'd')[] {
		switch (fmt) {
			case 'iso':
				return ['y', 'm', 'd']
			case 'us':
				return ['m', 'd', 'y']
			case 'eu':
			case 'dot':
				return ['d', 'm', 'y']
			case 'locale': {
				const sample = new Intl.DateTimeFormat().format(new Date(2000, 0, 2))
				if (/^2000/.test(sample)) return ['y', 'm', 'd']
				if (/^2[^0]/.test(sample)) return ['d', 'm', 'y']
				return ['m', 'd', 'y']
			}
		}
	}

	function getSeparator(fmt: DateFormat): string {
		return fmt === 'dot' ? '.' : fmt === 'iso' ? '-' : '/'
	}

	const segmentOrder = $derived(getSegmentOrder($dateFormat))
	const separator = $derived(getSeparator($dateFormat))

	$effect(() => {
		if (mixed || !value) {
			year = ''
			month = ''
			day = ''
		} else {
			const parts = value.slice(0, 10).split('-')
			year = parts[0] ?? ''
			month = parts[1] ?? ''
			day = parts[2] ?? ''
		}
	})

	function emitChange() {
		if (year.length === 4 && month.length === 2 && day.length === 2) {
			onchange?.(`${year}-${month}-${day}`)
		} else if (!year && !month && !day) {
			onchange?.(null)
		}
	}

	function getElForSeg(seg: 'y' | 'm' | 'd'): HTMLInputElement | undefined {
		return seg === 'y' ? yearEl : seg === 'm' ? monthEl : dayEl
	}

	function getNextEl(current: 'y' | 'm' | 'd'): HTMLInputElement | undefined {
		const idx = segmentOrder.indexOf(current)
		const next = segmentOrder[idx + 1]
		if (!next) return undefined
		return getElForSeg(next)
	}

	function getPrevEl(current: 'y' | 'm' | 'd'): HTMLInputElement | undefined {
		const idx = segmentOrder.indexOf(current)
		const prev = segmentOrder[idx - 1]
		if (!prev) return undefined
		return getElForSeg(prev)
	}

	function focusFirstUnfilled() {
		for (const seg of segmentOrder) {
			const val = seg === 'y' ? year : seg === 'm' ? month : day
			const full = seg === 'y' ? 4 : 2
			if (val.length < full) {
				getElForSeg(seg)?.focus()
				return
			}
		}
		getElForSeg(segmentOrder[segmentOrder.length - 1])?.focus()
	}

	function handleContainerClick(e: MouseEvent) {
		const target = e.target as HTMLElement
		if (target.tagName === 'INPUT') return
		e.preventDefault()
		focusFirstUnfilled()
	}

	function handleKeydown(seg: 'y' | 'm' | 'd', e: KeyboardEvent) {
		if ((e.metaKey || e.ctrlKey) && e.key === 'a') {
			e.preventDefault()
			year = month = day = ''
			if (yearEl) yearEl.value = ''
			if (monthEl) monthEl.value = ''
			if (dayEl) dayEl.value = ''
			onchange?.(null)
			getElForSeg(segmentOrder[0])?.focus()
			return
		}
		if (e.key === 'Backspace') {
			const val = seg === 'y' ? year : seg === 'm' ? month : day
			if (val === '') getPrevEl(seg)?.focus()
		}
	}

	function getDaysInMonth(y: string, m: string): number {
		const yn = parseInt(y)
		const mn = parseInt(m)
		if (!yn || !mn || mn < 1 || mn > 12) return 31
		return new Date(yn, mn, 0).getDate()
	}

	function filterMonth(raw: string, current: string): string {
		const digits = raw.replace(/\D/g, '')
		if (!digits) return ''
		if (parseInt(digits[0]) > 1) return current
		if (digits.length >= 2) {
			const mn = parseInt(digits.slice(0, 2))
			if (mn > 12) return current
			return digits.slice(0, 2)
		}
		return digits[0]
	}

	function filterDay(raw: string, current: string, m: string, y: string): string {
		const digits = raw.replace(/\D/g, '')
		if (!digits) return ''
		if (parseInt(digits[0]) > 3) return current
		if (digits.length >= 2) {
			if (parseInt(digits[0]) === 3 && parseInt(digits[1]) > 1) return current
			const dn = parseInt(digits.slice(0, 2))
			if (dn > getDaysInMonth(y, m)) return current
			return digits.slice(0, 2)
		}
		return digits[0]
	}

	function handleYearInput(e: Event) {
		const target = e.target as HTMLInputElement
		year = target.value.replace(/\D/g, '').slice(0, 4)
		target.value = year
		emitChange()
		if (year.length === 4) {
			const nextEl = getNextEl('y')
			nextEl?.focus()
			nextEl?.select()
		}
	}

	function handleMonthInput(e: Event) {
		const target = e.target as HTMLInputElement
		const filtered = filterMonth(target.value, month)
		if (filtered === month) {
			target.value = month
			return
		}
		month = filtered
		target.value = month
		emitChange()
		if (month.length === 2) {
			const nextEl = getNextEl('m')
			nextEl?.focus()
			nextEl?.select()
		}
	}

	function handleDayInput(e: Event) {
		const target = e.target as HTMLInputElement
		const filtered = filterDay(target.value, day, month, year)
		if (filtered === day) {
			target.value = day
			return
		}
		day = filtered
		target.value = day
		emitChange()
	}

	function handleContainerFocusOut(e: FocusEvent) {
		if (!containerEl?.contains(e.relatedTarget as Node)) {
			onblur?.()
		}
	}
</script>

<label class="block space-y-1">
	<Text as="span" size="xs" weight="medium" color="secondary" class="block">{label}</Text>
	<div
		bind:this={containerEl}
		onclick={handleContainerClick}
		onfocusout={handleContainerFocusOut}
		class="flex w-full items-center gap-0 rounded-md border border-stroke bg-surface-2 px-3 py-1.5 text-sm text-text-primary focus-within:border-transparent focus-within:ring-2 focus-within:ring-brand-primary {disabled
			? 'pointer-events-none cursor-not-allowed opacity-50'
			: ''}"
	>
		{#each segmentOrder as seg, i (seg)}
			{#if i > 0}
				<span class="text-text-tertiary select-none">{separator}</span>
			{/if}
			{#if seg === 'y'}
				<input
					bind:this={yearEl}
					type="text"
					inputmode="numeric"
					maxlength="4"
					size="4"
					placeholder={mixed ? '····' : 'YYYY'}
					value={year}
					oninput={handleYearInput}
					onkeydown={(e) => handleKeydown('y', e)}
					class="bg-transparent text-center placeholder-text-tertiary focus:outline-none"
				/>
			{:else if seg === 'm'}
				<input
					bind:this={monthEl}
					type="text"
					inputmode="numeric"
					maxlength="2"
					size="2"
					placeholder={mixed ? '··' : 'MM'}
					value={month}
					oninput={handleMonthInput}
					onkeydown={(e) => handleKeydown('m', e)}
					class="bg-transparent text-center placeholder-text-tertiary focus:outline-none"
				/>
			{:else}
				<input
					bind:this={dayEl}
					type="text"
					inputmode="numeric"
					maxlength="2"
					size="2"
					placeholder={mixed ? '··' : 'DD'}
					value={day}
					oninput={handleDayInput}
					onkeydown={(e) => handleKeydown('d', e)}
					class="bg-transparent text-center placeholder-text-tertiary focus:outline-none"
				/>
			{/if}
		{/each}
	</div>
</label>
