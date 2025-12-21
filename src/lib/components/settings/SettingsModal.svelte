<script lang="ts">
	import type { Theme, AccentColor, Font, DiagnosticsReport, Language } from '$lib/types'
	import { settingsStore, theme, accentColor, font, audioDevice, audioDevices, language } from '$lib/stores/settings'
	import { diagnosticsStore, diagnosticEntries, systemInfo } from '$lib/stores/diagnostics'
	import { appInfo } from '$lib/stores/app'
	import { Button, Select, Text, IconButton } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { save } from '@tauri-apps/plugin-dialog'
	import { writeTextFile } from '@tauri-apps/plugin-fs'
	import { writeText } from '@tauri-apps/plugin-clipboard-manager'
	import { scale } from 'svelte/transition'
	import { SUPPORTED_LANGUAGES, translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	type Props = {
		open: boolean
		onClose: () => void
	}

	type SettingsPage = 'general' | 'appearance' | 'sound' | 'diagnostics' | 'about'

	let { open, onClose }: Props = $props()

	let dialogEl: HTMLDialogElement | undefined = $state()
	let activePage: SettingsPage = $state('general')
	let copyTooltip: ReturnType<typeof Tooltip> | undefined = $state()
	let copySuccess = $state(false)
	let visible = $state(false)

	// Reset to first page when opening
	$effect(() => {
		if (open) {
			activePage = 'general'
		}
	})

	// Open dialog when open becomes true
	$effect(() => {
		if (!dialogEl) return
		if (open) {
			visible = true
			dialogEl.showModal()
		}
	})

	// Handle transition end to close dialog
	function handleOutroEnd() {
		dialogEl?.close()
		visible = false
	}

	// Refresh audio devices when opening sound settings
	$effect(() => {
		if (open && activePage === 'sound') {
			settingsStore.refreshAudioDevices()
		}
	})

	// Load diagnostics when opening diagnostics settings
	$effect(() => {
		if (open && activePage === 'diagnostics') {
			diagnosticsStore.load()
		}
	})

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault()
			onClose()
		}
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === dialogEl) {
			onClose()
		}
	}

	// Theme options
	const themeOptions: { value: Theme; labelKey: string }[] = [
		{ value: 'light', labelKey: 'settings.appearance.themeLight' },
		{ value: 'dark', labelKey: 'settings.appearance.themeDark' },
		{ value: 'system', labelKey: 'settings.appearance.themeSystem' },
	]

	// Accent color options
	const accentColors: { value: AccentColor; hex: string; labelKey: string }[] = [
		{ value: 'blue', hex: '#3b82f6', labelKey: 'colors.blue' },
		{ value: 'indigo', hex: '#6366f1', labelKey: 'colors.indigo' },
		{ value: 'violet', hex: '#8b5cf6', labelKey: 'colors.violet' },
		{ value: 'purple', hex: '#a855f7', labelKey: 'colors.purple' },
		{ value: 'pink', hex: '#ec4899', labelKey: 'colors.pink' },
		{ value: 'rose', hex: '#f43f5e', labelKey: 'colors.rose' },
		{ value: 'orange', hex: '#f97316', labelKey: 'colors.orange' },
		{ value: 'amber', hex: '#f59e0b', labelKey: 'colors.amber' },
		{ value: 'emerald', hex: '#10b981', labelKey: 'colors.emerald' },
		{ value: 'teal', hex: '#14b8a6', labelKey: 'colors.teal' },
	]

	// Font options
	const fontOptions: { value: Font; label: string; style: string }[] = [
		{ value: 'ibm-plex-mono', label: 'IBM Plex Mono', style: "font-family: 'IBM Plex Mono', monospace" },
		{ value: 'jetbrains-mono', label: 'JetBrains Mono', style: "font-family: 'JetBrains Mono', monospace" },
		{ value: 'fira-code', label: 'Fira Code', style: "font-family: 'Fira Code', monospace" },
		{ value: 'inter', label: 'Inter', style: "font-family: 'Inter', sans-serif" },
		{ value: 'open-sans', label: 'Open Sans', style: "font-family: 'Open Sans', sans-serif" },
	]

	function handleThemeChange(newTheme: Theme) {
		settingsStore.setTheme(newTheme)
	}

	function handleAccentChange(newColor: AccentColor) {
		settingsStore.setAccentColor(newColor)
	}

	function handleFontChange(value: string) {
		settingsStore.setFont(value as Font)
	}

	function handleAudioDeviceChange(value: string) {
		settingsStore.setAudioDevice(value === '' ? null : value)
	}

	function handleLanguageChange(value: string) {
		settingsStore.setLanguage(value as Language)
	}

	// Language options for Select component
	const languageOptions = SUPPORTED_LANGUAGES.map((lang) => ({
		value: lang.value,
		label: lang.nativeLabel,
	}))

	// Build grouped audio device options for Select component
	const audioDeviceOptions = $derived.by(() => {
		type SelectOption = { value: string; label: string }
		type SelectOptionGroup = { label: string; options: SelectOption[] }

		const defaultLabel = $translate('common.default')
		const systemDevices: SelectOption[] = [{ value: '', label: defaultLabel }]
		const externalDevices: SelectOption[] = []

		for (const device of $audioDevices) {
			const option: SelectOption = {
				value: device.name,
				label: device.isDefault ? `${device.name} (${defaultLabel})` : device.name,
			}

			if (device.isBuiltIn) {
				systemDevices.push(option)
			} else {
				externalDevices.push(option)
			}
		}

		const groups: SelectOptionGroup[] = [{ label: $translate('settings.sound.system'), options: systemDevices }]

		// Only add External section if there are external devices
		if (externalDevices.length > 0) {
			groups.push({ label: $translate('settings.sound.external'), options: externalDevices })
		}

		return groups
	})

	// Format bytes to human readable
	function formatBytes(bytes: number | null | undefined): string {
		if (bytes === null || bytes === undefined) return 'Unknown'
		const units = ['B', 'KB', 'MB', 'GB']
		let value = bytes
		let unitIndex = 0
		while (value >= 1024 && unitIndex < units.length - 1) {
			value /= 1024
			unitIndex++
		}
		return `${value.toFixed(1)} ${units[unitIndex]}`
	}

	// Format timestamp
	function formatTimestamp(iso: string): string {
		return new Date(iso).toLocaleString()
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

			const path = await save({
				defaultPath: `${filename}.json`,
				filters: [{ name: 'JSON', extensions: ['json'] }],
			})

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

			const path = await save({
				defaultPath: `${filename}.txt`,
				filters: [{ name: 'Text', extensions: ['txt'] }],
			})

			if (path) {
				await writeTextFile(path, formatReportAsText(report))
			}
		} catch (error) {
			console.error('Export failed:', error)
		}
	}
