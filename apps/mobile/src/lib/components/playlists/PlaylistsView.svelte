<script lang="ts">
	import { onMount } from 'svelte'
	import { playlistsStore } from '$shared/stores/playlists'
	import PlaylistLevel from './PlaylistLevel.svelte'

	// The Playlists tab root: the top level of the playlist tree under the shell chrome. Folder levels are
	// NOT rendered here — each entry of the persisted folder trail is a full-screen push (`PlaylistFolderView`,
	// mounted by +page like the playlist / tag / follow details), so every drill-in reads the same. The load
	// lives here rather than in the level so a push doesn't re-fetch the tree.
	onMount(() => {
		playlistsStore.load()
	})
</script>

<div class="flex h-full flex-col">
	<PlaylistLevel folderId={null} />
</div>
