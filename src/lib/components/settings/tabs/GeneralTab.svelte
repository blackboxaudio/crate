<script lang="ts">
	import type { Language, DateFormat, BackupStatus } from '$lib/types'
	import { Button, Select, Text } from '$lib/components/common'
	import ConfirmModal from '$lib/components/common/ConfirmModal.svelte'
	import { settingsStore, language, dateFormat, lastBackupAt } from '$lib/stores/settings'
	import { backupStore, isBackupBusy, backupProgress } from '$lib/stores/backup'
	import { toastStore } from '$lib/stores/toast'
	import { SUPPORTED_LANGUAGES, translate } from '$lib/i18n'
	import { save, open } from '@tauri-apps/plugin-dialog'
	import { withNativeDialog } from '$lib/utils'
	import * as backupApi from '$lib/api/backup'
	import { get } from 'svelte/store'
	import { slide } from 'svelte/transition'

	const languageOptions = SUPPORTED_LANGUAGES.map((lang) => ({
		value: lang.value,
		label: lang.nativeLabel,
		sublabel: lang.label,
	}))

	let dateFormatOptions = $derived([
		{ value: 'locale', label: $translate('settings.general.dateFormatLocale') },
		{ value: 'iso', label: $translate('settings.general.dateFormatIso') },
		{ value: 'us', label: $translate('settings.general.dateFormatUs') },
		{ value: 'eu', label: $translate('settings.general.dateFormatEu') },
		{ value: 'dot', label: $translate('settings.general.dateFormatDot') },
	])

	let showRestoreConfirm = $state(false)
	let pendingRestorePath = $state<string | null>(null)

	function handleLanguageChange(value: string) {
		settingsStore.setLanguage(value as Language)
	}

	function handleDateFormatChange(value: string) {
		settingsStore.setDateFormat(value as DateFormat)
	}

	function formatLastBackupDate(isoDate: string): string {
		try {
			const date = new Date(isoDate)
			return date.toLocaleDateString(undefined, {
				year: 'numeric',
				month: 'short',
				day: 'numeric',
				hour: 'numeric',
				minute: '2-digit',
			})
		} catch {
			return isoDate
		}
	}

	function getDefaultFilename(): string {
		const now = new Date()
		const year = now.getFullYear()
		const month = String(now.getMonth() + 1).padStart(2, '0')
		const day = String(now.getDate()).padStart(2, '0')
		return `crate-backup-${year}-${month}-${day}`
	}

	async function handleCreateBackup() {
		const path = await withNativeDialog(() =>
			save({
				defaultPath: `${getDefaultFilename()}.cratebackup`,
				filters: [{ name: 'Crate Backup', extensions: ['cratebackup'] }],
			})
		)
		if (!path) return

		backupStore.startBackup()
		try {
			await backupApi.createBackup(path)
			await settingsStore.load()
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error)
			backupStore.fail(message)
			toastStore.error(message)
		}
	}

	async function handleRestoreFromBackup() {
		const path = await withNativeDialog(() =>
			open({
				filters: [{ name: 'Crate Backup', extensions: ['cratebackup'] }],
				multiple: false,
			})
		)
		if (!path) return

		pendingRestorePath = path as string
		showRestoreConfirm = true
	}

	async function confirmRestore() {
		showRestoreConfirm = false
		const path = pendingRestorePath
		pendingRestorePath = null
		if (!path) return

		backupStore.startRestore()
		try {
			await backupApi.restoreFromBackup(path)
			// Full reload — all library data was replaced, so every store and
			// its derived/cached state needs a clean slate.
			window.location.reload()
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error)
			backupStore.fail(message)
			toastStore.error(message)
		}
	}

	function cancelRestore() {
		showRestoreConfirm = false
		pendingRestorePath = null
	}

	function getProgressLabel(status: string): string {
		const t = get(translate)
		switch (status) {
			case 'reading_data':
				return t('settings.general.backup.readingData')
			case 'collecting_artwork':
				return t('settings.general.backup.collectingArtwork')
			case 'writing_file':
				return t('settings.general.backup.writingFile')
			case 'restoring_data':
				return t('settings.general.backup.restoringData')
			case 'restoring_artwork':
				return t('settings.general.backup.restoringArtwork')
			case 'completed':
				return t('settings.general.backup.backupComplete')
			default:
				return ''
		}
	}

	function getProgressPercent(status: BackupStatus): number {
		switch (status) {
			case 'pending':
				return 5
			case 'reading_data':
				return 20
			case 'collecting_artwork':
				return 40
			case 'writing_file':
			case 'restoring_data':
				return 60
			case 'restoring_artwork':
				return 80
			case 'completed':
				return 100
		}
	}

	$effect(() => {
		backupStore.startListening()
		return () => backupStore.stopListening()
	})
