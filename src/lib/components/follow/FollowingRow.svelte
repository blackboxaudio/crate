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
		onCheck?: () => void
		onUnfollow?: () => void
		onOpen?: () => void
		onSetType?: (type: 'artist' | 'label') => void
		onContextMenu?: (e: MouseEvent) => void
	}

	let {
		source,
		selectMode = false,
		selected = false,
		checking = false,
		onToggleSelect,
		onCheck,
		onUnfollow,
		onOpen,
		onSetType,
		onContextMenu,
	}: Props = $props()

	function domain(url: string): string {
		try {
			return new URL(url).host
		} catch {
			return url
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="group flex items-center rounded px-2 py-2 hover:bg-surface-2/50"
	oncontextmenu={(e) => {
		e.preventDefault()
		onContextMenu?.(e)
	}}
>
	<!-- Checkbox slot: width + opacity animate so the row content slides aside to make room -->
	<div
		class="flex shrink-0 items-center overflow-hidden transition-[width,opacity] duration-200 ease-out motion-reduce:transition-none {selectMode
			? 'w-7 opacity-100'
			: 'pointer-events-none w-0 opacity-0'}"
	>
		<Checkbox checked={selected} onchange={() => onToggleSelect?.()} />
	</div>

	<div class="flex min-w-0 flex-1 items-center gap-3">
		<Tooltip
			text={$translate('discovery.following.changeType', {
				values: {
					type:
						source.followType === 'label'
							? $translate('discovery.following.label')
							: $translate('discovery.following.artist'),
				},
			})}
			position="top"
			delay={250}
		>
			<button
				type="button"
				class="shrink-0 rounded p-0.5 text-text-tertiary transition-colors hover:cursor-pointer hover:text-text-secondary"
				aria-label={$translate('discovery.following.changeType', {
					values: {
						type:
							source.followType === 'label'
								? $translate('discovery.following.label')
								: $translate('discovery.following.artist'),
					},
				})}
				onclick={() => onSetType?.(source.followType === 'label' ? 'artist' : 'label')}
			>
				<Icon name={source.followType === 'label' ? 'disc' : 'user'} class="h-4 w-4" />
			</button>
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
				<div class="flex h-6 w-6 items-center justify-center">
					<Spinner class="h-4 w-4" />
				</div>
			{:else}
				<Tooltip text={$translate('discovery.following.checkNow')} position="top" delay={250}>
					<IconButton icon="refresh" size="sm" onclick={() => onCheck?.()} />
				</Tooltip>
			{/if}
			<Tooltip text={$translate('discovery.openInBrowser')} position="top" delay={250}>
				<IconButton icon="external-link" size="sm" disabled={checking} onclick={() => onOpen?.()} />
			</Tooltip>
			<Tooltip text={$translate('discovery.following.unfollow')} position="top" delay={250}>
				<IconButton icon="x" size="sm" disabled={checking} onclick={() => onUnfollow?.()} />
			</Tooltip>
		</div>
	</div>
</div>
