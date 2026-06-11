<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import { translate } from '$shared/i18n'
	import { getPendingCheckpoint } from '$shared/api/export'
	import type { ExportCheckpoint } from '$shared/types'

	type Props = {
		open: boolean
		error: string
		deviceId: string
		filesCopied: number
		onKeepPartial: () => void
		onCleanUp: () => void
		onResume?: () => void
	}

	let { open, error, deviceId, filesCopied, onKeepPartial, onCleanUp, onResume }: Props = $props()

	let checkpoint = $state<ExportCheckpoint | null>(null)
	let checkingCheckpoint = $state(false)

	// Auto-detect checkpoint when modal opens
	$effect(() => {
		if (open && deviceId) {
			checkForCheckpoint()
		} else {
			checkpoint = null
		}
	})

	async function checkForCheckpoint() {
		checkingCheckpoint = true
		try {
			checkpoint = await getPendingCheckpoint(deviceId)
		} catch (e) {
			console.error('Failed to check for checkpoint:', e)
			checkpoint = null
		} finally {
			checkingCheckpoint = false
		}
	}

	function handleResume() {
		if (onResume) {
			onResume()
		}
	}

	const filesRemaining = $derived(
		checkpoint ? (checkpoint.playlist_ids.length > 0 ? checkpoint.tracks_completed.length : 0) : 0
	)
</script>

<Modal {open} title={$translate('export.failed')} onClose={onKeepPartial} size="sm">
	<div class="failure-content">
		<div class="error-icon">⚠️</div>

		<Text>{error}</Text>

		{#if filesCopied > 0}
			<Text color="secondary" class="rounded-md bg-surface-2 px-3 py-2">
				{$translate('export.filesCopiedBeforeFailure', { values: { count: filesCopied } })}
			</Text>
		{/if}

		{#if checkpoint && !checkingCheckpoint}
			<div class="checkpoint-info">
				<Text variant="body-2" class="mb-1">{$translate('export.checkpointAvailable')}</Text>
				<Text variant="caption" color="secondary">
					{$translate('export.tracksCompleted', { values: { count: checkpoint.tracks_completed.length } })}
				</Text>
			</div>
		{/if}

		<Text color="tertiary">{$translate('export.whatToDo')}</Text>
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onCleanUp}>
			{$translate('export.cleanUp')}
		</Button>
		{#if checkpoint && onResume}
			<Button variant="secondary" onclick={handleResume}>
				{$translate('export.resume')}
			</Button>
		{/if}
		<Button variant="primary" onclick={onKeepPartial}>
			{$translate('export.keepPartial')}
		</Button>
	{/snippet}
</Modal>

<style>
	.failure-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
		text-align: center;
		padding: 8px 0;
	}

	.error-icon {
		font-size: 48px;
	}

	.checkpoint-info {
		padding: 12px 16px;
		background: var(--bg-tertiary);
		border-radius: 8px;
		border: 1px solid var(--border-primary);
	}
</style>
