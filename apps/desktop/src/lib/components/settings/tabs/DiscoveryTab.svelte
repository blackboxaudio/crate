<script lang="ts">
	import { Text, Checkbox, Button, ConfirmModal, Select, Input, IconButton, Tooltip } from '$lib/components/common'
	import {
		settingsStore,
		autoFetchMetadata,
		transferTagsOnImport,
		removeReleaseAfterImport,
		followCheckCadence,
		collectionRefreshCadence,
		autoFollowOnImport,
		releaseDayReminders,
		newReleasesSummary,
	} from '$shared/stores/settings'
	import { followStore, collectionStore, collectionAccounts } from '$lib/stores'
	import type { CollectionAccount, FollowCheckCadence, AutoFollowOnImport } from '$shared/types'
	import { translate } from '$shared/i18n'
	import * as discoveryApi from '$shared/api/discovery'
	import { formatFileSize } from '$shared/utils/format'
	import CollectionGapModal from '$lib/components/collection/CollectionGapModal.svelte'

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

	// Purchased collection: link-by-URL + accounts roster + library gap check.
	let linkUrl = $state('')
	let showGapModal = $state(false)

	async function handleLink() {
		const trimmed = linkUrl.trim()
		if (!trimmed || $collectionStore.linking) return
		const account = await collectionStore.linkFromUrl(trimmed)
		if (account) linkUrl = ''
	}

	function accountLabel(account: CollectionAccount): string {
		return account.name ?? account.username ?? account.url
	}

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

	<!-- Collection Section (linked Bandcamp fan accounts → owned badges) -->
	<section>
		<Text variant="header-3" class="mb-2">{$translate('settings.collection.title')}</Text>
		<div class="space-y-4">
			<div>
				<Text variant="caption" as="p" class="mb-2 text-text-tertiary">
					{$translate('settings.collection.description')}
				</Text>
				<div class="flex items-center gap-2">
					<Input
						bind:value={linkUrl}
						placeholder={$translate('settings.collection.linkPlaceholder')}
						class="w-72"
						onkeydown={(e: KeyboardEvent) => e.key === 'Enter' && handleLink()}
					/>
					<Button
						variant="secondary"
						size="sm"
						disabled={!linkUrl.trim() || $collectionStore.linking}
						onclick={handleLink}
					>
						{$collectionStore.linking ? $translate('common.loading') : $translate('settings.collection.link')}
					</Button>
				</div>
			</div>

			{#if $collectionAccounts.length > 0}
				<div class="divide-y divide-stroke-subtle rounded-md border border-stroke">
					{#each $collectionAccounts as account (account.id)}
						<div class="flex items-center gap-3 px-3 py-2">
							{#if account.avatarUrl}
								<img src={account.avatarUrl} alt="" class="h-7 w-7 rounded-full object-cover" />
							{:else}
								<div class="h-7 w-7 rounded-full bg-surface-2"></div>
							{/if}
							<div class="flex min-w-0 flex-1 flex-col">
								<Text as="span" size="sm" weight="medium" truncate>{accountLabel(account)}</Text>
								<Text as="span" variant="caption" truncate>
									{$translate('settings.collection.itemCount', { values: { count: account.itemCount } })}
									{#if account.health === 'error' || account.health === 'rate_limited'}
										· {account.lastError ?? $translate('errors.generic')}
									{/if}
								</Text>
							</div>
							<Checkbox
								checked={account.enabled}
								onchange={(c) => collectionStore.setEnabled(account.id, c)}
								label={$translate('settings.collection.enabledToggle')}
							/>
							<Tooltip text={$translate('settings.collection.refreshNow')} position="top" delay={250}>
								<IconButton
									icon="refresh"
									size="sm"
									disabled={$collectionStore.refreshingIds.has(account.id)}
									onclick={() => collectionStore.refreshAccount(account.id)}
								/>
							</Tooltip>
							<Tooltip text={$translate('common.remove')} position="top" delay={250}>
								<IconButton icon="trash" size="sm" onclick={() => collectionStore.unlink(account.id)} />
							</Tooltip>
						</div>
					{/each}
				</div>

				<div class="flex items-center gap-3">
					<Select
						value={$collectionRefreshCadence}
						options={cadenceOptions}
						onchange={(v) => settingsStore.setCollectionRefreshCadence(v as FollowCheckCadence)}
						class="w-56"
					/>
					<Button
						variant="secondary"
						size="sm"
						disabled={$collectionStore.refreshingAll}
						onclick={() => collectionStore.refreshAllAccounts()}
					>
						{$collectionStore.refreshingAll
							? $translate('common.loading')
							: $translate('settings.collection.refreshNow')}
					</Button>
					<Button variant="secondary" size="sm" onclick={() => (showGapModal = true)}>
						{$translate('collection.gap.title')}
					</Button>
				</div>
			{/if}

			<Text variant="caption" as="p" class="text-text-tertiary">
				{$translate('settings.collection.privacyNote')}
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

<CollectionGapModal open={showGapModal} onClose={() => (showGapModal = false)} />

<ConfirmModal
	open={showClearConfirm}
	title={$translate('settings.discovery.clearCache')}
	message={$translate('settings.discovery.clearCacheConfirmMessage')}
	warnings={[$translate('settings.discovery.clearCacheWarning')]}
	destructive={true}
	onConfirm={handleClearCache}
	onCancel={() => (showClearConfirm = false)}
/>
