<script lang="ts">
	import { onMount } from 'svelte'
	import { scale } from 'svelte/transition'
	import Icon from './Icon.svelte'
	import Text from './Text.svelte'

	type SelectOption = {
		value: string
		label: string
		sublabel?: string
		style?: string
	}

	type SelectOptionGroup = {
		label: string
		options: SelectOption[]
	}

	type Props = {
		value: string
		options: SelectOption[] | SelectOptionGroup[]
		placeholder?: string
		disabled?: boolean
		class?: string
		onchange?: (value: string) => void
	}

	let {
		value = $bindable(''),
		options,
		placeholder = 'Select an option',
		disabled = false,
		class: className = '',
		onchange,
	}: Props = $props()

	let open = $state(false)
	let triggerEl: HTMLButtonElement | undefined = $state()
	let menuEl: HTMLDivElement | undefined = $state()
	let focusedIndex = $state(-1)

	// Determine if options are grouped
	const isGrouped = $derived(options.length > 0 && 'options' in options[0])

	// Flatten options for keyboard navigation and value lookup
	const flatOptions = $derived(
		isGrouped ? (options as SelectOptionGroup[]).flatMap((g) => g.options) : (options as SelectOption[])
	)

	// Get the label for the current value
	const selectedLabel = $derived(flatOptions.find((o) => o.value === value)?.label ?? '')

	// Position the dropdown using fixed positioning, portaled to document.body
	// to escape modal overflow and transform containing blocks
	const GAP = 4
	const VIEWPORT_PADDING = 8

	let dropdownStyle = $state('')
	let rafId: number | undefined

	// Portal action: moves the element out of overflow/transform containers.
	// If inside a <dialog>, portal to the dialog (stays in top layer).
	// Otherwise, portal to document.body.
	function portal(node: HTMLElement) {
		const dialog = triggerEl?.closest('dialog')
		const target = dialog ?? document.body
		target.appendChild(node)
		return {
			destroy() {
				node.remove()
			},
		}
	}

	function computePosition() {
		if (!triggerEl || !menuEl) return

		const triggerRect = triggerEl.getBoundingClientRect()
		const menuHeight = menuEl.offsetHeight
		const viewportHeight = window.innerHeight
		const spaceBelow = viewportHeight - triggerRect.bottom
		const openUpward = spaceBelow < menuHeight && triggerRect.top > menuHeight

		let top: number
		if (openUpward) {
			top = triggerRect.top - menuHeight - GAP
		} else {
			top = triggerRect.bottom + GAP
		}

		// Clamp to viewport
		top = Math.max(VIEWPORT_PADDING, Math.min(top, viewportHeight - menuHeight - VIEWPORT_PADDING))

		dropdownStyle = `position:fixed;top:${top}px;left:${triggerRect.left}px;width:${triggerRect.width}px;`
	}

	function onScroll() {
		computePosition()
	}

	$effect(() => {
		if (open && triggerEl && menuEl) {
			if (rafId) cancelAnimationFrame(rafId)
			dropdownStyle = ''
			rafId = requestAnimationFrame(() => {
				computePosition()
			})

			// Listen for scroll on any ancestor so the dropdown follows the trigger
			window.addEventListener('scroll', onScroll, true)
		} else {
			window.removeEventListener('scroll', onScroll, true)
		}

		return () => {
			if (rafId) cancelAnimationFrame(rafId)
			window.removeEventListener('scroll', onScroll, true)
		}
	})

	function handleTriggerClick() {
		if (disabled) return
		open = !open
		if (open) {
			// Set initial focus to current value
			focusedIndex = flatOptions.findIndex((o) => o.value === value)
			if (focusedIndex === -1) focusedIndex = 0
		}
	}

	function handleOptionClick(optionValue: string) {
		value = optionValue
		onchange?.(optionValue)
		open = false
		triggerEl?.focus()
	}

	function handleClickOutside(e: MouseEvent) {
		const target = e.target as Node
		if (triggerEl?.contains(target) || menuEl?.contains(target)) return
		open = false
	}

	function handleKeydown(e: KeyboardEvent) {
		if (!open) {
			if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown') {
				e.preventDefault()
				open = true
				focusedIndex = flatOptions.findIndex((o) => o.value === value)
				if (focusedIndex === -1) focusedIndex = 0
			}
			return
		}

		switch (e.key) {
			case 'Escape':
				e.preventDefault()
				open = false
				triggerEl?.focus()
				break
			case 'ArrowDown':
				e.preventDefault()
				focusedIndex = Math.min(focusedIndex + 1, flatOptions.length - 1)
				break
			case 'ArrowUp':
				e.preventDefault()
				focusedIndex = Math.max(focusedIndex - 1, 0)
				break
			case 'Enter':
			case ' ':
				e.preventDefault()
				if (focusedIndex >= 0 && focusedIndex < flatOptions.length) {
					handleOptionClick(flatOptions[focusedIndex].value)
				}
				break
			case 'Tab':
				open = false
				break
		}
	}

	function getFlatIndex(option: SelectOption): number {
		return flatOptions.findIndex((o) => o.value === option.value)
	}

	onMount(() => {
		return () => {
			document.removeEventListener('click', handleClickOutside)
		}
	})

	$effect(() => {
		if (open) {
			document.addEventListener('click', handleClickOutside)
		} else {
			document.removeEventListener('click', handleClickOutside)
		}
	})
