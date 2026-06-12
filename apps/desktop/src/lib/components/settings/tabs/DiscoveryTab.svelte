<script lang="ts">
	import { Text, Checkbox, Button, ConfirmModal, Select } from '$lib/components/common'
	import {
		settingsStore,
		autoFetchMetadata,
		transferTagsOnImport,
		removeReleaseAfterImport,
		followCheckCadence,
		autoFollowOnImport,
		releaseDayReminders,
		newReleasesSummary,
	} from '$shared/stores/settings'
	import { followStore } from '$lib/stores'
	import type { FollowCheckCadence, AutoFollowOnImport } from '$shared/types'
	import { translate } from '$shared/i18n'
	import * as discoveryApi from '$shared/api/discovery'
	import { formatFileSize } from '$shared/utils/format'

	let cacheSize = $state(0)
	let clearing = $state(false)
	let showClearConfirm = $state(false)

	async function loadCacheSize() {
		try {
			cacheSize = await discoveryApi.getAudioCacheSize()
		} catch {
			cacheSize = 0
		}
	}

	async function handleClearCache() {
		showClearConfirm = false
		clearing = true
		try {
			await discoveryApi.clearAudioCache()
			cacheSize = 0
		} finally {
			clearing = false
		}
	}

	function handleAutoFetchMetadataChange(checked: boolean) {
		settingsStore.setAutoFetchMetadata(checked)
	}

	function handleTransferTagsOnImportChange(checked: boolean) {
		settingsStore.setTransferTagsOnImport(checked)
	}

	function handleRemoveReleaseAfterImportChange(checked: boolean) {
		settingsStore.setRemoveReleaseAfterImport(checked)
	}

	const cadenceOptions = $derived([
		{ value: 'on-launch', label: $translate('settings.following.cadenceLaunch') },
		{ value: 'hourly', label: $translate('settings.following.cadenceHourly') },
		{ value: 'daily', label: $translate('settings.following.cadenceDaily') },
		{ value: 'manual', label: $translate('settings.following.cadenceManual') },
	])

	const autoFollowOptions = $derived([
		{ value: 'off', label: $translate('settings.following.autoFollowOff') },
		{ value: 'artist', label: $translate('settings.following.autoFollowArtist') },
		{ value: 'label', label: $translate('settings.following.autoFollowLabel') },
		{ value: 'both', label: $translate('settings.following.autoFollowBoth') },
	])

	$effect(() => {
		loadCacheSize()
	})
</script>

<div class="space-y-8">
	<!-- Metadata Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.discovery.metadata')}</Text>
		<Text variant="caption" as="p" class="mb-2">{$translate('settings.discovery.autoFetchMetadataDescription')}</Text>

		<Checkbox
			checked={$autoFetchMetadata}
			onchange={handleAutoFetchMetadataChange}
			label={$translate('settings.discovery.autoFetchMetadata')}
		/>
	</section>

	<!-- Import Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.discovery.import')}</Text>

		<div class="space-y-4">
			<div>
				<Text variant="caption" as="p" class="mb-2 text-text-tertiary">
					{$translate('settings.discovery.transferTagsOnImportDescription')}
				</Text>
				<Checkbox
					checked={$transferTagsOnImport}
					onchange={handleTransferTagsOnImportChange}
					label={$translate('settings.discovery.transferTagsOnImport')}
				/>
			</div>

			<div>
				<Text variant="caption" as="p" class="mb-2 text-text-tertiary">
					{$translate('settings.discovery.removeReleaseAfterImportDescription')}
				</Text>
				<Checkbox
					checked={$removeReleaseAfterImport}
					onchange={handleRemoveReleaseAfterImportChange}
					label={$translate('settings.discovery.removeReleaseAfterImport')}
				/>
			</div>
		</div>
	</section>

	<!-- Following Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.following.title')}</Text>
		<div class="space-y-4">
			<div>
				<Text variant="caption" as="p" class="mb-2 text-text-tertiary">
					{$translate('settings.following.checkCadenceDescription')}
				</Text>
				<div class="flex items-center gap-3">
					<Select
						value={$followCheckCadence}
						options={cadenceOptions}
						onchange={(v) => settingsStore.setFollowCheckCadence(v as FollowCheckCadence)}
						class="w-56"
					/>
					<Button variant="secondary" size="sm" onclick={() => followStore.checkAll()}>
						{$translate('settings.following.checkAllNow')}
					</Button>
				</div>
			</div>

			<div>
				<Text variant="caption" as="p" class="mb-2 text-text-tertiary">
					{$translate('settings.following.autoFollowDescription')}
				</Text>
				<Select
					value={$autoFollowOnImport}
					options={autoFollowOptions}
					onchange={(v) => settingsStore.setAutoFollowOnImport(v as AutoFollowOnImport)}
					class="w-56"
				/>
			</div>

			<div>
				<Checkbox
					checked={$releaseDayReminders}
					onchange={(c) => settingsStore.setReleaseDayReminders(c)}
					label={$translate('settings.following.releaseDayReminders')}
				/>
				<Text variant="caption" as="p" class="mt-2 text-text-tertiary">
					{$translate('settings.following.releaseDayRemindersHelper')}
				</Text>
			</div>

			<Checkbox
				checked={$newReleasesSummary}
				onchange={(c) => settingsStore.setNewReleasesSummary(c)}
				label={$translate('settings.following.newReleasesSummary')}
			/>

			<Text variant="caption" as="p" class="text-text-tertiary">
				{$translate('settings.following.discogsRateLimitNote')}
			</Text>
		</div>
	</section>

	<!-- Preview Cache Section -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.discovery.previewCache')}</Text>
		<Text variant="caption" as="p" class="mb-2 text-text-tertiary">
			{$translate('settings.discovery.previewCacheDescription')}
		</Text>

		<div class="flex items-center gap-4">
			<Text>{$translate('settings.discovery.cacheSize')}: {formatFileSize(cacheSize)}</Text>
			<Button variant="secondary" onclick={() => (showClearConfirm = true)} disabled={cacheSize === 0 || clearing}>
				{clearing ? $translate('common.loading') : $translate('settings.discovery.clearCache')}
			</Button>
		</div>
	</section>
</div>

<ConfirmModal
	open={showClearConfirm}
	title={$translate('settings.discovery.clearCache')}
	message={$translate('settings.discovery.clearCacheConfirmMessage')}
	warnings={[$translate('settings.discovery.clearCacheWarning')]}
	destructive={true}
	onConfirm={handleClearCache}
	onCancel={() => (showClearConfirm = false)}
/>
