<script lang="ts">
	// Mobile app shell: the Discovery feed in the navigation shell (header + left/right drawers), with
	// the release-detail screen, the persistent mini-player, and the full-screen expanded player layered
	// on top. Layering (low → high): shell < detail (z-30) < mini-player (z-40) < expanded player (z-50).
	import MobileShell from '$lib/components/layout/MobileShell.svelte'
	import MobileDiscoveryView from '$lib/components/discovery/MobileDiscoveryView.svelte'
	import ReleaseDetail from '$lib/components/discovery/ReleaseDetail.svelte'
	import MiniPlayer from '$lib/components/player/MiniPlayer.svelte'
	import ExpandedPlayer from '$lib/components/player/ExpandedPlayer.svelte'
	import { detailReleaseId, isPlayerExpanded } from '$lib/stores/mobileUI'
	import { sortedReleases } from '$shared/stores/discovery'
	import { previewInfo } from '$shared/stores/player'

	// Resolve the open detail release from the store so notes/tag edits reflect live; null closes it.
	const detailRelease = $derived($sortedReleases.find((r) => r.id === $detailReleaseId) ?? null)
</script>

<MobileShell>
	<MobileDiscoveryView />
</MobileShell>

{#if detailRelease}
	<ReleaseDetail release={detailRelease} />
{/if}

<!-- Persistent preview bar; renders only while a preview is active and persists across navigation. -->
<MiniPlayer />

{#if $isPlayerExpanded && $previewInfo}
	<ExpandedPlayer />
{/if}
