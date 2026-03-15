<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Checkbox from '$lib/components/common/Checkbox.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import { SelectablePlaylistTree } from '$lib/components/playlists'
	import { translate } from '$shared/i18n'
	import { formatBytes } from '$shared/utils'
	import type { Playlist, UsbDevice, ExportRequest } from '$shared/types'
	import { SvelteSet } from 'svelte/reactivity'
	import { exportFormat } from '$shared/stores/settings'

	type Props = {
		open: boolean
		mode: 'selectPlaylists' | 'selectDevice'
		device?: UsbDevice
		playlist?: Playlist
		playlists: Playlist[]
		devices: UsbDevice[]
		onExport: (request: ExportRequest) => void
		onClose: () => void
	}

	let { open, mode, device, playlist, playlists, devices, onExport, onClose }: Props = $props()

	// Selected items
	let selectedPlaylistIds = $state<Set<string>>(new Set())
	let selectedDeviceId = $state<string | null>(null)

	// Sync option
	let enableSync = $state(true)

	// Reset state when modal opens
	$effect(() => {
		if (open) {
			selectedPlaylistIds = new Set()
			selectedDeviceId = null
			enableSync = true

			// If we're exporting a specific playlist, pre-select it
			if (mode === 'selectDevice' && playlist) {
				selectedPlaylistIds = new Set([playlist.id])
			}
		}
	})

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

	// Select device
	function selectDevice(deviceId: string) {
		selectedDeviceId = deviceId
	}

	// Get selected device
	const selectedDevice = $derived(mode === 'selectPlaylists' ? device : devices.find((d) => d.id === selectedDeviceId))

	// Calculate selected playlist count (excluding folders)
	const selectedPlaylistCount = $derived.by(() => {
		let count = 0
		for (const id of selectedPlaylistIds) {
			const p = playlists.find((pl) => pl.id === id)
			if (p && !p.is_folder) count++
		}
		return count
	})

	// Calculate total track count for selected playlists
	const totalTrackCount = $derived.by(() => {
		let count = 0
		for (const id of selectedPlaylistIds) {
			const p = playlists.find((pl) => pl.id === id)
			if (p && !p.is_folder) {
				count += p.track_count
			}
		}
		return count
	})

	// Check if export is valid
	const canExport = $derived(
		mode === 'selectPlaylists'
			? selectedPlaylistIds.size > 0 && device != null
			: selectedPlaylistIds.size > 0 && selectedDeviceId != null
	)

	// Handle export
	function handleExport() {
		const targetDevice = selectedDevice
		if (!targetDevice) return

		const request: ExportRequest = {
			device_id: targetDevice.id,
			mount_point: targetDevice.mount_point,
			device_name: targetDevice.name,
			playlist_ids: Array.from(selectedPlaylistIds),
			enable_sync: enableSync,
			use_device_library_plus: $exportFormat === 'device_library_plus',
		}

		onExport(request)
	}

	// Modal title
	const title = $derived(
		mode === 'selectPlaylists'
			? $translate('export.exportTo', { values: { deviceName: device?.name || '' } })
			: $translate('export.exportPlaylist', { values: { playlistName: playlist?.name || '' } })
	)
</script>

<Modal {open} {title} {onClose} size="md">
	<div class="export-content">
		{#if mode === 'selectPlaylists'}
			<!-- Playlist selection mode -->
			<Text color="secondary">{$translate('export.selectPlaylists')}</Text>

			<div class="playlist-tree-container">
				<SelectablePlaylistTree {playlists} selectedIds={selectedPlaylistIds} onToggle={togglePlaylist} />
			</div>
		{:else}
			<!-- Device selection mode -->
			<Text color="secondary">{$translate('export.selectDevice')}</Text>

			<div class="device-list">
				{#each devices as d (d.id)}
					<button class="device-item" class:selected={selectedDeviceId === d.id} onclick={() => selectDevice(d.id)}>
						<div class="device-info">
							<Text as="span" variant="body-2">{d.name}</Text>
							<Text as="span" variant="caption" color="secondary">
								{formatBytes(d.available_space_bytes)}
								{$translate('devices.available')}
							</Text>
						</div>
						<div class="device-capacity">
							<div class="capacity-bar">
								<div
									class="capacity-used"
									style="width: {((d.total_space_bytes - d.available_space_bytes) / d.total_space_bytes) * 100}%"
								></div>
							</div>
						</div>
					</button>
				{:else}
					<Text color="tertiary" class="py-6 text-center">{$translate('devices.noDevices')}</Text>
				{/each}
			</div>
		{/if}

		{#if selectedDevice}
			<div class="export-summary">
				<div class="summary-row">
					<Text as="span" variant="caption" color="secondary">{$translate('export.destination')}:</Text>
					<Text as="span" variant="caption" weight="medium">{selectedDevice.name}</Text>
				</div>
				<div class="summary-row">
					<Text as="span" variant="caption" color="secondary">{$translate('export.playlistsToExport')}:</Text>
					<Text as="span" variant="caption" weight="medium">{selectedPlaylistCount}</Text>
				</div>
				<div class="summary-row">
					<Text as="span" variant="caption" color="secondary">{$translate('export.tracksToExport')}:</Text>
					<Text as="span" variant="caption" weight="medium">{totalTrackCount}</Text>
				</div>
				<div class="summary-row">
					<Text as="span" variant="caption" color="secondary">{$translate('devices.available')}:</Text>
					<Text as="span" variant="caption" weight="medium">{formatBytes(selectedDevice.available_space_bytes)}</Text>
				</div>
			</div>
		{/if}

		<div class="sync-option">
			<Checkbox bind:checked={enableSync} label={$translate('export.enableSync')} />
		</div>
	</div>

	{#snippet footer()}
		<Button variant="ghost" onclick={onClose}>{$translate('common.cancel')}</Button>
		<Button variant="primary" onclick={handleExport} disabled={!canExport}>
			{$translate('export.export')}
		</Button>
	{/snippet}
</Modal>

<style>
	.export-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.playlist-tree-container {
		max-height: 300px;
		overflow-y: auto;
		border: 1px solid var(--stroke);
		border-radius: 6px;
		padding: 6px 6px;
	}

	.device-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.device-item {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 12px;
		border: 1px solid var(--border-color);
		border-radius: 8px;
		background: var(--bg-primary);
		cursor: pointer;
		text-align: left;
		width: 100%;
	}

	.device-item:hover {
		background: var(--bg-hover);
	}

	.device-item.selected {
		border-color: var(--accent-color);
		background: var(--accent-color-10);
	}

	.device-info {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.device-capacity {
		width: 100%;
	}

	.capacity-bar {
		height: 4px;
		background: var(--bg-tertiary);
		border-radius: 2px;
		overflow: hidden;
	}

	.capacity-used {
		height: 100%;
		background: var(--accent-color);
		border-radius: 2px;
	}

	.export-summary {
		background: var(--bg-secondary);
		border-radius: 8px;
		padding: 12px;
	}

	.summary-row {
		display: flex;
		justify-content: space-between;
		font-size: 13px;
		padding: 4px 0;
	}

	.sync-option {
		padding-top: 8px;
	}
</style>
