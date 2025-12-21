<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import { translate } from '$lib/i18n'

	type Props = {
		open: boolean
		error: string
		filesCopied: number
		onKeepPartial: () => void
		onCleanUp: () => void
	}

	let { open, error, filesCopied, onKeepPartial, onCleanUp }: Props = $props()
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

		<p class="action-prompt">{$translate('export.whatToDo')}</p>
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onCleanUp}>
			{$translate('export.cleanUp')}
		</Button>
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

	.action-prompt {
		color: var(--text-tertiary);
		font-size: 13px;
		margin: 0;
	}
</style>
