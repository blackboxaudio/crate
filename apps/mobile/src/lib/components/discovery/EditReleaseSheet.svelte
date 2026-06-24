<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease } from '$shared/types'
	import { discoveryStore } from '$shared/stores/discovery'
	import MobileModal from '$lib/components/common/MobileModal.svelte'

	type Props = {
		open: boolean
		release: DiscoveryRelease
		onClose: () => void
	}
	let { open, release, onClose }: Props = $props()

	let artist = $state('')
	let title = $state('')
	let label = $state('')
	let releaseDate = $state('')
	let notes = $state('')

	$effect(() => {
		if (open) {
			artist = release.artist ?? ''
			title = release.title ?? ''
			label = release.label ?? ''
			releaseDate = release.release_date ?? ''
			notes = release.notes ?? ''
		}
	})

	async function handleDone() {
		const update: Record<string, string | undefined> = {}
		if (artist !== (release.artist ?? '')) update.artist = artist
		if (title !== (release.title ?? '')) update.title = title
		if (label !== (release.label ?? '')) update.label = label
		if (releaseDate !== (release.release_date ?? '')) update.release_date = releaseDate
		if (notes !== (release.notes ?? '')) update.notes = notes

		if (Object.keys(update).length > 0) {
			await discoveryStore.updateRelease(release.id, update)
		}
		onClose()
	}
</script>

<MobileModal {open} onClose={handleDone} onSubmit={handleDone} title={$translate('discovery.editRelease')}>
	<div class="flex flex-col gap-4 py-1">
		<div>
			<label for="edit-artist" class="mb-1.5 block text-xs font-medium text-text-secondary">
				{$translate('discovery.editor.artist')}
			</label>
			<input
				id="edit-artist"
				type="text"
				bind:value={artist}
				placeholder={$translate('discovery.editor.artist')}
				class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
			/>
		</div>

		<div>
			<label for="edit-title" class="mb-1.5 block text-xs font-medium text-text-secondary">
				{$translate('discovery.editor.title')}
			</label>
			<input
				id="edit-title"
				type="text"
				bind:value={title}
				placeholder={$translate('discovery.editor.title')}
				class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
			/>
		</div>

		<div>
			<label for="edit-label" class="mb-1.5 block text-xs font-medium text-text-secondary">
				{$translate('discovery.editor.label')}
			</label>
			<input
				id="edit-label"
				type="text"
				bind:value={label}
				placeholder={$translate('discovery.editor.label')}
				class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
			/>
		</div>

		<div>
			<label for="edit-release-date" class="mb-1.5 block text-xs font-medium text-text-secondary">
				{$translate('discovery.editor.releaseDate')}
			</label>
			<input
				id="edit-release-date"
				type="text"
				bind:value={releaseDate}
				placeholder="YYYY-MM-DD"
				class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
			/>
		</div>

		<div>
			<label for="edit-notes" class="mb-1.5 block text-xs font-medium text-text-secondary">
				{$translate('discovery.editor.notes')}
			</label>
			<textarea
				id="edit-notes"
				bind:value={notes}
				rows="3"
				placeholder={$translate('discovery.editor.notesPlaceholder')}
				class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
			></textarea>
		</div>
	</div>

	{#snippet footer()}
		<div class="flex w-full justify-end gap-2">
			<button
				type="button"
				class="rounded-lg px-4 py-2 text-sm font-medium text-text-secondary active:bg-surface-2"
				onclick={onClose}
			>
				{$translate('common.cancel')}
			</button>
			<button
				type="button"
				class="rounded-lg bg-brand-primary px-4 py-2 text-sm font-semibold text-white active:opacity-90"
				onclick={handleDone}
			>
				{$translate('common.done')}
			</button>
		</div>
	{/snippet}
</MobileModal>
