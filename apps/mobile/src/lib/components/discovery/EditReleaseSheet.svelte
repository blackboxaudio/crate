<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease } from '$shared/types'
	import { discoveryStore } from '$shared/stores/discovery'
	import FormSheet from '$lib/components/common/FormSheet.svelte'

	// Metadata editor for a discovery release, on the shared FormSheet: Save commits, every dismiss path
	// (Cancel, scrim, swipe, Back) cancels — guarded by the discard confirm while edits exist.
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

	const update = $derived.by(() => {
		const u: Record<string, string | undefined> = {}
		if (artist !== (release.artist ?? '')) u.artist = artist
		if (title !== (release.title ?? '')) u.title = title
		if (label !== (release.label ?? '')) u.label = label
		if (releaseDate !== (release.release_date ?? '')) u.release_date = releaseDate
		if (notes !== (release.notes ?? '')) u.notes = notes
		return u
	})
	const dirty = $derived(Object.keys(update).length > 0)

	async function handleSave() {
		if (dirty) {
			await discoveryStore.updateRelease(release.id, update)
		}
		onClose()
	}
</script>

<FormSheet
	{open}
	{onClose}
	onSubmit={handleSave}
	submitLabel={$translate('common.save')}
	submitDisabled={!dirty}
	{dirty}
	title={$translate('discovery.editRelease')}
>
	<div class="flex flex-col gap-4 px-4 py-4">
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
</FormSheet>
