<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Checkbox from '$lib/components/common/Checkbox.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import { SelectablePlaylistTree } from '$lib/components/playlists'
	import { DeviceItem } from '$lib/components/devices'
	import { translate } from '$lib/i18n'
	import { activeDeviceId } from '$lib/stores/export'
	import type { Playlist, UsbDevice, ExportRequest } from '$lib/types'
	import { SvelteSet } from 'svelte/reactivity'

	type Props = {
		open: boolean
		playlists: Playlist[]
		devices: UsbDevice[]
		onExport: (requests: ExportRequest[]) => void
		onClose: () => void
	}

	let { open, playlists, devices, onExport, onClose }: Props = $props()

	// Wizard step
	let step = $state<'selectDevices' | 'selectPlaylists'>('selectDevices')

	// Selected items
	let selectedDeviceIds = $state<Set<string>>(new Set())
	let selectedPlaylistIds = $state<Set<string>>(new Set())

	// Sync option
	let enableSync = $state(true)

	// Reset state when modal opens
	$effect(() => {
		if (open) {
			step = 'selectDevices'
			selectedDeviceIds = new Set()
			selectedPlaylistIds = new Set()
			enableSync = true
		}
	})

	// Toggle device selection
	function toggleDevice(deviceId: string) {
		const newSet = new SvelteSet(selectedDeviceIds)
		if (newSet.has(deviceId)) {
			newSet.delete(deviceId)
		} else {
			newSet.add(deviceId)
		}
		selectedDeviceIds = newSet
	}

	// Get children of a folder
	function getChildren(parentId: string): Playlist[] {
		return playlists.filter((p) => p.parent_id === parentId).sort((a, b) => a.sort_order - b.sort_order)
	}

	// Get parent of a playlist
	function getParent(playlistId: string): Playlist | null {
		const playlist = playlists.find((p) => p.id === playlistId)
		if (!playlist?.parent_id) return null
		return playlists.find((p) => p.id === playlist.parent_id) || null
	}

	// Check if all children of a folder are selected
	function allChildrenSelected(folderId: string, selectedSet: Set<string>): boolean {
		const children = getChildren(folderId)
		if (children.length === 0) return false
		return children.every((child) => selectedSet.has(child.id))
	}

	// Toggle playlist selection
	function togglePlaylist(playlistId: string, isFolder: boolean) {
		const newSet = new SvelteSet(selectedPlaylistIds)

		if (newSet.has(playlistId)) {
			// Deselect this and all children
			newSet.delete(playlistId)
			if (isFolder) {
				const descendants = getAllDescendants(playlistId)
				for (const id of descendants) {
					newSet.delete(id)
				}
			}
		} else {
			// Select this and all children (if folder)
			newSet.add(playlistId)
			if (isFolder) {
				const descendants = getAllDescendants(playlistId)
				for (const id of descendants) {
					newSet.add(id)
				}
			}
		}

		// Cascade up: auto-select/deselect parent folders based on children state
		let parent = getParent(playlistId)
		while (parent) {
			if (allChildrenSelected(parent.id, newSet)) {
				newSet.add(parent.id)
			} else {
				newSet.delete(parent.id)
			}
			parent = getParent(parent.id)
		}

		selectedPlaylistIds = newSet
	}

	// Get all descendant IDs of a folder
	function getAllDescendants(folderId: string): string[] {
		const result: string[] = []
		const children = getChildren(folderId)

		for (const child of children) {
			result.push(child.id)
			if (child.is_folder) {
				result.push(...getAllDescendants(child.id))
			}
		}

		return result
	}

	// Get selected devices
	const selectedDevices = $derived(devices.filter((d) => selectedDeviceIds.has(d.id)))

	// Calculate selected playlist count (excluding folders)
	const selectedPlaylistCount = $derived(() => {
		let count = 0
		for (const id of selectedPlaylistIds) {
			const p = playlists.find((pl) => pl.id === id)
			if (p && !p.is_folder) count++
		}
		return count
	})

	// Calculate total track count for selected playlists
	const totalTrackCount = $derived(() => {
		let count = 0
		for (const id of selectedPlaylistIds) {
			const p = playlists.find((pl) => pl.id === id)
			if (p && !p.is_folder) {
				count += p.track_count
			}
		}
		return count
	})

	// Check if can proceed to next step
	const canProceedToPlaylists = $derived(selectedDeviceIds.size > 0)

	// Check if export is valid
	const canExport = $derived(selectedDeviceIds.size > 0 && selectedPlaylistIds.size > 0)

	// Navigate to next step
	function goToPlaylists() {
		if (canProceedToPlaylists) {
			step = 'selectPlaylists'
		}
	}

	// Navigate back
	function goBack() {
		step = 'selectDevices'
	}

	// Handle export
	function handleExport() {
		if (!canExport) return

		const requests: ExportRequest[] = selectedDevices.map((device) => ({
			device_id: device.id,
			mount_point: device.mount_point,
			device_name: device.name,
			playlist_ids: Array.from(selectedPlaylistIds),
			enable_sync: enableSync,
		}))

		onExport(requests)
	}

	// Modal title based on step
	const title = $derived(
		step === 'selectDevices' ? $translate('export.quickExport') : $translate('export.selectPlaylists')
	)
