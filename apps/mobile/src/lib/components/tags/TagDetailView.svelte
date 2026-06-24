<script lang="ts">
	import type { Tag } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { discoveryStore, isDiscoveryLoading } from '$shared/stores/discovery'
	import { mobileUIStore, selectMode, selectedReleaseIds } from '$lib/stores/mobileUI'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import ReleaseCard from '$lib/components/discovery/ReleaseCard.svelte'
	import ReleaseFeedList from '$lib/components/discovery/ReleaseFeedList.svelte'
	import ReleaseContextMenu from '$lib/components/discovery/ReleaseContextMenu.svelte'
	import SelectionBar from '$lib/components/discovery/SelectionBar.svelte'
	import PlaylistPickerSheet from '$lib/components/playlists/PlaylistPickerSheet.svelte'

	// Tag detail: a full-screen drill-in feed of every discovery release carrying this tag (opened by tapping a
	// tag in the Tags tab). Mirrors PlaylistDetailView — a right-edge Drawer over the shell rendering the same
	// virtualized ReleaseFeedList the Discovery feed uses — but the release set is derived client-side from the
	// already-loaded discovery releases (tag membership rides on each release), so it needs no separate store or
	// fetch. Long-press / multi-select reuse the shared release machinery via a dedicated `tag` actions context
	// (so this instance and the shell's feed instance never both open for the same release).
	type Props = {
		tag: Tag
		categoryColor: string | null
	}
	let { tag, categoryColor }: Props = $props()

	let open = $state(true)

	// Tags inherit their category's color; fall back to the app's default indigo if neither is set.
	const dotColor = $derived(tag.color ?? categoryColor ?? '#6366f1')

	// Releases carrying this tag, taken from the shared discovery set (re-derives as tags change / sync lands).
	const releases = $derived($discoveryStore.releases.filter((r) => r.tags.some((t) => t.id === tag.id)))

	const isSelectMode = $derived($selectMode)

	let pickerOpen = $state(false)
	let pickerReleaseIds = $state<string[]>([])

	// The feed normally loads these on boot, but the Tags tab can be reached first — fetch if the set is empty.
	$effect(() => {
		if ($discoveryStore.releases.length === 0) discoveryStore.loadReleases()
	})

	function startClose() {
		open = false
		mobileUIStore.beginCloseTag()
	}

	function onClosed() {
		mobileUIStore.closeTag()
	}

	function openPickerForSingle(releaseId: string) {
		pickerReleaseIds = [releaseId]
		pickerOpen = true
	}

	function openPickerForSelection() {
		pickerReleaseIds = [...$selectedReleaseIds]
		pickerOpen = true
	}
</script>

<Drawer
	{open}
	direction="right"
	onClose={startClose}
	{onClosed}
	z={30}
	scrimZ={20}
	scrimDismiss={false}
	closeEdgeFrom="left"
	closeEdgeSize={24}
	ariaLabel={tag.name}
	class="pt-safe flex w-full flex-col bg-surface-0"
>
	{#snippet children({ animating })}
		<!-- Header: back chevron + the tag's color dot + its name. -->
		<div class="flex items-center gap-1 px-2 py-2">
			<button
				type="button"
				class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
				aria-label={$translate('common.close')}
				onclick={startClose}
			>
				<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M15 18l-6-6 6-6" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</button>
			<span class="h-3 w-3 flex-shrink-0 rounded-full" style="background-color: {dotColor}"></span>
			<h1 class="truncate text-lg font-semibold text-text-primary">{tag.name}</h1>
		</div>

		<!-- Content: the same virtualized list the Discovery feed uses, fed this tag's releases. -->
		{#if $isDiscoveryLoading && $discoveryStore.releases.length === 0}
			<div class="flex flex-1 items-center justify-center py-12">
				<Spinner class="h-6 w-6 text-text-tertiary" />
			</div>
		{:else if releases.length === 0}
			<div class="flex-1 px-4 py-12 text-center text-sm text-text-secondary">
				{$translate('discovery.noReleasesYet')}
			</div>
		{:else}
			<ReleaseFeedList {releases} scrollLocked={animating} row={releaseRow} />
		{/if}
	{/snippet}
</Drawer>

{#snippet releaseRow({ release })}
	<ReleaseCard {release} context="tag" />
{/snippet}

{#if isSelectMode}
	<SelectionBar onAddToPlaylist={openPickerForSelection} />
{/if}

<ReleaseContextMenu context="tag" {releases} onAddToPlaylist={openPickerForSingle} />

<PlaylistPickerSheet open={pickerOpen} releaseIds={pickerReleaseIds} onClose={() => (pickerOpen = false)} />
