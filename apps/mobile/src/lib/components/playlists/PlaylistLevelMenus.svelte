<script lang="ts">
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { Playlist, SmartRules } from '$shared/types'
	import { playlistsStore } from '$shared/stores/playlists'
	import { discoveryPlaylistStore } from '$shared/stores/discoveryPlaylist'
	import type { LongPressRect } from '$lib/actions/longPress'
	import { getPlaylistCovers, refreshPlaylistCovers } from '$lib/stores/playlistCovers'
	import { confirmDialog } from '$lib/utils/dialog'
	import { lightTap } from '$lib/utils/haptics'
	import MobilePromptDialog from '$lib/components/common/MobilePromptDialog.svelte'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import ContextMenuItem from '$lib/components/common/ContextMenuItem.svelte'
	import PlaylistThumbnail from './PlaylistThumbnail.svelte'
	import SmartPlaylistEditor from './SmartPlaylistEditor.svelte'

	// The menus and dialogs of one playlist level (root or a folder): the "+" add menu, the create /
	// rename prompts, the smart-playlist editor, and the row long-press menu. Split out of
	// `PlaylistLevel` so the level itself stays a list; the level drives these through the exported
	// openers. Creations land in `folderId` (null = the root).
	type Props = {
		folderId: string | null
	}
	let { folderId }: Props = $props()

	let createModalOpen = $state(false)
	let createType = $state<'playlist' | 'folder'>('playlist')
	let createName = $state('')

	let renameModalOpen = $state(false)
	let renameTarget = $state<Playlist | null>(null)
	let renameName = $state('')

	let smartEditorOpen = $state(false)
	// Set when the editor opens for an existing smart playlist; null for the create flow. Managed at
	// open time only — clearing it on close would flash the editor title back to "create" mid-animation.
	let smartEditTarget = $state<Playlist | null>(null)

	// The single "+" add menu (folder / playlist / smart playlist), anchored to the toolbar button.
	let addMenuOpen = $state(false)
	let addMenuRect = $state<{ top: number; left: number; width: number; height: number } | null>(null)

	let longPressTarget = $state<Playlist | null>(null)
	let rowActionsOpen = $state(false)
	// Viewport rect of the long-pressed row, so the context menu can lift it in place.
	let longPressRect = $state<LongPressRect | null>(null)

	export function openAdd(e: MouseEvent) {
		void lightTap()
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
		addMenuRect = { top: r.top, left: r.left, width: r.width, height: r.height }
		addMenuOpen = true
	}

	export function openCreate(type: 'playlist' | 'folder') {
		addMenuOpen = false
		createType = type
		createName = ''
		createModalOpen = true
	}

	export function openRowActions(playlist: Playlist, rect: LongPressRect) {
		longPressRect = rect
		longPressTarget = playlist
		rowActionsOpen = true
	}

	function openSmartEditor() {
		addMenuOpen = false
		smartEditTarget = null
		smartEditorOpen = true
	}

	function openSmartEdit(playlist: Playlist) {
		rowActionsOpen = false
		smartEditTarget = playlist
		smartEditorOpen = true
	}

	async function handleCreate() {
		const trimmed = createName.trim()
		if (!trimmed) return
		createModalOpen = false
		if (createType === 'folder') {
			await playlistsStore.createFolder(trimmed, folderId ?? undefined, 'discovery')
		} else {
			await playlistsStore.createPlaylist(trimmed, folderId ?? undefined, 'discovery')
		}
		createName = ''
	}

	async function handleSmartSubmit(name: string, rules: SmartRules) {
		smartEditorOpen = false
		const target = smartEditTarget
		if (target) {
			await playlistsStore.updateSmartRules(target.id, rules)
			if (name !== target.name) await playlistsStore.rename(target.id, name)
			// New rules can change membership: drop the cached detail releases (refetched on next open)
			// and refresh the mosaic thumbnail.
			discoveryPlaylistStore.deleteFromCache(target.id)
			void refreshPlaylistCovers(target.id)
		} else {
			await playlistsStore.createSmartPlaylist(name, rules, folderId ?? undefined, 'discovery')
		}
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

	// A level only lists its own descendants, so a deleted row is never one of the pushed levels above
	// it — the trail needs no truncation here.
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
		await playlistsStore.delete(playlist.id)
	}
</script>

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

<!-- Smart playlist rule editor (create, or edit when a row's smart playlist is targeted) -->
<SmartPlaylistEditor
	open={smartEditorOpen}
	context="discovery"
	playlist={smartEditTarget}
	onSubmit={handleSmartSubmit}
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
			<span class="min-w-0 flex-1 truncate text-sm font-medium text-text-primary">{longPressTarget.name}</span>
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

	{#if longPressTarget?.is_smart}
		<ContextMenuItem onclick={() => longPressTarget && openSmartEdit(longPressTarget)}>
			{$translate('smartPlaylist.editTitle')}
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
	{/if}

	<ContextMenuItem separatorBefore destructive onclick={() => longPressTarget && handleDelete(longPressTarget)}>
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
