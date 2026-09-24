<script lang="ts">
	import { translate } from '$shared/i18n'
	import { tagsStore } from '$shared/stores/tags'
	import { discoveryStore } from '$shared/stores/discovery'
	import { DEFAULT_TAG_COLOR } from '$shared/types'
	import MobileModal from '$lib/components/common/MobileModal.svelte'
	import { SvelteMap } from 'svelte/reactivity'

	// Bottom-sheet tag picker for one OR MANY discovery releases. Tags are grouped by category; tapping a
	// chip toggles its assignment across every passed release. With a single release a chip is plain on/off;
	// with several it's tri-state — "on" (check) only when EVERY selected release has the tag, "mixed"
	// (dash) when only some do, off otherwise. Assign/remove call through the discovery store, which reloads
	// releases, so the selection re-resolves against the fresh data and the chips re-render in place.
	// Track mode (`trackIds`) tags individual tracks instead — same chips, same tri-state — via the
	// track-level assign/remove, which patches the tracks in place.
	type Props = {
		open: boolean
		releaseIds?: string[]
		trackIds?: string[]
		onClose: () => void
	}
	let { open, releaseIds = [], trackIds = [], onClose }: Props = $props()
	const trackMode = $derived(trackIds.length > 0)

	// Search mirrors the playlist picker so the two "file it" sheets feel like one pattern: a query
	// narrows chips by name across every category and drops categories left with no match. Empty
	// categories are never shown (a heading with nothing to tap is a dead end).
	let query = $state('')
	const visibleCategories = $derived.by(() => {
		const q = query.trim().toLowerCase()
		return $tagsStore.categories
			.map((category) => ({
				...category,
				tags: q ? category.tags.filter((tag) => tag.name.toLowerCase().includes(q)) : category.tags,
			}))
			.filter((category) => category.tags.length > 0)
	})

	function handleClose() {
		query = ''
		onClose()
	}

	// Lazy-load categories the first time the sheet opens.
	let loadedOnce = $state(false)
	$effect(() => {
		if (open && !loadedOnce) {
			loadedOnce = true
			void tagsStore.load()
		}
	})

	// Resolve the selection live from the store so each assign/remove reload reflects immediately.
	const selectedReleases = $derived(trackMode ? [] : $discoveryStore.releases.filter((r) => releaseIds.includes(r.id)))
	const selectedTracks = $derived(
		trackMode ? $discoveryStore.releases.flatMap((r) => r.tracks.filter((t) => trackIds.includes(t.id))) : []
	)
	const selectionSize = $derived(trackMode ? selectedTracks.length : selectedReleases.length)

	// How many of the selected items carry each tag → drives the tri-state below.
	const tagCounts = $derived.by(() => {
		const counts = new SvelteMap<string, number>()
		for (const r of selectedReleases) {
			for (const t of r.tags) counts.set(t.id, (counts.get(t.id) ?? 0) + 1)
		}
		for (const track of selectedTracks) {
			for (const t of track.tags ?? []) counts.set(t.id, (counts.get(t.id) ?? 0) + 1)
		}
		return counts
	})

	function stateOf(tagId: string): 'active' | 'mixed' | 'inactive' {
		const count = tagCounts.get(tagId) ?? 0
		if (count === 0) return 'inactive'
		if (count === selectionSize) return 'active'
		return 'mixed'
	}

	function toggle(tagId: string) {
		if (selectionSize === 0) return
		const remove = stateOf(tagId) === 'active'
		if (trackMode) {
			// Tracks come from one release (the detail / player opens the picker per track).
			const releaseId = selectedTracks[0].release_id
			void (remove
				? discoveryStore.removeTrackTags(releaseId, trackIds, [tagId])
				: discoveryStore.assignTrackTags(releaseId, trackIds, [tagId]))
			return
		}
		// Fully assigned → remove from all; otherwise assign to all (fills in the mixed / none cases).
		if (remove) {
			void discoveryStore.removeTags(releaseIds, [tagId])
		} else {
			void discoveryStore.assignTags(releaseIds, [tagId])
		}
	}
</script>

<MobileModal {open} onClose={handleClose} title={$translate('nav.tags')}>
	{#if $tagsStore.loading && $tagsStore.categories.length === 0}
		<p class="py-6 text-center text-sm text-text-secondary">{$translate('common.loading')}</p>
	{:else if $tagsStore.categories.length === 0}
		<p class="py-6 text-center text-sm text-text-secondary">{$translate('tags.noTags')}</p>
	{:else}
		<div class="relative mb-4">
			<svg
				class="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-text-tertiary"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			>
				<circle cx="11" cy="11" r="7" />
				<path d="M21 21l-4.3-4.3" stroke-linecap="round" />
			</svg>
			<input
				type="text"
				bind:value={query}
				placeholder={$translate('common.search')}
				class="w-full rounded-lg border border-stroke bg-surface-1 py-2 pr-9 pl-9 text-sm text-text-primary placeholder:text-text-tertiary"
			/>
			{#if query}
				<button
					type="button"
					class="absolute top-1/2 right-1.5 flex h-7 w-7 -translate-y-1/2 items-center justify-center rounded-full text-text-tertiary active:bg-surface-2"
					aria-label={$translate('common.close')}
					onclick={() => (query = '')}
				>
					<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M6 6l12 12M18 6L6 18" stroke-linecap="round" />
					</svg>
				</button>
			{/if}
		</div>
		{#if visibleCategories.length === 0}
			<p class="py-6 text-center text-sm text-text-secondary">
				{query.trim() ? $translate('common.noResults') : $translate('tags.noTags')}
			</p>
		{/if}
		<div class="flex flex-col gap-5">
			{#each visibleCategories as category (category.id)}
				<div>
					<h3 class="mb-2 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
						{category.name}
					</h3>
					<div class="flex flex-wrap gap-2">
						{#each category.tags as tag (tag.id)}
							{@const color = tag.color ?? category.color ?? DEFAULT_TAG_COLOR}
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