</script>

<div class={className}>
	<button
		bind:this={triggerEl}
		type="button"
		class="flex w-full items-center justify-between rounded-lg border bg-surface-2 px-4 py-2.5 text-left
			text-sm transition-colors hover:cursor-pointer
			{disabled
			? 'cursor-not-allowed border-stroke opacity-50'
			: open
				? 'border-brand-primary ring-1 ring-brand-primary'
				: 'border-stroke hover:border-text-tertiary focus:border-brand-primary focus:ring-1 focus:ring-brand-primary'}"
		{disabled}
		onclick={handleTriggerClick}
		onkeydown={handleKeydown}
		aria-haspopup="listbox"
		aria-expanded={open}
	>
		<span class={selectedLabel ? 'text-text-primary' : 'text-text-tertiary'}>
			{selectedLabel || placeholder}
		</span>
		<Icon name="chevron-down" class="h-4 w-4 text-text-tertiary transition-transform {open ? 'rotate-180' : ''}" />
	</button>

	{#if open}
		<div
			bind:this={menuEl}
			use:portal
			class="z-50 max-h-60 overflow-auto rounded-lg border border-stroke bg-surface-1
				py-1 shadow-lg hover:cursor-pointer"
			style={dropdownStyle}
			role="listbox"
			transition:scale={{ start: 0.95, duration: 200 }}
		>
			{#if isGrouped}
				{#each options as group, groupIndex ((group as SelectOptionGroup).label)}
					{@const groupData = group as SelectOptionGroup}
					<!-- Group Header -->
					<Text
						variant="header-4"
						as="div"
						weight="semibold"
						class="px-3 py-1.5 {groupIndex > 0 ? 'mt-1 border-t border-stroke pt-2' : ''}"
					>
						{groupData.label}
					</Text>
					<!-- Group Options -->
					{#each groupData.options as option (option.value)}
						{@const flatIndex = getFlatIndex(option)}
						<button
							type="button"
							class="flex w-full items-center gap-3 px-4 py-2 text-left text-sm transition-colors hover:cursor-pointer
								{option.value === value
								? 'bg-brand-muted text-brand-primary'
								: focusedIndex === flatIndex
									? 'bg-surface-2 text-text-primary'
									: 'text-text-primary hover:bg-surface-2'}"
							style={option.style}
							onclick={() => handleOptionClick(option.value)}
							onmouseenter={() => (focusedIndex = flatIndex)}
							role="option"
							aria-selected={option.value === value}
						>
							<span class="flex flex-1 flex-col">
								<span>{option.label}</span>
								{#if option.sublabel}
									<Text variant="caption" as="span">{option.sublabel}</Text>
								{/if}
							</span>
							{#if option.value === value}
								<Icon name="check" class="h-4 w-4" />
							{/if}
						</button>
					{/each}
				{/each}
			{:else}
				<!-- Flat options (existing behavior) -->
				{#each options as option, index ((option as SelectOption).value)}
					{@const optionData = option as SelectOption}
					<button
						type="button"
						class="flex w-full items-center gap-3 px-4 py-2 text-left text-sm transition-colors hover:cursor-pointer
							{optionData.value === value
							? 'bg-brand-muted text-brand-primary'
							: focusedIndex === index
								? 'bg-surface-2 text-text-primary'
								: 'text-text-primary hover:bg-surface-2'}"
						style={optionData.style}
						onclick={() => handleOptionClick(optionData.value)}
						onmouseenter={() => (focusedIndex = index)}
						role="option"
						aria-selected={optionData.value === value}
					>
						<span class="flex flex-1 flex-col">
							<span>{optionData.label}</span>
							{#if optionData.sublabel}
								<Text variant="caption" as="span">{optionData.sublabel}</Text>
							{/if}
						</span>
						{#if optionData.value === value}
							<Icon name="check" class="h-4 w-4" />
						{/if}
					</button>
				{/each}
			{/if}
		</div>
	{/if}
</div>
