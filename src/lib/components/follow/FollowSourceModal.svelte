<script lang="ts">
	import { Modal, Button, Input, Icon } from '$lib/components/common'
	import { followStore } from '$lib/stores'
	import { translate } from '$lib/i18n'

	type Props = {
		open: boolean
		onClose: () => void
	}

	let { open, onClose }: Props = $props()

	let tab = $state<'url' | 'search'>('url')
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
	<div class="mb-3 flex gap-1 rounded-md border border-stroke bg-surface-2 p-0.5 text-xs font-medium">
		<button
			type="button"
			class="flex-1 rounded px-2 py-1 transition-colors hover:cursor-pointer {tab === 'url'
				? 'bg-surface-1 text-text-primary'
				: 'text-text-tertiary hover:text-text-secondary'}"
			onclick={() => (tab = 'url')}
		>
			{$translate('discovery.following.addSource.tabUrl')}
		</button>
		<button
			type="button"
			class="flex-1 rounded px-2 py-1 transition-colors hover:cursor-pointer {tab === 'search'
				? 'bg-surface-1 text-text-primary'
				: 'text-text-tertiary hover:text-text-secondary'}"
			onclick={() => (tab = 'search')}
		>
			{$translate('discovery.following.addSource.tabSearch')}
		</button>
	</div>

	{#if tab === 'url'}
		<Input bind:value={url} placeholder={$translate('discovery.following.addSource.urlPlaceholder')} autofocus />
		<div
			class="mt-3 flex gap-2 rounded-md border border-stroke-subtle bg-surface-2/50 p-2.5 text-[11px] text-text-tertiary"
		>
			<Icon name="info" class="h-3.5 w-3.5 shrink-0" />
			<span>{$translate('discovery.following.addSource.urlInfo')}</span>
		</div>
	{:else}
		<div class="flex flex-col items-center gap-2 py-8 text-center">
			<Icon name="search" class="h-6 w-6 text-text-tertiary" />
			<span class="text-sm text-text-secondary">{$translate('discovery.following.addSource.searchComingSoon')}</span>
			<span class="max-w-xs text-[11px] text-text-tertiary"
				>{$translate('discovery.following.addSource.searchHint')}</span
			>
		</div>
	{/if}

	{#snippet footer()}
		<Button variant="secondary" size="sm" onclick={onClose}>{$translate('common.cancel')}</Button>
		<Button variant="primary" size="sm" onclick={submit} disabled={tab !== 'url' || !url.trim() || busy}>
			{$translate('discovery.following.follow')}
		</Button>
	{/snippet}
</Modal>
