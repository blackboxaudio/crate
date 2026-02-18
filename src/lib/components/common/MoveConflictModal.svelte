<script lang="ts">
	import Modal from './Modal.svelte'
	import Button from './Button.svelte'
	import Icon from './Icon.svelte'
	import Text from './Text.svelte'
	import { translate } from '$lib/i18n'
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

	const itemTypeName = $derived(movingItem?.is_folder ? $translate('common.folder') : $translate('common.playlist'))
	const conflictTypeName = $derived(
		conflictingItem?.is_folder ? $translate('common.folder') : $translate('common.playlist')
	)
	const isFolder = $derived(movingItem?.is_folder && conflictingItem?.is_folder)
</script>

<Modal {open} title={$translate('modals.conflict.title')} onClose={onCancel}>
	<div class="space-y-4">
		{#if pendingCount > 0}
			<div class="rounded-md bg-surface-2 px-3 py-2 text-xs text-text-secondary">
				{$translate('modals.conflict.resolving')}
				{pendingCount > 1 ? $translate('modals.conflict.remaining', { values: { count: pendingCount } }) : ''}
			</div>
		{/if}

		<Text color="secondary">
			{$translate('modals.conflict.message', { values: { type: conflictTypeName, name: conflictingItem?.name } })}
		</Text>

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
						<Text variant="body-2">{$translate('modals.conflict.merge')}</Text>
						<Text variant="caption" color="secondary" class="mt-0.5">
							{#if canMerge}
								{#if isFolder}
									{$translate('modals.conflict.mergeFolder')}
								{:else}
									{$translate('modals.conflict.mergePlaylist')}
								{/if}
							{:else}
								{$translate('modals.conflict.mergeNotAvailable')}
							{/if}
						</Text>
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
						<Text variant="body-2">{$translate('modals.conflict.replace')}</Text>
						<Text variant="caption" color="secondary" class="mt-0.5">
							{#if canOverwrite}
								{$translate('modals.conflict.replaceDescription', {
									values: { type: conflictTypeName, item: itemTypeName },
								})}
							{:else}
								{$translate('modals.conflict.replaceNotAvailable')}
							{/if}
						</Text>
					</div>
				</div>
			</button>
		</div>

		{#if sameType}
			<div class="rounded-md border border-warning/20 bg-warning/10 p-3">
				<div class="flex gap-2">
					<Icon name="warning" class="h-5 w-5 flex-shrink-0 text-warning" />
					<Text color="warning">
						{#if isFolder}
							{$translate('modals.conflict.replaceFolderWarning')}
						{:else}
							{$translate('modals.conflict.replacePlaylistWarning')}
						{/if}
					</Text>
				</div>
			</div>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onCancel}>{$translate('common.cancel')}</Button>
		{#if canMerge}
			<Button variant="primary" onclick={onMerge}>{$translate('modals.conflict.merge')}</Button>
		{/if}
		{#if canOverwrite}
			<Button variant="danger" onclick={onOverwrite}>{$translate('modals.conflict.replace')}</Button>
		{/if}
	{/snippet}
</Modal>
