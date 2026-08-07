<script lang="ts">
	import { tick } from 'svelte'
	import { readText } from '@tauri-apps/plugin-clipboard-manager'
	import { translate } from '$shared/i18n'
	import { followStore } from '$shared/stores/follow'
	import { extractFirstUrl } from '$shared/utils/discoveryLinks'
	import FormSheet from '$lib/components/common/FormSheet.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'

	// Follow-a-source by URL (paste an artist/label page), presented exactly like the add-release sheet:
	// the same FormSheet with the URL field pinned at the top (clear of the keyboard) and the primary
	// action in the nav bar. The platform and artist-vs-label type are detected from the URL by the
	// backend. Kept open while the URL is being checked, so a failed URL keeps the user's input.
	type Props = {
		open: boolean
		onClose: () => void
	}
	let { open, onClose }: Props = $props()

	let url = $state('')
	let busy = $state(false)
	let clipboardMiss = $state(false)

	$effect(() => {
		if (open) {
			url = ''
			busy = false
			clipboardMiss = false
		}
	})

	// Explicit "Paste link" button (iOS shows a system paste-permission prompt on any programmatic
	// clipboard read, so tying the read to a user tap keeps that prompt expected — see AddReleaseModal).
	async function pasteFromClipboard() {
		clipboardMiss = false
		try {
			const text = await readText()
			const found = text ? extractFirstUrl(text) : null
			if (!found) {
				clipboardMiss = true
				return
			}
			url = found
		} catch {
			clipboardMiss = true
		}
	}

	// Focus the URL field as the sheet mounts; `preventScroll` stops iOS from scrolling the document to
	// "reveal" the field while the panel is still sliding in (see AddReleaseModal's focusOnOpen).
	function focusOnOpen(node: HTMLInputElement) {
		void tick().then(() => node.focus({ preventScroll: true }))
	}

	async function handleSubmit() {
		const trimmed = url.trim()
		if (!trimmed || busy) return
		busy = true
		const source = await followStore.followFromUrl(trimmed)
		busy = false
		if (source) onClose()
	}
</script>

<FormSheet
	{open}
	{onClose}
	onSubmit={handleSubmit}
	submitLabel={$translate('discovery.following.follow')}
	submitDisabled={!url.trim() || busy}
	dirty={url.trim().length > 0}
	positionSlide
	title={$translate('discovery.following.addSource.title')}
>
	<div class="flex flex-col gap-4 px-4 py-4">
		<div>
			<div class="mb-1.5 flex items-center justify-between">
				<label for="follow-url" class="block text-xs font-medium text-text-secondary">
					{$translate('discovery.url')}
				</label>
				<button
					type="button"
					class="text-xs font-medium text-brand-primary active:opacity-70"
					onclick={pasteFromClipboard}
				>
					{$translate('discovery.pasteLink')}
				</button>
			</div>
			<input
				id="follow-url"
				type="url"
				use:focusOnOpen
				bind:value={url}
				placeholder="https://..."
				class="w-full rounded-md border border-stroke bg-surface-1 px-3 py-2 text-sm text-text-primary placeholder:text-text-tertiary"
			/>
			{#if clipboardMiss}
				<p class="mt-1.5 text-xs text-text-tertiary">{$translate('discovery.clipboardNoUrl')}</p>
			{/if}
			{#if busy}
				<div class="mt-2 flex items-center gap-2">
					<Spinner class="h-3.5 w-3.5" />
					<span class="text-xs text-text-tertiary">{$translate('common.loading')}</span>
				</div>
			{/if}
		</div>

		<p class="text-xs text-text-tertiary">{$translate('discovery.following.addSource.urlInfo')}</p>
	</div>
</FormSheet>
