<script lang="ts">
	import { fly, fade } from 'svelte/transition'
	import { cubicOut } from 'svelte/easing'
	import { Button, StepIndicator } from '$lib/components/common'
	import { translate } from '$lib/i18n'
	import WelcomeStep from './WelcomeStep.svelte'
	import LanguageStep from './LanguageStep.svelte'
	import AppearanceStep from './AppearanceStep.svelte'
	import ReadyStep from './ReadyStep.svelte'

	type Props = {
		onComplete: () => void
	}

	let { onComplete }: Props = $props()

	const TOTAL_STEPS = 4

	let currentStep = $state(0)
	let direction = $state(1) // 1 = forward, -1 = backward

	function goToStep(step: number) {
		if (step === currentStep || step < 0 || step >= TOTAL_STEPS) return
		direction = step > currentStep ? 1 : -1
		currentStep = step
	}

	function next() {
		if (currentStep < TOTAL_STEPS - 1) {
			direction = 1
			currentStep++
		}
	}

	function back() {
		if (currentStep > 0) {
			direction = -1
			currentStep--
		}
	}

	const isFirstStep = $derived(currentStep === 0)
	const isLastStep = $derived(currentStep === TOTAL_STEPS - 1)
</script>

<div
	class="fixed inset-0 z-40 flex flex-col items-center justify-center bg-surface-0"
	in:fade={{ duration: 400, easing: cubicOut }}
	out:fade={{ duration: 400, easing: cubicOut }}
>
	<div class="flex w-full max-w-xl flex-1 flex-col items-center justify-center px-12">
		<!-- Step Content -->
		<div class="relative flex w-full flex-1 items-center justify-center overflow-hidden">
			{#key currentStep}
				<div
					class="absolute flex w-full items-center justify-center px-2"
					in:fly={{ x: direction * 300, duration: 300, easing: cubicOut }}
					out:fly={{ x: direction * -300, duration: 300, easing: cubicOut }}
				>
					{#if currentStep === 0}
						<WelcomeStep />
					{:else if currentStep === 1}
						<LanguageStep />
					{:else if currentStep === 2}
						<AppearanceStep />
					{:else if currentStep === 3}
						<ReadyStep />
					{/if}
				</div>
			{/key}
		</div>

		<!-- Navigation + Step Indicator -->
		<div class="flex w-full items-center justify-between pb-8">
			<div class="w-28">
				<Button variant="ghost" size="md" onclick={back} disabled={isFirstStep}>
					{$translate('common.back')}
				</Button>
			</div>

			<StepIndicator totalSteps={TOTAL_STEPS} {currentStep} onStepClick={goToStep} />

			<div class="flex w-28 justify-end">
				{#if isLastStep}
					<Button variant="primary" size="md" onclick={onComplete} class="whitespace-nowrap">
						{$translate('onboarding.getStarted')}
					</Button>
				{:else}
					<Button variant="primary" size="md" onclick={next}>
						{$translate('common.next')}
					</Button>
				{/if}
			</div>
		</div>
	</div>
</div>
