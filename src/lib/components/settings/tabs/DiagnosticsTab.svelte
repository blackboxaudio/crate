<script lang="ts">
	import type { DiagnosticsReport } from '$lib/types'
	import { Button, Text, IconButton } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { diagnosticsStore, diagnosticEntries, systemInfo } from '$lib/stores/diagnostics'
	import { language } from '$lib/stores/settings'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'
	import { save } from '@tauri-apps/plugin-dialog'
	import { withNativeDialog } from '$lib/utils'
	import { writeTextFile } from '@tauri-apps/plugin-fs'
	import { writeText } from '@tauri-apps/plugin-clipboard-manager'
	import { formatBytes } from '$lib/utils'

	let copyTooltip: ReturnType<typeof Tooltip> | undefined = $state()
	let copySuccess = $state(false)

	// Format timestamp
	function formatTimestamp(iso: string): string {
		return new Date(iso).toLocaleString($language)
	}

	// Format report as text
	function formatReportAsText(report: DiagnosticsReport): string {
		const lines: string[] = [
			'========================================',
			'CRATE DIAGNOSTICS REPORT',
			'========================================',
			'',
			`App Version: ${report.appVersion}`,
			`Environment: ${report.environment}`,
			`Generated: ${formatTimestamp(report.generatedAt)}`,
			'',
			'--- SYSTEM INFO ---',
			`OS: ${report.systemInfo.osName} ${report.systemInfo.osVersion}`,
			`CPU: ${report.systemInfo.cpuBrand} (${report.systemInfo.cpuCores} cores)`,
			`Memory: ${formatBytes(report.systemInfo.usedMemoryBytes)} / ${formatBytes(report.systemInfo.totalMemoryBytes)}`,
			`Data Directory: ${formatBytes(report.systemInfo.dataDirSizeBytes)}`,
			`Database: ${formatBytes(report.systemInfo.databaseSizeBytes)}`,
			'',
			'--- ERROR LOG ---',
			'',
		]

		if (report.entries.length === 0) {
			lines.push('No errors recorded.')
		} else {
			for (const entry of report.entries) {
				lines.push(`[${formatTimestamp(entry.timestamp)}] [${entry.level.toUpperCase()}] [${entry.category}]`)
				lines.push(entry.message)
				if (entry.details) {
					lines.push(`Details: ${entry.details}`)
				}
				lines.push('')
			}
		}

		return lines.join('\n')
	}

	// Copy to clipboard
	async function handleCopyToClipboard() {
		try {
			const report = await diagnosticsStore.getReport()
			const text = formatReportAsText(report)
			await writeText(text)
			copySuccess = true
			copyTooltip?.show(get(translate)('settings.diagnostics.copied'))
			setTimeout(() => {
				copySuccess = false
			}, 2000)
		} catch (error) {
			console.error('Failed to copy to clipboard:', error)
		}
	}

	// Export as JSON
	async function handleExportJson() {
		try {
			const report = await diagnosticsStore.getReport()
			const filename = `crate-diagnostics-${new Date().toISOString().split('T')[0]}`

			const path = await withNativeDialog(() =>
				save({
					defaultPath: `${filename}.json`,
					filters: [{ name: 'JSON', extensions: ['json'] }],
				})
			)

			if (path) {
				await writeTextFile(path, JSON.stringify(report, null, 2))
			}
		} catch (error) {
			console.error('Export failed:', error)
		}
	}

	// Export as text
	async function handleExportText() {
		try {
			const report = await diagnosticsStore.getReport()
			const filename = `crate-diagnostics-${new Date().toISOString().split('T')[0]}`

			const path = await withNativeDialog(() =>
				save({
					defaultPath: `${filename}.txt`,
					filters: [{ name: 'Text', extensions: ['txt'] }],
				})
			)

			if (path) {
				await writeTextFile(path, formatReportAsText(report))
			}
		} catch (error) {
			console.error('Export failed:', error)
		}
	}
