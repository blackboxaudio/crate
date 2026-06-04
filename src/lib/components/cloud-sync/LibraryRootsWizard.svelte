<script lang="ts">
	import { Button, Text } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import { cloudSyncStore, libraryRoots } from '$lib/stores/cloudSync'
	import { translate } from '$lib/i18n'
	import { open } from '@tauri-apps/plugin-dialog'

	async function handlePickFolder(rootId: string) {
		const selected = await open({ directory: true, multiple: false })
		if (selected && typeof selected === 'string') {
			await cloudSyncStore.setRootMapping(rootId, selected)
		}
	}
</script>

<div class="space-y-2">
	{#each $libraryRoots as root (root.id)}
		<div class="flex items-center justify-between rounded-lg border border-stroke bg-surface-1 px-4 py-3">
			<div class="min-w-0 flex-1">
				<Text variant="body" class="font-medium">{root.name}</Text>
				{#if root.local_path}
					<Text variant="caption" as="p" class="mt-0.5 truncate" title={root.local_path}>
						{root.local_path}
					</Text>
				{:else}
					<Text variant="caption" as="p" class="mt-0.5 text-amber-500">
						{$translate('cloudSync.roots.unmapped')}
					</Text>
				{/if}
			</div>
			<Button variant="secondary" size="sm" onclick={() => handlePickFolder(root.id)}>
				<Icon name="folder-open" class="mr-1.5 h-3.5 w-3.5" />
				{root.local_path ? $translate('cloudSync.roots.remap') : $translate('cloudSync.roots.map')}
			</Button>
		</div>
	{/each}
	{#if $libraryRoots.length === 0}
		<Text variant="caption" as="p" class="py-4 text-center">{$translate('cloudSync.roots.none')}</Text>
	{/if}
</div>
