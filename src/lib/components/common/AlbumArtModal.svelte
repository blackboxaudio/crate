<script lang="ts">
	import Modal from './Modal.svelte'
	import AlbumArt from './AlbumArt.svelte'
	import Button from './Button.svelte'
	import { translate } from '$lib/i18n'

	type Props = {
		open: boolean
		artworkPath: string | null
		trackTitle?: string
		onClose: () => void
	}

	let { open, artworkPath, trackTitle, onClose }: Props = $props()
</script>

<Modal {open} title={trackTitle || $translate('modals.albumArt')} {onClose}>
	<div class="flex justify-center">
		<AlbumArt {artworkPath} size="lg" />
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>{$translate('common.close')}</Button>
	{/snippet}
</Modal>
