<script lang="ts">
	// Mobile app shell: the bottom-tab navigation shell (header + Discovery / Playlists / Settings tabs),
	// with the release-detail screen, the persistent mini-player, and the full-screen expanded player
	// layered on top. Layering (low → high): shell < detail (z-30) < mini-player (z-40) < expanded player
	// (z-50). The detail push and expanded player cover the tab bar; the mini-player floats above it.
	import MobileShell from '$lib/components/layout/MobileShell.svelte'
	import ReleaseDetail from '$lib/components/discovery/ReleaseDetail.svelte'
	import MiniPlayer from '$lib/components/player/MiniPlayer.svelte'
	import ExpandedPlayer from '$lib/components/player/ExpandedPlayer.svelte'
	import { detailReleaseId } from '$lib/stores/mobileUI'
	import { sortedReleases } from '$shared/stores/discovery'

	// Resolve the open detail release from the store so notes/tag edits reflect live; null closes it.
	const detailRelease = $derived($sortedReleases.find((r) => r.id === $detailReleaseId) ?? null)
</script>

<MobileShell />

{#if detailRelease}
	<ReleaseDetail release={detailRelease} />
{/if}

<!-- Persistent preview bar; renders only while a preview is active and persists across navigation. -->
<MiniPlayer />

<!-- Stays mounted while a preview exists; opens/collapses internally on $isPlayerExpanded. -->
<ExpandedPlayer />
