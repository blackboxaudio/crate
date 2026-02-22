<script lang="ts">
	import type { BreadcrumbItem } from '$lib/types'
	import type { Snippet } from 'svelte'
	import Icon from './Icon.svelte'

	type Props = {
		items: BreadcrumbItem[]
		onNavigate: (item: BreadcrumbItem) => void
		onContextMenu: (e: MouseEvent, item: BreadcrumbItem) => void
		actions?: Snippet
	}

	let { items, onNavigate, onContextMenu, actions }: Props = $props()

	function getIconName(type: BreadcrumbItem['type']): string {
		switch (type) {
			case 'library':
				return 'library'
			case 'folder':
				return 'folder'
			case 'smart_playlist':
				return 'bolt'
			case 'playlist':
				return 'playlist'
		}
	}
</script>

<div class="flex items-center justify-between border-b border-stroke px-4 py-4">
	<div class="flex items-center gap-1">
		{#each items as item, index (item.id ?? 'library')}
			{#if index > 0}
				<Icon name="chevron-right" class="h-4 w-4 text-text-tertiary" />
			{/if}

			<button
				type="button"
				class="flex items-center gap-2 rounded px-2 py-1 text-sm transition-colors hover:cursor-pointer hover:bg-surface-2
					{index === items.length - 1 ? 'font-medium text-text-primary' : 'text-text-secondary'}"
				onclick={() => onNavigate(item)}
				oncontextmenu={(e) => {
					e.preventDefault()
					onContextMenu(e, item)
				}}
			>
				<Icon name={getIconName(item.type)} class="h-4 w-4" />
				<span>{item.name}</span>

				{#if index === items.length - 1 && item.count !== undefined}
					<span class="ml-2 text-text-tertiary">
						{item.count}
						{item.countLabel}
					</span>
				{/if}
			</button>
		{/each}
	</div>
	{#if actions}
		{@render actions()}
	{/if}
</div>