</script>

<dialog
	bind:this={dialogEl}
	class="fixed inset-0 m-0 h-full max-h-none w-full max-w-none bg-transparent p-0 backdrop:bg-black/60"
	onkeydown={handleKeydown}
	onclick={handleBackdropClick}
>
	{#if visible}
		<div
			class="fixed top-1/2 left-1/2 max-h-[80vh] w-full max-w-2xl -translate-x-1/2 -translate-y-1/2
				rounded-lg border border-stroke bg-surface-1 text-text-primary shadow-xl"
			transition:scale={{ start: 0.95, duration: 200 }}
			onoutroend={handleOutroEnd}
		>
			<div class="flex h-[500px]">
				<!-- Sidebar -->
				<div class="flex w-48 flex-col border-r border-stroke bg-surface-0 p-4">
					<Text variant="header-1" class="mb-4">{$translate('settings.title')}</Text>
					<nav class="space-y-1">
						<button
							type="button"
							class="flex w-full items-center gap-2 rounded-md px-3 py-2
							text-sm font-medium hover:cursor-pointer {activePage === 'general'
								? 'bg-brand-muted text-brand-primary'
								: 'text-text-secondary hover:bg-surface-2 hover:text-text-primary'}"
							onclick={() => (activePage = 'general')}
						>
							<Icon name="globe" class="h-4 w-4" />
							{$translate('settings.tabs.general')}
						</button>
						<button
							type="button"
							class="flex w-full items-center gap-2 rounded-md px-3 py-2
							text-sm font-medium hover:cursor-pointer {activePage === 'appearance'
								? 'bg-brand-muted text-brand-primary'
								: 'text-text-secondary hover:bg-surface-2 hover:text-text-primary'}"
							onclick={() => (activePage = 'appearance')}
						>
							<Icon name="palette" class="h-4 w-4" />
							{$translate('settings.tabs.appearance')}
						</button>
						<button
							type="button"
							class="flex w-full items-center gap-2 rounded-md px-3 py-2
							text-sm font-medium hover:cursor-pointer {activePage === 'sound'
								? 'bg-brand-muted text-brand-primary'
								: 'text-text-secondary hover:bg-surface-2 hover:text-text-primary'}"
							onclick={() => (activePage = 'sound')}
						>
							<Icon name="volume-full" class="h-4 w-4" fill />
							{$translate('settings.tabs.sound')}
						</button>
						<button
							type="button"
							class="flex w-full items-center gap-2 rounded-md px-3 py-2
							text-sm font-medium hover:cursor-pointer {activePage === 'diagnostics'
								? 'bg-brand-muted text-brand-primary'
								: 'text-text-secondary hover:bg-surface-2 hover:text-text-primary'}"
							onclick={() => (activePage = 'diagnostics')}
						>
							<Icon name="terminal" class="h-4 w-4" />
							{$translate('settings.tabs.diagnostics')}
						</button>
						<button
							type="button"
							class="flex w-full items-center gap-2 rounded-md px-3 py-2
							text-sm font-medium hover:cursor-pointer {activePage === 'about'
								? 'bg-brand-muted text-brand-primary'
								: 'text-text-secondary hover:bg-surface-2 hover:text-text-primary'}"
							onclick={() => (activePage = 'about')}
						>
							<Icon name="info" class="h-4 w-4" />
							{$translate('settings.tabs.about')}
						</button>
					</nav>
				</div>

				<!-- Content -->
				<div class="flex-1 overflow-auto p-6">
					{#if activePage === 'general'}
						<div class="space-y-8">
							<!-- Language Section -->
							<section>
								<Text variant="header-3" class="mb-4">{$translate('settings.general.language')}</Text>
								<div class="max-w-md">
									<Select
										value={$language}
										options={languageOptions}
										placeholder={$translate('settings.general.language')}
										onchange={handleLanguageChange}
									/>
									<Text variant="caption" as="p" class="mt-2">{$translate('settings.general.languageDescription')}</Text
									>
								</div>
							</section>
						</div>
					{:else if activePage === 'appearance'}
						<div class="space-y-8">
							<!-- Font Section -->
							<section>
								<Text variant="header-3" class="mb-4">{$translate('settings.appearance.font')}</Text>
								<div class="max-w-md">
									<Select
										value={$font}
										options={fontOptions}
										placeholder={$translate('settings.appearance.font')}
										onchange={handleFontChange}
									/>
									<Text variant="caption" as="p" class="mt-2">{$translate('settings.appearance.fontDescription')}</Text>
								</div>
							</section>

							<!-- Theme Section -->
							<section>
								<Text variant="header-3" class="mb-4">{$translate('settings.appearance.theme')}</Text>
								<div class="flex gap-3">
									{#each themeOptions as option (option.value)}
										<button
											type="button"
											class="flex flex-1 flex-col items-center gap-2 rounded-lg border-2 p-4
											transition-colors {$theme === option.value
												? 'border-brand-primary bg-brand-muted'
												: 'border-stroke hover:cursor-pointer hover:border-text-tertiary'}"
											onclick={() => handleThemeChange(option.value)}
										>
											{#if option.value === 'light'}
												<Icon name="sun" class="h-6 w-6" />
											{:else if option.value === 'dark'}
												<Icon name="moon" class="h-6 w-6" />
											{:else}
												<Icon name="monitor" class="h-6 w-6" />
											{/if}
											<Text variant="body-2" as="span">{$translate(option.labelKey)}</Text>
										</button>
									{/each}
								</div>
							</section>

							<!-- Accent Color Section -->
							<section>
								<Text variant="header-3" class="mb-4">{$translate('settings.appearance.accentColor')}</Text>
								<div class="grid grid-cols-5 gap-3">
									{#each accentColors as color (color.value)}
										<button
											type="button"
											class="group flex flex-col items-center gap-2 rounded-lg p-3
											transition-colors hover:cursor-pointer hover:bg-surface-2"
											onclick={() => handleAccentChange(color.value)}
											title={$translate(color.labelKey)}
										>
											<div
												class="h-8 w-8 rounded-full transition-transform
												group-hover:scale-110 {$accentColor === color.value
													? 'ring-2 ring-text-primary ring-offset-2 ring-offset-surface-1'
													: ''}"
												style="background-color: {color.hex};"
											></div>
											<Text variant="caption" color="secondary">{$translate(color.labelKey)}</Text>
										</button>
									{/each}
								</div>
							</section>
						</div>
					{:else if activePage === 'sound'}
						<div class="space-y-8">
							<!-- Output Device Section -->
							<section>
								<Text variant="header-3" class="mb-4">{$translate('settings.sound.outputDevice')}</Text>
								<div class="max-w-md">
									<Select
										value={$audioDevice ?? ''}
										options={audioDeviceOptions}
										placeholder={$translate('settings.sound.systemDefault')}
										onchange={handleAudioDeviceChange}
									/>
									<Text variant="caption" as="p" class="mt-2"
										>{$translate('settings.sound.outputDeviceDescription')}</Text
									>
								</div>
							</section>
						</div>
					{:else if activePage === 'diagnostics'}
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
														<span
															class="inline-block h-2 w-2 rounded-full {entry.level === 'error'
																? 'bg-danger'
																: 'bg-warning'}"
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
								<Text variant="header-3" class="mb-4">{$translate('settings.diagnostics.export')}</Text>
								<div class="flex items-center gap-3">
									<Button variant="secondary" onclick={handleExportJson}
										>{$translate('settings.diagnostics.saveAsJson')}</Button
									>
									<Button variant="secondary" onclick={handleExportText}
										>{$translate('settings.diagnostics.saveAsText')}</Button
									>
									<Tooltip bind:this={copyTooltip}>
										<IconButton
											title={$translate('settings.diagnostics.copyToClipboard')}
											icon={copySuccess ? 'check' : 'copy'}
											iconClass="h-5 w-5 {copySuccess ? 'text-success' : ''}"
											onclick={handleCopyToClipboard}
										/>
									</Tooltip>
								</div>
								<Text variant="caption" as="p" class="mt-2">
									{$translate('settings.diagnostics.exportDescription')}
								</Text>
							</section>
						</div>
					{:else if activePage === 'about'}
						<div class="space-y-8">
							<!-- Application Section -->
							<section>
								<Text variant="header-3" class="mb-4">{$translate('settings.about.application')}</Text>
								<div class="space-y-3">
									<div class="flex justify-between">
										<Text size="sm" color="secondary" as="span">{$translate('settings.about.version')}</Text>
										<Text size="sm" as="span">{$appInfo?.version ?? $translate('common.unknown')}</Text>
									</div>
									<div class="flex justify-between">
										<Text size="sm" color="secondary" as="span">{$translate('settings.about.environment')}</Text>
										<Text size="sm" as="span" class="capitalize"
											>{$appInfo?.environment ?? $translate('common.unknown')}</Text
										>
									</div>
									<div class="flex justify-between">
										<Text size="sm" color="secondary" as="span">{$translate('settings.diagnostics.dataDirectory')}</Text
										>
										<Text variant="code" truncate class="max-w-xs" title={$appInfo?.dataDir}>
											{$appInfo?.dataDir ?? $translate('common.unknown')}
										</Text>
									</div>
								</div>
							</section>
						</div>
					{/if}
				</div>
			</div>

			<!-- Footer -->
			<div class="flex justify-end border-t border-stroke px-6 py-4">
				<Button variant="secondary" onclick={onClose}>{$translate('common.close')}</Button>
			</div>
		</div>
	{/if}
</dialog>
