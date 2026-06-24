<script lang="ts">
	import { get } from 'svelte/store'
	import { untrack } from 'svelte'
	import { fade } from 'svelte/transition'
	import { translate } from '$shared/i18n'
	import { playlistsStore, getPlaylistChildren } from '$shared/stores/playlists'
	import { toastStore } from '$shared/stores/toast'
	import { getPlaylistCovers, ensurePlaylistCovers, refreshPlaylistCovers } from '$lib/stores/playlistCovers'
	import { lightTap } from '$lib/utils/haptics'
	import MobileModal from '$lib/components/common/MobileModal.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'
	import PlaylistThumbnail from './PlaylistThumbnail.svelte'

	type Props = {
		open: boolean
		releaseIds: string[]
		onClose: () => void
	}
	let { open, releaseIds, onClose }: Props = $props()

	let creating = $state(false)
	let newName = $state('')
	let query = $state('')
	// Folder-drilldown state, mirroring the Playlists tab so navigation feels identical.
	let folderStack = $state<string[]>([])

	// Releases live in the discovery context. Folders are kept for navigation; smart playlists are
	// excluded as add-targets — their contents are rule-generated, so a manually-added release would
	// never show up (matches desktop's Add-to-Playlist menu, which excludes them for the same reason).
	const discoveryPlaylists = $derived($playlistsStore.playlists.filter((p) => p.context === 'discovery'))

	const currentFolderId = $derived(folderStack.length > 0 ? folderStack[folderStack.length - 1] : null)
	const currentFolder = $derived(
		currentFolderId ? (discoveryPlaylists.find((p) => p.id === currentFolderId) ?? null) : null
	)

	// Browse level: folders + regular (non-smart) playlists directly under the current folder.
	const browseItems = $derived.by(() => {
		const atLevel = currentFolderId
			? getPlaylistChildren(discoveryPlaylists, currentFolderId)
			: discoveryPlaylists.filter((p) => p.parent_id === null)
		return atLevel
			.filter((p) => !p.is_smart)
			.sort((a, b) => {
				if (a.is_folder !== b.is_folder) return a.is_folder ? -1 : 1
				return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
			})
	})

	// A search query flattens the whole tree to regular playlists matching the name (folders/smart excluded).
	const searchResults = $derived.by(() => {
		const q = query.trim().toLowerCase()
		if (!q) return []
		return discoveryPlaylists
			.filter((p) => !p.is_folder && !p.is_smart && p.name.toLowerCase().includes(q))
			.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }))
	})

	// Batch-load mosaic covers for whichever playlists are currently visible (the browse level or the
	// search results). Untrack the cover-map reads inside `ensure` so loading covers doesn't re-trigger this.
	$effect(() => {
		const visible = query.trim() ? searchResults : browseItems.filter((i) => !i.is_folder)
		const ids = visible.map((i) => i.id)
		if (ids.length > 0) untrack(() => ensurePlaylistCovers(ids))
	})

	function reset() {
		creating = false
		newName = ''
		query = ''
		folderStack = []
	}

	function pushFolder(folderId: string) {
		void lightTap()
		folderStack = [...folderStack, folderId]
	}

	function popFolder() {
		void lightTap()
		folderStack = folderStack.slice(0, -1)
	}

	function enterCreate() {
		// Seed the name with the current query so "search, then create what you typed" is one tap.
		newName = query.trim()
		creating = true
	}

	async function addTo(playlistId: string) {
		await playlistsStore.addReleases(playlistId, releaseIds)
		void refreshPlaylistCovers(playlistId)
		toastStore.success(get(translate)('contextMenu.addToPlaylist'))
		handleClose()
	}

	async function createAndAdd() {
		const trimmed = newName.trim()
		if (!trimmed) return
		const playlist = await playlistsStore.createPlaylist(trimmed, currentFolderId ?? undefined, 'discovery')
		if (!playlist) return
		await playlistsStore.addReleases(playlist.id, releaseIds)
		void refreshPlaylistCovers(playlist.id)
		toastStore.success(get(translate)('contextMenu.addToPlaylist'))
		handleClose()
	}

	function handleClose() {
		reset()
		onClose()
	}
