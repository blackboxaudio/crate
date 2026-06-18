<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease } from '$shared/types'
	import { tagsStore } from '$shared/stores/tags'
	import { discoveryStore } from '$shared/stores/discovery'
	import MobileModal from '$lib/components/common/MobileModal.svelte'

	// Bottom-sheet tag picker for a discovery release. Tags are grouped by category; tapping a chip
	// toggles its assignment immediately (assign/remove call through the discovery store, which reloads
	// releases — so the passed-in `release.tags` updates and the chips re-render in place).
	type Props = {
		open: boolean
		release: DiscoveryRelease
		onClose: () => void
	}
	let { open, release, onClose }: Props = $props()

	// Lazy-load categories the first time the sheet opens.
	let loadedOnce = $state(false)
	$effect(() => {
		if (open && !loadedOnce) {
			loadedOnce = true
			void tagsStore.load()
		}
	})

	const assigned = $derived(new Set(release.tags.map((t) => t.id)))

	function toggle(tagId: string) {
		if (assigned.has(tagId)) {
			void discoveryStore.removeTags([release.id], [tagId])
		} else {
			void discoveryStore.assignTags([release.id], [tagId])
		}
	}
</script>

<MobileModal {open} {onClose} title={$translate('nav.tags')}>
	{#if $tagsStore.loading && $tagsStore.categories.length === 0}
		<p class="py-6 text-center text-sm text-text-secondary">{$translate('common.loading')}</p>
	{:else if $tagsStore.categories.length === 0}
		<p class="py-6 text-center text-sm text-text-secondary">{$translate('tags.noTags')}</p>
	{:else}
		<div class="flex flex-col gap-5">
			{#each $tagsStore.categories as category (category.id)}
				<div>
					<h3 class="mb-2 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
						{category.name}
					</h3>
					<div class="flex flex-wrap gap-1.5">
						{#each category.tags as tag (tag.id)}
							{@const color = tag.color ?? category.color ?? '#888888'}
							{@const on = assigned.has(tag.id)}
							<button
								type="button"
								class="inline-flex items-center gap-1 rounded px-2 py-1 text-sm font-medium transition-colors {on
									? ''
									: 'border border-stroke bg-surface-2 text-text-secondary'}"
								style={on ? `background-color: ${color}20; color: ${color}; border: 1px solid ${color}40;` : ''}
								aria-pressed={on}
								onclick={() => toggle(tag.id)}
							>
								{#if on}
									<svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
										<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
									</svg>
								{/if}
								{tag.name}
							</button>
						{/each}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</MobileModal>
