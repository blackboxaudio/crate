<script lang="ts">
	import { onMount, untrack } from 'svelte'
	import { get } from 'svelte/store'
	import { fade } from 'svelte/transition'
	import { translate } from '$shared/i18n'
	import type { Playlist, SmartRules } from '$shared/types'
	import { playlistsStore, getPlaylistChildren } from '$shared/stores/playlists'
	import { mobileUIStore } from '$lib/stores/mobileUI'
	import { easeFluid } from '$lib/easing'
	import { swipe, type SwipeOptions } from '$lib/actions/swipe'
	import { getPlaylistCovers, ensurePlaylistCovers } from '$lib/stores/playlistCovers'
	import { confirmDialog } from '$lib/utils/dialog'
	import { lightTap, rigidTap } from '$lib/utils/haptics'
	import MobileList from '$lib/components/common/MobileList.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'
	import MobilePromptDialog from '$lib/components/common/MobilePromptDialog.svelte'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import ContextMenuItem from '$lib/components/common/ContextMenuItem.svelte'
	import PlaylistThumbnail from './PlaylistThumbnail.svelte'
	import SmartPlaylistEditor from './SmartPlaylistEditor.svelte'

	onMount(() => {
		playlistsStore.load()
		const mq = window.matchMedia('(prefers-reduced-motion: reduce)')
		reduceMotion = mq.matches
		const onMq = () => (reduceMotion = mq.matches)
		mq.addEventListener('change', onMq)
		return () => mq.removeEventListener('change', onMq)
	})

	const allPlaylists = $derived($playlistsStore.playlists.filter((p) => p.context === 'discovery'))

	let folderStack = $state<string[]>([])
	const currentFolderId = $derived(folderStack.length > 0 ? folderStack[folderStack.length - 1] : null)
	const currentFolder = $derived(currentFolderId ? (allPlaylists.find((p) => p.id === currentFolderId) ?? null) : null)

	const children = $derived.by(() => {
		const items = currentFolderId
			? getPlaylistChildren(allPlaylists, currentFolderId)
			: allPlaylists.filter((p) => p.parent_id === null)
		return [...items].sort((a, b) => {
			if (a.is_folder !== b.is_folder) return a.is_folder ? -1 : 1
			return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
		})
	})

	// Batch-load mosaic covers for the playlists shown at the current level; re-runs as folder
	// navigation changes which playlists are visible. Folders have no covers, so they're excluded.
	$effect(() => {
		const ids = children.filter((c) => !c.is_folder).map((c) => c.id)
		// Re-run only when the visible playlists change — untrack the cover-map reads inside `ensure`
		// (its `.has()` checks are reactive) so loading covers doesn't re-trigger this effect.
		if (ids.length > 0) untrack(() => ensurePlaylistCovers(ids))
	})

	// --- Drill-down transition --------------------------------------------------------------------
	// The level swaps via `{#key currentFolderId}`; a directional slide+fade signals depth — drilling in
	// pushes the incoming level from the right (outgoing exits left), going back reverses it. Matches the
	// app's `--ease-fluid` sheet motion; collapses to an instant swap under reduced motion.
	let reduceMotion = $state(false)
	let navDirection = $state<'forward' | 'back'>('forward')
	const NAV_SLIDE = 28 // px — a subtle parallax, not a full-width push

	function levelTransition(_node: Element, { incoming }: { incoming: boolean }) {
		if (reduceMotion) return { duration: 0 }
		const sign = navDirection === 'forward' ? 1 : -1
		// Incoming enters from the +sign edge (forward → right, back → left); outgoing exits to the −sign edge.
		const dir = incoming ? sign : -sign
		return {
			duration: incoming ? 260 : 200,
			easing: easeFluid,
			css: (t: number, u: number) => `transform: translateX(${dir * u * NAV_SLIDE}px); opacity: ${t};`,
		}
	}

	function pushFolder(folderId: string) {
		void lightTap()
		navDirection = 'forward'
		folderStack = [...folderStack, folderId]
	}

	function popFolder() {
		void lightTap()
		navDirection = 'back'
		folderStack = folderStack.slice(0, -1)
	}

	// Interactive back-swipe to the parent folder: an edge-swipe from the left edge dragging right, mirroring
	// the detail-view dismiss gesture (`closeEdgeFrom: 'left'`). Active only inside a folder; on commit it runs
	// the same `popFolder` (and its 'back' slide) as the header back button. Lives on the persistent content
	// container below — outside the `{#key}` — so the gesture survives level swaps.
	const backSwipe = $derived<SwipeOptions>({
		side: 'right',
		mode: 'close',
		closeEdgeFrom: 'left',
		closeEdgeSize: 24,
		enabled: folderStack.length > 0,
		onClose: popFolder,
	})

	function openPlaylist(playlistId: string) {
		void lightTap()
		mobileUIStore.openPlaylist(playlistId)
	}

	// --- Create / Rename / Delete ---
	let createModalOpen = $state(false)
	let createType = $state<'playlist' | 'folder'>('playlist')
	let createName = $state('')

	let renameModalOpen = $state(false)
	let renameTarget = $state<Playlist | null>(null)
	let renameName = $state('')

	let smartEditorOpen = $state(false)

	// The single "+" add menu (folder / playlist / smart playlist), anchored to the toolbar button.
	let addMenuOpen = $state(false)
	let addMenuRect = $state<{ top: number; left: number; width: number; height: number } | null>(null)

	let longPressTimer = 0
	let longPressTarget = $state<Playlist | null>(null)
	let rowActionsOpen = $state(false)
	// Viewport rect of the long-pressed row, so the context menu can lift it in place.
	let longPressRect = $state<{ top: number; left: number; width: number; height: number } | null>(null)
	// A stationary long-press also synthesizes a click on release; this latches so we can swallow that one
	// click (otherwise opening the menu would also navigate into the row — `MobileListItem` is a real <button>).
	let suppressNextClick = false

	function openAddMenu(e: MouseEvent) {
		void lightTap()
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
		addMenuRect = { top: r.top, left: r.left, width: r.width, height: r.height }
		addMenuOpen = true
	}

	function openCreate(type: 'playlist' | 'folder') {
		addMenuOpen = false
		createType = type
		createName = ''
		createModalOpen = true
	}

	function openSmartEditor() {
		addMenuOpen = false
		smartEditorOpen = true
	}

	async function handleCreate() {
		const trimmed = createName.trim()
		if (!trimmed) return
		createModalOpen = false
		if (createType === 'folder') {
			await playlistsStore.createFolder(trimmed, currentFolderId ?? undefined, 'discovery')
		} else {
			await playlistsStore.createPlaylist(trimmed, currentFolderId ?? undefined, 'discovery')
		}
		createName = ''
	}

	async function handleCreateSmart(name: string, rules: SmartRules) {
		smartEditorOpen = false
		await playlistsStore.createSmartPlaylist(name, rules, currentFolderId ?? undefined, 'discovery')
	}

	function openRename(playlist: Playlist) {
		rowActionsOpen = false
		renameTarget = playlist
		renameName = playlist.name
		renameModalOpen = true
	}

	async function handleRename() {
		if (!renameTarget || !renameName.trim()) return
		renameModalOpen = false
		await playlistsStore.rename(renameTarget.id, renameName.trim())
		renameTarget = null
	}

	async function handleDelete(playlist: Playlist) {
		rowActionsOpen = false
		const t = get(translate)
		const key = playlist.is_folder ? 'modals.confirm.deleteFolderMessage' : 'modals.confirm.deletePlaylistMessage'
		const titleKey = playlist.is_folder ? 'modals.confirm.deleteFolderTitle' : 'modals.confirm.deletePlaylistTitle'
		const ok = await confirmDialog(t(key), {
			title: t(titleKey),
			confirmLabel: t('common.delete'),
		})
		if (!ok) return
		if (folderStack.includes(playlist.id)) {
			folderStack = folderStack.slice(0, folderStack.indexOf(playlist.id))
		}
		await playlistsStore.delete(playlist.id)
	}

	function onRowLongPress(playlist: Playlist) {
		void rigidTap()
		longPressTarget = playlist
		rowActionsOpen = true
	}

	function startLongPress(e: PointerEvent, playlist: Playlist) {
		suppressNextClick = false
		if (longPressTimer) clearTimeout(longPressTimer)
		// Capture the row element now; `currentTarget` is nulled once the event finishes dispatching.
		const el = e.currentTarget as HTMLElement
		longPressTimer = window.setTimeout(() => {
			longPressTimer = 0
			const r = el?.getBoundingClientRect()
			longPressRect = r ? { top: r.top, left: r.left, width: r.width, height: r.height } : null
			suppressNextClick = true
			onRowLongPress(playlist)
		}, 450)
		window.addEventListener('pointermove', cancelLongPress, { once: true, passive: true })
		window.addEventListener('pointerup', cancelLongPress, { once: true })
		window.addEventListener('pointercancel', cancelLongPress, { once: true })
	}

	function cancelLongPress() {
		if (longPressTimer) {
			clearTimeout(longPressTimer)
			longPressTimer = 0
		}
	}

	// Swallow the synthesized click that follows a long-press so the menu doesn't also open the row.
	function onRowClickCapture(e: MouseEvent) {
		if (!suppressNextClick) return
		suppressNextClick = false
		e.preventDefault()
		e.stopPropagation()
	}
