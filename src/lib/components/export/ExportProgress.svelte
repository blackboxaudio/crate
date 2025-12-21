<script lang="ts">
	import Button from '$lib/components/common/Button.svelte'
	import { translate } from '$lib/i18n'
	import {
		isExporting,
		exportProgress,
		exportProgressPercent,
		exportStatusLabel,
		activeDeviceName,
	} from '$lib/stores/export'

	type Props = {
		onCancel: () => void
	}

	let { onCancel }: Props = $props()

	// Truncate filename to max length
	function truncateFilename(filename: string | null, maxLength = 30): string {
		if (!filename) return ''
		if (filename.length <= maxLength) return filename
		return '...' + filename.slice(-(maxLength - 3))
	}
</script>

{#if $isExporting && $exportProgress}
	<div class="export-progress">
		<div class="progress-header">
			<span class="progress-title"
				>{$translate('export.exportingTo', { values: { deviceName: $activeDeviceName || '' } })}</span
			>
			<Button variant="ghost" size="sm" onclick={onCancel}>
				{$translate('export.cancel')}
			</Button>
		</div>

		<div class="progress-status">
			<span class="status-label">{$exportStatusLabel}</span>
			<span class="status-count">
				{$exportProgress.files_copied} / {$exportProgress.files_total}
			</span>
		</div>

		<div class="progress-bar-container">
			<div class="progress-bar" style="width: {$exportProgressPercent}%"></div>
		</div>

		{#if $exportProgress.current_file}
			<div class="current-file">
				{truncateFilename($exportProgress.current_file)}
			</div>
		{/if}
	</div>
{/if}

<style>
	.export-progress {
		position: fixed;
		bottom: 16px;
		right: 16px;
		width: 320px;
		background: var(--bg-primary);
		border: 1px solid var(--border-color);
		border-radius: 12px;
		padding: 16px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2);
		z-index: 1000;
	}

	.progress-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 12px;
	}

	.progress-title {
		font-weight: 600;
		font-size: 14px;
	}

	.progress-status {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 8px;
		font-size: 13px;
	}

	.status-label {
		color: var(--text-secondary);
	}

	.status-count {
		font-weight: 500;
	}

	.progress-bar-container {
		height: 6px;
		background: var(--bg-tertiary);
		border-radius: 3px;
		overflow: hidden;
		margin-bottom: 8px;
	}

	.progress-bar {
		height: 100%;
		background: var(--accent-color);
		border-radius: 3px;
		transition: width 0.2s ease;
	}

	.current-file {
		font-size: 12px;
		color: var(--text-tertiary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
