<script lang="ts">
	import Modal from './Modal.svelte'
	import Button from './Button.svelte'
	import Icon from './Icon.svelte'
	import Text from './Text.svelte'
	import { translate } from '$shared/i18n'
	import type { DuplicateTrack, DuplicateResolutionAction } from '$shared/types'

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

<Modal {open} title={$translate('modals.duplicate.title')} onClose={onCancel}>
	<div class="space-y-4">
		{#if totalCount > 1}
			<div class="rounded-md bg-surface-2 px-3 py-2 text-xs text-text-secondary">
				{$translate('modals.duplicate.progress', { values: { current: currentIndex + 1, total: totalCount } })}
			</div>
		{/if}

		<div class="space-y-2">
			<Text color="secondary">{$translate('modals.duplicate.message')}</Text>

			{#if duplicate}
				<div class="space-y-1 rounded-md bg-surface-2 p-3">
					<Text variant="body-2">
						{duplicate.existing_track.title || $translate('common.untitled')}
					</Text>
					<Text variant="caption" color="secondary">
						{duplicate.existing_track.artist || $translate('common.unknownArtist')}
					</Text>
					<Text variant="caption" mono truncate>
						{$translate('modals.duplicate.current')}
						{formatPath(duplicate.existing_track.file_path)}
					</Text>
					<Text variant="caption" mono truncate>
						{$translate('modals.duplicate.new')}
						{formatPath(duplicate.new_file_path)}
					</Text>
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
						<Text variant="body-2">{$translate('modals.duplicate.skip')}</Text>
						<Text variant="caption" color="secondary" class="mt-0.5"
							>{$translate('modals.duplicate.skipDescription')}</Text
						>
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
						<Text variant="body-2">{$translate('modals.duplicate.updatePath')}</Text>
						<Text variant="caption" color="secondary" class="mt-0.5">
							{$translate('modals.duplicate.updatePathDescription')}
						</Text>
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
						<Text variant="body-2">{$translate('modals.duplicate.replace')}</Text>
						<Text variant="caption" color="secondary" class="mt-0.5">
							{$translate('modals.duplicate.replaceDescription')}
						</Text>
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
				<Text as="span" color="secondary">
					{$translate('modals.duplicate.applyToAll', { values: { count: totalCount - currentIndex - 1 } })}
				</Text>
			</label>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onCancel}>{$translate('common.cancel')}</Button>
	{/snippet}
</Modal>