</script>

<div class="flex h-full flex-col">
	<!-- Navigation header (stays put; the title cross-fades as the level changes). -->
	<div class="flex items-center justify-between gap-1 px-2 py-2">
		<div class="relative flex h-10 min-w-0 flex-1 items-center">
			{#key currentFolderId}
				<div
					class="absolute inset-0 flex items-center gap-1"
					in:fade|local={{ duration: 160 }}
					out:fade|local={{ duration: 120 }}
				>
					{#if folderStack.length > 0}
						<button
							type="button"
							class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
							aria-label={$translate('common.back')}
							onclick={popFolder}
						>
							<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M15 18l-6-6 6-6" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						</button>
						<span class="truncate text-base font-medium text-text-primary">{currentFolder?.name ?? ''}</span>
					{:else}
						<span class="truncate px-2 text-base font-medium text-text-primary"
							>{$translate('playlists.allPlaylists')}</span
						>
					{/if}
				</div>
			{/key}
		</div>
		<button
			type="button"
			class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-md text-text-secondary active:bg-surface-2"
			aria-label={$translate('common.create')}
			onclick={openAddMenu}
		>
			<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M12 5v14M5 12h14" stroke-linecap="round" />
			</svg>
		</button>
	</div>

	<!-- Sliding content: each level owns its own scroll container so they can slide over one another. -->
	<div class="relative min-h-0 flex-1 overflow-hidden" use:swipe={backSwipe}>
		{#key currentFolderId}
			<div
				class="absolute inset-0 overflow-y-auto"
				style="padding-bottom: var(--mini-player-inset, 0px)"
				in:levelTransition|local={{ incoming: true }}
				out:levelTransition|local={{ incoming: false }}
			>
				<MobileList isEmpty={children.length === 0} empty={emptyState}>
					{#each children as item (item.id)}
						{#if item.is_folder}
							<div onpointerdown={(e) => startLongPress(e, item)} onclickcapture={onRowClickCapture}>
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
									<span class="truncate">{item.name}</span>
								</MobileListItem>
							</div>
						{:else}
							<div onpointerdown={(e) => startLongPress(e, item)} onclickcapture={onRowClickCapture}>
								<MobileListItem onclick={() => openPlaylist(item.id)}>
									{#snippet leading()}
										<PlaylistThumbnail urls={getPlaylistCovers(item.id)} smart={item.is_smart} />
									{/snippet}
									{#snippet trailing()}
										<span class="text-xs tabular-nums">{item.track_count}</span>
									{/snippet}
									<span class="truncate">{item.name}</span>
								</MobileListItem>
							</div>
						{/if}
					{/each}
				</MobileList>
			</div>
		{/key}
	</div>
</div>

{#snippet emptyState()}
	{#if folderStack.length > 0}
		{$translate('playlists.folderEmpty')}
	{:else}
		{$translate('playlists.noPlaylists')}
	{/if}
{/snippet}

<!-- "+" add menu: folder / playlist / smart playlist, anchored to the toolbar button (no lifted preview). -->
<ContextMenu open={addMenuOpen} anchorRect={addMenuRect} tapTriggered onClose={() => (addMenuOpen = false)}>
	<ContextMenuItem onclick={() => openCreate('folder')}>
		{$translate('playlists.newFolder')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path
					d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={() => openCreate('playlist')}>
		{$translate('playlists.newPlaylist')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M3 6h11M3 12h11M3 18h7M16 9v9M16 9l5-2v9" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={openSmartEditor}>
		{$translate('playlists.newSmartPlaylist')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path
					d="M5 3v4M3 5h4M6 17v4M4 19h4M13 3l2.5 6.5L22 12l-6.5 2.5L13 21l-2.5-6.5L4 12l6.5-2.5L13 3z"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
		{/snippet}
	</ContextMenuItem>
</ContextMenu>

<!-- Create playlist/folder dialog (centered so the iOS keyboard never covers it). -->
<MobilePromptDialog
	open={createModalOpen}
	bind:value={createName}
	title={createType === 'folder' ? $translate('modals.createFolder.title') : $translate('modals.createPlaylist.title')}
	placeholder={createType === 'folder'
		? $translate('modals.createFolder.placeholder')
		: $translate('modals.createPlaylist.placeholder')}
	confirmDisabled={!createName.trim()}
	onConfirm={handleCreate}
	onCancel={() => (createModalOpen = false)}
/>

<!-- Rename dialog -->
<MobilePromptDialog
	open={renameModalOpen}
	bind:value={renameName}
	title={$translate('modals.rename.title')}
	placeholder={$translate('modals.rename.placeholder')}
	confirmLabel={$translate('common.save')}
	confirmDisabled={!renameName.trim()}
	onConfirm={handleRename}
	onCancel={() => (renameModalOpen = false)}
/>

<!-- Smart playlist rule editor -->
<SmartPlaylistEditor
	open={smartEditorOpen}
	context="discovery"
	onSubmit={handleCreateSmart}
	onCancel={() => (smartEditorOpen = false)}
/>

<!-- Row long-press context menu -->
<ContextMenu
	open={rowActionsOpen}
	anchorRect={longPressRect}
	onClose={() => (rowActionsOpen = false)}
	onClosed={() => {
		longPressTarget = null
		longPressRect = null
	}}
>
	{#snippet preview()}
		{#if longPressTarget}
			<span class="flex-shrink-0">
				{#if longPressTarget.is_folder}
					<div class="flex h-11 w-11 items-center justify-center rounded bg-surface-2 text-text-secondary">
						<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path
								d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
					</div>
				{:else}
					<PlaylistThumbnail urls={getPlaylistCovers(longPressTarget.id)} smart={longPressTarget.is_smart} />
				{/if}
			</span>
			<span class="min-w-0 flex-1 truncate text-sm text-text-primary">{longPressTarget.name}</span>
		{/if}
	{/snippet}

	<ContextMenuItem onclick={() => longPressTarget && openRename(longPressTarget)}>
		{$translate('common.rename')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M17 3a2.828 2.828 0 114 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem destructive onclick={() => longPressTarget && handleDelete(longPressTarget)}>
		{$translate('common.delete')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path
					d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m2 0v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
		{/snippet}
	</ContextMenuItem>
</ContextMenu>
