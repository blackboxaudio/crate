<script lang="ts">
	import type { DiscoveryReleaseCreate, DiscoverySourceType } from '$lib/types'
	import { Modal, Input, Select, Button } from '$lib/components/common'
	import { translate } from '$lib/i18n'

	type Props = {
		open: boolean
		onClose: () => void
		onSubmit: (create: DiscoveryReleaseCreate) => Promise<void>
	}

	let { open, onClose, onSubmit }: Props = $props()

	let url = $state('')
	let sourceType = $state<DiscoverySourceType>('other')
	let artist = $state('')
	let title = $state('')
	let label = $state('')

	const sourceOptions = [
		{ value: 'bandcamp', label: 'Bandcamp' },
		{ value: 'soundcloud', label: 'SoundCloud' },
		{ value: 'youtube', label: 'YouTube' },
		{ value: 'discogs', label: 'Discogs' },
		{ value: 'other', label: 'Other' },
	]

	function detectSource(input: string) {
		const lower = input.toLowerCase()
		if (lower.includes('bandcamp.com')) sourceType = 'bandcamp'
		else if (lower.includes('soundcloud.com')) sourceType = 'soundcloud'
		else if (lower.includes('youtube.com') || lower.includes('youtu.be')) sourceType = 'youtube'
		else if (lower.includes('discogs.com')) sourceType = 'discogs'
	}

	function handleUrlInput() {
		detectSource(url)
	}

	function resetForm() {
		url = ''
		sourceType = 'other'
		artist = ''
		title = ''
		label = ''
	}

	function handleClose() {
		resetForm()
		onClose()
	}

	async function handleSubmit() {
		if (!url.trim()) return

		const create: DiscoveryReleaseCreate = {
			url: url.trim(),
			source_type: sourceType,
		}

		if (artist.trim()) create.artist = artist.trim()
		if (title.trim()) create.title = title.trim()
		if (label.trim()) create.label = label.trim()

		await onSubmit(create)
	}

	// Reset form when modal opens
	$effect(() => {
		if (open) resetForm()
	})
</script>

<Modal {open} title={$translate('discovery.addRelease')} onClose={handleClose}>
	<div class="flex flex-col gap-4">
		<div>
			<label for="release-url" class="mb-1.5 block text-sm font-medium text-text-secondary">
				{$translate('discovery.url')}
			</label>
			<Input
				bind:value={url}
				placeholder="https://..."
				oninput={handleUrlInput}
				onkeydown={(e) => e.key === 'Enter' && handleSubmit()}
			/>
		</div>

		<div>
			<label for="release-source" class="mb-1.5 block text-sm font-medium text-text-secondary">
				{$translate('discovery.source')}
			</label>
			<Select bind:value={sourceType} options={sourceOptions} />
		</div>

		<div>
			<label for="release-artist" class="mb-1.5 block text-sm font-medium text-text-secondary">
				{$translate('library.columns.artist')}
			</label>
			<Input bind:value={artist} placeholder={$translate('library.columns.artist')} />
		</div>

		<div>
			<label for="release-title" class="mb-1.5 block text-sm font-medium text-text-secondary">
				{$translate('library.columns.title')}
			</label>
			<Input bind:value={title} placeholder={$translate('library.columns.title')} />
		</div>

		<div>
			<label for="release-label" class="mb-1.5 block text-sm font-medium text-text-secondary">
				{$translate('editor.label')}
			</label>
			<Input bind:value={label} placeholder={$translate('editor.label')} />
		</div>
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={handleClose}>
			{$translate('common.cancel')}
		</Button>
		<Button variant="primary" disabled={!url.trim()} onclick={handleSubmit}>
			{$translate('discovery.addRelease')}
		</Button>
	{/snippet}
</Modal>
