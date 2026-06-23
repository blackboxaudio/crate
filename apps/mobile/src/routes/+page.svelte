<script lang="ts">
	// Mobile app shell: the bottom-tab navigation shell (header + Discovery / Playlists / Settings tabs),
	// with the release-detail screen, the persistent mini-player, and the full-screen expanded player
	// layered on top. Layering (low → high): shell < detail (z-30) < mini-player (z-40) < expanded player
	// (z-50). The detail push and expanded player cover the tab bar; the mini-player floats above it.
	import MobileShell from '$lib/components/layout/MobileShell.svelte'
	import ReleaseDetail from '$lib/components/discovery/ReleaseDetail.svelte'
	import PlaylistDetailView from '$lib/components/playlists/PlaylistDetailView.svelte'
	import MiniPlayer from '$lib/components/player/MiniPlayer.svelte'
	import ExpandedPlayer from '$lib/components/player/ExpandedPlayer.svelte'
	import { detailReleaseId, detailPlaylistId } from '$lib/stores/mobileUI'
	import { sortedReleases } from '$shared/stores/discovery'
	import { playlistsStore } from '$shared/stores/playlists'

	const detailRelease = $derived($sortedReleases.find((r) => r.id === $detailReleaseId) ?? null)
	const detailPlaylist = $derived(
		$detailPlaylistId ? ($playlistsStore.playlists.find((p) => p.id === $detailPlaylistId) ?? null) : null
	)
</script>

<MobileShell />

{#if detailRelease}
	<ReleaseDetail release={detailRelease} />
{/if}

{#if detailPlaylist}
	<PlaylistDetailView playlist={detailPlaylist} />
{/if}

<MiniPlayer />
<ExpandedPlayer />
