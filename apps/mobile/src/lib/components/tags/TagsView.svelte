<script lang="ts">
	import { onMount } from 'svelte'
	import { translate } from '$shared/i18n'
	import { tagsStore } from '$shared/stores/tags'

	// Tags tab: the user's tag categories, each shown as a row of colored tag chips. Owns its own scroll
	// container (the shell frame is overflow-hidden and reserves the bottom inset for the tab bar /
	// mini-player). Read-only listing for now — tag editing is a later issue.
	onMount(() => {
		tagsStore.load()
	})

	const categories = $derived($tagsStore.categories)
</script>

<div class="h-full overflow-y-auto" style="padding-bottom: var(--mini-player-inset, 0px)">
	<h2 class="px-4 pt-4 pb-1 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
		{$translate('nav.tags')}
	</h2>
	{#if categories.length === 0}
		<div class="px-4 py-6 text-sm text-text-secondary">{$translate('tags.noTags')}</div>
	{:else}
		{#each categories as category (category.id)}
			<div class="px-4 py-2">
				<h3 class="mb-1.5 text-sm font-medium text-text-secondary">{category.name}</h3>
				<div class="flex flex-wrap gap-2">
					{#each category.tags as tag (tag.id)}
						{@const color = tag.color ?? category.color ?? '#6366f1'}
						<span
							class="rounded-md px-3 py-2 text-sm font-medium"
							style="background-color: {color}20; color: {color}; border: 1px solid {color}40;"
						>
							{tag.name}
						</span>
					{/each}
				</div>
			</div>
		{/each}
	{/if}
</div>
