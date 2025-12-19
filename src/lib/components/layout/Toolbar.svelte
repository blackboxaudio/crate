<script lang="ts">
	import { Button, IconButton } from '$lib/components/common'
	import { SearchBar } from '$lib/components/library'

	type Props = {
		onImport?: () => void
		onSettings?: () => void
	}

	let { onImport, onSettings }: Props = $props()
</script>

<div class="flex items-center gap-4 border-b border-stroke bg-surface-1 px-4 py-2">
	<!-- Logo/Title -->
	<div class="flex items-center gap-2">
		<svg class="h-6 w-6 text-brand-primary" fill="currentColor" viewBox="0 0 24 24">
			<path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z" />
		</svg>
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
			<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
				/>
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
			</svg>
		</IconButton>
		<Button variant="primary" size="sm" onclick={onImport}>
			<svg class="mr-1.5 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
			</svg>
			Import
		</Button>
	</div>
</div>
