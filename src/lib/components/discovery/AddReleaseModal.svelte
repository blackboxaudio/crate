<script lang="ts">
	import type { DiscoveryReleaseCreate, DiscoverySourceType } from '$lib/types'
	import { Modal, Input, Select, Button, Text } from '$lib/components/common'
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
			<Text as="label" for="release-url" size="sm" weight="medium" color="secondary" class="mb-1.5 block">
				{$translate('discovery.url')}
			</Text>
			<Input
				bind:value={url}
				placeholder="https://..."
				autofocus
				oninput={handleUrlInput}
				onkeydown={(e) => e.key === 'Enter' && handleSubmit()}
			/>
		</div>

		<div>
			<Text as="label" for="release-source" size="sm" weight="medium" color="secondary" class="mb-1.5 block">
				{$translate('discovery.source')}
			</Text>
			<Select bind:value={sourceType} options={sourceOptions} />
		</div>

		<div>
			<Text as="label" for="release-artist" size="sm" weight="medium" color="secondary" class="mb-1.5 block">
				{$translate('library.columns.artist')}
			</Text>
			<Input bind:value={artist} placeholder={$translate('library.columns.artist')} />
		</div>

		<div>
			<Text as="label" for="release-title" size="sm" weight="medium" color="secondary" class="mb-1.5 block">
				{$translate('library.columns.title')}
			</Text>
			<Input bind:value={title} placeholder={$translate('library.columns.title')} />
		</div>

		<div>
			<Text as="label" for="release-label" size="sm" weight="medium" color="secondary" class="mb-1.5 block">
				{$translate('editor.label')}
			</Text>
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
