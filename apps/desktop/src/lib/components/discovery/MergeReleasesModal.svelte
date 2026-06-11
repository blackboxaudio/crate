<script lang="ts">
	import type { DiscoveryRelease } from '$shared/types'
	import { Modal, Button, Text } from '$lib/components/common'
	import { AlbumArt } from '$lib/components/common'
	import { translate } from '$shared/i18n'
	import { SvelteSet } from 'svelte/reactivity'

	type Props = {
		open: boolean
		releases: DiscoveryRelease[]
		onClose: () => void
		onMerge: (targetId: string, sourceIds: string[]) => Promise<void>
	}

	let { open, releases, onClose, onMerge }: Props = $props()

	let selectedTargetId = $state('')

	// Initialize with first release selected
	$effect(() => {
		if (open && releases.length > 0 && !selectedTargetId) {
			selectedTargetId = releases[0].id
		}
	})

	const combinedTrackCount = $derived(() => {
		const targetRelease = releases.find((r) => r.id === selectedTargetId)
		if (!targetRelease) return 0

		const targetNames = new SvelteSet(targetRelease.tracks.map((t) => t.name.toLowerCase()))
		let count = targetRelease.tracks.length

		for (const release of releases) {
			if (release.id === selectedTargetId) continue
			for (const track of release.tracks) {
				if (!targetNames.has(track.name.toLowerCase())) {
					targetNames.add(track.name.toLowerCase())
					count++
				}
			}
		}
		return count
	})

	const combinedTagCount = $derived(() => {
		const tagIds = new SvelteSet<string>()
		for (const release of releases) {
			for (const tag of release.tags) {
				tagIds.add(tag.id)
			}
		}
		return tagIds.size
	})

	async function handleMerge() {
		if (!selectedTargetId) return
		const sourceIds = releases.filter((r) => r.id !== selectedTargetId).map((r) => r.id)
		await onMerge(selectedTargetId, sourceIds)
	}

	function handleClose() {
		selectedTargetId = ''
		onClose()
	}
</script>

<Modal {open} title={$translate('discovery.mergeReleases')} onClose={handleClose} onSubmit={handleMerge}>
	<div class="flex flex-col gap-4">
		<Text size="sm" color="secondary">{$translate('discovery.mergeConfirm')}</Text>

		<div class="flex flex-col gap-2">
			{#each releases as release (release.id)}
				<button
					type="button"
					class="flex items-center gap-3 rounded-md border p-3 text-left transition-colors hover:cursor-pointer {selectedTargetId ===
					release.id
						? 'border-brand-primary bg-brand-muted'
						: 'border-stroke hover:bg-surface-2/50'}"
					onclick={() => (selectedTargetId = release.id)}
				>
					<AlbumArt artworkPath={release.artwork_path} artworkUrl={release.artwork_url} size="sm" />
					<div class="flex-1 truncate">
						<Text weight="medium" truncate>{release.title || $translate('common.untitled')}</Text>
						<Text variant="caption" truncate>{release.artist || $translate('common.unknownArtist')}</Text>
					</div>
					{#if selectedTargetId === release.id}
						<div class="shrink-0 rounded-full bg-brand-primary px-2 py-0.5 text-xs text-white">Target</div>
					{/if}
				</button>
			{/each}
		</div>

		<div class="rounded-md bg-surface-2 p-3 text-sm text-text-secondary">
			<div>{$translate('discovery.mergeTrackCount', { values: { count: combinedTrackCount() } })}</div>
			{#if combinedTagCount() > 0}
				<div>{combinedTagCount()} tags</div>
			{/if}
		</div>
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={handleClose}>
			{$translate('common.cancel')}
		</Button>
		<Button variant="primary" disabled={!selectedTargetId} onclick={handleMerge}>
			{$translate('discovery.mergeReleases')}
		</Button>
	{/snippet}
</Modal>
