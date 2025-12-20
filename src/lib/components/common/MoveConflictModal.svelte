<script lang="ts">
	import Modal from './Modal.svelte'
	import Button from './Button.svelte'
	import Icon from './Icon.svelte'
	import type { Playlist } from '$lib/types'

	type Props = {
		open: boolean
		movingItem: Playlist | null
		conflictingItem: Playlist | null
		pendingCount?: number
		onCancel: () => void
		onOverwrite: () => void
		onMerge: () => void
	}

	let { open, movingItem, conflictingItem, pendingCount = 0, onCancel, onOverwrite, onMerge }: Props = $props()

	// Can only overwrite/merge if both items are the same type
	const sameType = $derived(movingItem && conflictingItem && movingItem.is_folder === conflictingItem.is_folder)
	const canOverwrite = $derived(sameType)
	const canMerge = $derived(sameType)

	const itemTypeName = $derived(movingItem?.is_folder ? 'folder' : 'playlist')
	const conflictTypeName = $derived(conflictingItem?.is_folder ? 'folder' : 'playlist')
	const isFolder = $derived(movingItem?.is_folder && conflictingItem?.is_folder)
</script>

<Modal {open} title="Name Conflict" onClose={onCancel}>
	<div class="space-y-4">
		{#if pendingCount > 0}
			<div class="rounded-md bg-surface-2 px-3 py-2 text-xs text-text-secondary">
				Resolving conflict {pendingCount > 1 ? `(${pendingCount} remaining)` : ''}
			</div>
		{/if}

		<p class="text-sm text-text-secondary">
			A {conflictTypeName} named "<span class="font-medium text-text-primary">{conflictingItem?.name}</span>" already
			exists in this location.
		</p>

		<div class="space-y-3">
			<!-- Merge option -->
			<button
				type="button"
				disabled={!canMerge}
				onclick={onMerge}
				class="w-full rounded-md border p-3 text-left {canMerge
					? 'border-stroke bg-surface-2 transition-all hover:cursor-pointer hover:bg-stroke/40'
					: 'cursor-not-allowed border-stroke/50 bg-surface-2/50 opacity-60'}"
			>
				<div class="flex items-start gap-3">
					<div class="mt-0.5 flex-shrink-0">
						<Icon name={isFolder ? 'folder' : 'music-note'} class="h-4 w-4" />
					</div>
					<div class="flex-1">
						<p class="text-sm font-medium text-text-primary">Merge</p>
						<p class="mt-0.5 text-xs text-text-secondary">
							{#if canMerge}
								{#if isFolder}
									Move contents of this folder into the existing folder. You'll be prompted for any nested conflicts.
								{:else}
									Combine tracks from both playlists into one.
								{/if}
							{:else}
								Not available: both items must be the same type to merge.
							{/if}
						</p>
					</div>
				</div>
			</button>

			<!-- Replace option -->
			<button
				type="button"
				disabled={!canOverwrite}
				onclick={onOverwrite}
				class="w-full rounded-md border p-3 text-left {canOverwrite
					? 'border-stroke bg-surface-2 transition-all hover:cursor-pointer hover:bg-stroke/40'
					: 'cursor-not-allowed border-stroke/50 bg-surface-2/50 opacity-60'}"
			>
				<div class="flex items-start gap-3">
					<div class="mt-0.5 flex-shrink-0">
						<Icon name="refresh" class="h-4 w-4" />
					</div>
					<div class="flex-1">
						<p class="text-sm font-medium text-text-primary">Replace</p>
						<p class="mt-0.5 text-xs text-text-secondary">
							{#if canOverwrite}
								Delete the existing {conflictTypeName} and move this {itemTypeName} in its place.
							{:else}
								Not available: can only replace items of the same type.
							{/if}
						</p>
					</div>
				</div>
			</button>
		</div>

		{#if sameType}
			<div class="rounded-md border border-warning/20 bg-warning/10 p-3">
				<div class="flex gap-2">
					<Icon name="warning" class="h-5 w-5 flex-shrink-0 text-warning" />
					<p class="text-sm text-warning">
						{#if isFolder}
							Replacing will permanently delete the existing folder and all its contents.
						{:else}
							Replacing will permanently delete the existing playlist and its track associations.
						{/if}
					</p>
				</div>
			</div>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onCancel}>Cancel</Button>
		{#if canMerge}
			<Button variant="primary" onclick={onMerge}>Merge</Button>
		{/if}
		{#if canOverwrite}
			<Button variant="danger" onclick={onOverwrite}>Replace</Button>
		{/if}
	{/snippet}
</Modal>
