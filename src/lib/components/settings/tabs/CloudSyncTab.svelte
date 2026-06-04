<script lang="ts">
	import { Button, Text } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import Input from '$lib/components/common/Input.svelte'
	import ConfirmModal from '$lib/components/common/ConfirmModal.svelte'
	import { SignInPanel, LibraryRootsWizard } from '$lib/components/cloud-sync'
	import { cloudSyncStore, syncStatus, syncPhase, isSignedIn, cloudDevices, libraryRoots } from '$lib/stores/cloudSync'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'
	import { formatRelativeDate } from '$lib/utils'

	let renamingDevice = $state(false)
	let newDeviceName = $state('')
	let confirmRevokeDeviceId = $state<string | null>(null)

	$effect(() => {
		if ($isSignedIn) {
			cloudSyncStore.loadDevices()
			cloudSyncStore.loadLibraryRoots()
		}
	})

	function handleSyncNow() {
		cloudSyncStore.syncNow()
	}

	function handleSignOut() {
		cloudSyncStore.signOut()
	}

	function startRename() {
		newDeviceName = $syncStatus.device_name
		renamingDevice = true
	}

	function confirmRename() {
		if (newDeviceName.trim()) {
			cloudSyncStore.renameDevice(newDeviceName.trim())
		}
		renamingDevice = false
	}

	function handleRevoke(deviceId: string) {
		confirmRevokeDeviceId = deviceId
	}

	function confirmRevoke() {
		if (confirmRevokeDeviceId) {
			cloudSyncStore.revokeDevice(confirmRevokeDeviceId)
			confirmRevokeDeviceId = null
		}
	}

	function formatDeviceLastSeen(lastSeen: { secs_since_epoch: number; nanos_since_epoch: number }): string {
		const ms = lastSeen.secs_since_epoch * 1000
		const date = new Date(ms)
		const t = get(translate)
		return formatRelativeDate(date.toISOString(), t)
	}
</script>

{#if !$isSignedIn}
	<SignInPanel />
{:else}
	<div class="space-y-8">
		<!-- Account Section -->
		<section>
			<Text variant="header-3" class="mb-2">{$translate('cloudSync.account.title')}</Text>
			<div class="flex items-center justify-between rounded-lg border border-stroke bg-surface-1 p-4">
				<div>
					<Text variant="body" class="font-medium">{$syncStatus.email}</Text>
					{#if $syncStatus.last_synced_at}
						<Text variant="caption" as="p" class="mt-1">
							{$translate('cloudSync.account.lastSynced', {
								values: { time: formatRelativeDate($syncStatus.last_synced_at, get(translate)) },
							})}
						</Text>
					{/if}
				</div>
				<div class="flex items-center gap-2">
					{#if $syncPhase === 'syncing'}
						<div class="flex items-center gap-2 text-sm text-text-secondary">
							<svg class="h-4 w-4 animate-spin" fill="none" viewBox="0 0 24 24">
								<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
								<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
							</svg>
							{$translate('cloudSync.account.syncing')}
						</div>
					{:else}
						<Button variant="secondary" size="sm" onclick={handleSyncNow}>
							<Icon name="refresh" class="mr-1.5 h-3.5 w-3.5" />
							{$translate('cloudSync.account.syncNow')}
						</Button>
					{/if}
					<Button variant="secondary" size="sm" onclick={handleSignOut}>
						{$translate('cloudSync.account.signOut')}
					</Button>
				</div>
			</div>
			{#if $syncStatus.last_error}
				<div class="mt-2 rounded-md bg-red-500/10 px-3 py-2 text-sm text-red-500">
					{$syncStatus.last_error}
				</div>
			{/if}
		</section>

		<!-- Devices Section -->
		<section>
			<Text variant="header-3" class="mb-2">{$translate('cloudSync.devices.title')}</Text>
			<Text variant="caption" as="p" class="mb-3">{$translate('cloudSync.devices.description')}</Text>
			<div class="space-y-2">
				{#each $cloudDevices as device (device.device_id)}
					{@const isCurrentDevice = device.device_id === $syncStatus.device_id}
					<div
						class="flex items-center justify-between rounded-lg border px-4 py-3
							{isCurrentDevice ? 'border-brand-primary/30 bg-brand-muted' : 'border-stroke bg-surface-1'}"
					>
						<div>
							<div class="flex items-center gap-2">
								<Text variant="body" class="font-medium">{device.name}</Text>
								{#if isCurrentDevice}
									<span class="bg-brand-primary/20 rounded-full px-2 py-0.5 text-xs font-medium text-brand-primary">
										{$translate('cloudSync.devices.thisDevice')}
									</span>
								{/if}
							</div>
							<Text variant="caption" as="p" class="mt-0.5">
								{$translate('cloudSync.devices.lastSeen', { values: { time: formatDeviceLastSeen(device.last_seen) } })}
								· v{device.app_version}
							</Text>
						</div>
						<div class="flex items-center gap-2">
							{#if isCurrentDevice}
								{#if renamingDevice}
									<div class="flex items-center gap-2">
										<Input
											bind:value={newDeviceName}
											class="h-8 w-40 text-sm"
											onkeydown={(e) => e.key === 'Enter' && confirmRename()}
										/>
										<Button variant="primary" size="sm" onclick={confirmRename}>
											{$translate('common.save')}
										</Button>
										<Button variant="secondary" size="sm" onclick={() => (renamingDevice = false)}>
											{$translate('common.cancel')}
										</Button>
									</div>
								{:else}
									<Button variant="secondary" size="sm" onclick={startRename}>
										{$translate('common.rename')}
									</Button>
								{/if}
							{:else}
								<Button variant="secondary" size="sm" onclick={() => handleRevoke(device.device_id)}>
									{$translate('cloudSync.devices.revoke')}
								</Button>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		</section>

		<!-- Library Roots Section -->
		{#if $libraryRoots.length > 0}
			<section>
				<Text variant="header-3" class="mb-2">{$translate('cloudSync.roots.title')}</Text>
				<Text variant="caption" as="p" class="mb-3">{$translate('cloudSync.roots.description')}</Text>
				<LibraryRootsWizard />
			</section>
		{/if}
	</div>
{/if}

<ConfirmModal
	open={confirmRevokeDeviceId !== null}
	title={$translate('cloudSync.devices.revokeTitle')}
	message={$translate('cloudSync.devices.revokeMessage')}
	confirmLabel={$translate('cloudSync.devices.revoke')}
	destructive
	onConfirm={confirmRevoke}
	onCancel={() => (confirmRevokeDeviceId = null)}
/>
