<script lang="ts">
	type Props = {
		totalSteps: number
		currentStep: number
		onStepClick?: (step: number) => void
	}

	let { totalSteps, currentStep, onStepClick }: Props = $props()
</script>

<div class="flex gap-2">
	{#each Array(totalSteps) as _, i (i)}
		<button
			type="button"
			class="h-2 w-2 rounded-full transition-colors duration-200 hover:cursor-pointer {i === currentStep
				? 'bg-brand-primary'
				: 'bg-surface-2 hover:bg-text-tertiary/25'}"
			onclick={() => onStepClick?.(i)}
			aria-label="Step {i + 1}"
		></button>
	{/each}
</div>
