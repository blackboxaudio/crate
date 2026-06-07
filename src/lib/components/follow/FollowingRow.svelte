<script lang="ts">
	import type { FollowedSource } from '$lib/types'
	import { AlbumArt, Icon, IconButton, Tooltip, Checkbox, Spinner } from '$lib/components/common'
	import { formatRelativeDate } from '$lib/utils'
	import { translate } from '$lib/i18n'

	type Props = {
		source: FollowedSource
		selectMode?: boolean
		selected?: boolean
		checking?: boolean
		onToggleSelect?: () => void
		onToggleEnabled?: () => void
		onCheck?: () => void
		onUnfollow?: () => void
		onOpen?: () => void
	}

	let {
		source,
		selectMode = false,
		selected = false,
		checking = false,
		onToggleSelect,
		onToggleEnabled,
		onCheck,
		onUnfollow,
		onOpen,
	}: Props = $props()

	function domain(url: string): string {
		try {
			return new URL(url).host
		} catch {
			return url
		}
	}
</script>

<div class="group flex items-center gap-3 rounded px-2 py-2 hover:bg-surface-2/50">
	{#if selectMode}
		<Checkbox checked={selected} onchange={() => onToggleSelect?.()} />
	{/if}

	<Tooltip
		text={source.followType === 'label'
			? $translate('discovery.following.label')
			: $translate('discovery.following.artist')}
		position="top"
		delay={250}
	>
		<Icon name={source.followType === 'label' ? 'tag' : 'user'} class="h-4 w-4 shrink-0 text-text-tertiary" />
	</Tooltip>

	<AlbumArt artworkPath={source.artworkPath} artworkUrl={source.artworkUrl} size="xs" />

	<div class="min-w-0 flex-1">
		<div class="truncate text-sm font-medium text-text-primary">{source.name ?? domain(source.url)}</div>
		<div class="truncate text-[11px] text-text-tertiary">
			{domain(source.url)}{source.lastCheckedAt
				? ` · ${$translate('discovery.following.checkedAgo', { values: { time: formatRelativeDate(source.lastCheckedAt, $translate) } })}`
				: ''}
		</div>
	</div>

	<div class="shrink-0 text-right">
		{#if source.health === 'error' || source.health === 'rate_limited'}
			<Tooltip text={source.lastError ?? $translate('discovery.following.sourceError')} position="top" delay={250}>
				<span class="inline-flex items-center gap-1 text-[11px] text-amber-500">
					<Icon name="alert-circle" class="h-3.5 w-3.5" />
					{$translate('discovery.following.sourceError')}
				</span>
			</Tooltip>
		{:else if !source.enabled}
			<span class="text-[10px] font-semibold tracking-wide text-text-tertiary uppercase">
				{$translate('discovery.following.paused')}
			</span>
		{:else if source.newCount > 0}
			<span class="rounded-full bg-brand-muted px-1.5 py-0.5 text-[10px] font-semibold text-brand-primary">
				{$translate('discovery.following.newCount', { values: { count: source.newCount } })}
			</span>
		{:else}
			<span class="text-[10px] font-semibold tracking-wide text-text-tertiary uppercase">
				{$translate('discovery.following.upToDate')}
			</span>
		{/if}
	</div>

	<div class="flex shrink-0 items-center gap-0.5">
		{#if checking}
			<Spinner class="h-4 w-4" />
		{:else}
			<div class="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
				<Tooltip text={$translate('discovery.following.checkNow')} position="top" delay={250}>
					<IconButton icon="refresh" size="sm" onclick={() => onCheck?.()} />
				</Tooltip>
				<Tooltip text={$translate('discovery.openInBrowser')} position="top" delay={250}>
					<IconButton icon="external-link" size="sm" onclick={() => onOpen?.()} />
				</Tooltip>
				<Tooltip text={$translate('discovery.following.unfollow')} position="top" delay={250}>
					<IconButton icon="x" size="sm" onclick={() => onUnfollow?.()} />
				</Tooltip>
			</div>
		{/if}
		<button
			type="button"
			aria-label={$translate('discovery.following.enabledToggle')}
			class="ml-1 flex h-4 w-7 shrink-0 items-center rounded-full p-0.5 transition-colors hover:cursor-pointer {source.enabled
				? 'bg-brand-primary'
				: 'bg-stroke'}"
			onclick={() => onToggleEnabled?.()}
		>
			<div class="h-3 w-3 rounded-full bg-white transition-transform {source.enabled ? 'translate-x-3' : ''}"></div>
		</button>
	</div>
</div>