</script>

<div class="space-y-8">
	<!-- Language Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.general.language')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.general.languageDescription')}</Text>
		<div class="max-w-md">
			<Select
				value={$language}
				options={languageOptions}
				placeholder={$translate('settings.general.language')}
				onchange={handleLanguageChange}
			/>
		</div>
	</section>

	<!-- Date Format Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.general.dateFormat')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.general.dateFormatDescription')}</Text>
		<div class="max-w-md">
			<Select
				value={$dateFormat}
				options={dateFormatOptions}
				placeholder={$translate('settings.general.dateFormat')}
				onchange={handleDateFormatChange}
			/>
		</div>
	</section>

	<!-- Backup & Restore Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.general.backup.title')}</Text>
		<Text variant="caption" as="p" class="mb-4">{$translate('settings.general.backup.description')}</Text>

		{#if $lastBackupAt}
			<Text variant="caption" as="p" class="text-fg-secondary mb-4">
				{$translate('settings.general.backup.lastBackupAt', { values: { date: formatLastBackupDate($lastBackupAt) } })}
			</Text>
		{:else}
			<Text variant="caption" as="p" class="text-fg-secondary mb-4">
				{$translate('settings.general.backup.noBackups')}
			</Text>
		{/if}

		{#if $backupProgress && $isBackupBusy}
			<div class="mb-4 max-w-md" transition:slide={{ duration: 200 }}>
				<Text variant="caption" as="p" class="mb-2">{getProgressLabel($backupProgress.status)}</Text>
				<div class="bg-bg-tertiary h-1.5 w-full overflow-hidden rounded-full">
					<div
						class="bg-accent h-full rounded-full transition-[width] ease-out"
						style="width: {getProgressPercent($backupProgress.status)}%; transition-duration: 300ms"
					></div>
				</div>
			</div>
		{/if}

		<div class="flex items-start gap-6">
			<div class="flex-1">
				<Text variant="caption" as="p" class="mb-2"
					>{$translate('settings.general.backup.createBackupDescription')}</Text
				>
				<Button variant="secondary" size="sm" onclick={handleCreateBackup} disabled={$isBackupBusy}>
					{$isBackupBusy
						? $translate('settings.general.backup.creatingBackup')
						: $translate('settings.general.backup.createBackup')}
				</Button>
			</div>
			<div class="flex-1">
				<Text variant="caption" as="p" class="mb-2"
					>{$translate('settings.general.backup.restoreFromBackupDescription')}</Text
				>
				<Button variant="secondary" size="sm" onclick={handleRestoreFromBackup} disabled={$isBackupBusy}>
					{$isBackupBusy
						? $translate('settings.general.backup.restoringBackup')
						: $translate('settings.general.backup.restoreFromBackup')}
				</Button>
			</div>
		</div>
	</section>
</div>

<ConfirmModal
	open={showRestoreConfirm}
	title={$translate('settings.general.backup.restoreFromBackup')}
	message={$translate('settings.general.backup.restoreWarning')}
	confirmLabel={$translate('settings.general.backup.restoreFromBackup')}
	destructive={true}
	onConfirm={confirmRestore}
	onCancel={cancelRestore}
/>
