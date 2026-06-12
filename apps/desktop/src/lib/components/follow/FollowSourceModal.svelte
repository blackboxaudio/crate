<script lang="ts">
	import { Modal, Button, Input, Icon } from '$lib/components/common'
	import { followStore } from '$lib/stores'
	import { translate } from '$shared/i18n'

	type Props = {
		open: boolean
		onClose: () => void
	}

	let { open, onClose }: Props = $props()

	let url = $state('')
	let busy = $state(false)

	async function submit() {
		const trimmed = url.trim()
		if (!trimmed || busy) return
		busy = true
		const source = await followStore.followFromUrl(trimmed)
		busy = false
		if (source) {
			url = ''
			onClose()
		}
	}
</script>

<Modal {open} size="md" title={$translate('discovery.following.addSource.title')} {onClose} onSubmit={submit}>
	<Input bind:value={url} placeholder={$translate('discovery.following.addSource.urlPlaceholder')} autofocus />
	<div
		class="mt-3 flex gap-2 rounded-md border border-stroke-subtle bg-surface-2/50 p-2.5 text-[11px] text-text-tertiary"
	>
		<Icon name="info" class="h-3.5 w-3.5 shrink-0" />
		<span>{$translate('discovery.following.addSource.urlInfo')}</span>
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>{$translate('common.cancel')}</Button>
		<Button variant="primary" onclick={submit} disabled={!url.trim() || busy}>
			{$translate('discovery.following.follow')}
		</Button>
	{/snippet}
</Modal>
