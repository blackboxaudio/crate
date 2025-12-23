<script lang="ts">
	import { onMount } from 'svelte'
	import Modal from '$lib/components/common/Modal.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import { translate } from '$lib/i18n'
	import { getPendingCheckpoint } from '$lib/api/export'
	import type { ExportCheckpoint } from '$lib/types'

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

	let checkpoint: ExportCheckpoint | null = $state(null)
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

		<p class="error-message">{error}</p>

		{#if filesCopied > 0}
			<p class="files-info">
				{$translate('export.filesCopiedBeforeFailure', { values: { count: filesCopied } })}
			</p>
		{/if}

		{#if checkpoint && !checkingCheckpoint}
			<div class="checkpoint-info">
				<p class="checkpoint-label">{$translate('export.checkpointAvailable')}</p>
				<p class="checkpoint-detail">
					{$translate('export.tracksCompleted', { values: { count: checkpoint.tracks_completed.length } })}
				</p>
			</div>
		{/if}

		<p class="action-prompt">{$translate('export.whatToDo')}</p>
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

	.error-message {
		color: var(--text-primary);
		font-size: 14px;
		margin: 0;
	}

	.files-info {
		color: var(--text-secondary);
		font-size: 13px;
		margin: 0;
		padding: 8px 12px;
		background: var(--bg-secondary);
		border-radius: 6px;
	}

	.checkpoint-info {
		padding: 12px 16px;
		background: var(--bg-tertiary);
		border-radius: 8px;
		border: 1px solid var(--border-primary);
	}

	.checkpoint-label {
		color: var(--text-primary);
		font-size: 13px;
		font-weight: 500;
		margin: 0 0 4px 0;
	}

	.checkpoint-detail {
		color: var(--text-secondary);
		font-size: 12px;
		margin: 0;
	}

	.action-prompt {
		color: var(--text-tertiary);
		font-size: 13px;
		margin: 0;
	}
</style>
