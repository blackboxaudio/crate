<script lang="ts">
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { Modal, Button, Input, Select, Icon, Text } from '$lib/components/common'
	import { followStore, followedSources, sortedFollowedSources, type FollowSort } from '$lib/stores'
	import { translate } from '$lib/i18n'
	import FollowingRow from './FollowingRow.svelte'
	import FollowSourceModal from './FollowSourceModal.svelte'

	type Props = {
		open: boolean
		onClose: () => void
	}

	let { open, onClose }: Props = $props()

	let showAddSource = $state(false)

	const sortOptions = $derived([
		{ value: 'newCount', label: $translate('discovery.following.sort.newCount') },
		{ value: 'name', label: $translate('discovery.following.sort.name') },
		{ value: 'recentlyReleased', label: $translate('discovery.following.sort.recentlyReleased') },
	])
</script>

<Modal {open} size="lg" flush {onClose}>
	<!-- Header -->
	<div class="flex items-center gap-2 border-b border-stroke-subtle px-4 py-3">
		<div class="min-w-0 flex-1">
			<Text variant="header-1" weight="medium" truncate>{$translate('discovery.following.title')}</Text>
		</div>
		{#if $followedSources.length > 0}
			<Button variant="ghost" onclick={() => followStore.checkAll()}>
				{$translate('discovery.following.checkAll')}
			</Button>
			<Button variant="primary" onclick={() => (showAddSource = true)}>
				{$translate('discovery.following.followSource')}
			</Button>
		{/if}
	</div>

	{#if $followedSources.length > 0}
		<!-- Toolbar -->
		<div class="flex items-center gap-2 border-b border-stroke-subtle px-4 py-2">
			<Button variant={$followStore.selectMode ? 'primary' : 'ghost'} onclick={() => followStore.toggleSelectMode()}>
				{$translate('discovery.following.select')}
			</Button>
			<div class="flex items-center gap-1.5">
				<span class="shrink-0 text-[11px] text-text-tertiary">{$translate('discovery.following.sortBy')}</span>
				<Select
					value={$followStore.sort}
					options={sortOptions}
					onchange={(v) => followStore.setSort(v as FollowSort)}
					class="w-44"
				/>
			</div>
			<div class="flex-1">
				<Input
					value={$followStore.search}
					placeholder={$translate('discovery.following.searchPlaceholder')}
					oninput={(e) => followStore.setSearch((e.target as HTMLInputElement).value)}
				/>
			</div>
		</div>

		{#if $followStore.selectMode && $followStore.selectedIds.size > 0}
			<div class="flex items-center gap-2 border-b border-stroke-subtle bg-surface-2/40 px-4 py-1.5">
				<span class="text-xs text-text-tertiary">{$followStore.selectedIds.size}</span>
				<div class="flex-1"></div>
				<Button variant="ghost-danger" onclick={() => followStore.unfollowMany([...$followStore.selectedIds])}>
					{$translate('discovery.following.unfollowSelected')}
				</Button>
			</div>
		{/if}
	{/if}

	<!-- List -->
	<div class="max-h-[55vh] min-h-0 overflow-y-auto px-2 py-2">
		{#if $followedSources.length === 0}
			<div class="flex flex-col items-center gap-2 px-4 py-12 text-center">
				<Icon name="rss" class="h-7 w-7 text-text-tertiary" />
				<div class="text-sm font-medium text-text-primary">{$translate('discovery.following.empty.title')}</div>
				<div class="max-w-xs text-[11px] text-text-tertiary">{$translate('discovery.following.empty.hint')}</div>
				<div class="mt-1">
					<Button variant="primary" onclick={() => (showAddSource = true)}>
						{$translate('discovery.following.followSource')}
					</Button>
				</div>
			</div>
		{:else}
			{#each $sortedFollowedSources as source (source.id)}
				<FollowingRow
					{source}
					selectMode={$followStore.selectMode}
					selected={$followStore.selectedIds.has(source.id)}
					checking={$followStore.checkingIds.has(source.id)}
					onToggleSelect={() => followStore.toggleSelected(source.id)}
					onCheck={() => followStore.check(source.id)}
					onUnfollow={() => followStore.unfollow(source.id)}
					onOpen={() => openUrl(source.url).catch(() => {})}
					onSetType={(type) => followStore.setType(source.id, type)}
				/>
			{/each}
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>{$translate('common.done')}</Button>
	{/snippet}
</Modal>

{#if showAddSource}
	<FollowSourceModal open={showAddSource} onClose={() => (showAddSource = false)} />
{/if}