</script>

<MobileModal {open} onClose={handleClose} title={$translate('contextMenu.addToPlaylist')}>
	<!-- Cancel the modal body's px-4 so rows sit edge-to-edge; the search field re-pads to align with row content. -->
	<div class="-mx-4 flex flex-col">
		<!-- Search -->
		<div class="px-4 pb-2">
			<div class="relative">
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
		</div>

		{#if query.trim()}
			<!-- Search: flat results across the whole tree, regular playlists only. -->
			{#each searchResults as playlist (playlist.id)}
				<MobileListItem onclick={() => addTo(playlist.id)}>
					{#snippet leading()}
						<PlaylistThumbnail urls={getPlaylistCovers(playlist.id)} />
					{/snippet}
					{#snippet trailing()}
						<span class="text-xs tabular-nums">{playlist.track_count}</span>
					{/snippet}
					<span class="truncate text-sm">{playlist.name}</span>
				</MobileListItem>
			{/each}
			{@render createRow()}
		{:else}
			<!-- Browse the folder/playlist hierarchy. -->
			{#if folderStack.length > 0}
				<button
					type="button"
					class="flex min-h-[44px] w-full items-center gap-1 px-4 text-left text-text-primary active:bg-surface-2"
					onclick={popFolder}
				>
					<svg class="h-5 w-5 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M15 18l-6-6 6-6" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
					<span class="truncate text-sm font-medium">{currentFolder?.name ?? ''}</span>
				</button>
			{/if}

			{@render createRow()}

			{#key currentFolderId}
				<div class="flex flex-col" in:fade|local={{ duration: 120 }}>
					{#each browseItems as item (item.id)}
						{#if item.is_folder}
							<MobileListItem onclick={() => pushFolder(item.id)}>
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
								<span class="truncate text-sm">{item.name}</span>
							</MobileListItem>
						{:else}
							<MobileListItem onclick={() => addTo(item.id)}>
								{#snippet leading()}
									<PlaylistThumbnail urls={getPlaylistCovers(item.id)} />
								{/snippet}
								{#snippet trailing()}
									<span class="text-xs tabular-nums">{item.track_count}</span>
								{/snippet}
								<span class="truncate text-sm">{item.name}</span>
							</MobileListItem>
						{/if}
					{/each}

					{#if browseItems.length === 0}
						<p class="px-4 py-6 text-center text-sm text-text-secondary">
							{folderStack.length > 0 ? $translate('playlists.folderEmpty') : $translate('playlists.noPlaylists')}
						</p>
					{/if}
				</div>
			{/key}
		{/if}
	</div>
</MobileModal>

<!-- Create-a-playlist row (and its inline name field). Shared between browse and search modes. -->
{#snippet createRow()}
	{#if creating}
		<div class="flex items-center gap-2 px-4 py-2">
			<input
				type="text"
				bind:value={newName}
				placeholder={$translate('modals.createPlaylist.placeholder')}
				class="min-w-0 flex-1 rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
				onkeydown={(e) => e.key === 'Enter' && createAndAdd()}
				autofocus
			/>
			<button
				type="button"
				class="rounded-md bg-brand-primary px-3 py-2 text-sm font-medium text-white disabled:opacity-40"
				disabled={!newName.trim()}
				onclick={createAndAdd}
			>
				{$translate('common.create')}
			</button>
		</div>
	{:else}
		<MobileListItem onclick={enterCreate}>
			{#snippet leading()}
				<div
					class="flex h-11 w-11 items-center justify-center rounded border border-dashed border-stroke text-brand-primary"
				>
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M12 5v14M5 12h14" stroke-linecap="round" />
					</svg>
				</div>
			{/snippet}
			<span class="text-sm font-medium text-brand-primary">{$translate('playlists.newPlaylist')}</span>
		</MobileListItem>
	{/if}
{/snippet}
