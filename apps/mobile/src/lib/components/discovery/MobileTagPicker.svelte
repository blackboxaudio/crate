<script lang="ts">
	import { translate } from '$shared/i18n'
	import { tagsStore } from '$shared/stores/tags'
	import { discoveryStore } from '$shared/stores/discovery'
	import MobileModal from '$lib/components/common/MobileModal.svelte'
	import { SvelteMap } from 'svelte/reactivity'

	// Bottom-sheet tag picker for one OR MANY discovery releases. Tags are grouped by category; tapping a
	// chip toggles its assignment across every passed release. With a single release a chip is plain on/off;
	// with several it's tri-state — "on" (check) only when EVERY selected release has the tag, "mixed"
	// (dash) when only some do, off otherwise. Assign/remove call through the discovery store, which reloads
	// releases, so the selection re-resolves against the fresh data and the chips re-render in place.
	type Props = {
		open: boolean
		releaseIds: string[]
		onClose: () => void
	}
	let { open, releaseIds, onClose }: Props = $props()

	// Lazy-load categories the first time the sheet opens.
	let loadedOnce = $state(false)
	$effect(() => {
		if (open && !loadedOnce) {
			loadedOnce = true
			void tagsStore.load()
		}
	})

	// Resolve the selected releases live from the store so each assign/remove reload reflects immediately.
	const selectedReleases = $derived($discoveryStore.releases.filter((r) => releaseIds.includes(r.id)))

	// How many of the selected releases carry each tag → drives the tri-state below.
	const tagCounts = $derived.by(() => {
		const counts = new SvelteMap<string, number>()
		for (const r of selectedReleases) {
			for (const t of r.tags) counts.set(t.id, (counts.get(t.id) ?? 0) + 1)
		}
		return counts
	})

	function stateOf(tagId: string): 'active' | 'mixed' | 'inactive' {
		const count = tagCounts.get(tagId) ?? 0
		if (count === 0) return 'inactive'
		if (count === selectedReleases.length) return 'active'
		return 'mixed'
	}

	function toggle(tagId: string) {
		if (releaseIds.length === 0) return
		// Fully assigned → remove from all; otherwise assign to all (fills in the mixed / none cases).
		if (stateOf(tagId) === 'active') {
			void discoveryStore.removeTags(releaseIds, [tagId])
		} else {
			void discoveryStore.assignTags(releaseIds, [tagId])
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
					<div class="flex flex-wrap gap-2">
						{#each category.tags as tag (tag.id)}
							{@const color = tag.color ?? category.color ?? '#888888'}
							{@const st = stateOf(tag.id)}
							{@const lit = st === 'active' || st === 'mixed'}
							<button
								type="button"
								class="inline-flex items-center gap-1 rounded-md px-3 py-2 text-sm font-medium transition-colors {lit
									? ''
									: 'border border-stroke bg-surface-2 text-text-secondary'}"
								style={lit ? `background-color: ${color}20; color: ${color}; border: 1px solid ${color}40;` : ''}
								aria-pressed={st === 'active'}
								onclick={() => toggle(tag.id)}
							>
								{#if st === 'active'}
									<svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
										<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
									</svg>
								{:else if st === 'mixed'}
									<svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
										<path d="M5 12h14" stroke-linecap="round" />
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
