<script lang="ts">
	import Modal from './Modal.svelte'
	import Button from './Button.svelte'
	import Icon from './Icon.svelte'
	import type { DuplicateTrack, DuplicateResolutionAction } from '$lib/types'

	type Props = {
		open: boolean
		duplicate: DuplicateTrack | null
		currentIndex: number
		totalCount: number
		onResolve: (action: DuplicateResolutionAction, applyToAll: boolean) => void
		onCancel: () => void
	}

	let { open, duplicate, currentIndex, totalCount, onResolve, onCancel }: Props = $props()

	let applyToAllChecked = $state(false)

	// Reset checkbox when modal opens with new duplicate
	$effect(() => {
		if (open && duplicate) {
			applyToAllChecked = false
		}
	})

	// Format file path for display (show last 2-3 components)
	function formatPath(path: string): string {
		const parts = path.split(/[/\\]/)
		return parts.slice(-3).join('/')
	}

	function handleAction(action: DuplicateResolutionAction) {
		onResolve(action, applyToAllChecked)
	}
</script>

<Modal {open} title="Duplicate Track Detected" onClose={onCancel}>
	<div class="space-y-4">
		{#if totalCount > 1}
			<div class="rounded-md bg-surface-2 px-3 py-2 text-xs text-text-secondary">
				Duplicate {currentIndex + 1} of {totalCount}
			</div>
		{/if}

		<div class="space-y-2">
			<p class="text-sm text-text-secondary">This file matches an existing track in your library:</p>

			{#if duplicate}
				<div class="space-y-1 rounded-md bg-surface-2 p-3">
					<p class="text-sm font-medium text-text-primary">
						{duplicate.existing_track.title || 'Untitled'}
					</p>
					<p class="text-xs text-text-secondary">
						{duplicate.existing_track.artist || 'Unknown Artist'}
					</p>
					<p class="truncate font-mono text-xs text-text-tertiary">
						Current: {formatPath(duplicate.existing_track.file_path)}
					</p>
					<p class="truncate font-mono text-xs text-text-tertiary">
						New: {formatPath(duplicate.new_file_path)}
					</p>
				</div>
			{/if}
		</div>

		<div class="space-y-3">
			<!-- Skip option -->
			<button
				type="button"
				onclick={() => handleAction('skip')}
				class="w-full rounded-md border border-stroke bg-surface-2 p-3 text-left transition-all hover:cursor-pointer hover:bg-stroke/40"
			>
				<div class="flex items-start gap-3">
					<div class="mt-0.5 flex-shrink-0">
						<Icon name="x" class="h-4 w-4" />
					</div>
					<div class="flex-1">
						<p class="text-sm font-medium text-text-primary">Skip</p>
						<p class="mt-0.5 text-xs text-text-secondary">Don't import this file. Keep the existing track unchanged.</p>
					</div>
				</div>
			</button>

			<!-- Update path option -->
			<button
				type="button"
				onclick={() => handleAction('update_path')}
				class="w-full rounded-md border border-stroke bg-surface-2 p-3 text-left transition-all hover:cursor-pointer hover:bg-stroke/40"
			>
				<div class="flex items-start gap-3">
					<div class="mt-0.5 flex-shrink-0">
						<Icon name="folder-arrow" class="h-4 w-4" />
					</div>
					<div class="flex-1">
						<p class="text-sm font-medium text-text-primary">Update path</p>
						<p class="mt-0.5 text-xs text-text-secondary">
							Keep all existing data (cue points, tags, playlists, play history) but update the file location.
						</p>
					</div>
				</div>
			</button>

			<!-- Replace option -->
			<button
				type="button"
				onclick={() => handleAction('replace')}
				class="w-full rounded-md border border-stroke bg-surface-2 p-3 text-left transition-all hover:cursor-pointer hover:bg-stroke/40"
			>
				<div class="flex items-start gap-3">
					<div class="mt-0.5 flex-shrink-0">
						<Icon name="refresh" class="h-4 w-4" />
					</div>
					<div class="flex-1">
						<p class="text-sm font-medium text-text-primary">Replace</p>
						<p class="mt-0.5 text-xs text-text-secondary">
							Fresh import from new file. Keeps playlist memberships only. Resets cue points, tags, and play history.
						</p>
					</div>
				</div>
			</button>
		</div>

		{#if totalCount > 1 && currentIndex < totalCount - 1}
			<label class="flex items-center gap-2 pt-2">
				<input
					type="checkbox"
					bind:checked={applyToAllChecked}
					class="h-4 w-4 rounded border-stroke bg-surface-2 text-brand-primary focus:ring-0"
				/>
				<span class="text-sm text-text-secondary">
					Apply to all remaining duplicates ({totalCount - currentIndex - 1})
				</span>
			</label>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onCancel}>Cancel</Button>
	{/snippet}
</Modal>
