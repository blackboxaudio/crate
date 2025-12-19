<script lang="ts">
	import { Button, IconButton } from '$lib/components/common'
	import { SearchBar } from '$lib/components/library'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		onImport?: () => void
		onSettings?: () => void
	}

	let { onImport, onSettings }: Props = $props()
</script>

<div class="flex items-center gap-4 border-b border-stroke bg-surface-1 px-4 py-2">
	<!-- Logo/Title -->
	<div class="flex items-center gap-2">
		<Icon name="logo" class="h-6 w-6 text-brand-primary" fill />
		<span class="text-lg font-bold text-text-primary">Crate</span>
	</div>

	<!-- Search -->
	<div class="max-w-md flex-1">
		<SearchBar />
	</div>

	<!-- Spacer -->
	<div class="flex-1"></div>

	<!-- Actions -->
	<div class="flex items-center gap-2">
		<IconButton title="Settings" onclick={onSettings}>
			<Icon name="settings" class="h-5 w-5" />
		</IconButton>
		<Button variant="primary" size="sm" onclick={onImport}>
			<Icon name="plus" class="mr-1.5 h-4 w-4" />
			Import
		</Button>
	</div>
</div>
