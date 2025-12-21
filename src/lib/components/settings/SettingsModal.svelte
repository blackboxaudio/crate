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
	import { SUPPORTED_LANGUAGES } from '$lib/i18n'

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

	// Reset to first page when opening
	$effect(() => {
		if (open) {
			activePage = 'general'
		}
	})

	// Sync dialog open state
	$effect(() => {
		if (!dialogEl) return
		if (open && !dialogEl.open) {
			dialogEl.showModal()
		} else if (!open && dialogEl.open) {
			dialogEl.close()
		}
	})

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
	const themeOptions: { value: Theme; label: string }[] = [
		{ value: 'light', label: 'Light' },
		{ value: 'dark', label: 'Dark' },
		{ value: 'system', label: 'System' },
	]

	// Accent color options
	const accentColors: { value: AccentColor; hex: string; label: string }[] = [
		{ value: 'blue', hex: '#3b82f6', label: 'Blue' },
		{ value: 'indigo', hex: '#6366f1', label: 'Indigo' },
		{ value: 'violet', hex: '#8b5cf6', label: 'Violet' },
		{ value: 'purple', hex: '#a855f7', label: 'Purple' },
		{ value: 'pink', hex: '#ec4899', label: 'Pink' },
		{ value: 'rose', hex: '#f43f5e', label: 'Rose' },
		{ value: 'orange', hex: '#f97316', label: 'Orange' },
		{ value: 'amber', hex: '#f59e0b', label: 'Amber' },
		{ value: 'emerald', hex: '#10b981', label: 'Emerald' },
		{ value: 'teal', hex: '#14b8a6', label: 'Teal' },
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

		const systemDevices: SelectOption[] = [{ value: '', label: 'Default' }]
		const externalDevices: SelectOption[] = []

		for (const device of $audioDevices) {
			const option: SelectOption = {
				value: device.name,
				label: device.isDefault ? `${device.name} (Default)` : device.name,
			}

			if (device.isBuiltIn) {
				systemDevices.push(option)
			} else {
				externalDevices.push(option)
			}
		}

		const groups: SelectOptionGroup[] = [{ label: 'System', options: systemDevices }]

		// Only add External section if there are external devices
		if (externalDevices.length > 0) {
			groups.push({ label: 'External', options: externalDevices })
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
			copyTooltip?.show('Copied!')
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
	class="fixed top-1/2 left-1/2 max-h-[80vh] w-full max-w-2xl -translate-x-1/2 -translate-y-1/2
		rounded-lg border border-stroke bg-surface-1 p-0 text-text-primary shadow-xl
		backdrop:bg-black/60"
	onkeydown={handleKeydown}
	onclick={handleBackdropClick}
>
	{#if open}
		<div class="flex h-[500px]" transition:scale={{ start: 0.95, duration: 200 }}>
			<!-- Sidebar -->
			<div class="flex w-48 flex-col border-r border-stroke bg-surface-0 p-4">
				<Text variant="header-1" class="mb-4">Settings</Text>
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
						General
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
						Appearance
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
						Sound
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
						Diagnostics
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
						About
					</button>
				</nav>
			</div>

			<!-- Content -->
			<div class="flex-1 overflow-auto p-6">
				{#if activePage === 'general'}
					<div class="space-y-8">
						<!-- Language Section -->
						<section>
							<Text variant="header-3" class="mb-4">Language</Text>
							<div class="max-w-md">
								<Select
									value={$language}
									options={languageOptions}
									placeholder="Select a language"
									onchange={handleLanguageChange}
								/>
								<Text variant="caption" as="p" class="mt-2">Choose the display language for the application.</Text>
							</div>
						</section>
					</div>
				{:else if activePage === 'appearance'}
					<div class="space-y-8">
						<!-- Font Section -->
						<section>
							<Text variant="header-3" class="mb-4">Font</Text>
							<div class="max-w-md">
								<Select value={$font} options={fontOptions} placeholder="Select a font" onchange={handleFontChange} />
								<Text variant="caption" as="p" class="mt-2">Choose the font used throughout the application.</Text>
							</div>
						</section>

						<!-- Theme Section -->
						<section>
							<Text variant="header-3" class="mb-4">Theme</Text>
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
										<Text variant="body-2" as="span">{option.label}</Text>
									</button>
								{/each}
							</div>
						</section>

						<!-- Accent Color Section -->
						<section>
							<Text variant="header-3" class="mb-4">Accent Color</Text>
							<div class="grid grid-cols-5 gap-3">
								{#each accentColors as color (color.value)}
									<button
										type="button"
										class="group flex flex-col items-center gap-2 rounded-lg p-3
											transition-colors hover:cursor-pointer hover:bg-surface-2"
										onclick={() => handleAccentChange(color.value)}
										title={color.label}
									>
										<div
											class="h-8 w-8 rounded-full transition-transform
												group-hover:scale-110 {$accentColor === color.value
												? 'ring-2 ring-text-primary ring-offset-2 ring-offset-surface-1'
												: ''}"
											style="background-color: {color.hex};"
										></div>
										<Text variant="caption" color="secondary">{color.label}</Text>
									</button>
								{/each}
							</div>
						</section>
					</div>
				{:else if activePage === 'sound'}
					<div class="space-y-8">
						<!-- Output Device Section -->
						<section>
							<Text variant="header-3" class="mb-4">Output Device</Text>
							<div class="max-w-md">
								<Select
									value={$audioDevice ?? ''}
									options={audioDeviceOptions}
									placeholder="System Default"
									onchange={handleAudioDeviceChange}
								/>
								<Text variant="caption" as="p" class="mt-2">Select the audio output device for playback.</Text>
							</div>
						</section>
					</div>
				{:else if activePage === 'diagnostics'}
					<div class="space-y-6">
						<!-- System Info Section -->
						<section>
							<Text variant="header-3" class="mb-4">System Information</Text>
							{#if $systemInfo}
								<div class="space-y-2 rounded-md bg-surface-0 p-4">
									<div class="grid grid-cols-2 gap-x-4 gap-y-2">
										<div>
											<Text size="xs" color="secondary">Operating System</Text>
											<Text size="sm">{$systemInfo.osName} {$systemInfo.osVersion}</Text>
										</div>
										<div>
											<Text size="xs" color="secondary">CPU</Text>
											<Text size="sm">{$systemInfo.cpuBrand}</Text>
										</div>
										<div>
											<Text size="xs" color="secondary">Memory</Text>
											<Text size="sm">
												{formatBytes($systemInfo.usedMemoryBytes)} / {formatBytes($systemInfo.totalMemoryBytes)}
											</Text>
										</div>
										<div>
											<Text size="xs" color="secondary">Data Directory</Text>
											<Text size="sm">{formatBytes($systemInfo.dataDirSizeBytes)}</Text>
										</div>
									</div>
								</div>
							{:else}
								<Text size="sm" color="secondary">Loading system info...</Text>
							{/if}
						</section>

						<!-- Error Log Section -->
						<section>
							<div class="mb-4 flex items-center justify-between">
								<Text variant="header-3">Error Log</Text>
								<Text size="xs" color="secondary">
									{$diagnosticEntries.length} of 100 entries
								</Text>
							</div>

							{#if $diagnosticEntries.length === 0}
								<div class="rounded-md bg-surface-0 p-6 text-center">
									<Icon name="check" class="mx-auto h-8 w-8 text-success" />
									<Text size="sm" color="secondary" class="mt-2">No errors recorded</Text>
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
							<Text variant="header-3" class="mb-4">Export</Text>
							<div class="flex items-center gap-3">
								<Button variant="secondary" onclick={handleExportJson}>Save as JSON</Button>
								<Button variant="secondary" onclick={handleExportText}>Save as Text</Button>
								<Tooltip bind:this={copyTooltip}>
									<IconButton title="Copy to Clipboard" onclick={handleCopyToClipboard}>
										<Icon name={copySuccess ? 'check' : 'copy'} class="h-5 w-5 {copySuccess ? 'text-success' : ''}" />
									</IconButton>
								</Tooltip>
							</div>
							<Text variant="caption" as="p" class="mt-2">
								Export diagnostics report for bug reports or troubleshooting.
							</Text>
						</section>
					</div>
				{:else if activePage === 'about'}
					<div class="space-y-8">
						<!-- Application Section -->
						<section>
							<Text variant="header-3" class="mb-4">Application</Text>
							<div class="space-y-3">
								<div class="flex justify-between">
									<Text size="sm" color="secondary" as="span">Version</Text>
									<Text size="sm" as="span">{$appInfo?.version ?? 'Unknown'}</Text>
								</div>
								<div class="flex justify-between">
									<Text size="sm" color="secondary" as="span">Environment</Text>
									<Text size="sm" as="span" class="capitalize">{$appInfo?.environment ?? 'Unknown'}</Text>
								</div>
								<div class="flex justify-between">
									<Text size="sm" color="secondary" as="span">Data Directory</Text>
									<Text variant="code" truncate class="max-w-xs" title={$appInfo?.dataDir}>
										{$appInfo?.dataDir ?? 'Unknown'}
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
			<Button variant="secondary" onclick={onClose}>Close</Button>
		</div>
	{/if}
</dialog>
