<script lang="ts">
	import { untrack } from 'svelte'
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { Playlist } from '$shared/types'
	import { playlistsStore, getPlaylistChildren } from '$shared/stores/playlists'
	import {
		mobileUIStore,
		scrollTopNonce,
		playlistFolderTrail,
		playlistsSort,
		type PlaylistsSortField,
	} from '$lib/stores/mobileUI'
	import type { SortOption } from '$lib/utils/listControls'
	import SortSheet from '$lib/components/discovery/SortSheet.svelte'
	import { longPress } from '$lib/actions/longPress'
	import { getPlaylistCovers, ensurePlaylistCovers } from '$lib/stores/playlistCovers'
	import { lightTap } from '$lib/utils/haptics'
	import EmptyState from '$lib/components/common/EmptyState.svelte'
	import MobileList from '$lib/components/common/MobileList.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'
	import MobileListSkeleton from '$lib/components/common/MobileListSkeleton.svelte'
	import MobileSearchInput from '$lib/components/common/MobileSearchInput.svelte'
	import PlaylistThumbnail from './PlaylistThumbnail.svelte'
	import PlaylistLevelMenus from './PlaylistLevelMenus.svelte'

	// One level of the playlist tree — the tab root (`folderId` null) or a folder's contents — rendered
	// identically wherever it sits: a pinned glass toolbar (search + sort + add) over a scrolling list of
	// folder and playlist rows. The tab root mounts it directly under the shell chrome; every folder level is
	// a full-screen push (`PlaylistFolderView`) wrapping it in a Drawer with a back header, so drilling into a
	// folder and drilling into a playlist read as the same navigation.
	type Props = {
		folderId: string | null
		/** Freeze scrolling while the enclosing Drawer slides (a scroll mid-slide fights the transform). */
		scrollLocked?: boolean
	}
	let { folderId, scrollLocked = false }: Props = $props()

	let menus = $state<ReturnType<typeof PlaylistLevelMenus>>()

	const allPlaylists = $derived($playlistsStore.playlists.filter((p) => p.context === 'discovery'))

	// Folders-first, then the user's chosen sort within each group (persisted in mobileUI so it survives
	// remounts and restarts). Name ties break date sorts for stability.
	function sortLevel(items: Playlist[]): Playlist[] {
		const { field, direction } = $playlistsSort
		const dir = direction === 'asc' ? 1 : -1
		return [...items].sort((a, b) => {
			if (a.is_folder !== b.is_folder) return a.is_folder ? -1 : 1
			if (field === 'name') return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }) * dir
			// ISO timestamps — string comparison is chronological.
			const aVal = a[field] ?? ''
			const bVal = b[field] ?? ''
			if (aVal < bVal) return -1 * dir
			if (aVal > bVal) return 1 * dir
			return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
		})
	}

	// This level's direct children.
	const children = $derived(
		sortLevel(folderId ? getPlaylistChildren(allPlaylists, folderId) : allPlaylists.filter((p) => p.parent_id === null))
	)

	// Folder-listing sort sheet (name / created / modified) — shares the generalized SortSheet.
	let sortOpen = $state(false)
	const playlistsSortOptions: SortOption[] = [
		{ field: 'name', labelKey: 'discovery.following.sort.name', defaultDir: 'asc' },
		{ field: 'date_created', labelKey: 'playlists.sort.dateCreated', defaultDir: 'desc' },
		{ field: 'date_modified', labelKey: 'playlists.sort.dateModified', defaultDir: 'desc' },
	]

	// Client-side search by name over this level AND everything beneath it — a playlist buried in a nested
	// folder must still be findable from wherever the user is standing. Matches carry the folder path
	// relative to this level so the row can say where it lives, and tapping one pushes every level on the
	// way. The query belongs to this level instance: a pushed child starts empty, and the level keeps its
	// query underneath (still filtered when the child pops back).
	let query = $state('')
	interface SearchHit {
		item: Playlist
		// Ancestor folders between this level and the item, top-down (empty for direct children).
		path: Playlist[]
	}
	const searchHits = $derived.by((): SearchHit[] => {
		const q = query.trim().toLowerCase()
		if (!q) return children.map((item) => ({ item, path: [] }))
		const hits: SearchHit[] = []
		const walk = (level: Playlist[], path: Playlist[]) => {
			for (const item of sortLevel(level)) {
				if (item.name.toLowerCase().includes(q)) hits.push({ item, path })
				if (item.is_folder) walk(getPlaylistChildren(allPlaylists, item.id), [...path, item])
			}
		}
		walk(children, [])
		return hits
	})

	let scrollEl = $state<HTMLElement | null>(null)
	// iOS "re-tap the active tab to scroll to top". Only the root level ever receives it (with a level
	// pushed, the re-tap pops instead). Ignore the initial nonce so a normal mount doesn't scroll.
	let seenScrollNonce = get(mobileUIStore).scrollTopNonce
	$effect(() => {
		const n = $scrollTopNonce
		if (n === seenScrollNonce) return
		seenScrollNonce = n
		scrollEl?.scrollTo({ top: 0, behavior: 'smooth' })
	})

	// Batch-load mosaic covers for the playlists currently shown (the level, or the search hits beneath it).
	// Folders have no covers, so they're excluded.
	$effect(() => {
		const ids = searchHits.filter((h) => !h.item.is_folder).map((h) => h.item.id)
		// Re-run only when the visible playlists change — untrack the cover-map reads inside `ensure`
		// (its `.has()` checks are reactive) so loading covers doesn't re-trigger this effect.
		if (ids.length > 0) untrack(() => ensurePlaylistCovers(ids))
	})

	// The trail up to and including THIS level. Built from the level's own position rather than the live
	// trail's end: a child level that is still sliding out remains in the trail while this level is already
	// tappable, and a push from here must replace it, not stack on top of it.
	function trailToHere(): string[] {
		const trail = get(playlistFolderTrail)
		if (folderId === null) return []
		const i = trail.indexOf(folderId)
		return i === -1 ? trail : trail.slice(0, i + 1)
	}

	// The search input is pinned above the list, so drop focus explicitly before a push — otherwise the
	// iOS keyboard stays up and hovers over the incoming level.
	function blurSearch() {
		;(document.activeElement as HTMLElement | null)?.blur()
	}

	// `via` = the intermediate folders between this level and the target (a search hit beneath a nested
	// folder), pushed along with it so the trail stays a true ancestor chain and back walks every level.
	function pushFolder(target: Playlist, via: Playlist[] = []) {
		void lightTap()
		blurSearch()
		mobileUIStore.setPlaylistFolderTrail([...trailToHere(), ...via.map((f) => f.id), target.id])
	}

	// A search hit inside a nested folder also pushes its folders, so closing the playlist lands the user
	// next to it rather than back at the level they searched from.
	function openPlaylist(target: Playlist, via: Playlist[] = []) {
		void lightTap()
		blurSearch()
		if (via.length > 0) mobileUIStore.setPlaylistFolderTrail([...trailToHere(), ...via.map((f) => f.id)])
		mobileUIStore.openPlaylist(target.id)
	}

	const pathLabel = (path: Playlist[]) => path.map((f) => f.name).join(' › ')
