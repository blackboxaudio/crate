<script lang="ts">
	import { updaterStore } from '$lib/stores/updater'
	import { translate } from '$lib/i18n'
	import Modal from './Modal.svelte'
	import Button from './Button.svelte'
	import Text from './Text.svelte'
	import Spinner from './Spinner.svelte'

	type Props = {
		open: boolean
		onClose: () => void
	}

	let { open, onClose }: Props = $props()

	const busy = $derived($updaterStore.status === 'downloading' || $updaterStore.status === 'installing')
</script>

<Modal {open} title={$translate('modals.update.title')} onClose={busy ? () => {} : onClose}>
	<div class="space-y-4">
		<Text size="sm" color="secondary">
			{$translate('modals.update.description', { values: { version: $updaterStore.version ?? '' } })}
		</Text>

		{#if $updaterStore.body}
			<div>
				<Text size="sm" weight="medium" class="mb-2">{$translate('modals.update.releaseNotes')}</Text>
				<pre
					class="max-h-48 overflow-y-auto rounded-md bg-surface-2 p-3 text-xs text-text-secondary">{$updaterStore.body}</pre>
			</div>
		{/if}

		{#if $updaterStore.status === 'downloading'}
			<div>
				<Text size="xs" color="secondary" class="mb-1">
					{$translate('modals.update.downloading', { values: { progress: $updaterStore.progress } })}
				</Text>
				<div class="h-1.5 w-full overflow-hidden rounded-full bg-surface-2">
					<div
						class="h-full rounded-full bg-brand-primary transition-[width] duration-200"
						style="width: {$updaterStore.progress}%"
					></div>
				</div>
			</div>
		{/if}

		{#if $updaterStore.status === 'installing'}
			<div class="flex items-center gap-2">
				<Spinner class="h-3.5 w-3.5" />
				<Text size="xs" color="secondary">{$translate('modals.update.installing')}</Text>
			</div>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onClose} disabled={busy}>
			{$translate('modals.update.later')}
		</Button>
		<Button variant="primary" onclick={() => updaterStore.install()} disabled={busy}>
			{#if busy}
				<Spinner class="mr-2 h-3.5 w-3.5" />
			{/if}
			{$translate('modals.update.installAndRestart')}
		</Button>
	{/snippet}
</Modal>
