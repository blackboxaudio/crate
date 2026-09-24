<script lang="ts">
	import { readText } from '@tauri-apps/plugin-clipboard-manager'
	import { translate } from '$shared/i18n'
	import { followStore } from '$shared/stores/follow'
	import { extractFirstUrl } from '$shared/utils/discoveryLinks'
	import FormSheet from '$lib/components/common/FormSheet.svelte'
	import FormSection from '$lib/components/common/FormSection.svelte'
	import FormTextField from '$lib/components/common/FormTextField.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'

	// Follow-a-source by URL (paste an artist/label page): a single grouped URL row on the content-hugging
	// FormSheet detent (one field doesn't earn a full-height sheet — the 'auto' detent's keyboard spacer
	// keeps the row above the keyboard). The platform and artist-vs-label type are detected from the URL
	// by the backend. Kept open while the URL is being checked, so a failed URL keeps the user's input.
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
	height="auto"
	title={$translate('discovery.following.addSource.title')}
>
	<div class="flex flex-col gap-5 px-4 py-4">
		<FormSection footer={$translate('discovery.following.addSource.urlInfo')}>
			<FormTextField
				id="follow-url"
				label={$translate('discovery.url')}
				bind:value={url}
				type="url"
				placeholder="https://..."
				inputmode="url"
				autocapitalize="off"
				autocorrect="off"
				enterkeyhint="go"
				focusOnOpen
				onenter={handleSubmit}
			>
				{#snippet trailing()}
					<button
						type="button"
						class="text-xs font-medium text-brand-primary active:opacity-70"
						onclick={pasteFromClipboard}
					>
						{$translate('discovery.pasteLink')}
					</button>
				{/snippet}
			</FormTextField>

			{#snippet footerExtra()}
				{#if clipboardMiss}
					<p class="text-xs text-text-tertiary">{$translate('discovery.clipboardNoUrl')}</p>
				{/if}
				{#if busy}
					<div class="flex items-center gap-2">
						<Spinner class="h-3.5 w-3.5" />
						<span class="text-xs text-text-tertiary">{$translate('common.loading')}</span>
					</div>
				{/if}
			{/snippet}
		</FormSection>
	</div>
</FormSheet>
