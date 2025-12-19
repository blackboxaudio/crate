<script lang="ts">
	import Modal from './Modal.svelte'
	import Button from './Button.svelte'

	type Props = {
		open: boolean
		title?: string
		selectedColor?: string | null
		onSelect: (color: string) => void
		onCancel: () => void
	}

	let { open, title = 'Choose Color', selectedColor = null, onSelect, onCancel }: Props = $props()

	// Preset colors for DJ-friendly palette
	const colors = [
		'#ef4444', // Red
		'#f97316', // Orange
		'#f59e0b', // Amber
		'#eab308', // Yellow
		'#84cc16', // Lime
		'#22c55e', // Green
		'#14b8a6', // Teal
		'#06b6d4', // Cyan
		'#0ea5e9', // Sky
		'#3b82f6', // Blue
		'#6366f1', // Indigo (default)
		'#8b5cf6', // Violet
		'#a855f7', // Purple
		'#d946ef', // Fuchsia
		'#ec4899', // Pink
		'#f43f5e', // Rose
	]

	let currentSelection = $derived(selectedColor || '#6366f1')

	// Reset selection when modal opens
	$effect(() => {
		if (open) {
			currentSelection = selectedColor || '#6366f1'
		}
	})

	function handleColorClick(color: string) {
		currentSelection = color
	}

	function handleSubmit() {
		onSelect(currentSelection)
	}
</script>

<Modal {open} {title} onClose={onCancel}>
	<div class="grid grid-cols-8 gap-2 py-2">
		{#each colors as color (color)}
			<button
				type="button"
				class="h-8 w-8 rounded-md transition-transform hover:scale-110 focus:ring-2 focus:ring-white focus:ring-offset-2 focus:ring-offset-zinc-800 focus:outline-none {currentSelection ===
				color
					? 'ring-2 ring-white ring-offset-2 ring-offset-zinc-800'
					: ''}"
				style="background-color: {color};"
				onclick={() => handleColorClick(color)}
				aria-label="Select color {color}"
			></button>
		{/each}
	</div>

	<div class="mt-4 flex items-center gap-3">
		<div class="h-6 w-6 rounded" style="background-color: {currentSelection};"></div>
		<span class="text-sm text-zinc-400">{currentSelection}</span>
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onCancel}>Cancel</Button>
		<Button variant="primary" onclick={handleSubmit}>Select</Button>
	{/snippet}
</Modal>
