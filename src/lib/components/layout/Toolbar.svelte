<script lang="ts">
	import { Button, IconButton, Tooltip } from '$lib/components/common'
	import Icon from '$lib/components/common/Icon.svelte'
	import { isDev } from '$lib/stores'
	import { translate } from '$lib/i18n'

	type Props = {
		onImport?: () => void
		onAddRelease?: () => void
		onSettings?: () => void
		onDevTools?: () => void
	}

	let { onImport, onAddRelease, onSettings, onDevTools }: Props = $props()
</script>

<div class="flex flex-1 items-center justify-end gap-2 rounded-bl-md py-4 pr-3 pl-4">
	{#if onAddRelease}
		<Button variant="primary" size="sm" onclick={onAddRelease}>
			<Icon name="plus" class="mr-1.5 h-4 w-4" />
			{$translate('discovery.addRelease')}
		</Button>
	{:else}
		<Button variant="primary" size="sm" onclick={onImport}>
			<Icon name="upload" class="mr-1.5 h-4 w-4" />
			{$translate('library.importTracks')}
		</Button>
	{/if}
	{#if $isDev}
		<Tooltip text={$translate('common.developerTools')} position="bottom" delay={250}>
			<IconButton icon="terminal" iconClass="h-5 w-5" onclick={onDevTools} />
		</Tooltip>
	{/if}
	<Tooltip text={$translate('settings.title')} position="bottom" delay={250}>
		<IconButton icon="settings" iconClass="h-5 w-5" onclick={onSettings} />
	</Tooltip>
</div>