</script>

<Modal {open} {title} {onClose} size="md">
	<div class="export-content">
		{#if step === 'selectDevices'}
			<!-- Step 1: Device selection -->
			<Text color="secondary">{$translate('export.selectDevicesDescription')}</Text>

			<div class="device-list">
				{#each devices as device (device.id)}
					<DeviceItem
						{device}
						selectable
						disabled={$activeDeviceId === device.id}
						selected={selectedDeviceIds.has(device.id)}
						onSelect={() => toggleDevice(device.id)}
					/>
				{:else}
					<Text color="tertiary" class="text-center py-6">{$translate('devices.noDevices')}</Text>
				{/each}
			</div>

			{#if selectedDeviceIds.size > 0}
				<div class="selection-summary">
					<Text color="secondary" size="xs" as="span">
						{$translate('export.devicesSelected', { values: { count: selectedDeviceIds.size } })}
					</Text>
				</div>
			{/if}
		{:else}
			<!-- Step 2: Playlist selection -->
			<Text color="secondary">{$translate('export.selectPlaylistsDescription')}</Text>

			<div class="playlist-tree-container">
				<SelectablePlaylistTree {playlists} selectedIds={selectedPlaylistIds} onToggle={togglePlaylist} />
			</div>

			<div class="export-summary">
				<div class="summary-row">
					<Text size="xs" as="span">{$translate('export.devicesToExport')}:</Text>
					<Text size="xs" weight="medium" as="span">{selectedDeviceIds.size}</Text>
				</div>
				<div class="summary-row">
					<Text size="xs" as="span">{$translate('export.playlistsToExport')}:</Text>
					<Text size="xs" weight="medium" as="span">{selectedPlaylistCount()}</Text>
				</div>
				<div class="summary-row">
					<Text size="xs" as="span">{$translate('export.tracksToExport')}:</Text>
					<Text size="xs" weight="medium" as="span">{totalTrackCount()}</Text>
				</div>
			</div>

			<div class="sync-option">
				<Checkbox bind:checked={enableSync} label={$translate('export.enableSync')} />
			</div>
		{/if}
	</div>

	{#snippet footer()}
		{#if step === 'selectDevices'}
			<Button variant="ghost" onclick={onClose}>{$translate('common.cancel')}</Button>
			<Button variant="primary" onclick={goToPlaylists} disabled={!canProceedToPlaylists}>
				{$translate('common.next')}
			</Button>
		{:else}
			<Button variant="ghost" onclick={goBack}>{$translate('common.back')}</Button>
			<div class="footer-spacer"></div>
			<Button variant="ghost" onclick={onClose}>{$translate('common.cancel')}</Button>
			<Button variant="primary" onclick={handleExport} disabled={!canExport}>
				{$translate('export.export')}
			</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.export-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.device-list {
		display: flex;
		flex-direction: column;
		gap: 4px;
		max-height: 300px;
		overflow-y: auto;
	}

	.selection-summary {
		padding: 8px 0;
	}

	.playlist-tree-container {
		max-height: 250px;
		overflow-y: auto;
		border: 1px solid var(--stroke);
		border-radius: 6px;
		padding: 6px 6px;
	}

	.export-summary {
		background: var(--bg-secondary);
		border-radius: 8px;
		padding: 12px;
	}

	.summary-row {
		display: flex;
		justify-content: space-between;
		padding: 4px 0;
	}

	.sync-option {
		padding-top: 8px;
	}

	.footer-spacer {
		flex: 1;
	}
</style>