</script>

<!-- Pinned level chrome: the glass toolbar (search + sort + add) stays put while the list scrolls, matching
     the Discovery and Following toolbars and the detail views' control bar. -->
<div class="glass flex items-center gap-2 border-b border-stroke-subtle px-3 py-2">
	<MobileSearchInput
		value={query}
		oninput={(v) => (query = v)}
		placeholder={$translate('playlists.searchPlaceholder')}
	/>
	<button
		type="button"
		aria-label={$translate('discovery.sortBy')}
		class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md text-text-secondary active:bg-surface-2"
		onclick={() => (sortOpen = true)}
	>
		<svg
			viewBox="0 0 24 24"
			class="h-5 w-5"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
		>
			<path d="M3 8l4-4 4 4M7 4v16M21 16l-4 4-4-4M17 20V4" />
		</svg>
	</button>
	<button
		type="button"
		class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md text-text-secondary active:bg-surface-2"
		aria-label={$translate('common.create')}
		onclick={(e) => menus?.openAdd(e)}
	>
		<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
			<path d="M12 5v14M5 12h14" stroke-linecap="round" />
		</svg>
	</button>
</div>

<SortSheet
	open={sortOpen}
	onClose={() => (sortOpen = false)}
	options={playlistsSortOptions}
	current={$playlistsSort}
	onSelect={(field, direction) => mobileUIStore.setPlaylistsSort({ field: field as PlaylistsSortField, direction })}