</script>

<div class="space-y-6">
	<!-- System Info Section -->
	<section>
		<Text variant="header-3" class="mb-4">{$translate('settings.diagnostics.systemInfo')}</Text>
		{#if $systemInfo}
			<div class="space-y-2 rounded-md bg-surface-0 p-4">
				<div class="grid grid-cols-2 gap-x-4 gap-y-2">
					<div>
						<Text size="xs" color="secondary">{$translate('settings.diagnostics.operatingSystem')}</Text>
						<Text size="sm">{$systemInfo.osName} {$systemInfo.osVersion}</Text>
					</div>
					<div>
						<Text size="xs" color="secondary">{$translate('settings.diagnostics.cpu')}</Text>
						<Text size="sm">{$systemInfo.cpuBrand}</Text>
					</div>
					<div>
						<Text size="xs" color="secondary">{$translate('settings.diagnostics.memory')}</Text>
						<Text size="sm">
							{formatBytes($systemInfo.usedMemoryBytes)} / {formatBytes($systemInfo.totalMemoryBytes)}
						</Text>
					</div>
					<div>
						<Text size="xs" color="secondary">{$translate('settings.diagnostics.dataDirectory')}</Text>
						<Text size="sm">{formatBytes($systemInfo.dataDirSizeBytes)}</Text>
					</div>
				</div>
			</div>
		{:else}
			<Text size="sm" color="secondary">{$translate('settings.diagnostics.loadingSystemInfo')}</Text>
		{/if}
	</section>

	<!-- Error Log Section -->
	<section>
		<div class="mb-4 flex items-center justify-between">
			<Text variant="header-3">{$translate('settings.diagnostics.errorLog')}</Text>
			<Text size="xs" color="secondary">
				{$translate('settings.diagnostics.entriesCount', { values: { count: $diagnosticEntries.length } })}
			</Text>
		</div>

		{#if $diagnosticEntries.length === 0}
			<div class="rounded-md bg-surface-0 p-6 text-center">
				<Icon name="check" class="mx-auto h-8 w-8 text-success" />
				<Text size="sm" color="secondary" class="mt-2">{$translate('settings.diagnostics.noErrors')}</Text>
			</div>
		{:else}
			<div class="max-h-48 space-y-2 overflow-y-auto">
				{#each $diagnosticEntries.toReversed() as entry (entry.id)}
					<div class="rounded-md bg-surface-0 p-3">
						<div class="flex items-start justify-between gap-2">
							<div class="flex items-center gap-2">
								<span class="inline-block h-2 w-2 rounded-full {entry.level === 'error' ? 'bg-danger' : 'bg-warning'}"
								></span>
								<Text size="xs" color="secondary">[{entry.category}]</Text>
							</div>
							<Text size="xs" color="tertiary">
								{formatTimestamp(entry.timestamp)}
							</Text>
						</div>
						<Text size="sm" class="mt-1">{entry.message}</Text>
						{#if entry.details}
							<Text variant="code" size="xs" class="mt-2 block opacity-70">
								{entry.details}
							</Text>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</section>

	<!-- Export Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.diagnostics.export')}</Text>
		<Text variant="caption" as="p" class="mb-2">
			{$translate('settings.diagnostics.exportDescription')}
		</Text>
		<div class="flex items-center gap-3">
			<Button variant="secondary" onclick={handleExportJson}>{$translate('settings.diagnostics.saveAsJson')}</Button>
			<Button variant="secondary" onclick={handleExportText}>{$translate('settings.diagnostics.saveAsText')}</Button>
			<Tooltip bind:this={copyTooltip}>
				<IconButton
					title={$translate('settings.diagnostics.copyToClipboard')}
					icon={copySuccess ? 'check' : 'copy'}
					iconClass="h-5 w-5 {copySuccess ? 'text-success' : ''}"
					onclick={handleCopyToClipboard}
				/>
			</Tooltip>
		</div>
	</section>
</div>
