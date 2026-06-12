<script lang="ts">
	import type { SettingsPage } from '$shared/types'
	import { settingsStore } from '$shared/stores/settings'
	import { diagnosticsStore } from '$lib/stores/diagnostics'
	import { Button, Text } from '$lib/components/common'
	import Modal from '$lib/components/common/Modal.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import { translate } from '$shared/i18n'
	import {
		GeneralTab,
		LibraryTab,
		DiscoveryTab,
		AppearanceTab,
		SoundTab,
		CloudSyncTab,
		DiagnosticsTab,
		AboutTab,
	} from './tabs'
	import { isSyncAvailable, cloudSyncStore } from '$shared/stores/cloudSync'

	type Props = {
		open: boolean
		initialTab?: SettingsPage
		onClose: () => void
	}

	let { open, initialTab, onClose }: Props = $props()

	let contentEl: HTMLDivElement | undefined = $state()
	let activePage: SettingsPage = $state('general')

	const allTabs: { page: SettingsPage; icon: string; fill?: boolean; requireSync?: boolean }[] = [
		{ page: 'general', icon: 'sliders-horizontal' },
		{ page: 'appearance', icon: 'palette' },
		{ page: 'discovery', icon: 'globe' },
		{ page: 'library', icon: 'library' },
		{ page: 'sound', icon: 'volume-full', fill: true },
		{ page: 'cloudSync', icon: 'cloud', requireSync: true },
		{ page: 'diagnostics', icon: 'terminal' },
		{ page: 'about', icon: 'info' },
	]

	const tabs = $derived(allTabs.filter((t) => !t.requireSync || $isSyncAvailable))

	// Set active page when opening (use initialTab if provided, otherwise default to 'general')
	$effect(() => {
		if (open) {
			activePage = initialTab ?? 'general'
		}
	})

	// Refresh audio devices when opening sound settings
	$effect(() => {
		if (open && activePage === 'sound') {
			settingsStore.refreshAudioDevices()
		}
	})

	// Load cloud sync status when opening the cloud sync tab
	$effect(() => {
		if (open && activePage === 'cloudSync') {
			cloudSyncStore.load()
		}
	})

	// Load diagnostics when opening diagnostics settings
	$effect(() => {
		if (open && activePage === 'diagnostics') {
			diagnosticsStore.load()
		}
	})

	// Reset scroll position when switching tabs
	$effect(() => {
		/* eslint-disable-next-line @typescript-eslint/no-unused-expressions */
		activePage
		contentEl?.scrollTo(0, 0)
	})
</script>

<Modal {open} {onClose} size="xl" flush>
	<div class="flex h-[500px]">
		<!-- Sidebar -->
		<div class="flex w-48 flex-col border-r border-stroke bg-surface-0 p-4">
			<Text variant="header-1" class="mb-4">{$translate('settings.title')}</Text>
			<nav class="space-y-1">
				{#each tabs as tab (tab.page)}
					<button
						type="button"
						class="flex w-full items-center gap-2 rounded-md px-3 py-2
						text-sm font-medium hover:cursor-pointer {activePage === tab.page
							? 'bg-brand-muted text-brand-primary'
							: 'text-text-secondary hover:bg-surface-2 hover:text-text-primary'}"
						onclick={() => (activePage = tab.page)}
					>
						<Icon name={tab.icon} class="h-4 w-4" fill={tab.fill} />
						{$translate(`settings.tabs.${tab.page}`)}
					</button>
				{/each}
			</nav>
		</div>

		<!-- Content -->
		<div bind:this={contentEl} class="flex-1 overflow-auto p-6">
			{#if activePage === 'general'}
				<GeneralTab />
			{:else if activePage === 'appearance'}
				<AppearanceTab />
			{:else if activePage === 'discovery'}
				<DiscoveryTab />
			{:else if activePage === 'library'}
				<LibraryTab />
			{:else if activePage === 'sound'}
				<SoundTab />
			{:else if activePage === 'cloudSync'}
				<CloudSyncTab />
			{:else if activePage === 'diagnostics'}
				<DiagnosticsTab />
			{:else if activePage === 'about'}
				<AboutTab />
			{/if}
		</div>
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>{$translate('common.close')}</Button>
	{/snippet}
</Modal>
