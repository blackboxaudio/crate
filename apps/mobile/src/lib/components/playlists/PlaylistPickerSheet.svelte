<script lang="ts">
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import { playlistsStore } from '$shared/stores/playlists'
	import { toastStore } from '$shared/stores/toast'
	import MobileModal from '$lib/components/common/MobileModal.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'

	type Props = {
		open: boolean
		releaseIds: string[]
		onClose: () => void
	}
	let { open, releaseIds, onClose }: Props = $props()

	let creating = $state(false)
	let newName = $state('')

	const discoveryPlaylists = $derived(
		$playlistsStore.playlists.filter((p) => p.context === 'discovery' && !p.is_folder)
	)

	function reset() {
		creating = false
		newName = ''
	}

	async function addTo(playlistId: string) {
		await playlistsStore.addReleases(playlistId, releaseIds)
		const count = releaseIds.length
		const t = get(translate)
		toastStore.success(t('contextMenu.addToPlaylist'))
		onClose()
	}

	async function createAndAdd() {
		const trimmed = newName.trim()
		if (!trimmed) return
		const playlist = await playlistsStore.createPlaylist(trimmed, undefined, 'discovery')
		if (!playlist) return
		await playlistsStore.addReleases(playlist.id, releaseIds)
		const t = get(translate)
		toastStore.success(t('contextMenu.addToPlaylist'))
		reset()
		onClose()
	}

	function handleClose() {
		reset()
		onClose()
	}
</script>

<MobileModal {open} onClose={handleClose} title={$translate('contextMenu.addToPlaylist')}>
	<div class="flex flex-col">
		{#if creating}
			<div class="flex items-center gap-2 px-1 py-2">
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
			<MobileListItem onclick={() => (creating = true)}>
				{#snippet leading()}
					<svg
						class="h-5 w-5 text-brand-primary"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
					>
						<path d="M12 5v14M5 12h14" stroke-linecap="round" />
					</svg>
				{/snippet}
				<span class="text-sm font-medium text-brand-primary">{$translate('modals.createPlaylist.title')}</span>
			</MobileListItem>
		{/if}

		{#each discoveryPlaylists as playlist (playlist.id)}
			<MobileListItem onclick={() => addTo(playlist.id)}>
				{#snippet leading()}
					<svg
						class="h-5 w-5 text-text-secondary"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
					>
						<path d="M9 18V5l12-2v13" stroke-linecap="round" stroke-linejoin="round" />
						<circle cx="6" cy="18" r="3" />
						<circle cx="18" cy="16" r="3" />
					</svg>
				{/snippet}
				{#snippet trailing()}
					<span class="text-xs tabular-nums">{playlist.track_count}</span>
				{/snippet}
				<span class="truncate text-sm">{playlist.name}</span>
			</MobileListItem>
		{/each}

		{#if discoveryPlaylists.length === 0 && !creating}
			<p class="px-4 py-6 text-center text-sm text-text-secondary">
				{$translate('playlists.noPlaylists')}
			</p>
		{/if}
	</div>
</MobileModal>