/>

<div
	bind:this={scrollEl}
	class="min-h-0 flex-1 overflow-x-hidden overscroll-y-none {scrollLocked ? 'overflow-y-hidden' : 'overflow-y-auto'}"
	style="padding-bottom: var(--mini-player-inset, 0px)"
>
	{#if $playlistsStore.loading && allPlaylists.length === 0}
		<!-- First load only: the root remounts (and re-loads) on every tab visit, so gating on emptiness
		     or a populated list would flash back to skeleton on each return. -->
		<div role="status" aria-label={$translate('common.loading')}>
			<MobileListSkeleton />
		</div>
	{:else}
		<MobileList isEmpty={searchHits.length === 0} empty={emptyState}>
			{#each searchHits as { item, path } (item.id)}
				{#if item.is_folder}
					{@const childCount = getPlaylistChildren(allPlaylists, item.id).length}
					<div use:longPress={{ onLongPress: (rect) => menus?.openRowActions(item, rect) }}>
						<MobileListItem onclick={() => pushFolder(item, path)}>
							{#snippet leading()}
								<div class="flex h-11 w-11 items-center justify-center rounded bg-surface-2 text-text-secondary">
									<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
										<path
											d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"
											stroke-linecap="round"
											stroke-linejoin="round"
										/>
									</svg>
								</div>
							{/snippet}
							{#snippet trailing()}
								<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path d="M9 18l6-6-6-6" stroke-linecap="round" stroke-linejoin="round" />
								</svg>
							{/snippet}
							<span class="block truncate text-sm font-medium text-text-primary">{item.name}</span>
							<span class="block truncate text-xs text-text-tertiary">
								{#if path.length > 0}{pathLabel(path)} ·
								{/if}{childCount}
								{childCount === 1 ? $translate('library.item') : $translate('library.items')}
							</span>
						</MobileListItem>
					</div>
				{:else}
					<div use:longPress={{ onLongPress: (rect) => menus?.openRowActions(item, rect) }}>
						<MobileListItem onclick={() => openPlaylist(item, path)}>
							{#snippet leading()}
								<PlaylistThumbnail urls={getPlaylistCovers(item.id)} smart={item.is_smart} />
							{/snippet}
							<span class="block truncate text-sm font-medium text-text-primary">{item.name}</span>
							<span class="block truncate text-xs text-text-tertiary">
								{#if path.length > 0}{pathLabel(path)} ·
								{/if}{item.track_count}
								{item.track_count === 1 ? $translate('library.track') : $translate('library.tracks')}
							</span>
						</MobileListItem>
					</div>
				{/if}
			{/each}
		</MobileList>
	{/if}
</div>

{#snippet emptyState()}
	{#if query.trim()}
		<div class="py-4 text-center">{$translate('common.noResults')}</div>
	{:else if folderId !== null}
		<EmptyState title={$translate('playlists.folderEmpty')}>
			{#snippet icon()}
				<svg class="h-8 w-8" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path
						d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
			{/snippet}
		</EmptyState>
	{:else}
		<EmptyState
			title={$translate('playlists.noPlaylistsYet')}
			hint={$translate('playlists.emptyHint')}
			ctaLabel={$translate('playlists.newPlaylist')}
			onCta={() => menus?.openCreate('playlist')}
		>
			{#snippet icon()}
				<svg class="h-8 w-8" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M3 6h11M3 12h11M3 18h7M16 9v9M16 9l5-2v9" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			{/snippet}
		</EmptyState>
	{/if}
{/snippet}

<PlaylistLevelMenus bind:this={menus} {folderId} />
