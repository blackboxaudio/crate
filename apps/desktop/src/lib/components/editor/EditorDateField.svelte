<script lang="ts">
	import { onMount } from 'svelte'
	import { scale } from 'svelte/transition'
	import { language, dateFormat } from '$shared/stores/settings'
	import { formatDate } from '$shared/utils/format'
	import { translate } from '$shared/i18n'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'

	type CalendarCell = {
		day: number
		month: number
		year: number
		isCurrentMonth: boolean
		isSelected: boolean
		isToday: boolean
		isBlank?: boolean
	}

	type Props = {
		label: string
		value: string | null
		mixed?: boolean
		disabled?: boolean
		onchange?: (value: string | null) => void
		onblur?: () => void
	}

	let { label, value = $bindable(), mixed = false, disabled = false, onchange, onblur }: Props = $props()

	let open = $state(false)
	let pickerMode: 'days' | 'months' | 'years' = $state('days')
	let viewYear = $state(new Date().getFullYear())
	let viewMonth = $state(new Date().getMonth())
	let triggerEl: HTMLButtonElement | undefined = $state()
	let panelEl: HTMLDivElement | undefined = $state()
	let panelSide: 'above' | 'below' = $state('below')
	let panelStyle = $state('position:fixed;visibility:hidden;top:0;left:0;z-index:50;')

	const parsedValue = $derived.by(() => {
		if (!value || mixed) return null
		const parts = value.slice(0, 10).split('-')
		if (parts.length < 3) return null
		const y = parseInt(parts[0])
		const m = parseInt(parts[1]) - 1
		const d = parseInt(parts[2])
		if (isNaN(y) || isNaN(m) || isNaN(d)) return null
		return { year: y, month: m, day: d }
	})

	const triggerLabel = $derived.by(() => {
		if (mixed) return '---'
		if (!value) return ''
		return formatDate(value, $dateFormat, $language)
	})

	const viewMonthLabel = $derived(
		new Intl.DateTimeFormat($language, { month: 'long' }).format(new Date(viewYear, viewMonth))
	)

	const monthLabels = Array.from({ length: 12 }, (_, i) =>
		new Intl.DateTimeFormat(undefined, { month: 'short' }).format(new Date(2024, i))
	)

	const yearRangeStart = $derived(viewYear - (viewYear % 12))
	const yearRange = $derived(Array.from({ length: 12 }, (_, i) => yearRangeStart + i))

	// Jan 1 2023 is a Sunday — used to generate locale-aware weekday labels
	const weekDayLabels = Array.from({ length: 7 }, (_, i) =>
		new Intl.DateTimeFormat(undefined, { weekday: 'short' }).format(new Date(2023, 0, 1 + i)).slice(0, 2)
	)

	const calendarDays = $derived.by((): CalendarCell[] => {
		const firstDay = new Date(viewYear, viewMonth, 1).getDay()
		const daysInMonth = new Date(viewYear, viewMonth + 1, 0).getDate()
		const prevMonthDays = new Date(viewYear, viewMonth, 0).getDate()
		const today = new Date()
		const pv = parsedValue
		const cells: CalendarCell[] = []

		// Previous month padding
		for (let i = firstDay - 1; i >= 0; i--) {
			const d = prevMonthDays - i
			const m = viewMonth === 0 ? 11 : viewMonth - 1
			const y = viewMonth === 0 ? viewYear - 1 : viewYear
			cells.push({ day: d, month: m, year: y, isCurrentMonth: false, isSelected: false, isToday: false })
		}

		// Current month days
		for (let d = 1; d <= daysInMonth; d++) {
			const isToday = viewYear === today.getFullYear() && viewMonth === today.getMonth() && d === today.getDate()
			const isSelected = pv !== null && pv.year === viewYear && pv.month === viewMonth && pv.day === d
			cells.push({ day: d, month: viewMonth, year: viewYear, isCurrentMonth: true, isSelected, isToday })
		}

		// Visible next-month days — only enough to complete the last partial row
		const lastRowCells = cells.length % 7
		const nextVisible = lastRowCells === 0 ? 0 : 7 - lastRowCells
		for (let d = 1; d <= nextVisible; d++) {
			const m = viewMonth === 11 ? 0 : viewMonth + 1
			const y = viewMonth === 11 ? viewYear + 1 : viewYear
			cells.push({ day: d, month: m, year: y, isCurrentMonth: false, isSelected: false, isToday: false })
		}
		// Blank cells — fill to 42 to keep consistent grid height (no day number shown)
		while (cells.length < 42) {
			cells.push({ day: 0, month: 0, year: 0, isCurrentMonth: false, isSelected: false, isToday: false, isBlank: true })
		}

		return cells
	})

	function pad(n: number): string {
		return n.toString().padStart(2, '0')
	}

	function selectDay(cell: CalendarCell) {
		if (!cell.isCurrentMonth) return
		const iso = `${cell.year}-${pad(cell.month + 1)}-${pad(cell.day)}`
		onchange?.(iso)
		open = false
		onblur?.()
		triggerEl?.focus()
	}

	function handleClear() {
		onchange?.(null)
		open = false
		onblur?.()
		triggerEl?.focus()
	}

	function closePicker() {
		open = false
		onblur?.()
	}

	function handleTriggerClick() {
		if (disabled) return
		if (!open) {
			pickerMode = 'days'
			const pv = parsedValue
			if (pv) {
				viewYear = pv.year
				viewMonth = pv.month
			} else {
				const now = new Date()
				viewYear = now.getFullYear()
				viewMonth = now.getMonth()
			}
			if (triggerEl) {
				const r = triggerEl.getBoundingClientRect()
				const spaceBelow = window.innerHeight - r.bottom - 8
				const hasFooter = value !== null && !mixed
				panelSide = spaceBelow >= (hasFooter ? 320 : 280) ? 'below' : 'above'
			}
		}
		open = !open
	}

	function navigatePrev() {
		if (pickerMode === 'days') {
			if (viewMonth === 0) {
				viewMonth = 11
				viewYear--
			} else {
				viewMonth--
			}
		} else if (pickerMode === 'months') {
			viewYear--
		} else {
			viewYear = yearRangeStart - 12
		}
	}

	function navigateNext() {
		if (pickerMode === 'days') {
			if (viewMonth === 11) {
				viewMonth = 0
				viewYear++
			} else {
				viewMonth++
			}
		} else if (pickerMode === 'months') {
			viewYear++
		} else {
			viewYear = yearRangeStart + 12
		}
	}

	function selectMonth(month: number) {
		viewMonth = month
		pickerMode = 'days'
	}

	function selectYear(year: number) {
		viewYear = year
		pickerMode = 'days'
	}

	function handleClickOutside(e: MouseEvent) {
		const target = e.target as Node
		if (triggerEl?.contains(target) || panelEl?.contains(target)) return
		closePicker()
	}

	function handlePanelKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault()
			closePicker()
			triggerEl?.focus()
		}
	}

	$effect(() => {
		// Re-run when pickerMode changes since panel height differs per mode
		void pickerMode
		if (open && triggerEl && panelEl) {
			const r = triggerEl.getBoundingClientRect()
			const panelH = panelEl.offsetHeight
			const top = panelSide === 'below' ? r.bottom + 4 : r.top - panelH - 4
			const left = Math.max(8, Math.min(r.left, window.innerWidth - 232))
			panelStyle = `position:fixed;top:${top}px;left:${left}px;z-index:50;`
		}
	})

	$effect(() => {
		if (open) {
			document.addEventListener('click', handleClickOutside)
		} else {
			document.removeEventListener('click', handleClickOutside)
		}
	})

	onMount(() => {
		return () => {
			document.removeEventListener('click', handleClickOutside)
		}
	})
