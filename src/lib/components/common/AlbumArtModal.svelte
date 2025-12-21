<script lang="ts">
	import Modal from './Modal.svelte'
	import AlbumArt from './AlbumArt.svelte'
	import Button from './Button.svelte'

	type Props = {
		open: boolean
		artworkPath: string | null
		trackTitle?: string
		onClose: () => void
	}

	let { open, artworkPath, trackTitle, onClose }: Props = $props()
</script>

<Modal {open} title={trackTitle || 'Album Art'} {onClose}>
	<div class="flex justify-center">
		<AlbumArt {artworkPath} size="lg" />
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>Close</Button>
	{/snippet}
</Modal>
