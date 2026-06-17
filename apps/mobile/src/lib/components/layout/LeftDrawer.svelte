<script lang="ts">
	import { translate } from '$shared/i18n'
	import { playlistsStore } from '$shared/stores/playlists'
	import { tagsStore } from '$shared/stores/tags'
	import { swipe } from '$lib/actions/swipe'
	import MobileList from '$lib/components/common/MobileList.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'

	// Left navigation drawer: Playlists + Tags. Slides in from the left edge; supports finger-follow
	// swipe-to-close. `dragOpenness` is driven by the shell's left-edge open gesture (0→1); the
	// drawer's own close gesture drives `closeDrag`. Effective openness combines both with the
	// store-backed `open` state.
	type Props = {
		open: boolean
		dragOpenness?: number | null
		widthPx: number
		onClose: () => void
	}

	let { open, dragOpenness = null, widthPx, onClose }: Props = $props()

	let closeDrag = $state<number | null>(null)
	let loadedOnce = false

	const openness = $derived(closeDrag ?? dragOpenness ?? (open ? 1 : 0))
	const transitionOn = $derived(closeDrag === null && dragOpenness === null)
	const visible = $derived(openness > 0)
	const offset = $derived(-(1 - openness) * 100)

	// Lazy-load playlists + tags the first time the drawer becomes visible, so app start stays light.
	$effect(() => {
		if (visible && !loadedOnce) {
			loadedOnce = true
			playlistsStore.load()
			tagsStore.load()
		}
	})

	// Flat list of non-folder playlists (folder-tree navigation is a later issue).
	const playlists = $derived($playlistsStore.playlists.filter((p) => !p.is_folder))
	const categories = $derived($tagsStore.categories)
</script>

<aside
	class="fixed inset-y-0 left-0 z-50 flex w-[85%] max-w-[320px] flex-col overflow-hidden border-r border-stroke bg-surface-1 {transitionOn
		? 'transition-transform duration-300 ease-out motion-reduce:transition-none'
		: ''}"
	style="transform: translateX({offset}%)"
	aria-hidden={!open}
	use:swipe={{
		side: 'left',
		mode: 'close',
		enabled: open,
		onProgress: (o) => (closeDrag = o),
		onOpen: () => (closeDrag = null),
		onClose: () => {
			closeDrag = null
			onClose()
		},
	}}
>
	{#if visible}
		<div class="pt-safe pl-safe flex-1 overflow-y-auto pb-6">
			<MobileList title={$translate('nav.playlists')} isEmpty={playlists.length === 0} empty={emptyPlaylists}>
				{#each playlists as playlist (playlist.id)}
					<MobileListItem>
						{#snippet trailing()}
							<span class="text-xs tabular-nums">{playlist.track_count}</span>
						{/snippet}
						<span class="truncate">{playlist.name}</span>
					</MobileListItem>
				{/each}
			</MobileList>

			<section class="flex flex-col">
				<h2 class="px-4 pt-4 pb-1 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
					{$translate('nav.tags')}
				</h2>
				{#if categories.length === 0}
					<div class="px-4 py-6 text-sm text-text-secondary">{$translate('tags.noTags')}</div>
				{:else}
					{#each categories as category (category.id)}
						<div class="px-4 py-2">
							<h3 class="mb-1.5 text-sm font-medium text-text-secondary">{category.name}</h3>
							<div class="flex flex-wrap gap-1.5">
								{#each category.tags as tag (tag.id)}
									{@const color = tag.color ?? category.color ?? '#6366f1'}
									<span
										class="rounded px-2 py-1 text-xs font-medium"
										style="background-color: {color}20; color: {color}; border: 1px solid {color}40;"
									>
										{tag.name}
									</span>
								{/each}
							</div>
						</div>
					{/each}
				{/if}
			</section>
		</div>
	{/if}
</aside>

{#if visible}
	<!-- Edge-grab strip on the backdrop side of the drawer's edge: grab to drag the drawer closed
	     (pairs with the panel's own swipe so the edge is grabbable from either side), or tap to
	     dismiss like the rest of the backdrop. -->
	<button
		type="button"
		aria-label={$translate('common.close')}
		class="fixed inset-y-0 z-[45] w-8 bg-transparent {transitionOn
			? 'transition-[left] duration-300 ease-out motion-reduce:transition-none'
			: ''}"
		style="left: {openness * widthPx}px"
		onclick={onClose}
		use:swipe={{
			side: 'left',
			mode: 'close',
			enabled: open,
			width: widthPx,
			onProgress: (o) => (closeDrag = o),
			onOpen: () => (closeDrag = null),
			onClose: () => {
				closeDrag = null
				onClose()
			},
		}}
	></button>
{/if}

{#snippet emptyPlaylists()}
	{$translate('playlists.noPlaylists')}
{/snippet}