</script>

<label class="block space-y-1">
	<Text as="span" size="xs" weight="medium" color="secondary" class="block">{label}</Text>
	<div class="relative">
		<button
			bind:this={triggerEl}
			type="button"
			onclick={handleTriggerClick}
			onkeydown={(e) => e.key === 'Escape' && closePicker()}
			class="flex w-full items-center justify-between rounded-md border bg-surface-2 px-3 py-1.5 text-left text-sm transition-colors
				{disabled
				? 'pointer-events-none cursor-not-allowed border-stroke opacity-50'
				: open
					? 'border-brand-primary ring-1 ring-brand-primary'
					: 'border-stroke hover:border-text-tertiary'}"
			{disabled}
		>
			<span class={triggerLabel ? 'text-text-primary' : 'text-text-tertiary'}>
				{triggerLabel}
			</span>
			<Icon name="chevron-down" class="h-4 w-4 text-text-tertiary transition-transform {open ? 'rotate-180' : ''}" />
		</button>

		{#if open}
			<div
				bind:this={panelEl}
				style={panelStyle}
				class="w-56 rounded-lg border border-stroke bg-surface-1 py-1 shadow-lg"
				onclick={(e) => {
					e.stopPropagation()
					e.preventDefault()
					if (!(e.target as HTMLElement).closest('button')) {
						pickerMode = 'days'
					}
				}}
				onkeydown={handlePanelKeydown}
				transition:scale={{ start: 0.95, duration: 200 }}
				role="dialog"
				tabindex="-1"
			>
				<!-- Nav header -->
				<div class="flex items-center justify-between px-2 pt-2 pb-1">
					<button
						type="button"
						onclick={navigatePrev}
						class="cursor-pointer rounded p-1 text-text-tertiary transition-colors hover:bg-surface-2 hover:text-text-primary"
					>
						<Icon name="chevron-right" class="h-4 w-4 rotate-180" />
					</button>
					<div class="flex items-center gap-1">
						{#if pickerMode === 'days'}
							<button
								type="button"
								onclick={() => (pickerMode = 'months')}
								class="cursor-pointer rounded px-1 py-0.5 text-xs font-medium text-text-primary transition-colors hover:bg-surface-2"
							>
								{viewMonthLabel}
							</button>
							<button
								type="button"
								onclick={() => (pickerMode = 'years')}
								class="cursor-pointer rounded px-1 py-0.5 text-xs font-medium text-text-primary transition-colors hover:bg-surface-2"
							>
								{viewYear}
							</button>
						{:else if pickerMode === 'months'}
							<button
								type="button"
								onclick={() => (pickerMode = 'years')}
								class="cursor-pointer rounded px-1 py-0.5 text-xs font-medium text-text-primary transition-colors hover:bg-surface-2"
							>
								{viewYear}
							</button>
						{:else}
							<span class="text-xs font-medium text-text-primary">
								{yearRangeStart} – {yearRangeStart + 11}
							</span>
						{/if}
					</div>
					<button
						type="button"
						onclick={navigateNext}
						class="cursor-pointer rounded p-1 text-text-tertiary transition-colors hover:bg-surface-2 hover:text-text-primary"
					>
						<Icon name="chevron-right" class="h-4 w-4" />
					</button>
				</div>

				{#if pickerMode === 'days'}
					<!-- Day-of-week headers -->
					<div class="mb-1 grid grid-cols-7 px-1">
						{#each weekDayLabels as dayLabel (dayLabel)}
							<div class="flex h-7 items-center justify-center text-[10px] font-medium text-text-tertiary">
								{dayLabel}
							</div>
						{/each}
					</div>

					<!-- 42-cell calendar grid -->
					<div class="grid grid-cols-7 gap-y-0.5 px-1">
						{#each calendarDays as cell, i (i)}
							{#if cell.isBlank}
								<div class="h-8 w-8"></div>
							{:else}
								<button
									type="button"
									onclick={() => selectDay(cell)}
									class="flex h-8 w-8 items-center justify-center rounded-md text-xs transition-colors
									{!cell.isCurrentMonth
										? 'pointer-events-none text-text-tertiary opacity-30'
										: cell.isSelected
											? 'cursor-pointer bg-brand-primary text-white'
											: cell.isToday
												? 'cursor-pointer text-text-primary ring-1 ring-brand-primary hover:bg-surface-2'
												: 'cursor-pointer text-text-primary hover:bg-surface-2'}"
								>
									{cell.day}
								</button>
							{/if}
						{/each}
					</div>
				{:else if pickerMode === 'months'}
					<!-- Month grid -->
					<div class="grid grid-cols-3 gap-1 px-2 py-1">
						{#each monthLabels as monthLabel, i (i)}
							{@const isSelected = i === viewMonth}
							{@const isCurrentMonth = i === new Date().getMonth() && viewYear === new Date().getFullYear()}
							<button
								type="button"
								onclick={() => selectMonth(i)}
								class="flex h-8 items-center justify-center rounded-md text-xs transition-colors
								{isSelected
									? 'cursor-pointer bg-brand-primary text-white'
									: isCurrentMonth
										? 'cursor-pointer text-text-primary ring-1 ring-brand-primary hover:bg-surface-2'
										: 'cursor-pointer text-text-primary hover:bg-surface-2'}"
							>
								{monthLabel}
							</button>
						{/each}
					</div>
				{:else}
					<!-- Year grid -->
					<div class="grid grid-cols-3 gap-1 px-2 py-1">
						{#each yearRange as year (year)}
							{@const isSelected = year === viewYear}
							{@const isCurrentYear = year === new Date().getFullYear()}
							<button
								type="button"
								onclick={() => selectYear(year)}
								class="flex h-8 items-center justify-center rounded-md text-xs transition-colors
								{isSelected
									? 'cursor-pointer bg-brand-primary text-white'
									: isCurrentYear
										? 'cursor-pointer text-text-primary ring-1 ring-brand-primary hover:bg-surface-2'
										: 'cursor-pointer text-text-primary hover:bg-surface-2'}"
							>
								{year}
							</button>
						{/each}
					</div>
				{/if}

				<!-- Clear footer -->
				{#if value !== null && !mixed}
					<div class="mt-1 border-t border-stroke px-2 py-1.5">
						<button
							type="button"
							onclick={handleClear}
							class="w-full cursor-pointer rounded px-2 py-1 text-left text-xs text-red-500 transition-colors hover:bg-red-500/10"
						>
							{$translate('common.remove')}
						</button>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</label>
