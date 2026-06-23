<script lang="ts">
	import { translate } from '$shared/i18n'
	import { discoveryStore } from '$shared/stores/discovery'
	import { mobileUIStore, selectedReleaseIds, selectedReleaseCount } from '$lib/stores/mobileUI'
	import { confirmDialog } from '$lib/utils/dialog'
	import MobileTagPicker from './MobileTagPicker.svelte'

	// Bottom action bar for multi-select mode. Overlays the tab bar (same bottom slot) while the feed is in
	// select mode; the mini-player keeps floating above it. Batch tag-assign reuses the (now multi-release)
	// MobileTagPicker; batch delete confirms first, then clears the selection.
	let tagPickerOpen = $state(false)

	const count = $derived($selectedReleaseCount)
	const ids = $derived([...$selectedReleaseIds])

	async function confirmDelete() {
		const ok = await confirmDialog($translate('discovery.confirmDeleteMessage'), {
			title: $translate('discovery.confirmDeleteTitle', { values: { count } }),
			confirmLabel: $translate('common.delete'),
		})
		if (!ok) return
		const toDelete = [...$selectedReleaseIds]
		mobileUIStore.exitSelectMode()
		await discoveryStore.deleteReleases(toDelete)
	}
</script>

<div class="pb-safe fixed inset-x-0 bottom-0 z-30 border-t border-stroke-subtle bg-surface-1">
	<div class="flex h-14 items-center justify-between px-3">
		<div class="flex items-center gap-2">
			<button
				type="button"
				class="flex h-9 items-center rounded-md px-3 text-sm font-medium text-text-secondary active:bg-surface-2"
				onclick={mobileUIStore.exitSelectMode}
			>
				{$translate('common.cancel')}
			</button>
			<span class="text-sm font-semibold text-text-primary">
				{$translate('discovery.selectedCount', { values: { count } })}
			</span>
		</div>
		<div class="flex items-center gap-1">
			<button
				type="button"
				disabled={count === 0}
				aria-label={$translate('discovery.editor.addTags')}
				class="flex h-9 w-9 items-center justify-center rounded-md text-text-secondary active:bg-surface-2 disabled:opacity-40"
				onclick={() => (tagPickerOpen = true)}
			>
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path
						d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A2 2 0 013 12V7a4 4 0 014-4z"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
			</button>
			<button
				type="button"
				disabled={count === 0}
				aria-label={$translate('discovery.deleteReleases')}
				class="flex h-9 w-9 items-center justify-center rounded-md text-danger active:bg-surface-2 disabled:opacity-40"
				onclick={confirmDelete}
			>
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path
						d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m2 0v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
			</button>
		</div>
	</div>
</div>

<MobileTagPicker open={tagPickerOpen} releaseIds={ids} onClose={() => (tagPickerOpen = false)} />
