<script lang="ts">
	// Mobile app shell: the bottom-tab navigation shell (header + Discovery / Playlists / Tags / Settings
	// tabs), with the playlist-, tag-, and follow-detail screens, the release-detail screen, the persistent mini-player,
	// and the full-screen expanded player layered on top. Layering (low → high): shell < playlist/tag detail
	// (z-30) < release detail (z-35) < mini-player (z-40) < expanded player (z-50). The release detail sits
	// above the playlist/tag detail so it can be pushed open from within either; the mini-player floats above
	// all of them.
	import MobileShell from '$lib/components/layout/MobileShell.svelte'
	import ReleaseDetail from '$lib/components/discovery/ReleaseDetail.svelte'
	import PlaylistDetailView from '$lib/components/playlists/PlaylistDetailView.svelte'
	import TagDetailView from '$lib/components/tags/TagDetailView.svelte'
	import FollowSheet from '$lib/components/following/FollowSheet.svelte'
	import FollowDetailView from '$lib/components/following/FollowDetailView.svelte'
	import MiniPlayer from '$lib/components/player/MiniPlayer.svelte'
	import ExpandedPlayer from '$lib/components/player/ExpandedPlayer.svelte'
	import {
		mobileUIStore,
		detailReleaseId,
		detailPlaylistId,
		detailTagId,
		detailFollowSourceId,
		followReleaseId,
		mobileDisplayedReleases,
		queueOrigin,
	} from '$lib/stores/mobileUI'
	import { sortedReleases, discoveryStore } from '$shared/stores/discovery'
	import { discoveryPlaylistReleases } from '$shared/stores/discoveryPlaylist'
	import { playlistsStore } from '$shared/stores/playlists'
	import { tagsStore } from '$shared/stores/tags'
	import { followedSources } from '$shared/stores/follow'
	import * as playbackQueue from '$shared/stores/playbackQueue'
	import { get } from 'svelte/store'

	// Resolve the open detail release from the feed first, then the open playlist's loaded set, then the full
	// discovery set. The feed's `sortedReleases` applies the search / liked / new filters, so a release tapped
	// inside a playlist or tag feed that those filters would hide isn't in it — without the fallbacks the detail
	// would resolve to null and tapping the row would appear to do nothing (smart playlists and tag feeds
	// especially surface releases that are off the filtered feed).
	const detailRelease = $derived(
		$sortedReleases.find((r) => r.id === $detailReleaseId) ??
			$discoveryPlaylistReleases.find((r) => r.id === $detailReleaseId) ??
			$discoveryStore.releases.find((r) => r.id === $detailReleaseId) ??
			null
	)
	const detailPlaylist = $derived(
		$detailPlaylistId ? ($playlistsStore.playlists.find((p) => p.id === $detailPlaylistId) ?? null) : null
	)
	// Resolve the open tag detail plus the color of its owning category (tags inherit category color).
	const detailTagInfo = $derived.by(() => {
		const id = $detailTagId
		if (!id) return null
		for (const category of $tagsStore.categories) {
			const tag = category.tags.find((t) => t.id === id)
			if (tag) return { tag, categoryColor: category.color }
		}
		return null
	})

	// The release whose inline follow sheet is open, resolved with the same fallbacks as the detail release
	// (a release followed from a playlist or tag feed may sit off the filtered discovery feed).
	const followRelease = $derived(
		$sortedReleases.find((r) => r.id === $followReleaseId) ??
			$discoveryPlaylistReleases.find((r) => r.id === $followReleaseId) ??
			$discoveryStore.releases.find((r) => r.id === $followReleaseId) ??
			null
	)

	// The followed source whose drill-in detail (its releases) is open.
	const detailFollowSource = $derived(
		$detailFollowSourceId ? ($followedSources.find((s) => s.id === $detailFollowSourceId) ?? null) : null
	)

	// Keep a discovery-feed-originated playback queue in sync with the feed's live filter: when the on-screen
	// list changes — a tag filter / search / sort applied or reset, or releases synced in — re-scope the active
	// queue to it so next / shuffle keep spanning exactly what's on screen. Gated on the recorded origin: a
	// tag / follow / playlist queue is a fixed snapshot of the list it began with, so those are left untouched.
	$effect(() => {
		const list = $mobileDisplayedReleases
		if (get(queueOrigin) === 'discovery' && playbackQueue.currentPick()) {
			playbackQueue.updateContext(list)
		}
	})
</script>

<MobileShell />

{#if detailRelease}
	<ReleaseDetail release={detailRelease} />
{/if}

{#if detailPlaylist}
	<PlaylistDetailView playlist={detailPlaylist} />
{/if}

{#if detailTagInfo}
	<TagDetailView tag={detailTagInfo.tag} categoryColor={detailTagInfo.categoryColor} />
{/if}

{#if detailFollowSource}
	<FollowDetailView source={detailFollowSource} />
{/if}

<!-- Inline follow sheet (follow a release's artist / label). Always mounted so it animates out on dismiss;
     opens when a release context menu fires its "Follow" action. -->
<FollowSheet release={followRelease} onClose={() => mobileUIStore.closeFollowSheet()} />

<MiniPlayer />
<ExpandedPlayer />
