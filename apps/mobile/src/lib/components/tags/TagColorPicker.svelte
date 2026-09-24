<script lang="ts">
	import { translate } from '$shared/i18n'
	import { TAG_CATEGORY_COLORS } from '$shared/types'
	import MobileModal from '$lib/components/common/MobileModal.svelte'

	// Bottom-sheet color picker for a tag category — the 10 preset swatches (TAG_CATEGORY_COLORS). The current
	// color is check-marked. Picking one fires `onSelect(hex)` and closes; the parent applies it via
	// tagsStore.updateCategory. Tags inherit their category's color, so this recolors every tag in the category
	// at once. There's no "remove color" — the backend's update can't clear a color (a null means "leave
	// unchanged"), and every category is created with an auto-assigned color anyway.
	type Props = {
		open: boolean
		current: string | null
		onSelect: (hex: string) => void
		onClose: () => void
	}
	let { open, current, onSelect, onClose }: Props = $props()

	function choose(hex: string) {
		onSelect(hex)
		onClose()
	}
</script>

<MobileModal {open} {onClose} title={$translate('contextMenu.setColor')}>
	<div class="grid grid-cols-5 gap-3 py-1">
		{#each TAG_CATEGORY_COLORS as color (color.id)}
			{@const selected = current === color.hex}
			<button
				type="button"
				class="flex aspect-square items-center justify-center rounded-full transition-transform active:scale-95"
				style="background-color: {color.hex}"
				aria-label={color.label}
				aria-pressed={selected}
				onclick={() => choose(color.hex)}
			>
				{#if selected}
					<svg class="h-5 w-5 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
						<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				{/if}
			</button>
		{/each}
	</div>
</MobileModal>
