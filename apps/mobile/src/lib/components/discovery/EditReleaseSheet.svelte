<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease } from '$shared/types'
	import { discoveryStore } from '$shared/stores/discovery'
	import FormSheet from '$lib/components/common/FormSheet.svelte'
	import FormSection from '$lib/components/common/FormSection.svelte'
	import FormTextField from '$lib/components/common/FormTextField.svelte'

	// Metadata editor for a discovery release: grouped field rows on the content-hugging FormSheet detent.
	// Save commits, every dismiss path (Cancel, scrim, swipe, Back) cancels — guarded by the discard
	// confirm while edits exist.
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
	height="auto"
	title={$translate('discovery.editRelease')}
>
	<div class="flex flex-col gap-5 px-4 py-4">
		<FormSection>
			<FormTextField
				id="edit-artist"
				label={$translate('discovery.editor.artist')}
				bind:value={artist}
				placeholder={$translate('discovery.editor.artist')}
				autocapitalize="words"
			/>
			<FormTextField
				id="edit-title"
				label={$translate('discovery.editor.title')}
				bind:value={title}
				placeholder={$translate('discovery.editor.title')}
				autocapitalize="words"
			/>
			<FormTextField
				id="edit-label"
				label={$translate('discovery.editor.label')}
				bind:value={label}
				placeholder={$translate('discovery.editor.label')}
				autocapitalize="words"
			/>
			<FormTextField
				id="edit-release-date"
				label={$translate('discovery.editor.releaseDate')}
				bind:value={releaseDate}
				placeholder="YYYY-MM-DD"
				autocapitalize="off"
				autocorrect="off"
			/>
		</FormSection>

		<FormSection>
			<FormTextField
				id="edit-notes"
				label={$translate('discovery.editor.notes')}
				bind:value={notes}
				placeholder={$translate('discovery.editor.notesPlaceholder')}
				multiline
				rows={3}
			/>
		</FormSection>
	</div>
</FormSheet>
