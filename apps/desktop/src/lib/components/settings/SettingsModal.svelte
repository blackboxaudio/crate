<script lang="ts">
	import type { SettingsPage } from '$shared/types'
	import { settingsStore } from '$shared/stores/settings'
	import { diagnosticsStore } from '$lib/stores/diagnostics'
	import { Button, Text } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import { scale } from 'svelte/transition'
	import { translate } from '$shared/i18n'
	import { GeneralTab, LibraryTab, DiscoveryTab, AppearanceTab, SoundTab, DiagnosticsTab, AboutTab } from './tabs'

	type Props = {
		open: boolean
		initialTab?: SettingsPage
		onClose: () => void
	}

	let { open, initialTab, onClose }: Props = $props()

	let dialogEl: HTMLDialogElement | undefined = $state()
	let contentEl: HTMLDivElement | undefined = $state()
	let activePage: SettingsPage = $state('general')
	let visible = $state(false)
	let mousedownTarget: EventTarget | null = $state(null)

	// Set active page when opening (use initialTab if provided, otherwise default to 'general')
	$effect(() => {
		if (open) {
			activePage = initialTab ?? 'general'
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

	// Reset scroll position when switching tabs
	$effect(() => {
		/* eslint-disable-next-line @typescript-eslint/no-unused-expressions */
		activePage
		contentEl?.scrollTo(0, 0)
	})

	function handleKeydown(e: KeyboardEvent) {
		e.stopPropagation()
		if (e.key === 'Escape') {
			e.preventDefault()
			onClose()
		}
	}

	function handleBackdropMousedown(e: MouseEvent) {
		mousedownTarget = e.target
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === dialogEl && mousedownTarget === dialogEl) {
			onClose()
		}
	}
</script>

<dialog
	bind:this={dialogEl}
	class="fixed inset-0 m-0 h-full max-h-none w-full max-w-none bg-transparent p-0 backdrop:bg-black/60"
	onkeydown={handleKeydown}
	onmousedown={handleBackdropMousedown}
	onclick={handleBackdropClick}
>
	{#if visible}
		<div
			class="fixed top-1/2 left-1/2 max-h-[80vh] w-full max-w-2xl -translate-x-1/2 -translate-y-1/2
				overflow-hidden rounded-lg border border-stroke bg-surface-1 text-text-primary shadow-xl"
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
							<Icon name="sliders-horizontal" class="h-4 w-4" />
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
							text-sm font-medium hover:cursor-pointer {activePage === 'discovery'
								? 'bg-brand-muted text-brand-primary'
								: 'text-text-secondary hover:bg-surface-2 hover:text-text-primary'}"
							onclick={() => (activePage = 'discovery')}
						>
							<Icon name="globe" class="h-4 w-4" />
							{$translate('settings.tabs.discovery')}
						</button>
						<button
							type="button"
							class="flex w-full items-center gap-2 rounded-md px-3 py-2
							text-sm font-medium hover:cursor-pointer {activePage === 'library'
								? 'bg-brand-muted text-brand-primary'
								: 'text-text-secondary hover:bg-surface-2 hover:text-text-primary'}"
							onclick={() => (activePage = 'library')}
						>
							<Icon name="library" class="h-4 w-4" />
							{$translate('settings.tabs.library')}
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
					{:else if activePage === 'diagnostics'}
						<DiagnosticsTab />
					{:else if activePage === 'about'}
						<AboutTab />
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
