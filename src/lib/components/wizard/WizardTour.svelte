<script lang="ts">
	import { fade } from 'svelte/transition'
	import { onMount } from 'svelte'
	import { Button, Text, StepIndicator } from '$lib/components/common'
	import { pageActions } from '$lib/stores'
	import { translate } from '$lib/i18n'

	type PopoverPosition = 'bottom' | 'right'

	interface WizardStep {
		targetId: string
		titleKey: string
		descriptionKey: string
		position: PopoverPosition
		setup?: () => void
	}

	type Props = {
		onComplete: () => void
		onSkip: () => void
	}

	let { onComplete, onSkip }: Props = $props()

	const steps: WizardStep[] = [
		{
			targetId: 'wizard-view-switcher',
			titleKey: 'wizard.discovery.title',
			descriptionKey: 'wizard.discovery.description',
			position: 'bottom',
			setup: () => $pageActions?.handleViewChange('discovery'),
		},
		{
			targetId: 'wizard-view-switcher',
			titleKey: 'wizard.library.title',
			descriptionKey: 'wizard.library.description',
			position: 'bottom',
			setup: () => $pageActions?.handleViewChange('library'),
		},
		{
			targetId: 'wizard-playlists-tab',
			titleKey: 'wizard.playlists.title',
			descriptionKey: 'wizard.playlists.description',
			position: 'right',
			setup: () => document.getElementById('wizard-playlists-tab')?.click(),
		},
		{
			targetId: 'wizard-tags-tab',
			titleKey: 'wizard.tags.title',
			descriptionKey: 'wizard.tags.description',
			position: 'right',
			setup: () => document.getElementById('wizard-tags-tab')?.click(),
		},
	]

	const SPOTLIGHT_TRANSITION_MS = 200

	let currentStep = $state(0)
	let direction = $state(1)
	let spotlightRect = $state({ x: 0, y: 0, width: 0, height: 0 })
	let popoverStyle = $state('')
	let ready = $state(false)
	let popoverVisible = $state(false)
	let transitionTimer: ReturnType<typeof setTimeout> | null = null

	const step = $derived(steps[currentStep])
	const isLastStep = $derived(currentStep === steps.length - 1)

	function updatePosition() {
		const el = document.getElementById(step.targetId)
		if (!el) return

		const rect = el.getBoundingClientRect()
		const padding = 8

		spotlightRect = {
			x: rect.x - padding,
			y: rect.y - padding,
			width: rect.width + padding * 2,
			height: rect.height + padding * 2,
		}

		const gap = 12
		if (step.position === 'bottom') {
			const popoverX = spotlightRect.x + spotlightRect.width / 2
			popoverStyle = `left: ${popoverX}px; top: ${spotlightRect.y + spotlightRect.height + gap}px; transform: translateX(-50%);`
		} else {
			const popoverY = spotlightRect.y + spotlightRect.height / 2
			popoverStyle = `left: ${spotlightRect.x + spotlightRect.width + gap}px; top: ${popoverY}px; transform: translateY(-50%);`
		}
	}

	function goToStep(index: number) {
		if (index === currentStep) return

		direction = index > currentStep ? 1 : -1
		currentStep = index
		popoverVisible = false
		if (transitionTimer) clearTimeout(transitionTimer)

		const s = steps[index]
		s.setup?.()

		// Double rAF to let DOM settle after view switch, then update spotlight position
		requestAnimationFrame(() => {
			requestAnimationFrame(() => {
				updatePosition()

				// Show popover after spotlight transition finishes
				transitionTimer = setTimeout(() => {
					popoverVisible = true
				}, SPOTLIGHT_TRANSITION_MS)
			})
		})
	}

	function resetNavigation() {
		$pageActions?.handleViewChange('discovery')
		document.getElementById('wizard-playlists-tab')?.click()
	}

	function handleNext() {
		if (isLastStep) {
			resetNavigation()
			onComplete()
		} else {
			goToStep(currentStep + 1)
		}
	}

	function handleBack() {
		if (currentStep > 0) {
			goToStep(currentStep - 1)
		}
	}

	function handleSkip() {
		resetNavigation()
		onSkip()
	}

	onMount(() => {
		steps[0].setup?.()

		// Double rAF to let DOM settle
		requestAnimationFrame(() => {
			requestAnimationFrame(() => {
				updatePosition()
				ready = true
				popoverVisible = true
			})
		})

		function handleResize() {
			updatePosition()
		}
		window.addEventListener('resize', handleResize)

		return () => {
			window.removeEventListener('resize', handleResize)
			if (transitionTimer) clearTimeout(transitionTimer)
		}
	})
</script>

{#if ready}
	<!-- Overlay -->
	<div class="fixed inset-0 z-[45]" transition:fade={{ duration: 300 }}>
		<!-- Spotlight cutout -->
		<div
			class="absolute inset-0"
			style="
				pointer-events: auto;
				background: transparent;
				box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.5);
				border-radius: 8px;
				left: {spotlightRect.x}px;
				top: {spotlightRect.y}px;
				width: {spotlightRect.width}px;
				height: {spotlightRect.height}px;
				transition: left 200ms ease, top 200ms ease, width 200ms ease, height 200ms ease;
			"
		></div>

		<!-- Popover -->
		{#if popoverVisible}
			<div
				class="fixed z-[46] w-72 rounded-xl border border-stroke bg-surface-0 p-4 shadow-xl"
				style={popoverStyle}
				in:fade={{ duration: 150 }}
			>
				<!-- Step indicator dots -->
				<div class="mb-3 flex items-center">
					<StepIndicator totalSteps={steps.length} {currentStep} onStepClick={goToStep} />
					<span class="ml-auto text-xs text-text-tertiary">{currentStep + 1}/{steps.length}</span>
				</div>

				<!-- Content -->
				<Text variant="header-4" class="mb-1">{$translate(step.titleKey)}</Text>
				<Text variant="body-2" color="secondary" class="mb-4">{$translate(step.descriptionKey)}</Text>

				<!-- Actions -->
				<div class="flex items-center justify-between">
					<button
						type="button"
						class="text-xs text-text-tertiary transition-colors hover:cursor-pointer hover:text-text-secondary"
						onclick={handleSkip}
					>
						{$translate('wizard.skip')}
					</button>
					<div class="flex items-center gap-2">
						{#if currentStep > 0}
							<Button variant="ghost" size="sm" onclick={handleBack}>
								{$translate('common.back')}
							</Button>
						{/if}
						<Button variant="primary" size="sm" onclick={handleNext}>
							{isLastStep ? $translate('wizard.finish') : $translate('common.next')}
						</Button>
					</div>
				</div>
			</div>
		{/if}
	</div>
{/if}
