<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { TagCategory } from '$shared/types'
	import { DEFAULT_TAG_COLOR } from '$shared/types'
	import MobileModal from '$lib/components/common/MobileModal.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'

	// Bottom-sheet category picker for a tag's "Move to Category" — one row per category (color dot + name),
	// with the tag's current category check-marked and disabled. Picking a row fires `onSelect(categoryId)`
	// and closes; the parent applies the move via tagsStore.moveTag.
	type Props = {
		open: boolean
		categories: TagCategory[]
		currentCategoryId: string | null
		onSelect: (categoryId: string) => void
		onClose: () => void
	}
	let { open, categories, currentCategoryId, onSelect, onClose }: Props = $props()

	function choose(categoryId: string) {
		onSelect(categoryId)
		onClose()
	}
</script>

<MobileModal {open} {onClose} title={$translate('tags.moveToCategory')}>
	<!-- Cancel the modal body's px-4 so rows sit edge-to-edge (PlaylistPickerSheet convention). -->
	<div class="-mx-4 flex flex-col">
		{#each categories as category (category.id)}
			{@const current = category.id === currentCategoryId}
			<MobileListItem disabled={current} onclick={() => choose(category.id)}>
				{#snippet leading()}
					<span class="block h-3.5 w-3.5 rounded-full" style="background-color: {category.color ?? DEFAULT_TAG_COLOR}"
					></span>
				{/snippet}
				<span class="block truncate text-sm">{category.name}</span>
				{#snippet trailing()}
					{#if current}
						<svg
							class="h-5 w-5 text-brand-primary"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2.5"
						>
							<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
						</svg>
					{/if}
				{/snippet}
			</MobileListItem>
		{/each}
	</div>
</MobileModal>
