<script lang="ts">
	import type { Theme, AccentColor } from '$lib/types'
	import { settingsStore, theme, accentColor } from '$lib/stores/settings'
	import { Button } from '$lib/components/common'

	type Props = {
		open: boolean
		onClose: () => void
	}

	let { open, onClose }: Props = $props()

	let dialogEl: HTMLDialogElement | undefined = $state()

	// Sync dialog open state
	$effect(() => {
		if (!dialogEl) return
		if (open && !dialogEl.open) {
			dialogEl.showModal()
		} else if (!open && dialogEl.open) {
			dialogEl.close()
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

	function handleThemeChange(newTheme: Theme) {
		settingsStore.setTheme(newTheme)
	}

	function handleAccentChange(newColor: AccentColor) {
		settingsStore.setAccentColor(newColor)
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
		<div class="flex h-[500px]">
			<!-- Sidebar -->
			<div class="flex w-48 flex-col border-r border-stroke bg-surface-0 p-4">
				<h2 class="mb-4 text-lg font-semibold">Settings</h2>
				<nav class="space-y-1">
					<button
						type="button"
						class="flex w-full items-center gap-2 rounded-md bg-brand-muted px-3 py-2
							text-sm font-medium text-brand-primary"
					>
						<!-- Paint brush icon -->
						<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01"
							/>
						</svg>
						Appearance
					</button>
				</nav>
			</div>

			<!-- Content -->
			<div class="flex-1 overflow-auto p-6">
				<div class="space-y-8">
					<!-- Theme Section -->
					<section>
						<h3 class="mb-4 text-sm font-semibold tracking-wide text-text-secondary uppercase">Theme</h3>
						<div class="flex gap-3">
							{#each themeOptions as option (option.value)}
								<button
									type="button"
									class="flex flex-1 flex-col items-center gap-2 rounded-lg border-2 p-4
										transition-colors {$theme === option.value
										? 'border-brand-primary bg-brand-muted'
										: 'border-stroke hover:border-text-tertiary'}"
									onclick={() => handleThemeChange(option.value)}
								>
									{#if option.value === 'light'}
										<!-- Sun icon -->
										<svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
											<path
												stroke-linecap="round"
												stroke-linejoin="round"
												stroke-width="2"
												d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
											/>
										</svg>
									{:else if option.value === 'dark'}
										<!-- Moon icon -->
										<svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
											<path
												stroke-linecap="round"
												stroke-linejoin="round"
												stroke-width="2"
												d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
											/>
										</svg>
									{:else}
										<!-- Computer/Monitor icon -->
										<svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
											<path
												stroke-linecap="round"
												stroke-linejoin="round"
												stroke-width="2"
												d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
											/>
										</svg>
									{/if}
									<span class="text-sm font-medium">{option.label}</span>
								</button>
							{/each}
						</div>
					</section>

					<!-- Accent Color Section -->
					<section>
						<h3 class="mb-4 text-sm font-semibold tracking-wide text-text-secondary uppercase">Accent Color</h3>
						<div class="grid grid-cols-5 gap-3">
							{#each accentColors as color (color.value)}
								<button
									type="button"
									class="group flex flex-col items-center gap-2 rounded-lg p-3
										transition-colors hover:bg-surface-2"
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
									<span class="text-xs text-text-secondary">{color.label}</span>
								</button>
							{/each}
						</div>
					</section>
				</div>
			</div>
		</div>

		<!-- Footer -->
		<div class="flex justify-end border-t border-stroke px-6 py-4">
			<Button variant="secondary" onclick={onClose}>Close</Button>
		</div>
	{/if}
</dialog>
