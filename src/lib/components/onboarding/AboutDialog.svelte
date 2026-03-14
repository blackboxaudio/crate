<script lang="ts">
	import { Modal, Text, Button } from '$lib/components/common'
	import { appInfo } from '$lib/stores/app'
	import { translate } from '$lib/i18n'

	type Props = {
		open: boolean
		onClose: () => void
	}

	let { open, onClose }: Props = $props()
</script>

<Modal {open} {onClose} size="sm">
	<div class="flex flex-col items-center gap-6 py-4 text-center">
		<div
			class="h-16 w-16 bg-brand-primary"
			style="-webkit-mask-image: url('/crate-logo.svg'); -webkit-mask-size: contain; -webkit-mask-repeat: no-repeat; -webkit-mask-position: center; mask-image: url('/crate-logo.svg'); mask-size: contain; mask-repeat: no-repeat; mask-position: center;"
		></div>
		<div class="flex flex-col gap-1">
			<Text variant="header-1" weight="bold">Crate</Text>
			<Text size="sm" color="secondary">
				{$translate('settings.about.version')}
				{$appInfo?.version ?? $translate('common.unknown')}
			</Text>
		</div>
	</div>
	{#snippet footer()}
		<Button size="sm" variant="secondary" onclick={onClose}>
			{$translate('common.close')}
		</Button>
	{/snippet}
</Modal>
